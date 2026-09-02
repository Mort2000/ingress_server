//! The match driver (P3-RPC/P3-TIMEOUT, D65): one thread owning the write
//! halves, the `GameState`, arena-core's `Resolver`, the IARP recorder,
//! the SHM publishers, the journal, and the metrics. Reader threads feed
//! it [`ConnEvent`]s; the server NEVER re-implements rules — settlement is
//! `Resolver::run_round` and nothing else.

use std::collections::BTreeSet;
use std::io;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use arena_core::map::{MapDefinition, generate_map};
use arena_core::observe::Observation;
use arena_core::resolve::{
    Resolver, RoundReport, SlotStatus, StreamRejection, SubmittedCommandStream, structural_check,
};
use arena_core::spawn::{InitialSpawns, validate_initial_spawn_set};
use arena_core::state::GameState;
use arena_core::types::{AgentId, Faction};

use arena_protocol::build::{
    envelope_frame, map_generation_bytes, private_roster_bytes, private_snapshot_bytes,
    public_snapshot_bytes,
};
use arena_protocol::decode::{
    END_CLIENT_FORFEIT, END_SPAWN_FORFEIT, Envelope, HELLO_DUPLICATE_CONNECTION, HELLO_OK,
    HELLO_VERSION_MISMATCH, MatchInfo, SUBMIT_ACCEPTED, SUBMIT_IGNORED_DUPLICATE, SUBMIT_LATE_DROP,
    SUBMIT_REJECTED_STRUCTURAL, SUBMIT_UNLOCATABLE, SpawnEntry,
};
use arena_protocol::{
    MAX_FRAME_PAYLOAD_BYTES, PRIVATE_SNAPSHOT_VERSION, PROTOCOL_VERSION, PUBLIC_SNAPSHOT_VERSION,
};

use crate::conn::{self, ConnEvent};
use crate::journal::Journal;
use crate::outcomes::{FactionMetrics, MatchOutcome, faction_index};
use crate::shm::{
    MAGIC, PRIVATE_CAPACITY, PUBLIC_CAPACITY, REGION_KIND_ENL, REGION_KIND_PUBLIC, REGION_KIND_RES,
    RegionHeader, ShmPublisher, read_snapshot, region_names,
};
use arena_replay::format::OUTCOME_CLIENT_FORFEIT;
use arena_replay::recorder::ReplayWriter;

/// Everything the server needs to run one match.
pub struct MatchSetup {
    pub game_seed: u64,
    pub map_seed: u64,
    pub config: arena_core::config::GameConfig,
    pub socket_path: PathBuf,
    pub match_id: String,
    pub out_dir: PathBuf,
}

/// Result of a served match (returned in memory; `serve_match` also writes
/// `journal.txt`, `result.txt`, and — when the match entered Round 1 —
/// `replay.iarp` under the output directory).
pub struct MatchReport {
    pub outcome: MatchOutcome,
    pub replay_path: Option<PathBuf>,
    pub journal: String,
    pub result: String,
    pub metrics: [FactionMetrics; 2],
    pub rounds_completed: u32,
}

/// The accept thread's share of the channel work.
struct AcceptShare {
    handle: std::thread::JoinHandle<()>,
}

/// Internal driver bundle.
struct Driver {
    cfg: arena_core::config::GameConfig,
    game_seed: u64,
    map: MapDefinition,
    match_id: String,
    out_dir: PathBuf,
    rx: Receiver<ConnEvent>,
    /// Write halves by connection slot.
    conns: [Option<UnixStream>; 2],
    /// Faction assigned per connection slot (after both HELLOs).
    conn_faction: [Option<Faction>; 2],
    journal: Journal,
    metrics: [FactionMetrics; 2],
    /// SHM publishers: public, ENL private, RES private.
    publishers: Option<[ShmPublisher; 3]>,
    /// Monotonic origin for the informational SHM timing fields.
    origin: Instant,
    state: Option<GameState>,
    resolver: Resolver,
    replay: Option<ReplayWriter>,
    snapshot_generation: u64,
    /// Set when an IARP file was written.
    replay_path: Option<PathBuf>,
    /// A connection slot whose writes stalled or errored (write timeout:
    /// the client stopped reading, or a broken pipe). The connection is
    /// dead for scoring purposes and its faction forfeits.
    write_stalled: Option<u8>,
}

impl Driver {
    fn faction_conn(&self, faction: Faction) -> Option<usize> {
        self.conn_faction
            .iter()
            .position(|assigned| *assigned == Some(faction))
    }

    fn conn_faction_of(&self, conn: u8) -> Option<Faction> {
        self.conn_faction.get(conn as usize).copied().flatten()
    }

    fn send_to(&mut self, faction: Faction, envelope: &Envelope) {
        if let Some(slot) = self.faction_conn(faction) {
            self.send_slot(slot, envelope);
        }
    }

    fn send_to_conn(&mut self, conn: u8, envelope: &Envelope) {
        self.send_slot(conn as usize, envelope);
    }

    /// Writes one envelope to a connection slot. Every server stream is
    /// armed with [`conn::SERVER_WRITE_TIMEOUT`], so a client that stays
    /// connected but stops READING makes this ERROR instead of blocking
    /// the driver forever; the connection is then marked stalled (no
    /// further writes — each would only re-block) and its faction
    /// forfeits (see [`Driver::collect_round`]).
    fn send_slot(&mut self, slot: usize, envelope: &Envelope) {
        if self.write_stalled == Some(slot as u8) {
            return; // Dead pipe: another write would only re-block.
        }
        let Some(stream) = self.conns.get_mut(slot).and_then(|c| c.as_mut()) else {
            return;
        };
        let frame = envelope_frame(envelope);
        if let Err(error) = conn::write_frame(stream, &frame) {
            self.journal.note("write_error", &format!("{error}"));
            self.journal.note(
                "write_stalled",
                &format!("conn={slot} client stopped reading or the pipe broke"),
            );
            self.write_stalled = Some(slot as u8);
        }
    }

    fn close_conn(&mut self, conn: u8) {
        if let Some(stream) = self.conns.get_mut(conn as usize).and_then(|c| c.as_mut()) {
            conn::hard_close(stream);
        }
    }

    fn elapsed_ms(&self, at: Instant) -> u64 {
        at.saturating_duration_since(self.origin).as_millis() as u64
    }

    fn publish_round_snapshots(&mut self, round: u32, round_start: Instant, deadline: Instant) {
        let state = self
            .state
            .as_ref()
            .expect("round snapshots need a live state");
        let public = Observation::for_faction(state, Faction::Enl); // public part is faction-blind
        let private_enl = Observation::for_faction(state, Faction::Enl);
        let private_res = Observation::for_faction(state, Faction::Res);
        self.snapshot_generation += 1;
        let generation = self.snapshot_generation;
        let round_start_ms = self.elapsed_ms(round_start);
        let deadline_ms = self.elapsed_ms(deadline);
        let publishers = self
            .publishers
            .as_mut()
            .expect("regions exist once the match started");
        let result = publishers[0].publish(
            &public_snapshot_bytes(&public, generation, round_start_ms, deadline_ms),
            round,
            round_start_ms,
            deadline_ms,
        );
        if let Err(error) = result {
            self.journal.note("shm_publish_error", &format!("{error}"));
        }
        if let Err(error) = publishers[1].publish(
            &private_snapshot_bytes(&private_enl, generation),
            round,
            round_start_ms,
            deadline_ms,
        ) {
            self.journal.note("shm_publish_error", &format!("{error}"));
        }
        if let Err(error) = publishers[2].publish(
            &private_snapshot_bytes(&private_res, generation),
            round,
            round_start_ms,
            deadline_ms,
        ) {
            self.journal.note("shm_publish_error", &format!("{error}"));
        }
        // The round's public snapshot is immediately verifiable.
        let (public_name, _, _) = region_names(&self.match_id);
        let header = RegionHeader {
            magic: MAGIC,
            layout_version: arena_protocol::LAYOUT_VERSION,
            region_kind: REGION_KIND_PUBLIC,
            schema_version: PUBLIC_SNAPSHOT_VERSION,
            capacity: PUBLIC_CAPACITY as u32,
        };
        if let Err(error) = read_snapshot(&public_name, header, &verify_public) {
            self.journal
                .note("shm_self_check_failed", &format!("{error}"));
        }
    }
}

fn verify_public(bytes: &[u8]) -> bool {
    arena_protocol::decode::decode_public(bytes).is_ok()
}

fn verify_private(bytes: &[u8]) -> bool {
    arena_protocol::decode::decode_private(bytes).is_ok()
}

/// Runs one whole match. Blocks until the match ends (normal, forfeit, or
/// setup error), then writes the artifacts and returns the report.
pub fn serve_match(setup: MatchSetup) -> io::Result<MatchReport> {
    setup
        .config
        .validate()
        .map_err(|error| io::Error::other(format!("invalid GameConfig: {error:?}")))?;
    // The LEGAL frame maximum is compiled into arena_protocol (D62) and
    // enforced by the transport; GameConfig carries the EXCLUSIVE ceiling
    // (ADR-0008's 65,536) and INITIAL_SPAWN_START publishes the enforced
    // legal maximum — one below it. A config disagreeing with the
    // compiled-in limit would advertise a contract the transport does not
    // honor (an audited client sizing its largest frame to the published
    // value would be hard-closed), so refuse it as an infrastructure
    // defect before anything is published.
    if setup.config.max_frame_payload_bytes != arena_protocol::MAX_FRAME_PAYLOAD_BYTES as u32 + 1 {
        return Err(io::Error::other(format!(
            "GameConfig.max_frame_payload_bytes ({}) does not match the protocol frame ceiling ({} = legal max {} + 1)",
            setup.config.max_frame_payload_bytes,
            MAX_FRAME_PAYLOAD_BYTES as u32 + 1,
            MAX_FRAME_PAYLOAD_BYTES
        )));
    }
    let map = generate_map(setup.map_seed, &setup.config)
        .map_err(|error| io::Error::other(format!("map generation failed: {error:?}")))?;
    std::fs::create_dir_all(&setup.out_dir)?;
    // Remove a stale socket file from a previous run, then bind.
    let socket_path = setup.socket_path.clone();
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    let listener = UnixListener::bind(&socket_path)?;

    let (tx, rx) = std::sync::mpsc::channel::<ConnEvent>();
    // The handshake window bounds how long we wait for two authenticated
    // clients: the spawn window plus a fixed connect slack.
    let handshake_deadline =
        Instant::now() + Duration::from_millis(setup.config.initial_spawn_window_ms as u64 + 5_000);
    let accept_stop = Arc::new(AtomicBool::new(false));
    let accept = spawn_accept_loop(
        listener,
        tx.clone(),
        handshake_deadline,
        Arc::clone(&accept_stop),
    )?;

    let mut driver = Driver {
        cfg: setup.config,
        game_seed: setup.game_seed,
        map,
        match_id: setup.match_id.clone(),
        out_dir: setup.out_dir.clone(),
        rx,
        conns: [None, None],
        conn_faction: [None, None],
        journal: Journal::new(),
        metrics: [FactionMetrics::default(), FactionMetrics::default()],
        publishers: None,
        origin: Instant::now(),
        state: None,
        resolver: Resolver::with_seed(setup.game_seed),
        replay: None,
        snapshot_generation: 0,
        replay_path: None,
        write_stalled: None,
    };
    driver.journal.start_clock();
    driver.journal.event("match_id", &setup.match_id);
    driver
        .journal
        .event("game_seed", &setup.game_seed.to_string());

    let outcome = driver.drive(handshake_deadline);

    // Tear down: DROP the write halves without shutdown — a SHUT_RDWR at
    // match end can RST the socket while the final GAME_END frame is still
    // queued, destroying it for the client. The reader threads keep their
    // cloned read halves open, so no RST is generated here; the socket
    // fully closes once the clients disconnect (EOF) and the readers drop
    // their halves. Over-limit termination is the one place that DOES
    // hard-close without draining (D62), via `close_conn`.
    driver.conns = [None, None];
    driver.publishers = None;
    accept_stop.store(true, Ordering::Relaxed);
    let _ = accept.handle.join();
    let _ = std::fs::remove_file(&socket_path);

    driver.journal.outcome(outcome.tag());
    let rounds_completed = match &outcome {
        MatchOutcome::Normal {
            rounds_completed, ..
        } => *rounds_completed,
        MatchOutcome::ClientForfeit { at_round, .. } => *at_round,
        _ => 0,
    };
    let result = crate::outcomes::result_file_text(&outcome, &driver.metrics, rounds_completed);
    let journal_text = driver.journal.text();
    std::fs::write(setup.out_dir.join("journal.txt"), &journal_text)?;
    std::fs::write(setup.out_dir.join("result.txt"), &result)?;

    Ok(MatchReport {
        outcome,
        replay_path: driver.replay_path,
        journal: journal_text,
        result,
        metrics: driver.metrics,
        rounds_completed,
    })
}

/// Spawns the accept thread: exactly two connections are handed to the
/// driver; every further connection receives SERVER_HELLO status=2 and is
/// closed. Reports `HandshakeIncomplete` if the window closes early.
fn spawn_accept_loop(
    listener: UnixListener,
    tx: Sender<ConnEvent>,
    handshake_deadline: Instant,
    stop: Arc<AtomicBool>,
) -> io::Result<AcceptShare> {
    listener.set_nonblocking(true)?;
    let handle = std::thread::Builder::new()
        .name("accept-loop".into())
        .spawn(move || {
            let mut connected = 0u8;
            // Phase 1: accept two connections before the deadline.
            while connected < 2 {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let slot = connected;
                        connected += 1;
                        // Bound the driver's writes on this socket from
                        // the start: a client that never reads must not be
                        // able to pin the driver in write_all forever.
                        crate::conn::arm_write_timeout(&stream);
                        let read_half = match stream.try_clone() {
                            Ok(half) => half,
                            Err(_) => continue,
                        };
                        crate::conn::spawn_reader(slot, read_half, tx.clone());
                        if tx
                            .send(ConnEvent::Connected { conn: slot, stream })
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if Instant::now() >= handshake_deadline {
                            let _ = tx.send(ConnEvent::HandshakeIncomplete);
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => return,
                }
            }
            // Phase 2: reject every surplus connection until the driver is
            // done (its receiver drops and sends start failing).
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        crate::conn::arm_write_timeout(&stream);
                        let reject = envelope_frame(&Envelope::ServerHello {
                            status: HELLO_DUPLICATE_CONNECTION,
                            assigned_faction: Faction::Enl,
                            match_id: String::new(),
                        });
                        let _ = conn::write_frame(&mut stream, &reject);
                        conn::hard_close(&mut stream);
                        let _ = tx.send(ConnEvent::SurplusRejected);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => return,
                }
            }
        })
        .expect("accept thread spawns");
    Ok(AcceptShare { handle })
}

/// One in-progress round's bookkeeping.
struct RoundCollect {
    round: u32,
    round_start: Instant,
    /// The instant the first ROUND_START_SIGNAL for this round was
    /// written: the earliest moment an HONEST client could submit for
    /// this round (it acts on the signal; round identity is published
    /// with the snapshot just before). A frame claiming this round that
    /// completed earlier claimed a FUTURE round at arrival.
    signal_sent: Instant,
    deadline: Instant,
    submissions: Vec<SubmittedCommandStream>,
    frozen: BTreeSet<AgentId>,
    faction_committed: [bool; 2],
}

impl Driver {
    /// The whole FSM. Returns the outcome; `replay`/`state` are consumed
    /// by the caller path via `self`.
    fn drive(&mut self, handshake_deadline: Instant) -> MatchOutcome {
        match self.await_handshake(handshake_deadline) {
            HandshakeResult::BothReady => {}
            HandshakeResult::Abort(outcome) => return outcome,
        }
        match self.await_spawn() {
            SpawnResult::Both(enl, res) => {
                // Enter the match.
                match GameState::new(self.cfg, &self.map, &enl, &res) {
                    Ok(state) => {
                        self.state = Some(state);
                        self.replay = Some(ReplayWriter::new(
                            self.game_seed,
                            &self.cfg,
                            &self.map,
                            &enl,
                            &res,
                        ));
                    }
                    Err(error) => {
                        return MatchOutcome::SetupError {
                            reason: format!("state construction failed: {error:?}"),
                        };
                    }
                }
            }
            SpawnResult::Abort(outcome) => return outcome,
        }
        self.round_loop()
    }

    // -- Phase 1: handshake ----------------------------------------------

    fn await_handshake(&mut self, deadline: Instant) -> HandshakeResult {
        let mut hellos: [Option<(String, [u32; 3])>; 2] = [None, None];
        loop {
            let timeout = conn::remaining_until(deadline);
            let event = match self.rx.recv_timeout(timeout) {
                Ok(event) => event,
                Err(_) => {
                    return HandshakeResult::Abort(MatchOutcome::SetupError {
                        reason: "handshake window expired before two CLIENT_HELLOs".into(),
                    });
                }
            };
            match event {
                ConnEvent::Connected { conn, stream } => {
                    self.conns[conn as usize] = Some(stream);
                    self.journal.event("connected", &format!("conn={conn}"));
                }
                ConnEvent::Frame {
                    conn,
                    envelope:
                        Ok(Envelope::ClientHello {
                            client_name,
                            protocol_version,
                            public_snapshot_version,
                            private_snapshot_version,
                            ..
                        }),
                    ..
                } => {
                    let triple = [
                        protocol_version,
                        public_snapshot_version,
                        private_snapshot_version,
                    ];
                    self.journal.event(
                        "client_hello",
                        &format!("conn={conn} name={client_name:?} versions={triple:?}"),
                    );
                    if hellos[conn as usize].is_none() {
                        hellos[conn as usize] = Some((client_name, triple));
                    }
                    // Both HELLOs collected: exact-match check, then start.
                    if hellos.iter().all(|h| h.is_some()) {
                        let triples = [hellos[0].as_ref(), hellos[1].as_ref()];
                        let expected = [
                            PROTOCOL_VERSION,
                            PUBLIC_SNAPSHOT_VERSION,
                            PRIVATE_SNAPSHOT_VERSION,
                        ];
                        let mismatched: Vec<usize> = (0..2)
                            .filter(|&i| triples[i].map(|(_, t)| t) != Some(&expected))
                            .collect();
                        if !mismatched.is_empty() {
                            // Abort BEFORE any map publish (unscored, never
                            // a SPAWN_FORFEIT; P3-RPC).
                            self.journal.note(
                                "version_mismatch",
                                &format!("conns={mismatched:?} expected={expected:?}"),
                            );
                            for conn in 0..2u8 {
                                self.send_to_conn(
                                    conn,
                                    &Envelope::ServerHello {
                                        status: HELLO_VERSION_MISMATCH,
                                        assigned_faction: Faction::Enl,
                                        match_id: String::new(),
                                    },
                                );
                            }
                            return HandshakeResult::Abort(MatchOutcome::SetupError {
                                reason: format!("version mismatch on connection(s) {mismatched:?}"),
                            });
                        }
                        // First HELLO -> ENL, second -> RES (accept order).
                        self.conn_faction = [Some(Faction::Enl), Some(Faction::Res)];
                        for conn in 0..2u8 {
                            let faction = self.conn_faction[conn as usize].unwrap();
                            self.send_to_conn(
                                conn,
                                &Envelope::ServerHello {
                                    status: HELLO_OK,
                                    assigned_faction: faction,
                                    match_id: self.match_id.clone(),
                                },
                            );
                        }
                        return HandshakeResult::BothReady;
                    }
                }
                ConnEvent::Frame {
                    conn,
                    envelope: Ok(_),
                    ..
                } => {
                    self.journal
                        .note("pre_start_noise", &format!("conn={conn}"));
                }
                ConnEvent::Frame {
                    conn,
                    envelope: Err(_),
                    ..
                } => self.journal.envelope_reject(conn),
                ConnEvent::OverLimit { conn, declared } => {
                    self.journal.note(
                        "over_limit_pre_start",
                        &format!("conn={conn} declared={declared}"),
                    );
                    self.close_conn(conn);
                    return HandshakeResult::Abort(MatchOutcome::SetupError {
                        reason: format!(
                            "over-limit frame (declared {declared}) before match start"
                        ),
                    });
                }
                ConnEvent::Disconnected { conn } => {
                    self.journal
                        .note("disconnected_pre_start", &format!("conn={conn}"));
                    return HandshakeResult::Abort(MatchOutcome::SetupError {
                        reason: format!("connection {conn} lost before match start"),
                    });
                }
                ConnEvent::SurplusRejected => {
                    self.journal.event("surplus_rejected", "handshake");
                }
                ConnEvent::HandshakeIncomplete => {
                    return HandshakeResult::Abort(MatchOutcome::SetupError {
                        reason: "handshake incomplete".into(),
                    });
                }
            }
        }
    }

    // -- Phase 2: initial spawn window -----------------------------------

    fn await_spawn(&mut self) -> SpawnResult {
        // Publish generation 0: the frozen map plus both private rosters,
        // then INITIAL_SPAWN_START; the window runs from the publish.
        let (public_name, enl_name, res_name) = region_names(&self.match_id);
        let publishers = match [
            ShmPublisher::create(
                &public_name,
                REGION_KIND_PUBLIC,
                PUBLIC_SNAPSHOT_VERSION,
                PUBLIC_CAPACITY,
            ),
            ShmPublisher::create(
                &enl_name,
                REGION_KIND_ENL,
                PRIVATE_SNAPSHOT_VERSION,
                PRIVATE_CAPACITY,
            ),
            ShmPublisher::create(
                &res_name,
                REGION_KIND_RES,
                PRIVATE_SNAPSHOT_VERSION,
                PRIVATE_CAPACITY,
            ),
        ] {
            [Ok(a), Ok(b), Ok(c)] => [a, b, c],
            errors => {
                return SpawnResult::Abort(MatchOutcome::SetupError {
                    reason: format!("SHM region creation failed: {errors:?}"),
                });
            }
        };
        self.publishers = Some(publishers);
        let spawn_start = Instant::now();
        let spawn_deadline =
            spawn_start + Duration::from_millis(self.cfg.initial_spawn_window_ms as u64);
        let round_start_ms = self.elapsed_ms(spawn_start);
        let deadline_ms = round_start_ms + self.cfg.initial_spawn_window_ms as u64;
        {
            let publishers = self.publishers.as_mut().expect("just created");
            let map_bytes = map_generation_bytes(&self.map, round_start_ms, deadline_ms);
            if let Err(error) = publishers[0].publish(&map_bytes, 0, round_start_ms, deadline_ms) {
                return SpawnResult::Abort(MatchOutcome::SetupError {
                    reason: format!("generation-0 publish failed: {error}"),
                });
            }
            let enl_bytes = private_roster_bytes(&self.cfg, Faction::Enl);
            if let Err(error) = publishers[1].publish(&enl_bytes, 0, round_start_ms, deadline_ms) {
                return SpawnResult::Abort(MatchOutcome::SetupError {
                    reason: format!("ENL roster publish failed: {error}"),
                });
            }
            let res_bytes = private_roster_bytes(&self.cfg, Faction::Res);
            if let Err(error) = publishers[2].publish(&res_bytes, 0, round_start_ms, deadline_ms) {
                return SpawnResult::Abort(MatchOutcome::SetupError {
                    reason: format!("RES roster publish failed: {error}"),
                });
            }
        }
        // Self-check generation 0 before licensing reads.
        let header_public = RegionHeader {
            magic: MAGIC,
            layout_version: arena_protocol::LAYOUT_VERSION,
            region_kind: REGION_KIND_PUBLIC,
            schema_version: PUBLIC_SNAPSHOT_VERSION,
            capacity: PUBLIC_CAPACITY as u32,
        };
        if let Err(error) = read_snapshot(&public_name, header_public, &verify_public) {
            return SpawnResult::Abort(MatchOutcome::SetupError {
                reason: format!("generation-0 public snapshot failed verification: {error}"),
            });
        }
        let header_enl = RegionHeader {
            magic: MAGIC,
            layout_version: arena_protocol::LAYOUT_VERSION,
            region_kind: REGION_KIND_ENL,
            schema_version: PRIVATE_SNAPSHOT_VERSION,
            capacity: PRIVATE_CAPACITY as u32,
        };
        if let Err(error) = read_snapshot(&enl_name, header_enl, &verify_private) {
            return SpawnResult::Abort(MatchOutcome::SetupError {
                reason: format!("generation-0 ENL snapshot failed verification: {error}"),
            });
        }
        let header_res = RegionHeader {
            magic: MAGIC,
            layout_version: arena_protocol::LAYOUT_VERSION,
            region_kind: REGION_KIND_RES,
            schema_version: PRIVATE_SNAPSHOT_VERSION,
            capacity: PRIVATE_CAPACITY as u32,
        };
        if let Err(error) = read_snapshot(&res_name, header_res, &verify_private) {
            return SpawnResult::Abort(MatchOutcome::SetupError {
                reason: format!("generation-0 RES snapshot failed verification: {error}"),
            });
        }

        let remaining = conn::remaining_until(spawn_deadline).as_millis() as u32;
        let info = MatchInfo::from_core(&self.cfg);
        for faction in [Faction::Enl, Faction::Res] {
            self.send_to(
                faction,
                &Envelope::InitialSpawnStart {
                    spawn_deadline_remaining_ms: remaining,
                    info,
                },
            );
        }
        self.journal
            .event("initial_spawn_start", &format!("remaining_ms={remaining}"));

        // Collect spawn sets until both valid sets are in or the deadline.
        let (enl_roster, res_roster) = GameState::initial_roster(&self.cfg);
        let mut sets: [Option<InitialSpawns>; 2] = [None, None];
        loop {
            let timeout = conn::remaining_until(spawn_deadline);
            let event = match self.rx.recv_timeout(timeout) {
                Ok(event) => event,
                Err(_) => {
                    // Deadline: every faction without a valid set forfeits.
                    let missing_enl = sets[0].is_none();
                    let missing_res = sets[1].is_none();
                    return self.spawn_forfeit(missing_enl, missing_res, "missed spawn deadline");
                }
            };
            match event {
                ConnEvent::Frame {
                    conn,
                    received_at,
                    envelope: Ok(Envelope::SubmitInitialSpawns { positions }),
                    ..
                } => {
                    let Some(faction) = self.conn_faction_of(conn) else {
                        continue;
                    };
                    let index = faction_index(faction);
                    if sets[index].is_some() {
                        self.journal
                            .note("spawn_duplicate", &format!("conn={conn}"));
                        continue;
                    }
                    let roster = match faction {
                        Faction::Enl => &enl_roster,
                        Faction::Res => &res_roster,
                    };
                    let entries: Vec<(AgentId, arena_core::types::Coord)> = positions
                        .iter()
                        .map(|SpawnEntry { agent_id, position }| (*agent_id, *position))
                        .collect();
                    if received_at >= spawn_deadline {
                        self.send_to(
                            faction,
                            &Envelope::InitialSpawnResult {
                                status: 2,
                                detail: "spawn deadline passed".into(),
                            },
                        );
                        self.journal.spawn_set(faction, false, "deadline");
                        // This faction failed, but the OTHER faction's fate
                        // is still open (R-SPAWN-01/P3-TIMEOUT: both sides
                        // failing is UNSCORED). Its set may already sit in
                        // the queue — `recv_timeout` with a zero remaining
                        // window drains queued events before timing out —
                        // or it may never submit. Continuing reaches the
                        // deadline path above, which evaluates BOTH sets
                        // and yields the unscored SpawnForfeit when both
                        // are missing; returning here instead would score a
                        // technical win for a silent opponent.
                        continue;
                    }
                    match validate_initial_spawn_set(roster, &entries, &self.cfg) {
                        Ok(valid) => {
                            sets[index] = Some(valid);
                            self.send_to(
                                faction,
                                &Envelope::InitialSpawnResult {
                                    status: 0,
                                    detail: "accepted".into(),
                                },
                            );
                            self.journal.spawn_set(faction, true, "accepted");
                        }
                        Err(error) => {
                            self.send_to(
                                faction,
                                &Envelope::InitialSpawnResult {
                                    status: 1,
                                    detail: format!("{error:?}"),
                                },
                            );
                            self.journal
                                .spawn_set(faction, false, &format!("{error:?}"));
                            return self.spawn_forfeit(
                                index == 0,
                                index == 1,
                                &format!("invalid initial spawn set: {error:?}"),
                            );
                        }
                    }
                    if sets[0].is_some() && sets[1].is_some() {
                        return SpawnResult::Both(
                            sets[0].take().expect("enl set"),
                            sets[1].take().expect("res set"),
                        );
                    }
                }
                ConnEvent::Frame {
                    conn,
                    envelope: Ok(_),
                    ..
                } => self
                    .journal
                    .note("spawn_phase_noise", &format!("conn={conn}")),
                ConnEvent::Frame {
                    conn,
                    envelope: Err(_),
                    ..
                } => self.journal.envelope_reject(conn),
                ConnEvent::OverLimit { conn, declared } => {
                    // After INITIAL_SPAWN_START: CLIENT_FORFEIT.
                    self.close_conn(conn);
                    let Some(faction) = self.conn_faction_of(conn) else {
                        continue;
                    };
                    self.journal.note(
                        "client_forfeit",
                        &format!("faction={faction:?} reason=over-limit frame declared={declared}"),
                    );
                    return SpawnResult::Abort(MatchOutcome::ClientForfeit {
                        winner: Some(faction.opposite()),
                        at_round: 0,
                        reason_losing: format!("over-limit frame (declared {declared})"),
                    });
                }
                ConnEvent::Disconnected { conn } => {
                    let Some(faction) = self.conn_faction_of(conn) else {
                        continue;
                    };
                    self.journal.note(
                        "client_forfeit",
                        &format!("faction={faction:?} reason=disconnected during spawn window"),
                    );
                    return SpawnResult::Abort(MatchOutcome::ClientForfeit {
                        winner: Some(faction.opposite()),
                        at_round: 0,
                        reason_losing: "process exited during the spawn window".into(),
                    });
                }
                ConnEvent::Connected { .. } => {}
                ConnEvent::SurplusRejected => {
                    self.journal.event("surplus_rejected", "spawn");
                }
                ConnEvent::HandshakeIncomplete => {}
            }
        }
    }

    fn spawn_forfeit(&mut self, enl_failed: bool, res_failed: bool, reason: &str) -> SpawnResult {
        let winner = match (enl_failed, res_failed) {
            (true, false) => Some(Faction::Res),
            (false, true) => Some(Faction::Enl),
            _ => None,
        };
        for faction in [Faction::Enl, Faction::Res] {
            self.send_to(
                faction,
                &Envelope::GameEnd {
                    outcome: END_SPAWN_FORFEIT,
                    detail: reason.into(),
                    score_enl: 0,
                    score_res: 0,
                    rounds_completed: 0,
                },
            );
        }
        SpawnResult::Abort(MatchOutcome::SpawnForfeit {
            winner,
            reason_losing: reason.into(),
        })
    }

    // -- Phase 3: the round loop -----------------------------------------

    fn round_loop(&mut self) -> MatchOutcome {
        let total_rounds = self.cfg.total_rounds;
        for round in 1..=total_rounds {
            // Publish fresh snapshots, then signal in parity order.
            let round_start = Instant::now();
            let deadline = round_start + Duration::from_millis(self.cfg.decision_window_ms as u64);
            self.publish_round_snapshots(round, round_start, deadline);
            let notified_first = if round % 2 == 1 {
                Faction::Res
            } else {
                Faction::Enl
            };
            let (first, second) = (notified_first, notified_first.opposite());
            let signal_sent = Instant::now();
            self.send_to(first, &Envelope::RoundStartSignal);
            self.send_to(second, &Envelope::RoundStartSignal);
            self.journal.round_start(round, notified_first);
            self.metrics[faction_index(first)].notified_first_rounds += 1;

            let mut collect = RoundCollect {
                round,
                round_start,
                signal_sent,
                deadline,
                submissions: Vec::new(),
                frozen: BTreeSet::new(),
                faction_committed: [false, false],
            };
            match self.collect_round(&mut collect) {
                CollectEnd::InputClosed => {}
                CollectEnd::Forfeit(faction, reason) => {
                    // Input closes for everyone; the interrupted round is
                    // settled with whatever arrived (deterministic,
                    // recorded), then the match ends technically.
                    self.send_to(Faction::Enl, &Envelope::RoundClosed { round });
                    self.send_to(Faction::Res, &Envelope::RoundClosed { round });
                    let (report, completed) = self.settle_round(&mut collect);
                    let _ = report;
                    let replay_bytes = self.finish_replay_abnormal();
                    let _ = replay_bytes;
                    for to in [Faction::Enl, Faction::Res] {
                        if to != faction {
                            self.send_to(
                                to,
                                &Envelope::GameEnd {
                                    outcome: END_CLIENT_FORFEIT,
                                    detail: reason.clone(),
                                    score_enl: self
                                        .state
                                        .as_ref()
                                        .map(|s| s.score(Faction::Enl))
                                        .unwrap_or(0),
                                    score_res: self
                                        .state
                                        .as_ref()
                                        .map(|s| s.score(Faction::Res))
                                        .unwrap_or(0),
                                    rounds_completed: completed,
                                },
                            );
                        }
                    }
                    return MatchOutcome::ClientForfeit {
                        winner: Some(faction.opposite()),
                        at_round: completed,
                        reason_losing: reason,
                    };
                }
            }
            // Normal close: announce Settlement Phase, then settle.
            self.send_to(Faction::Enl, &Envelope::RoundClosed { round });
            self.send_to(Faction::Res, &Envelope::RoundClosed { round });
            let (_, completed) = self.settle_round(&mut collect);
            if round == total_rounds {
                return self.finish_normal(completed);
            }
        }
        unreachable!("the loop returns on the final round")
    }

    fn finish_normal(&mut self, completed: u32) -> MatchOutcome {
        let state = self.state.as_ref().expect("state lives through the match");
        let result = state.final_result();
        let (score_enl, score_res) = (state.score(Faction::Enl), state.score(Faction::Res));
        let outcome = MatchOutcome::Normal {
            result,
            score_enl,
            score_res,
            rounds_completed: completed,
        };
        let replay = self
            .replay
            .take()
            .expect("replay writer lives through the match");
        let bytes = replay
            .finish(state)
            .expect("the recorded match produces a decodable IARP file");
        let path = self.out_dir.join("replay.iarp");
        std::fs::write(&path, &bytes).expect("replay file writes");
        self.replay_path = Some(path);
        for faction in [Faction::Enl, Faction::Res] {
            self.send_to(
                faction,
                &Envelope::GameEnd {
                    outcome: outcome.game_end_code(),
                    detail: outcome.tag().into(),
                    score_enl,
                    score_res,
                    rounds_completed: completed,
                },
            );
        }
        outcome
    }

    fn finish_replay_abnormal(&mut self) -> Option<Vec<u8>> {
        let state = self.state.as_ref()?;
        let replay = self.replay.take()?;
        match replay.finish_abnormal(state, OUTCOME_CLIENT_FORFEIT) {
            Ok(bytes) => {
                let path = self.out_dir.join("replay.iarp");
                std::fs::write(&path, &bytes).expect("replay file writes");
                self.replay_path = Some(path);
                Some(bytes)
            }
            Err(error) => {
                self.journal
                    .note("replay_abnormal_failed", &format!("{error:?}"));
                None
            }
        }
    }

    /// Collects one round's input until both factions complete or the
    /// deadline (P3-TIMEOUT). A slow client never blocks settlement: late
    /// frames are drained and dropped without freezing anything.
    fn collect_round(&mut self, collect: &mut RoundCollect) -> CollectEnd {
        loop {
            // A stalled write on a live connection is a disconnect by any
            // other name (P3-TIMEOUT): the client can no longer receive
            // verdicts or signals, so its faction forfeits and the match
            // ends instead of the server hanging on a full pipe.
            if let Some(conn) = self.write_stalled {
                if let Some(faction) = self.conn_faction_of(conn) {
                    return CollectEnd::Forfeit(
                        faction,
                        "write stalled: client stopped reading the connection".into(),
                    );
                }
            }
            if self.both_factions_complete(collect) {
                return CollectEnd::InputClosed;
            }
            let timeout = conn::remaining_until(collect.deadline);
            let event = match self.rx.recv_timeout(timeout) {
                Ok(event) => event,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    return CollectEnd::InputClosed;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Accept thread gone mid-match (infrastructure fault):
                    // close input; settlement proceeds with what arrived.
                    self.journal
                        .note("event_channel_closed", "accept loop vanished");
                    return CollectEnd::InputClosed;
                }
            };
            match event {
                ConnEvent::Frame {
                    conn,
                    received_at,
                    receive_seq,
                    envelope:
                        Ok(Envelope::SubmitCommandStream {
                            round: claimed_round,
                            agent_id,
                            commands,
                        }),
                } => {
                    let Some(faction) = self.conn_faction_of(conn) else {
                        continue;
                    };
                    if received_at >= collect.deadline {
                        // Completed at/after the deadline: dropped at the
                        // transport, never enters a slot (P3-TIMEOUT).
                        self.journal.late_drop(collect.round, agent_id.0);
                        self.send_to(
                            faction,
                            &Envelope::SubmissionResult {
                                round: claimed_round,
                                agent_id,
                                status: SUBMIT_LATE_DROP,
                                detail: "frame completed at/after the deadline".into(),
                            },
                        );
                        continue;
                    }
                    let received_at_ms = received_at
                        .saturating_duration_since(collect.round_start)
                        .as_millis() as u64;
                    collect.submissions.push(SubmittedCommandStream {
                        round: claimed_round,
                        faction,
                        agent_id,
                        received_at_ms,
                        receive_seq,
                        commands: commands.clone(),
                    });
                    let state = self.state.as_ref().expect("live state");
                    let slot_frozen = collect.frozen.contains(&agent_id);
                    // A frame claiming an ALREADY-CLOSED round completed
                    // after that round's input closed (the cross-deadline
                    // half-frame): dropped at the transport, never enters
                    // a slot (P3-TIMEOUT; D64). Claims of a FUTURE round
                    // stay envelope-unlocatable (WrongRound) below.
                    if claimed_round < collect.round {
                        self.journal.late_drop(collect.round - 1, agent_id.0);
                        self.send_to(
                            faction,
                            &Envelope::SubmissionResult {
                                round: claimed_round,
                                agent_id,
                                status: SUBMIT_LATE_DROP,
                                detail: "frame completed after its round's deadline".into(),
                            },
                        );
                        // Remove it again: a late frame never reaches the
                        // resolver's echo (it was dropped, not submitted).
                        collect.submissions.pop();
                        continue;
                    }
                    // A frame whose claim matches THIS round but which
                    // completed before this round's ROUND_START_SIGNAL
                    // was written (the pipelining exploit:
                    // SUBMIT_COMMAND_STREAM(round N+1) fired the instant
                    // ROUND_CLOSED(N) is observed, completing during round
                    // N's settlement or the N+1 snapshot publish) claimed
                    // a FUTURE round at arrival — no honest client can
                    // submit for a round before its signal, so this is
                    // envelope-unlocatable WrongRound exactly like the
                    // in-round future claim (R-RESOLVE-01, R-RESOLVE-03:
                    // no guaranteed bucket-0 priority, no frozen slot, no
                    // settlement-window read of a snapshot it never saw).
                    // It must NOT be forwarded: its claimed round equals
                    // the current round, so the resolver could not reject
                    // it on its own — pop it back off the echo.
                    if claimed_round == collect.round && received_at < collect.signal_sent {
                        self.journal.unlocatable(collect.round, agent_id.0);
                        self.send_to(
                            faction,
                            &Envelope::SubmissionResult {
                                round: claimed_round,
                                agent_id,
                                status: SUBMIT_UNLOCATABLE,
                                detail: "frame completed before this round's start signal was sent: its round claim was still in the future at arrival".into(),
                            },
                        );
                        collect.submissions.pop();
                        continue;
                    }
                    let locatable = claimed_round == collect.round
                        && state
                            .agent(agent_id)
                            .map(|agent| agent.faction == faction)
                            .unwrap_or(false);
                    if !locatable {
                        self.journal.unlocatable(collect.round, agent_id.0);
                        self.send_to(
                            faction,
                            &Envelope::SubmissionResult {
                                round: claimed_round,
                                agent_id,
                                status: SUBMIT_UNLOCATABLE,
                                detail: "wrong round, unknown agent, or faction mismatch".into(),
                            },
                        );
                        continue;
                    }
                    if slot_frozen {
                        self.send_to(
                            faction,
                            &Envelope::SubmissionResult {
                                round: claimed_round,
                                agent_id,
                                status: SUBMIT_IGNORED_DUPLICATE,
                                detail: "first locatable submission already froze this slot".into(),
                            },
                        );
                        continue;
                    }
                    collect.frozen.insert(agent_id);
                    let metrics = &mut self.metrics[faction_index(faction)];
                    metrics
                        .latency_histogram_ms
                        .entry(received_at_ms)
                        .and_modify(|c| *c += 1)
                        .or_insert(1);
                    metrics.submitted_slots += 1;
                    let verdict =
                        structural_check(&commands, self.cfg.max_commands_per_agent_round);
                    let (status, detail) = match verdict {
                        Ok(()) => (SUBMIT_ACCEPTED, "accepted".to_string()),
                        Err(StreamRejection::TooManyCommands { count, max }) => (
                            SUBMIT_REJECTED_STRUCTURAL,
                            format!("too many commands: {count} > {max}"),
                        ),
                        Err(StreamRejection::MultipleMajorActions { count }) => (
                            SUBMIT_REJECTED_STRUCTURAL,
                            format!("multiple major actions: {count}"),
                        ),
                    };
                    self.send_to(
                        faction,
                        &Envelope::SubmissionResult {
                            round: claimed_round,
                            agent_id,
                            status,
                            detail,
                        },
                    );
                }
                ConnEvent::Frame {
                    conn,
                    envelope: Ok(Envelope::CommitRound { round }),
                    ..
                } => {
                    let Some(faction) = self.conn_faction_of(conn) else {
                        continue;
                    };
                    if round == collect.round && !collect.faction_committed[faction_index(faction)]
                    {
                        collect.faction_committed[faction_index(faction)] = true;
                        self.journal.event(
                            "commit_round",
                            &format!("round={round} faction={faction:?}"),
                        );
                    }
                }
                ConnEvent::Frame {
                    conn,
                    envelope:
                        Ok(Envelope::ClientHello {
                            protocol_version, ..
                        }),
                    ..
                } => {
                    // "运行中声明错误版本" is an envelope reject (P3-RPC).
                    self.journal.note(
                        "mid_match_hello_rejected",
                        &format!("conn={conn} protocol_version={protocol_version}"),
                    );
                }
                ConnEvent::Frame {
                    conn,
                    envelope: Ok(_),
                    ..
                } => {
                    self.journal
                        .note("unexpected_message", &format!("conn={conn}"));
                }
                ConnEvent::Frame {
                    conn,
                    envelope: Err(_),
                    ..
                } => {
                    // In-limit undecodable: recoverable envelope reject.
                    self.journal.envelope_reject(conn);
                }
                ConnEvent::OverLimit { conn, declared } => {
                    self.close_conn(conn);
                    let Some(faction) = self.conn_faction_of(conn) else {
                        continue;
                    };
                    return CollectEnd::Forfeit(
                        faction,
                        format!("over-limit frame (declared {declared})"),
                    );
                }
                ConnEvent::Disconnected { conn } => {
                    let Some(faction) = self.conn_faction_of(conn) else {
                        continue;
                    };
                    return CollectEnd::Forfeit(faction, "client disconnected".into());
                }
                ConnEvent::Connected { .. } => {}
                ConnEvent::SurplusRejected => {
                    self.journal.event("surplus_rejected", "round");
                }
                ConnEvent::HandshakeIncomplete => {}
            }
        }
    }

    fn both_factions_complete(&self, collect: &RoundCollect) -> bool {
        let state = self.state.as_ref().expect("live state");
        for faction in [Faction::Enl, Faction::Res] {
            let index = faction_index(faction);
            if collect.faction_committed[index] {
                continue;
            }
            let any_open = state
                .agents()
                .values()
                .any(|agent| agent.faction == faction && !collect.frozen.contains(&agent.id));
            if any_open {
                return false;
            }
        }
        true
    }

    /// Settles one round through arena-core's Resolver (the server adds
    /// nothing), advances, records the IARP round, journals, and folds
    /// metrics. Returns the report and the count of completed rounds.
    fn settle_round(&mut self, collect: &mut RoundCollect) -> (RoundReport, u32) {
        let report = {
            let state = self.state.as_mut().expect("live state");
            let report = self.resolver.run_round(state, &collect.submissions);
            state.advance_round();
            report
        };
        if let Some(replay) = self.replay.as_mut() {
            let state = self.state.as_ref().expect("live state");
            if let Err(error) = replay.record_round(&report, state) {
                self.journal
                    .note("replay_record_failed", &format!("{error:?}"));
            }
        }
        let duration_ms = collect.round_start.elapsed().as_millis() as u64;
        self.journal
            .round_closed(collect.round, duration_ms, collect.submissions.len());

        // Metrics: bucket distribution + timeout rate from the report.
        for stream in &report.execution_order {
            let metrics = &mut self.metrics[faction_index(stream.faction)];
            metrics
                .bucket_distribution
                .entry(stream.receive_bucket)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
        for (agent, slot) in &report.slots {
            let Some(agent_ref) = self.state.as_ref().and_then(|s| s.agent(*agent)) else {
                continue;
            };
            let faction = agent_ref.faction;
            let metrics = &mut self.metrics[faction_index(faction)];
            match slot {
                SlotStatus::NotSubmitted => {
                    // An unfrozen agent is a genuine timeout only when its
                    // faction did not COMMIT: a prompt COMMIT_ROUND freezes
                    // the rest as deliberate empty passes (P3-RPC), not
                    // timeouts.
                    if collect.faction_committed[faction_index(faction)] {
                        metrics.committed_passes += 1;
                    } else {
                        metrics.not_submitted_slots += 1;
                    }
                }
                SlotStatus::Executed { .. } | SlotStatus::Pass | SlotStatus::Rejected(_) => {}
            }
        }
        (report, collect.round)
    }
}

enum HandshakeResult {
    BothReady,
    Abort(MatchOutcome),
}

enum SpawnResult {
    Both(InitialSpawns, InitialSpawns),
    Abort(MatchOutcome),
}

enum CollectEnd {
    InputClosed,
    Forfeit(Faction, String),
}

//! The transport matrix (P3-GATE items c-h, j, k, l, m + receive
//! semantics): every case drives the REAL server (`serve_match` on a
//! thread) over real UDS sockets with the scripted raw client from
//! `arena_server::testutil`.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use arena_core::types::{AgentId, Coord, Faction};
use arena_protocol::decode::{
    END_SPAWN_FORFEIT, Envelope, HELLO_DUPLICATE_CONNECTION, HELLO_OK, HELLO_VERSION_MISMATCH,
    SUBMIT_ACCEPTED, SUBMIT_IGNORED_DUPLICATE, SUBMIT_UNLOCATABLE, SpawnEntry,
};
use arena_server::testutil::RawClient;
use arena_server::{MatchOutcome, MatchReport, MatchSetup, fast_config, serve_match};

/// Limits how many matches this test binary runs concurrently (default
/// test-threads equals the core count; 13 simultaneous matches starve the
/// tens-of-milliseconds protocol windows and cause spurious deadline
/// misses that are not the behavior under test).
struct MatchPermit(Arc<MatchSlots>);

struct MatchSlots {
    active: std::sync::Mutex<usize>,
    wakeup: std::sync::Condvar,
}

impl MatchPermit {
    fn acquire() -> Self {
        static SLOTS: std::sync::OnceLock<Arc<MatchSlots>> = std::sync::OnceLock::new();
        let slots = SLOTS.get_or_init(|| {
            Arc::new(MatchSlots {
                active: std::sync::Mutex::new(0),
                wakeup: std::sync::Condvar::new(),
            })
        });
        let mut active = slots.active.lock().expect("uncontended");
        while *active >= 3 {
            active = slots.wakeup.wait(active).expect("uncontended");
        }
        *active += 1;
        MatchPermit(Arc::clone(slots))
    }
}

impl Drop for MatchPermit {
    fn drop(&mut self) {
        let mut active = self.0.active.lock().expect("uncontended");
        *active -= 1;
        self.0.wakeup.notify_one();
    }
}

use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m7-transport")
        .join(format!("{tag}-{}", std::process::id()));
    // Idempotent: tests read artifacts back from here after the match.
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

struct Server {
    socket: PathBuf,
    #[allow(dead_code)]
    match_id: String,
    /// Receiver is Send but not Sync; stash it behind a Mutex so `&Server`
    /// can cross scoped threads.
    report: std::sync::Mutex<Option<mpsc::Receiver<MatchReport>>>,
}

impl Server {
    fn start(tag: &str, rounds: u32) -> Server {
        Self::start_config(tag, fast_config(rounds))
    }

    fn start_config(tag: &str, mut config: arena_core::config::GameConfig) -> Server {
        let dir = temp_dir(tag);
        let socket = dir.join("arena.sock");
        let match_id = format!("{tag}-{}", std::process::id());
        config.total_rounds = config.total_rounds.max(1);
        let setup = MatchSetup {
            game_seed: 0x5EED_0001,
            map_seed: 5,
            config,
            socket_path: socket.clone(),
            match_id: match_id.clone(),
            out_dir: dir.join("out"),
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let report = serve_match(setup).expect("serve_match runs");
            let _ = tx.send(report);
        });
        // Wait for the socket.
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !socket.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "server never bound the socket"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        Server {
            socket,
            match_id,
            report: std::sync::Mutex::new(Some(rx)),
        }
    }

    fn finish(self) -> MatchReport {
        self.report
            .lock()
            .expect("uncontended")
            .take()
            .expect("finish called once")
            .recv_timeout(Duration::from_secs(60))
            .expect("server finishes")
    }
}

/// How `play` performs its handshake.
enum Hello {
    Normal,
    /// CLIENT_HELLO padded so the frame payload is exactly 65,535 bytes.
    GiantName,
}

/// A minimal compliant participant: HELLO (in the chosen shape), a valid
/// spawn set for the ASSIGNED faction, `on_round` hooks, commit every
/// round, return on GAME_END. Faction-agnostic: the first connection to
/// HELLO becomes ENL, the second RES.
fn play(
    server: &Server,
    name: &str,
    hello: Hello,
    mut on_round: impl FnMut(&mut RawClient, u32, &[AgentId]),
) {
    let mut client = match hello {
        Hello::Normal => RawClient::hello(&server.socket, name).expect("connect"),
        Hello::GiantName => {
            let mut client = RawClient::connect(&server.socket, name).expect("connect");
            let envelope_for = |len: usize| {
                arena_protocol::build::envelope_bytes(&Envelope::ClientHello {
                    client_name: "P".repeat(len),
                    client_version: "1".into(),
                    bot_version: String::new(),
                    sdk_version: String::new(),
                    protocol_version: arena_protocol::PROTOCOL_VERSION,
                    public_snapshot_version: arena_protocol::PUBLIC_SNAPSHOT_VERSION,
                    private_snapshot_version: arena_protocol::PRIVATE_SNAPSHOT_VERSION,
                })
            };
            let mut lo = 1usize;
            let mut hi = arena_protocol::MAX_FRAME_PAYLOAD_BYTES;
            while lo < hi {
                let mid = (lo + hi + 1) / 2;
                if envelope_for(mid).len() <= arena_protocol::MAX_FRAME_PAYLOAD_BYTES {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            let payload = envelope_for(lo);
            assert_eq!(
                payload.len(),
                arena_protocol::MAX_FRAME_PAYLOAD_BYTES,
                "exact legal boundary"
            );
            let frame = arena_protocol::frame::encode_frame_vec(&payload).expect("legal");
            assert_eq!(frame.len(), 4 + arena_protocol::MAX_FRAME_PAYLOAD_BYTES);
            client.send_raw(&frame);
            client
        }
    };
    let faction = match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::ServerHello {
            status: HELLO_OK,
            assigned_faction,
            ..
        }) => assigned_faction,
        Some(other) => panic!("expected SERVER_HELLO ok, got {other:?}"),
        None => panic!("connection closed at hello"),
    };
    let info = match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::InitialSpawnStart { info, .. }) => info,
        Some(other) => panic!("expected INITIAL_SPAWN_START, got {other:?}"),
        None => panic!("connection closed before spawn start"),
    };
    let n = info.initial_agents_per_faction;
    let offset = match faction {
        Faction::Enl => 0,
        Faction::Res => n,
    };
    let positions: Vec<SpawnEntry> = (0..n)
        .map(|i| SpawnEntry {
            agent_id: AgentId(offset + i),
            position: Coord::new(i as i32 + 1, i as i32),
        })
        .collect();
    let roster: Vec<AgentId> = positions.iter().map(|entry| entry.agent_id).collect();
    client.send_envelope(&Envelope::SubmitInitialSpawns { positions });
    match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::InitialSpawnResult { status: 0, .. }) => {}
        // The OTHER side forfeiting the spawn window ends the match here.
        Some(envelope @ Envelope::GameEnd { .. }) => {
            let _ = envelope;
            return;
        }
        Some(other) => panic!("spawn rejected: {other:?}"),
        None => panic!("connection closed at spawn result"),
    }
    let mut round = 0u32;
    loop {
        match client.next_envelope(Duration::from_secs(30)) {
            Some(Envelope::RoundStartSignal) => {
                round += 1;
                on_round(&mut client, round, &roster);
                client.send_envelope(&Envelope::CommitRound { round });
            }
            Some(Envelope::RoundClosed { .. }) | Some(Envelope::SubmissionResult { .. }) => {}
            Some(Envelope::GameEnd { .. }) => return,
            Some(other) => panic!("unexpected {other:?}"),
            None => return,
        }
    }
}

fn noop_round(_client: &mut RawClient, _round: u32, _roster: &[AgentId]) {}

/// Wide windows: injected traffic in these tests is subject to scheduler
/// jitter from the parallel suite, which must not turn into deadline
/// behavior that is not the subject of the test.
fn wide(tag: &str, rounds: u32) -> Server {
    let mut config = fast_config(rounds);
    config.decision_window_ms = 500;
    config.initial_spawn_window_ms = 800;
    Server::start_config(tag, config)
}

// (c) The exact legal maximum frame is processed: a CLIENT_HELLO padded
// to exactly 65,535 payload bytes.
#[test]
fn exact_legal_maximum_frame_is_processed() {
    let _permit = MatchPermit::acquire();
    let server = Server::start("maxframe", 2);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            play(&server, "big", Hello::GiantName, noop_round);
        });
        play(&server, "res", Hello::Normal, noop_round);
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
}

// (c) 65,536 / 65,537 prefixes close the connection WITHOUT draining, and
// after the match has started they produce CLIENT_FORFEIT.
#[test]
fn over_limit_prefix_forfeits_after_start() {
    let _permit = MatchPermit::acquire();
    let server = wide("overlimit", 5);
    let bad_socket = server.socket.clone();
    let (bad_tx, bad_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let mut bad = RawClient::hello(&bad_socket, "bad").expect("connect");
            let faction = match bad.next_envelope(Duration::from_secs(10)) {
                Some(Envelope::ServerHello {
                    status: HELLO_OK,
                    assigned_faction,
                    ..
                }) => assigned_faction,
                other => panic!("hello {other:?}"),
            };
            bad_tx.send(faction).expect("faction reported");
            let info = match bad.next_envelope(Duration::from_secs(10)) {
                Some(Envelope::InitialSpawnStart { info, .. }) => info,
                other => panic!("start {other:?}"),
            };
            let n = info.initial_agents_per_faction;
            let offset = match faction {
                Faction::Enl => 0,
                Faction::Res => n,
            };
            let positions: Vec<SpawnEntry> = (0..n)
                .map(|i| SpawnEntry {
                    agent_id: AgentId(offset + i),
                    position: Coord::new(4, 4),
                })
                .collect();
            bad.send_envelope(&Envelope::SubmitInitialSpawns { positions });
            // Round 1: an over-limit prefix mid-round — 65,536 then 65,537
            // in one write, no draining in between.
            loop {
                match bad.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::RoundStartSignal) => break,
                    Some(Envelope::InitialSpawnResult { .. }) => {}
                    Some(other) => panic!("unexpected {other:?}"),
                    None => return,
                }
            }
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&65_536u32.to_be_bytes());
            bytes.extend_from_slice(&65_537u32.to_be_bytes());
            bad.send_raw(&bytes);
            // The server closes without draining: our next read is EOF.
            assert!(bad.next_envelope(Duration::from_secs(5)).is_none());
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let bad_faction = bad_rx.recv().expect("bad faction");
    let report = server.finish();
    match &report.outcome {
        MatchOutcome::ClientForfeit {
            winner, at_round, ..
        } => {
            assert_eq!(
                *winner,
                Some(bad_faction.opposite()),
                "the over-limiting {bad_faction:?} client forfeits"
            );
            assert!(*at_round >= 1);
        }
        other => panic!("expected CLIENT_FORFEIT, got {other:?}"),
    }
}

// (d) Corrupt flatbuffers: an in-limit frame with flipped interior bytes,
// a truncated payload, and an EMPTY payload are all envelope-rejected;
// nothing freezes, the match continues to a normal end.
#[test]
fn corrupt_frames_are_envelope_rejected_and_recoverable() {
    let _permit = MatchPermit::acquire();
    let server = wide("corrupt", 3);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            play(&server, "enl", Hello::Normal, |client, round, roster| {
                if round == 1 {
                    // 1. Flipped interior bytes inside a real envelope.
                    let good =
                        arena_protocol::build::envelope_bytes(&Envelope::SubmitCommandStream {
                            round,
                            agent_id: AgentId(0),
                            commands: vec![arena_core::types::Command::Attack],
                        });
                    let mut damaged = good.clone();
                    for index in [8usize, 14, 22] {
                        damaged[index] ^= 0xFF;
                    }
                    client.send_envelope_bytes(&damaged);
                    // 2. Truncated payload.
                    client.send_envelope_bytes(&good[..good.len() / 2]);
                    // 3. Empty payload.
                    client.send_envelope_bytes(&[]);
                    // The agent slot is NOT frozen by undecodable junk; a
                    // decodable-but-corrupted variant may still look like a
                    // submission. Whatever verdicts come back, the REAL
                    // stream must be accepted-first or ignored-duplicate
                    // (the slot froze only on real frames), and the match
                    // keeps running.
                    client.send_envelope(&Envelope::SubmitCommandStream {
                        round,
                        agent_id: roster[0],
                        commands: vec![],
                    });
                    let mut saw_frozen_verdict = false;
                    for _ in 0..4 {
                        match client.next_envelope(Duration::from_secs(2)) {
                            Some(Envelope::SubmissionResult {
                                status: SUBMIT_ACCEPTED,
                                ..
                            })
                            | Some(Envelope::SubmissionResult {
                                status: SUBMIT_IGNORED_DUPLICATE,
                                ..
                            }) => saw_frozen_verdict = true,
                            Some(Envelope::SubmissionResult { .. }) => {}
                            Some(Envelope::RoundClosed { .. }) => break,
                            _ => break,
                        }
                    }
                    assert!(saw_frozen_verdict, "the intact submission froze the slot");
                }
            });
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
    assert!(
        report.journal.contains("envelope_reject"),
        "journal records the rejects"
    );
}

// (e) Split writes reassemble exactly: one envelope fed byte-by-byte, and
// two envelopes in ONE write are both processed in order.
#[test]
fn split_and_pipelined_writes_reassemble_in_order() {
    let _permit = MatchPermit::acquire();
    let server = wide("splitwrite", 2);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            play(&server, "enl", Hello::Normal, |client, round, roster| {
                if round == 1 {
                    // Two submissions pipelined in one write, second fed
                    // byte-by-byte across three more writes.
                    let first =
                        arena_protocol::build::envelope_frame(&Envelope::SubmitCommandStream {
                            round,
                            agent_id: AgentId(0),
                            commands: vec![],
                        });
                    let second =
                        arena_protocol::build::envelope_frame(&Envelope::SubmitCommandStream {
                            round,
                            agent_id: AgentId(1),
                            commands: vec![],
                        });
                    let mut one_write = first.clone();
                    one_write.extend_from_slice(&second);
                    client.send_raw(&one_write);
                    // Two results arrive in roster order.
                    for expected in [roster[0], roster[1]] {
                        match client.next_envelope(Duration::from_secs(5)) {
                            Some(Envelope::SubmissionResult {
                                status, agent_id, ..
                            }) => {
                                assert_eq!(agent_id, expected);
                                assert_eq!(status, SUBMIT_ACCEPTED);
                            }
                            other => panic!("expected result, got {other:?}"),
                        }
                    }
                    // Now a byte-by-byte commit for the round.
                    let commit =
                        arena_protocol::build::envelope_frame(&Envelope::CommitRound { round });
                    for byte in &commit {
                        client.send_raw(&[*byte]);
                    }
                }
            });
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
}

// (f) A half-frame that completes after the deadline is dropped without
// freezing; the round settles with that agent NotSubmitted; the
// connection stays open for the next round.
#[test]
fn cross_deadline_half_frame_dropped_without_freezing() {
    let _permit = MatchPermit::acquire();
    let mut config = fast_config(3);
    config.decision_window_ms = 500;
    let server = Server::start_config("halfframe", config);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            play(&server, "enl", Hello::Normal, |client, round, roster| {
                if round == 1 {
                    // First half now (well inside the window)...
                    let stream = Envelope::SubmitCommandStream {
                        round,
                        agent_id: roster[0],
                        commands: vec![arena_core::types::Command::Attack],
                    };
                    let frame_len = arena_protocol::build::envelope_frame(&stream).len();
                    let tail = client.send_partial_envelope(&stream, frame_len - 3);
                    // ...remainder after the window has certainly passed
                    // (the server settles at the deadline).
                    std::thread::sleep(Duration::from_millis(700));
                    client.send_raw(&tail);
                }
            });
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
    assert!(
        report.journal.contains("late_drop"),
        "journal records the cross-deadline drop"
    );
    // The dropped agent's slot was NotSubmitted in round 1: the replay
    // carries the intake fact.
    let bytes =
        std::fs::read(temp_dir("halfframe").join("out").join("replay.iarp")).expect("replay");
    let file = arena_replay::format::decode(&bytes).expect("decodes");
    let round1 = file.round_records().next().expect("round 1");
    let half_frame_agent = round1.slots.keys().min().copied().expect("slots exist");
    assert_eq!(
        round1.slots.get(&half_frame_agent),
        Some(&arena_core::resolve::SlotStatus::NotSubmitted),
        "the half-frame agent never froze"
    );
}

// (g) Version mismatch: each of the three versions bumped separately
// aborts BEFORE any map publish (no INITIAL_SPAWN_START, no SHM regions),
// answers SERVER_HELLO status=1, and is an unscored SetupError — never a
// SPAWN_FORFEIT.
#[test]
fn version_mismatch_aborts_before_map_publish() {
    let _permit = MatchPermit::acquire();
    for bump in 1u32..=3 {
        let server = Server::start(&format!("ver{bump}"), 3);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let mut bad = RawClient::hello_with_version_bump(&server.socket, "bad", bump)
                    .expect("connect");
                match bad.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::ServerHello { status, .. }) => {
                        assert_eq!(status, HELLO_VERSION_MISMATCH, "bump {bump}");
                    }
                    other => panic!("expected rejection, got {other:?}"),
                }
                // No INITIAL_SPAWN_START ever arrives: the connection ends.
                assert!(bad.next_envelope(Duration::from_millis(300)).is_none());
            });
            scope.spawn(|| {
                let mut good = RawClient::hello(&server.socket, "good").expect("connect");
                // The good client is also refused startup (no match).
                match good.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::ServerHello { status, .. }) => {
                        assert_eq!(status, HELLO_VERSION_MISMATCH);
                    }
                    other => panic!("expected the abort to reach the peer, got {other:?}"),
                }
            });
        });
        let report = server.finish();
        match &report.outcome {
            MatchOutcome::SetupError { .. } => {}
            other => panic!("bump {bump}: expected unscored SetupError, got {other:?}"),
        }
        assert!(
            !report.journal.contains("initial_spawn_start"),
            "no spawn start before abort (bump {bump})"
        );
    }
}

// (h) Duplicate/surplus connections: a third connection (and a second one
// from an already-assigned client) gets SERVER_HELLO status=2 and is
// closed; the match is unaffected.
#[test]
fn surplus_connections_rejected_match_unaffected() {
    let _permit = MatchPermit::acquire();
    let server = Server::start("surplus", 3);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            play(&server, "enl", Hello::Normal, |_client, _, _| {
                // A second connection from the already-assigned client.
                let mut second = RawClient::hello(&server.socket, "enl-again").expect("connect");
                match second.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::ServerHello { status, .. }) => {
                        assert_eq!(status, HELLO_DUPLICATE_CONNECTION);
                    }
                    other => panic!("expected duplicate rejection, got {other:?}"),
                }
                // And a bare third connection without any HELLO.
                let third = RawClient::connect(&server.socket, "third").expect("connect");
                drop(third);
            });
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
    assert!(report.journal.contains("surplus_rejected"));
}

// (j) Spawn forfeits: an internally invalid set (missing agent / out of
// bounds) and a missed deadline both produce SPAWN_FORFEIT for that side
// with the OTHER side winning; both failing is unscored; no retry.
#[test]
fn spawn_forfeit_invalid_set() {
    let _permit = MatchPermit::acquire();
    for variant in [1u32, 2] {
        let server = Server::start(&format!("spawnf{variant}"), 3);
        let bad_socket = server.socket.clone();
        let (bad_tx, bad_rx) = mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(move || {
                let mut client = RawClient::hello(&bad_socket, "bad").expect("connect");
                let bad_faction = match client.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::ServerHello {
                        status: HELLO_OK,
                        assigned_faction,
                        ..
                    }) => assigned_faction,
                    other => panic!("hello {other:?}"),
                };
                bad_tx.send(bad_faction).expect("send faction");
                let info = match client.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::InitialSpawnStart { info, .. }) => info,
                    other => panic!("unexpected {other:?}"),
                };
                let n = info.initial_agents_per_faction;
                let positions: Vec<SpawnEntry> = if variant == 1 {
                    // Out-of-bounds coordinate.
                    (0..n)
                        .map(|i| SpawnEntry {
                            agent_id: AgentId(i),
                            position: Coord::new(info.map_width_cm + 1, 0),
                        })
                        .collect()
                } else {
                    // Missing one agent.
                    (0..n - 1)
                        .map(|i| SpawnEntry {
                            agent_id: AgentId(i),
                            position: Coord::new(1, 1),
                        })
                        .collect()
                };
                client.send_envelope(&Envelope::SubmitInitialSpawns { positions });
                match client.next_envelope(Duration::from_secs(5)) {
                    Some(Envelope::InitialSpawnResult { status: 1, .. }) => {}
                    other => panic!("variant {variant}: expected rejection, got {other:?}"),
                }
                // The technical end reaches the forfeiting client too.
                match client.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::GameEnd { outcome, .. }) => {
                        assert_eq!(outcome, END_SPAWN_FORFEIT);
                    }
                    other => panic!("expected GAME_END, got {other:?}"),
                }
            });
            scope.spawn(|| {
                play(&server, "good", Hello::Normal, |_, _, _| {
                    panic!("no round runs after a spawn forfeit");
                });
            });
        });
        let bad_faction = bad_rx.recv().expect("bad faction");
        let report = server.finish();
        match &report.outcome {
            MatchOutcome::SpawnForfeit { winner, .. } => {
                assert_eq!(
                    *winner,
                    Some(bad_faction.opposite()),
                    "variant {variant}: {bad_faction:?} forfeits"
                );
            }
            other => panic!("variant {variant}: expected SPAWN_FORFEIT, got {other:?}"),
        }
        // No round was ever played and no retry happened.
        assert!(!report.journal.contains("round_start="), "no rounds");
        let forfeit_lines = report
            .journal
            .lines()
            .filter(|l| l.contains("spawn_set=") && l.contains("accepted=false"))
            .count();
        assert_eq!(forfeit_lines, 1, "no retry (variant {variant})");
    }
}

#[test]
fn spawn_deadline_miss_forfeits_both_sides_unscored() {
    let _permit = MatchPermit::acquire();
    let server = Server::start("spawndead", 3);
    // Nobody submits anything; both clients STAY CONNECTED and silent so
    // the spawn deadline (not a disconnect) decides.
    fn silent(socket: &std::path::Path) {
        let mut client = RawClient::hello(socket, "silent").expect("connect");
        loop {
            match client.next_envelope(Duration::from_secs(30)) {
                Some(Envelope::GameEnd { .. }) | None => return,
                Some(_) => {}
            }
        }
    }
    let socket = server.socket.clone();
    std::thread::scope(|scope| {
        let socket = &socket;
        scope.spawn(move || silent(socket));
        scope.spawn(move || silent(socket));
    });
    let report = server.finish();
    match &report.outcome {
        MatchOutcome::SpawnForfeit { winner: None, .. } => {}
        other => panic!("both sides missing the deadline is unscored, got {other:?}"),
    }
    assert!(report.outcome.is_unscored());
}

// (k) A slow client never blocks settlement: one faction never responds;
// every round closes at the deadline, the other side plays on, and the
// match still reaches GAME_END.
#[test]
fn slow_client_does_not_block_settlement() {
    let _permit = MatchPermit::acquire();
    let mut config = fast_config(3);
    config.decision_window_ms = 30;
    let server = Server::start_config("slow", config);
    let slow_socket = server.socket.clone();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            // The slow client authenticates and spawns, then never
            // responds again: rounds must still close at the deadline.
            let mut slow = RawClient::hello(&slow_socket, "slow").expect("connect");
            let faction = match slow.next_envelope(Duration::from_secs(10)) {
                Some(Envelope::ServerHello {
                    status: HELLO_OK,
                    assigned_faction,
                    ..
                }) => assigned_faction,
                other => panic!("hello {other:?}"),
            };
            let info = match slow.next_envelope(Duration::from_secs(10)) {
                Some(Envelope::InitialSpawnStart { info, .. }) => info,
                other => panic!("start {other:?}"),
            };
            let n = info.initial_agents_per_faction;
            let offset = match faction {
                Faction::Enl => 0,
                Faction::Res => n,
            };
            let positions: Vec<SpawnEntry> = (0..n)
                .map(|i| SpawnEntry {
                    agent_id: AgentId(offset + i),
                    position: Coord::new(3, 3),
                })
                .collect();
            slow.send_envelope(&Envelope::SubmitInitialSpawns { positions });
            let _ = slow.next_envelope(Duration::from_secs(10));
            // Silence: drain until the match ends.
            loop {
                match slow.next_envelope(Duration::from_secs(30)) {
                    Some(Envelope::GameEnd { .. }) | None => return,
                    Some(_) => {}
                }
            }
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
    // Every round had to wait the full window for the slow faction, and
    // its slots were all NotSubmitted.
    assert!(
        report.metrics[0].not_submitted_slots >= 9 || report.metrics[1].not_submitted_slots >= 9,
        "{:?}",
        report.metrics
    );
    let slow_side_timeout = report.metrics[0]
        .timeout_rate_permille()
        .max(report.metrics[1].timeout_rate_permille());
    let prompt_side_timeout = report.metrics[0]
        .timeout_rate_permille()
        .min(report.metrics[1].timeout_rate_permille());
    assert_eq!(slow_side_timeout, 1000);
    assert_eq!(prompt_side_timeout, 0);
}

// (l) Early completion: when both factions COMMIT (or all agents submit)
// before the deadline, the server advances immediately — round durations
// stay far below the window.
#[test]
fn early_completion_advances_immediately() {
    let _permit = MatchPermit::acquire();
    let server = Server::start("earlyclose", 10);
    let started = std::time::Instant::now();
    std::thread::scope(|scope| {
        scope.spawn(|| play(&server, "enl", Hello::Normal, noop_round));
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let elapsed = started.elapsed();
    let report = server.finish();
    assert!(matches!(report.outcome, MatchOutcome::Normal { .. }),);
    // 10 rounds with a 40 ms window: full-window play would take >= 400
    // ms; early completion lands well under that.
    assert!(
        elapsed < Duration::from_millis(350),
        "10 rounds took {elapsed:?}"
    );
    // And every individual round closed far inside its window.
    let durations: Vec<u64> = report
        .journal
        .lines()
        .filter_map(|l| l.split_once("duration_ms=").map(|(_, rest)| rest))
        .filter_map(|rest| rest.split_whitespace().next().and_then(|d| d.parse().ok()))
        .collect();
    assert_eq!(durations.len(), 10);
    assert!(
        durations.iter().all(|&d| d < 20),
        "rounds closed early: {durations:?}"
    );
}

// (m) Parity alternation: across >= 6 consecutive rounds the
// ROUND_START_SIGNAL send order strictly alternates — odd rounds RES
// first, even rounds ENL first.
#[test]
fn round_start_signal_order_strictly_alternates() {
    let _permit = MatchPermit::acquire();
    let server = Server::start("parity", 8);
    std::thread::scope(|scope| {
        scope.spawn(|| play(&server, "enl", Hello::Normal, noop_round));
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let report = server.finish();
    let orders: Vec<&str> = report
        .journal
        .lines()
        .filter(|l| l.contains("round_start="))
        .map(|l| l.rsplit("notified_first=").next().unwrap())
        .collect();
    assert_eq!(orders.len(), 8, "{orders:?}");
    for (index, first) in orders.iter().enumerate() {
        let round = index as u32 + 1;
        let expected = if round % 2 == 1 { "res" } else { "enl" };
        assert_eq!(*first, expected, "round {round} notified first {first}");
    }
}

// (19) Receive semantics: per-connection receive_seq is strictly
// increasing across the whole match, receive buckets equal
// floor(received_at_ms / priority_bucket_ms), and a stale-round frame is
// envelope-rejected (unlocatable) without freezing.
#[test]
fn receive_semantics_recorded_and_stale_rounds_unlocatable() {
    let _permit = MatchPermit::acquire();
    let server = wide("receive", 4);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            play(&server, "enl", Hello::Normal, |client, round, roster| {
                if round == 1 {
                    let agent = roster[0];
                    // A future-round submission (claims round 5).
                    client.send_envelope(&Envelope::SubmitCommandStream {
                        round: 5,
                        agent_id: agent,
                        commands: vec![],
                    });
                    match client.next_envelope(Duration::from_secs(5)) {
                        Some(Envelope::SubmissionResult {
                            status: SUBMIT_UNLOCATABLE,
                            ..
                        }) => {}
                        other => panic!("future round must be unlocatable, got {other:?}"),
                    }
                    // The slot stayed unfrozen: the real round-1 stream is
                    // accepted as the first.
                    client.send_envelope(&Envelope::SubmitCommandStream {
                        round: 1,
                        agent_id: agent,
                        commands: vec![],
                    });
                    match client.next_envelope(Duration::from_secs(5)) {
                        Some(Envelope::SubmissionResult {
                            status: SUBMIT_ACCEPTED,
                            ..
                        }) => {}
                        other => panic!("expected accepted-first, got {other:?}"),
                    }
                }
            });
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
    assert!(report.journal.contains("unlocatable"));

    // The replay carries per-stream receive metadata: verify seq and
    // bucket invariants from the file.
    let bytes = std::fs::read(temp_dir("receive").join("out").join("replay.iarp")).expect("replay");
    let file = arena_replay::format::decode(&bytes).expect("decodes");
    let bucket_ms = fast_config(4).priority_bucket_ms;
    let mut last_seq: [Option<u32>; 2] = [None, None];
    let mut seen_any = false;
    for record in file.round_records() {
        for submission in &record.submissions {
            let index = match submission.faction {
                Faction::Enl => 0,
                Faction::Res => 1,
            };
            if let Some(previous) = last_seq[index] {
                assert!(
                    submission.receive_seq > previous,
                    "per-connection receive_seq strictly increases: {} !> {previous}",
                    submission.receive_seq
                );
            }
            last_seq[index] = Some(submission.receive_seq);
            // Bucket derivation matches the harness-identical floor.
            assert_eq!(
                arena_core::resolve::receive_bucket(submission.received_at_ms, bucket_ms),
                submission.received_at_ms / bucket_ms as u64
            );
            seen_any = true;
        }
    }
    assert!(seen_any, "the replay carries receive metadata");
}

// P3-GATE "运行中声明错误版本": a CLIENT_HELLO with a bumped version sent
// MID-MATCH is an envelope reject — journaled, nothing frozen, the match
// continues to a normal end.
#[test]
fn mid_match_wrong_version_hello_is_an_envelope_reject() {
    let _permit = MatchPermit::acquire();
    let server = wide("midhello", 2);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            play(&server, "enl", Hello::Normal, |client, _round, _roster| {
                client.send_envelope(&Envelope::ClientHello {
                    client_name: "sneaky".into(),
                    client_version: "late".into(),
                    bot_version: String::new(),
                    sdk_version: String::new(),
                    protocol_version: arena_protocol::PROTOCOL_VERSION + 1,
                    public_snapshot_version: arena_protocol::PUBLIC_SNAPSHOT_VERSION,
                    private_snapshot_version: arena_protocol::PRIVATE_SNAPSHOT_VERSION,
                });
            });
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
    assert!(
        report.journal.contains("mid_match_hello_rejected"),
        "{}",
        report.journal
    );
}

// (R-RESOLVE-01 / R-RESOLVE-03, D64) A frame claiming round N+1 that
// completes before round N+1's ROUND_START_SIGNAL was written — pipelined
// by a client that cannot yet have seen that signal — claimed a FUTURE
// round at arrival. It must stay envelope-unlocatable: no bucket-0
// priority, no frozen slot, never executed (its claimed round equals the
// CURRENT round once collection for N+1 begins, so only the transport can
// reject it). The pipelining client must never gain first-mover priority.
//
// Determinism: firing the claim on observing ROUND_CLOSED(N) races the
// server's settlement of round N — in release the settle+publish path can
// beat the client's socket wakeup, and a claim landing after the N+1
// signal is a LEGAL in-round submission, so the verdict depended on
// timing. The race is removed without weakening what is pinned: the claim
// now rides BEHIND this client's own COMMIT_ROUND(N) in ONE write, after
// a test-side barrier proves the opponent's slots are already frozen.
// That commit is then the event that closes round N's input; the claim's
// last byte is stamped by the SAME reader wakeup, microseconds behind the
// commit's enqueue — strictly before the driver can dequeue the commit
// (settlement of round N cannot even start). And because collect_round
// re-checks BOTH-COMPLETE at the loop top after every dequeued event, the
// already-queued claim is NOT dequeued in round N: it surfaces as the
// FIRST event of round N+1's collection with claimed round == current
// round and a receive stamp older than that round's signal. Pre-signal
// arrival is forced by construction, not hoped for from the scheduler.
#[test]
fn settlement_pipelined_next_round_claim_is_unlocatable() {
    let _permit = MatchPermit::acquire();
    let total = 4u32;
    let server = wide("pipeline", total);
    let (turn_tx, turn_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| pipelining_client(&server.socket, total, turn_rx));
        scope.spawn(|| completing_opponent(&server.socket, turn_tx));
    });
    let report = server.finish();
    assert!(
        matches!(report.outcome, MatchOutcome::Normal { .. }),
        "{:?}",
        report.outcome
    );
    let unlocatable = report
        .journal
        .lines()
        .filter(|l| l.contains("unlocatable"))
        .count();
    assert!(
        unlocatable >= (total - 1) as usize,
        "every pipelined claim journaled unlocatable: {unlocatable}"
    );
}

/// Plays the match while pipelining next-round claims: every round it
/// submits its first two agents, then waits (test-side barrier) until the
/// opponent's whole roster is frozen, then writes COMMIT_ROUND(N) and a
/// SUBMIT_COMMAND_STREAM claiming round N+1 back-to-back in ONE write.
/// The claim's last byte therefore lands while round N's input is still
/// open — before round N+1 exists, pre-signal by construction — yet it is
/// only dequeued once collection for round N+1 is underway: the
/// pipelining window the transport must reject. Every such claim must
/// come back unlocatable, and the real first submissions of every round
/// must still be accepted-first (a pipelined claim never froze anything).
fn pipelining_client(socket: &std::path::Path, total_rounds: u32, turn: mpsc::Receiver<()>) {
    let mut client = RawClient::hello(socket, "pipeline").expect("connect");
    let faction = match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::ServerHello {
            status: HELLO_OK,
            assigned_faction,
            ..
        }) => assigned_faction,
        other => panic!("hello {other:?}"),
    };
    let info = match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::InitialSpawnStart { info, .. }) => info,
        other => panic!("start {other:?}"),
    };
    let n = info.initial_agents_per_faction;
    assert!(n >= 3, "the fixture assumes a 3-agent roster");
    let offset = match faction {
        Faction::Enl => 0,
        Faction::Res => n,
    };
    let roster: Vec<AgentId> = (0..n).map(|i| AgentId(offset + i)).collect();
    let positions: Vec<SpawnEntry> = roster
        .iter()
        .enumerate()
        .map(|(i, &id)| SpawnEntry {
            agent_id: id,
            position: Coord::new(i as i32 + 1, i as i32),
        })
        .collect();
    client.send_envelope(&Envelope::SubmitInitialSpawns { positions });
    match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::InitialSpawnResult { status: 0, .. }) => {}
        other => panic!("spawn {other:?}"),
    }
    let pipelined_agent = roster[2];
    let mut round = 0u32;
    loop {
        match client.next_envelope(Duration::from_secs(30)) {
            Some(Envelope::RoundStartSignal) => {
                round += 1;
                for &agent in &roster[..2] {
                    client.send_envelope(&Envelope::SubmitCommandStream {
                        round,
                        agent_id: agent,
                        commands: vec![],
                    });
                }
                // Hold this round's input open until the opponent's slots
                // are frozen: our COMMIT (riding in front of the claim
                // below) must be the event that closes collection, so the
                // claim queued behind it cannot be dequeued in round N.
                turn.recv_timeout(Duration::from_secs(30))
                    .expect("opponent completed the round");
                let mut burst =
                    arena_protocol::build::envelope_frame(&Envelope::CommitRound { round });
                if round < total_rounds {
                    // The exploit under test: a claim for the NEXT round,
                    // landing before round N's input closes (and so, with
                    // certainty, before round N+1's start signal).
                    burst.extend_from_slice(&arena_protocol::build::envelope_frame(
                        &Envelope::SubmitCommandStream {
                            round: round + 1,
                            agent_id: pipelined_agent,
                            commands: vec![],
                        },
                    ));
                }
                client.send_raw(&burst);
            }
            Some(Envelope::SubmissionResult {
                agent_id, status, ..
            }) => {
                if agent_id == pipelined_agent {
                    assert_eq!(
                        status, SUBMIT_UNLOCATABLE,
                        "a pipelined next-round claim is never located (round {round})"
                    );
                } else {
                    assert_eq!(
                        status, SUBMIT_ACCEPTED,
                        "real first submissions stay first (round {round})"
                    );
                }
            }
            Some(Envelope::RoundClosed { .. }) => {}
            Some(Envelope::GameEnd { .. }) => return,
            Some(other) => panic!("unexpected {other:?}"),
            None => return,
        }
    }
}

/// The compliant opponent, shaped only to make round completion
/// observable: every round it submits a stream for EVERY agent (freezing
/// the whole roster) and only then hands the turn to the pipelining
/// client. The handoff is strictly after the driver dequeued — and replied
/// to — the last of those streams, which is exactly when this faction
/// became complete for the round; the reply is what the loop observes.
fn completing_opponent(socket: &std::path::Path, turn: mpsc::Sender<()>) {
    let mut client = RawClient::hello(socket, "opponent").expect("connect");
    let faction = match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::ServerHello {
            status: HELLO_OK,
            assigned_faction,
            ..
        }) => assigned_faction,
        other => panic!("hello {other:?}"),
    };
    let info = match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::InitialSpawnStart { info, .. }) => info,
        other => panic!("start {other:?}"),
    };
    let n = info.initial_agents_per_faction;
    assert!(n >= 3, "the fixture assumes a 3-agent roster");
    let offset = match faction {
        Faction::Enl => 0,
        Faction::Res => n,
    };
    let roster: Vec<AgentId> = (0..n).map(|i| AgentId(offset + i)).collect();
    let positions: Vec<SpawnEntry> = roster
        .iter()
        .enumerate()
        .map(|(i, &id)| SpawnEntry {
            agent_id: id,
            position: Coord::new(i as i32 + 1, i as i32),
        })
        .collect();
    client.send_envelope(&Envelope::SubmitInitialSpawns { positions });
    match client.next_envelope(Duration::from_secs(10)) {
        Some(Envelope::InitialSpawnResult { status: 0, .. }) => {}
        other => panic!("spawn {other:?}"),
    }
    let mut round = 0u32;
    loop {
        match client.next_envelope(Duration::from_secs(30)) {
            Some(Envelope::RoundStartSignal) => {
                round += 1;
                for &agent in &roster {
                    client.send_envelope(&Envelope::SubmitCommandStream {
                        round,
                        agent_id: agent,
                        commands: vec![],
                    });
                }
                // Observe this round's ACCEPTED-first replies: the last
                // one proves the driver froze every slot of this faction
                // (the reply is sent after the freeze, and ROUND_CLOSED
                // cannot arrive first — input stays open until the
                // pipelining client's post-barrier commit).
                let mut accepted = 0usize;
                while accepted < roster.len() {
                    match client.next_envelope(Duration::from_secs(30)) {
                        Some(Envelope::SubmissionResult {
                            status: SUBMIT_ACCEPTED,
                            ..
                        }) => accepted += 1,
                        Some(other) => {
                            panic!("unexpected while completing round {round}: {other:?}")
                        }
                        None => return,
                    }
                }
                turn.send(()).expect("turn handoff");
            }
            Some(Envelope::RoundClosed { .. }) | Some(Envelope::SubmissionResult { .. }) => {}
            Some(Envelope::GameEnd { .. }) => return,
            Some(other) => panic!("unexpected {other:?}"),
            None => return,
        }
    }
}

// (R-SPAWN-01 / P3-TIMEOUT) A spawn set completing AFTER the deadline
// fails that side — but when the opponent also never submitted, BOTH
// failed and the match is an UNSCORED spawn forfeit, never a scored
// technical win for a silent opponent. Repeated across several matches
// because EITHER path may decide: the deadline timeout (queue empty)
// or the late-set branch (frame dequeued before the timeout fires).
// The invariant under fix: both paths agree on unscored; before the
// fix, the late-set branch alone scored a technical win for the silent
// side.
#[test]
fn late_spawn_set_with_silent_opponent_is_unscored() {
    for iteration in 0..6u32 {
        let _permit = MatchPermit::acquire();
        let mut config = fast_config(3);
        config.initial_spawn_window_ms = 150;
        let server = Server::start_config(&format!("spawnlate{iteration}"), config);
        let socket = server.socket.clone();
        let silent_socket = server.socket.clone();
        std::thread::scope(|scope| {
            scope.spawn(move || {
                let mut late = RawClient::hello(&socket, "late").expect("connect");
                let faction = match late.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::ServerHello {
                        status: HELLO_OK,
                        assigned_faction,
                        ..
                    }) => assigned_faction,
                    other => panic!("hello {other:?}"),
                };
                let info = match late.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::InitialSpawnStart { info, .. }) => info,
                    other => panic!("start {other:?}"),
                };
                let n = info.initial_agents_per_faction;
                let offset = match faction {
                    Faction::Enl => 0,
                    Faction::Res => n,
                };
                let positions: Vec<SpawnEntry> = (0..n)
                    .map(|i| SpawnEntry {
                        agent_id: AgentId(offset + i),
                        position: Coord::new(2, 2),
                    })
                    .collect();
                // Sleep past the deadline, then submit a perfectly valid
                // set: it is ineligible no matter how the server's queue
                // and deadline timer interleave.
                std::thread::sleep(Duration::from_millis(
                    info.initial_spawn_window_ms as u64 + 30,
                ));
                late.send_envelope(&Envelope::SubmitInitialSpawns { positions });
                // The match is over either way: drain to the technical end.
                loop {
                    match late.next_envelope(Duration::from_secs(10)) {
                        Some(Envelope::GameEnd { outcome, .. }) => {
                            assert_eq!(outcome, END_SPAWN_FORFEIT, "iteration {iteration}");
                            return;
                        }
                        Some(Envelope::InitialSpawnResult { status, .. }) => {
                            // If the late-set branch decided, the reply is
                            // the deadline refusal (status 2), never
                            // acceptance.
                            assert_eq!(status, 2, "iteration {iteration}");
                        }
                        Some(_) => {}
                        None => return,
                    }
                }
            });
            scope.spawn(move || {
                // The opponent authenticates and stays silent: connected
                // (no disconnect forfeit), it simply never submits.
                let mut silent = RawClient::hello(&silent_socket, "silent").expect("connect");
                loop {
                    match silent.next_envelope(Duration::from_secs(30)) {
                        Some(Envelope::GameEnd { .. }) | None => return,
                        Some(_) => {}
                    }
                }
            });
        });
        let report = server.finish();
        match &report.outcome {
            MatchOutcome::SpawnForfeit { winner: None, .. } => {}
            other => panic!(
                "iteration {iteration}: late set + silent opponent is unscored, got {other:?}"
            ),
        }
        assert!(report.outcome.is_unscored());
    }
}

// (P3-TIMEOUT / P4-GATE) A client that stays connected but stops READING
// never generates an EOF; once its socket's send buffer fills, a blocking
// unbounded write would pin the driver forever — no deadline could fire,
// no CLIENT_FORFEIT could be produced, serve_match would never return.
// The server's write timeout must turn this into a forfeit for the
// stalled side instead of a hang.
#[test]
fn client_that_stops_reading_forfeits_instead_of_hanging() {
    let _permit = MatchPermit::acquire();
    let server = wide("stallread", 8);
    let socket = server.socket.clone();
    let (faction_tx, faction_rx) = mpsc::channel();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let report = std::thread::scope(|scope| {
        scope.spawn(move || {
            let mut staller = RawClient::hello(&socket, "stall").expect("connect");
            let faction = match staller.next_envelope(Duration::from_secs(10)) {
                Some(Envelope::ServerHello {
                    status: HELLO_OK,
                    assigned_faction,
                    ..
                }) => assigned_faction,
                other => panic!("hello {other:?}"),
            };
            faction_tx.send(faction).expect("faction reported");
            let info = match staller.next_envelope(Duration::from_secs(10)) {
                Some(Envelope::InitialSpawnStart { info, .. }) => info,
                other => panic!("start {other:?}"),
            };
            let n = info.initial_agents_per_faction;
            let offset = match faction {
                Faction::Enl => 0,
                Faction::Res => n,
            };
            let positions: Vec<SpawnEntry> = (0..n)
                .map(|i| SpawnEntry {
                    agent_id: AgentId(offset + i),
                    position: Coord::new(3, 3),
                })
                .collect();
            staller.send_envelope(&Envelope::SubmitInitialSpawns { positions });
            match staller.next_envelope(Duration::from_secs(10)) {
                Some(Envelope::InitialSpawnResult { status: 0, .. }) => {}
                other => panic!("spawn {other:?}"),
            }
            loop {
                match staller.next_envelope(Duration::from_secs(10)) {
                    Some(Envelope::RoundStartSignal) => break,
                    Some(other) => panic!("unexpected {other:?}"),
                    None => return,
                }
            }
            // Every pipelined duplicate earns a SUBMISSION_RESULT reply;
            // the reply traffic (megabytes) has nowhere to go once we stop
            // reading, so the server's send buffer fills mid-burst.
            let frame = arena_protocol::build::envelope_frame(&Envelope::SubmitCommandStream {
                round: 1,
                agent_id: AgentId(offset),
                commands: vec![],
            });
            let mut burst = Vec::new();
            for _ in 0..60_000 {
                burst.extend_from_slice(&frame);
            }
            staller.send_raw(&burst);
            // Stay connected, never read again (no EOF is ever generated).
            // The 90 s cap is a backstop so even a REGRESSED server (which
            // hangs and makes the report recv time out) cannot wedge this
            // thread — and thus the scope — forever.
            let stall_deadline = std::time::Instant::now() + Duration::from_secs(90);
            while !stop_flag.load(std::sync::atomic::Ordering::Relaxed)
                && std::time::Instant::now() < stall_deadline
            {
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        scope.spawn(|| play(&server, "res", Hello::Normal, noop_round));
        // Bounded wait: without the write timeout the driver blocks inside
        // write_all forever and this recv times out (test fails).
        let report = server
            .report
            .lock()
            .expect("uncontended")
            .take()
            .expect("report consumed once")
            .recv_timeout(Duration::from_secs(60))
            .expect("server finishes despite the stalled reader");
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        report
    });
    let stalled = faction_rx.recv().expect("stalled faction");
    match &report.outcome {
        MatchOutcome::ClientForfeit {
            winner,
            reason_losing,
            ..
        } => {
            assert_eq!(*winner, Some(stalled.opposite()));
            assert!(
                reason_losing.contains("write stalled"),
                "forfeit reason: {reason_losing}"
            );
        }
        other => panic!("expected CLIENT_FORFEIT, got {other:?}"),
    }
    assert!(
        report.journal.contains("write_stalled"),
        "{}",
        report.journal
    );
}

// (D62) The advertised frame limit must match the enforced one: the
// published MatchInfo value is derived from the compiled-in protocol
// constant, and a GameConfig whose frame ceiling disagrees with it is
// refused before anything is bound or published.
#[test]
fn mismatched_frame_ceiling_config_is_refused() {
    let mut config = fast_config(1);
    config.max_frame_payload_bytes = arena_protocol::MAX_FRAME_PAYLOAD_BYTES as u32;
    let dir = temp_dir("badceiling");
    let socket = dir.join("arena.sock");
    let setup = MatchSetup {
        game_seed: 1,
        map_seed: 1,
        config,
        socket_path: socket.clone(),
        match_id: format!("badceiling-{}", std::process::id()),
        out_dir: dir.join("out"),
    };
    let error = match serve_match(setup) {
        Err(error) => error,
        Ok(_) => panic!("config disagrees with the protocol limit: must be refused"),
    };
    assert!(
        error.to_string().contains("max_frame_payload_bytes"),
        "{error}"
    );
    assert!(!socket.exists(), "nothing was bound");
}

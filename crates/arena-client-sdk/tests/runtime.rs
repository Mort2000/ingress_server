//! SDK runtime tests (M7 checklist 14/15/20 siblings): the cross-crate
//! SHM twin agreement (server publisher -> SDK reader), the context-close
//! and phase-violation refusals, scoped-thread usability of the view, and
//! the in-process happy path — a real `arena_client_sdk::run` client with
//! a real `ClientBot` playing a live `serve_match` server end to end.

use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use arena_client_sdk::bot::ClientBot;
use arena_client_sdk::conn::Connection;
use arena_client_sdk::round::{RoundContext, SpawnContext};
use arena_client_sdk::shm::{RegionHeader, ShmReader, region_names};
use arena_client_sdk::{ConnectOptions, RunSummary, SdkError, run};
use arena_core::types::{AgentId, Coord, Faction};
use arena_protocol::decode::{decode_private, decode_public};
use arena_protocol::{PRIVATE_SNAPSHOT_VERSION, PUBLIC_SNAPSHOT_VERSION, build};

use arena_server::shm::{
    PRIVATE_CAPACITY, PUBLIC_CAPACITY, REGION_KIND_ENL, REGION_KIND_PUBLIC, REGION_KIND_RES,
    ShmPublisher,
};

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m7-sdk")
        .join(format!("{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

// Checklist 15 sibling: the server's publisher and the SDK's reader agree
// on the region layout byte-for-byte — the SDK reads a region the SERVER
// published and decodes the very bytes it wrote.
#[test]
fn sdk_reader_reads_server_published_regions() {
    let match_id = format!("twin-{}", std::process::id());
    let (public_name, enl_name, res_name) = region_names(&match_id);
    let public = ShmPublisher::create(
        &public_name,
        REGION_KIND_PUBLIC,
        PUBLIC_SNAPSHOT_VERSION,
        PUBLIC_CAPACITY,
    )
    .expect("public region");
    let enl = ShmPublisher::create(
        &enl_name,
        REGION_KIND_ENL,
        PRIVATE_SNAPSHOT_VERSION,
        PRIVATE_CAPACITY,
    )
    .expect("enl region");
    let res = ShmPublisher::create(
        &res_name,
        REGION_KIND_RES,
        PRIVATE_SNAPSHOT_VERSION,
        PRIVATE_CAPACITY,
    )
    .expect("res region");

    // Publish a generation-0 map + rosters through the SERVER builders.
    let cfg = arena_core::config::GameConfig::default();
    let map = arena_core::map::MapDefinition::new(Vec::new());
    let mut public = public;
    let mut enl = enl;
    let mut res = res;
    public
        .publish(
            &build::map_generation_bytes(&map, 1_000, 6_000),
            0,
            1_000,
            6_000,
        )
        .expect("publish map");
    enl.publish(&build::private_roster_bytes(&cfg, Faction::Enl), 0, 0, 0)
        .expect("publish enl");
    res.publish(&build::private_roster_bytes(&cfg, Faction::Res), 0, 0, 0)
        .expect("publish res");

    // The SDK readers (their OWN twin definitions) read and decode them.
    let public_reader = ShmReader::open(&public_name, RegionHeader::public()).expect("open");
    let enl_reader = ShmReader::open(
        &enl_name,
        RegionHeader::private(arena_client_sdk::shm::REGION_KIND_ENL),
    )
    .expect("open");
    let snapshot = public_reader.read().expect("read");
    assert_eq!(snapshot.snapshot_version, 1);
    assert_eq!(snapshot.round_start_ms, 1_000);
    assert_eq!(snapshot.deadline_ms, 6_000);
    assert!(decode_public(snapshot.payload).is_ok());
    let roster = enl_reader.read().expect("read");
    let decoded = decode_private(roster.payload).expect("decode");
    assert_eq!(
        decoded.agents.len(),
        cfg.initial_agents_per_faction as usize
    );

    // Wrong-kind open is refused: the ENL name is not a public region.
    assert!(ShmReader::open(&enl_name, RegionHeader::public()).is_err());
    drop((public, enl, res));
}

/// D78 (M8): an SHM-DERIVED `payload_len` beyond the immutable
/// capacity — written into the live region from OUTSIDE the mapping,
/// exactly as non-compliant same-UID code could — is refused by the
/// SDK reader as `Corrupt` and never turned into a lent slice. The
/// bound check runs before the confirming seq load; the read path
/// also carries the D78 read-side Acquire fence.
#[test]
fn sdk_reader_refuses_oversize_shm_derived_payload_len() {
    use std::os::unix::fs::FileExt;

    let match_id = format!("twin-corrupt-{}", std::process::id());
    let (public_name, _enl_name, _res_name) = region_names(&match_id);
    let capacity = 64 * 1024usize;
    let mut publisher = ShmPublisher::create(
        &public_name,
        REGION_KIND_PUBLIC,
        PUBLIC_SNAPSHOT_VERSION,
        capacity,
    )
    .expect("region");
    publisher.publish(&[9u8; 32], 1, 0, 0).expect("publish");

    let reader = ShmReader::open(
        &public_name,
        RegionHeader {
            magic: arena_client_sdk::shm::MAGIC,
            layout_version: arena_protocol::LAYOUT_VERSION,
            region_kind: arena_client_sdk::shm::REGION_KIND_PUBLIC,
            schema_version: PUBLIC_SNAPSHOT_VERSION,
            capacity: capacity as u32,
        },
    )
    .expect("open");
    let first = reader.read().expect("healthy read");
    assert_eq!(first.payload.len(), 32);

    // Corrupt the ACTIVE buffer's length field (after one publish the
    // active buffer is B; the field offset is pinned by the twin
    // layout's compile-time assertions).
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(format!("/dev/shm{public_name}"))
        .expect("raw region file");
    use std::mem::offset_of;
    let offset = offset_of!(arena_server::shm::MetadataBlock, payload_len_b);
    file.write_all_at(&((capacity as u32 + 1).to_le_bytes()), offset as u64)
        .expect("corrupt payload_len_b");
    match reader.read() {
        Err(arena_client_sdk::shm::ShmError::Corrupt {
            field: "payload_len",
        }) => {}
        Err(other) => panic!("expected Corrupt, got {other:?}"),
        Ok(_) => panic!("expected Corrupt, got a snapshot"),
    }
    // Repair: the reader serves the healthy generation again.
    file.write_all_at(&(32u32.to_le_bytes()), offset as u64)
        .expect("repair");
    let repaired = reader.read().expect("repaired read");
    assert_eq!(repaired.payload.len(), 32);
}

// A session harness on real (server-published) regions plus a dummy
// connected socket, for the context/phase refusals.
struct Fixture {
    _publishers: [ShmPublisher; 3],
    session: arena_client_sdk::round::Session,
    conn: Connection,
    _listener: UnixListener,
}

fn fixture(tag: &str) -> Fixture {
    let match_id = format!("ctx-{}-{tag}", std::process::id());
    let (public_name, enl_name, res_name) = region_names(&match_id);
    let mut publishers = [
        ShmPublisher::create(
            &public_name,
            REGION_KIND_PUBLIC,
            PUBLIC_SNAPSHOT_VERSION,
            PUBLIC_CAPACITY,
        )
        .expect("public"),
        ShmPublisher::create(
            &enl_name,
            REGION_KIND_ENL,
            PRIVATE_SNAPSHOT_VERSION,
            PRIVATE_CAPACITY,
        )
        .expect("enl"),
        ShmPublisher::create(
            &res_name,
            REGION_KIND_RES,
            PRIVATE_SNAPSHOT_VERSION,
            PRIVATE_CAPACITY,
        )
        .expect("res"),
    ];
    // Round-1-like generation: roster with real positions. The decision
    // window is tiny so the deadline-closure path is testable fast.
    let mut cfg = arena_core::config::GameConfig::default();
    cfg.decision_window_ms = 1;
    let map = arena_core::map::MapDefinition::new(Vec::new());
    publishers[0]
        .publish(&build::map_generation_bytes(&map, 0, 0), 0, 0, 0)
        .expect("map gen");
    publishers[1]
        .publish(&build::private_roster_bytes(&cfg, Faction::Enl), 1, 0, 0)
        .expect("enl gen");
    publishers[2]
        .publish(&build::private_roster_bytes(&cfg, Faction::Res), 1, 0, 0)
        .expect("res gen");

    // A dummy socket pair so RoundContext can hold a real Connection.
    let dir = temp(tag);
    let socket_path = dir.join("dummy.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind");
    let conn = Connection::connect(&socket_path).expect("connect");

    let session = arena_client_sdk::round::Session::new(&match_id, Faction::Enl, cfg)
        .expect("session builds over the regions");
    Fixture {
        _publishers: publishers,
        session,
        conn,
        _listener: listener,
    }
}

// Checklist 14 (runtime half): the context closes on last-submit, commit,
// and the deadline; every later borrow or submission is ContextClosed.
#[test]
fn context_closes_on_last_submit_and_commit() {
    let mut f = fixture("close");
    f.session.phase().on_round_start();
    let mut ctx = f
        .session
        .round_context(&mut f.conn, 1)
        .expect("context opens in an open phase");
    assert!(!ctx.is_closed());
    let roster = f.session.own_agent_ids().expect("roster");

    // Submit every agent: the LAST submission closes the context.
    for (index, agent) in roster.iter().enumerate() {
        assert!(!ctx.is_closed(), "still open until the last agent");
        ctx.submit(*agent, Vec::new()).expect("submit");
        if index + 1 == roster.len() {
            assert!(ctx.is_closed(), "last submit closes");
        }
    }
    assert!(matches!(ctx.observation(), Err(SdkError::ContextClosed)));
    assert!(matches!(
        ctx.submit(AgentId(999), Vec::new()),
        Err(SdkError::ContextClosed)
    ));
    // Commit after completion is the IDEMPOTENT no-op (submitting every
    // agent already completed this faction's input).
    ctx.commit().expect("commit is idempotent after completion");

    // A fresh context closes on explicit commit immediately.
    let mut ctx = f.session.round_context(&mut f.conn, 2).expect("opens");
    ctx.commit().expect("commit");
    assert!(ctx.is_closed());
    assert!(matches!(ctx.observation(), Err(SdkError::ContextClosed)));

    // A DEADLINE closure refuses everything, commit included.
    let ctx = f.session.round_context(&mut f.conn, 3).expect("opens");
    std::thread::sleep(Duration::from_millis(5));
    let mut ctx = ctx;
    assert!(matches!(
        ctx.submit(AgentId(0), Vec::new()),
        Err(SdkError::ContextClosed)
    ));
    assert!(matches!(ctx.commit(), Err(SdkError::ContextClosed)));
    assert!(matches!(ctx.observation(), Err(SdkError::ContextClosed)));
}

// Checklist 14 (runtime half): between ROUND_CLOSED and the next
// ROUND_START_SIGNAL every SHM read is refused (Settlement Phase).
#[test]
fn settlement_phase_refuses_reads() {
    let mut f = fixture("phase");
    f.session.phase().on_round_start();
    let mut ctx = f.session.round_context(&mut f.conn, 1).expect("opens");
    let _view = ctx.observation().expect("licensed while open");
    ctx.commit().expect("commit");

    // The server announces ROUND_CLOSED: phase -> Settlement.
    f.session.phase().on_round_closed();
    assert!(matches!(
        f.session.round_context(&mut f.conn, 2),
        Err(SdkError::PhaseViolation)
    ));
    assert!(matches!(
        f.session.own_agent_ids(),
        Err(SdkError::PhaseViolation)
    ));
    // The next ROUND_START_SIGNAL re-licenses.
    f.session.phase().on_round_start();
    assert!(f.session.round_context(&mut f.conn, 2).is_ok());
}

// P3-RPC: "从 ROUND_CLOSED 到下一次 ROUND_START_SIGNAL，Client 读取任何
// SHM 都属于作弊" — the official SDK refusing IS the v0.1 enforcement.
// A view lent while the round was open (e.g. a COPY held by a scoped
// worker thread while the main loop waits for the next ROUND_START_SIGNAL)
// must refuse every read the moment the phase leaves RoundOpen.
#[test]
fn lent_views_refuse_reads_after_round_closed() {
    let mut f = fixture("viewphase");
    f.session.phase().on_round_start();
    let ctx = f.session.round_context(&mut f.conn, 1).expect("opens");
    let view = ctx.observation().expect("view while open");
    assert!(view.portals().is_ok());
    assert!(view.round().is_ok());
    assert!(view.own_agents().is_ok());

    // The participant loop saw ROUND_CLOSED: Settlement Phase.
    f.session.phase().on_round_closed();
    assert!(matches!(view.portals(), Err(SdkError::PhaseViolation)));
    assert!(matches!(view.links(), Err(SdkError::PhaseViolation)));
    assert!(matches!(view.fields(), Err(SdkError::PhaseViolation)));
    assert!(matches!(view.events(), Err(SdkError::PhaseViolation)));
    assert!(matches!(view.scores(), Err(SdkError::PhaseViolation)));
    assert!(matches!(view.own_agents(), Err(SdkError::PhaseViolation)));
    assert!(matches!(view.round(), Err(SdkError::PhaseViolation)));
    assert!(matches!(
        view.snapshot_version(),
        Err(SdkError::PhaseViolation)
    ));
    assert!(matches!(
        view.to_observation(),
        Err(SdkError::PhaseViolation)
    ));

    // The pre-match map view is equally bound to the phase gate.
    f.session.phase().on_initial_spawn_start();
    let map = f
        .session
        .spawn_context()
        .expect("spawn ctx")
        .map_view()
        .expect("map view in spawn phase");
    assert!(map.portals().is_ok());
    assert!(map.snapshot_version().is_ok());
    f.session.phase().on_round_closed();
    assert!(matches!(map.portals(), Err(SdkError::PhaseViolation)));
    assert!(matches!(
        map.to_map_definition(),
        Err(SdkError::PhaseViolation)
    ));
    assert!(matches!(
        map.snapshot_version(),
        Err(SdkError::PhaseViolation)
    ));

    // The next ROUND_START_SIGNAL re-licenses the SAME lent view.
    f.session.phase().on_round_start();
    assert!(view.round().is_ok());
    assert!(map.portals().is_ok());
}

// The positive counterpart of the compile-fail suite: a COPY of the view
// works inside std::thread::scope and the workers join by construction.
#[test]
fn scoped_threads_can_share_view_copies() {
    let mut f = fixture("scope");
    f.session.phase().on_round_start();
    let ctx = f.session.round_context(&mut f.conn, 1).expect("opens");
    let view = ctx.observation().expect("view");
    let rounds: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let view = view; // Copy
                scope.spawn(move || view.round())
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("worker joins"))
            .collect()
    });
    assert!(rounds.iter().all(|r| r.is_ok()));
}

// ---------------------------------------------------------------------------
// Checklist 20: the in-process happy path — a real SDK client with a real
// ClientBot plays a live serve_match server.
// ---------------------------------------------------------------------------

struct PassBot {
    rounds: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

struct FeedbackBot {
    receipts: std::sync::Arc<std::sync::Mutex<Vec<arena_client_sdk::SubmissionFeedback>>>,
    timings: std::sync::Arc<std::sync::Mutex<Vec<arena_client_sdk::RoundTiming>>>,
}

impl ClientBot for FeedbackBot {
    fn choose_spawns(&mut self, ctx: &SpawnContext<'_>) -> Vec<(AgentId, Coord)> {
        ctx.roster()
            .into_iter()
            .map(|id| (id, Coord::new(0, 0)))
            .collect()
    }
    fn on_round(&mut self, mut ctx: RoundContext<'_>) {
        let agents = ctx.observation().unwrap().own_agents().unwrap();
        // A second submission cannot replace the first, but its receipt
        // must remain observable separately from the accepted receipt.
        ctx.submit(agents[0].id, vec![]).unwrap();
        ctx.submit(agents[0].id, vec![]).unwrap();
        for agent in &agents[1..] {
            ctx.submit(agent.id, vec![]).unwrap();
        }
    }
    fn on_submission_feedback(&mut self, feedback: &arena_client_sdk::SubmissionFeedback) {
        self.receipts.lock().unwrap().push(feedback.clone());
    }
    fn on_round_timing(&mut self, timing: &arena_client_sdk::RoundTiming) {
        self.timings.lock().unwrap().push(timing.clone());
    }
}

#[test]
fn callbacks_preserve_round_duplicate_receipts_and_local_submission_timing() {
    let dir = temp("feedback");
    let socket = dir.join("arena.sock");
    let setup = arena_server::MatchSetup {
        game_seed: 71,
        map_seed: 9,
        config: arena_server::fast_config(2),
        socket_path: socket.clone(),
        match_id: format!("feedback-{}", std::process::id()),
        out_dir: dir.join("out"),
    };
    let server = std::thread::spawn(move || arena_server::serve_match(setup).unwrap());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !socket.exists() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let receipts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let timings = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let receipts = receipts.clone();
            let timings = timings.clone();
            let socket = &socket;
            scope.spawn(move || {
                run(
                    |_, _, _| FeedbackBot { receipts, timings },
                    ConnectOptions::new(socket, "feedback"),
                )
                .unwrap()
            });
        }
    });
    assert!(matches!(
        server.join().unwrap().outcome,
        arena_server::MatchOutcome::Normal { .. }
    ));
    let receipts = receipts.lock().unwrap();
    for round in 1..=2 {
        let rs: Vec<_> = receipts.iter().filter(|r| r.round == round).collect();
        assert_eq!(
            rs.iter()
                .filter(|r| r.status == arena_client_sdk::SubmissionStatus::Accepted)
                .count(),
            6
        );
        assert_eq!(
            rs.iter()
                .filter(|r| r.status == arena_client_sdk::SubmissionStatus::IgnoredDuplicate)
                .count(),
            2
        );
    }
    let timings = timings.lock().unwrap();
    assert_eq!(timings.len(), 4);
    for timing in timings.iter() {
        assert_eq!(timing.submissions.len(), 4);
        assert!(
            timing
                .submissions
                .windows(2)
                .all(|w| w[0].sent_after_us <= w[1].sent_after_us)
        );
        assert!(timing.submissions.last().unwrap().sent_after_us <= timing.callback_elapsed_us);
    }
}

impl ClientBot for PassBot {
    fn choose_spawns(&mut self, ctx: &SpawnContext<'_>) -> Vec<(AgentId, Coord)> {
        let roster = ctx.roster();
        roster
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, Coord::new(i as i32, i as i32)))
            .collect()
    }

    fn on_round(&mut self, mut ctx: RoundContext<'_>) {
        self.rounds
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let view = ctx.observation().expect("observation licensed");
        let round = view.round().expect("round");
        assert_eq!(round, ctx.round());
        let _ = view.portals().expect("portals");
        let _ = view.own_agents().expect("own agents");
        // Submit every agent an explicit empty pass, then commit (early
        // completion path).
        for agent in view.own_agents().expect("own agents") {
            ctx.submit(agent.id, Vec::new()).expect("submit");
        }
        ctx.commit().expect("commit");
    }
}

#[test]
fn sdk_client_plays_a_live_match_end_to_end() {
    let dir = temp("happy");
    let socket = dir.join("arena.sock");
    let match_id = format!("happy-{}", std::process::id());
    let setup = arena_server::MatchSetup {
        game_seed: 0xABCD,
        map_seed: 9,
        config: arena_server::fast_config(6),
        socket_path: socket.clone(),
        match_id: match_id.clone(),
        out_dir: dir.join("out"),
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        tx.send(arena_server::serve_match(setup).expect("server runs"))
            .expect("send report");
    });
    while !socket.exists() {
        std::thread::sleep(Duration::from_millis(2));
    }
    let socket2 = socket.clone();

    let rounds = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let enl_rounds = std::sync::Arc::clone(&rounds);
    let res_rounds = std::sync::Arc::clone(&rounds);
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let summary = run(
                |_faction, _cfg, _map| PassBot { rounds: enl_rounds },
                ConnectOptions::new(&socket2, "sdk-enl"),
            )
            .expect("client 1 plays the whole match");
            assert!(matches!(
                summary,
                RunSummary::Normal {
                    rounds_completed: 6,
                    ..
                }
            ));
        });
        scope.spawn(move || {
            let summary = run(
                |_faction, _cfg, _map| PassBot { rounds: res_rounds },
                ConnectOptions::new(&socket, "sdk-res"),
            )
            .expect("client 2 plays the whole match");
            assert!(matches!(
                summary,
                RunSummary::Normal {
                    rounds_completed: 6,
                    ..
                }
            ));
        });
    });

    let report = rx
        .recv_timeout(Duration::from_secs(60))
        .expect("server done");
    assert!(matches!(
        report.outcome,
        arena_server::MatchOutcome::Normal { .. }
    ));
    assert!(report.replay_path.is_some(), "the replay was written");
    assert_eq!(rounds.load(std::sync::atomic::Ordering::Relaxed), 12);
    // Both factions submitted every agent each round (explicit passes).
    assert_eq!(report.metrics[0].timeout_rate_permille(), 0);
    assert_eq!(report.metrics[1].timeout_rate_permille(), 0);
}

//! P3-GATE acceptance (a) + (b): a full live match — server binary plus
//! two real external protocol fixture processes —
//! completes 500 rounds, both clients receive GAME_END, the normal result
//! and IARP replay are written, and the external runtime is EQUIVALENT to
//! the local simulator: the recorded IARP replays through arena-replay,
//! and an independent in-process re-execution of the recorded submissions
//! through the same Resolver reproduces every per-round state hash and
//! the final result.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use arena_core::resolve::Resolver;
use arena_core::state::GameState;
use arena_replay::format::{Block, EndOfMatch};
use arena_replay::{format, replay};

/// Genuinely slow (spawns real child processes and runs 500 rounds);
/// included by default because it finishes in a few seconds, but can be
/// skipped with `ARENA_SKIP_LIVE=1` during tight iteration.
fn maybe_skip() {
    if std::env::var("ARENA_SKIP_LIVE").is_ok() {
        eprintln!("skipping live match (ARENA_SKIP_LIVE=1)");
    }
}

fn target_bin(name: &str) -> PathBuf {
    // Use the same Cargo target directory/profile as this test binary.
    // Looking in debug first can silently execute a stale fixture in release tests.
    let executable = std::env::current_exe().expect("test executable");
    let profile = executable.parent().unwrap().parent().unwrap();
    let candidate = profile.join(name);
    assert!(
        candidate.is_file(),
        "{} not built; run cargo build --workspace with this profile",
        candidate.display()
    );
    candidate
}

struct Match {
    server: Child,
    out_dir: PathBuf,
    _socket: PathBuf,
}

fn start_match(rounds: u32) -> Match {
    let tag = format!("live-{}-{}", std::process::id(), rounds);
    let dir = std::env::temp_dir().join("ingress-m7").join(&tag);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let socket = dir.join("arena.sock");
    let out_dir = dir.join("out");
    let server = Command::new(target_bin("arena-server"))
        .arg("--socket")
        .arg(&socket)
        .arg("--seed")
        .arg("0x00C0FFEE")
        .arg("--map-seed")
        .arg("3")
        .arg("--config")
        .arg("fast")
        .arg("--rounds")
        .arg(rounds.to_string())
        .arg("--match-id")
        .arg(&tag)
        .arg("--out")
        .arg(&out_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("server binary spawns");
    // Wait for the socket to appear.
    let deadline = Instant::now() + Duration::from_secs(20);
    while !socket.exists() {
        assert!(Instant::now() < deadline, "server never bound its socket");
        std::thread::sleep(Duration::from_millis(10));
    }
    Match {
        server,
        out_dir,
        _socket: socket,
    }
}

/// Waits for a child and returns (exit code, captured stdout).
fn client_exit_status(child: Child) -> (Option<i32>, String) {
    let mut child = child;
    let mut buffer = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_string(&mut buffer);
    }
    let code = child.wait().ok().and_then(|status| status.code());
    (code, buffer)
}

fn spawn_client(bin: &str, socket: &Path, seed: u64) -> Child {
    Command::new(target_bin(bin))
        .arg(socket)
        .arg(seed.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("client binary spawns")
}

#[test]
fn live_match_500_rounds_with_external_clients() {
    maybe_skip();
    let started = Instant::now();
    let total_rounds = 500;
    let mut m = start_match(total_rounds);
    let socket = m._socket.clone();
    let c1 = spawn_client("probe-client", &socket, 11);
    let c2 = spawn_client("probe-client", &socket, 22);
    let (code1, out1) = client_exit_status(c1);
    let (code2, out2) = client_exit_status(c2);
    let server_status = m.server.wait().expect("server exits");
    let elapsed = started.elapsed();

    // Both external clients received GAME_END and reported the SAME
    // result line (outcome + rounds + scores).
    assert_eq!(code1, Some(0), "probe-client failed: {out1}");
    assert_eq!(code2, Some(0), "probe-client failed: {out2}");
    assert!(out1.starts_with("outcome="), "{out1}");
    assert_eq!(out1, out2, "both clients saw the same GAME_END payload");

    // (a) P3-GATE: the match completes 500 rounds with a normal result and
    // writes the IARP; the server exits 0.
    assert!(server_status.success(), "server status {server_status:?}");
    let match_dir = m.out_dir.join(
        std::fs::read_dir(&m.out_dir)
            .expect("out dir")
            .next()
            .expect("one match dir")
            .ok()
            .expect("dir entry")
            .file_name(),
    );
    let result = std::fs::read_to_string(match_dir.join("result.txt")).expect("result.txt");
    assert!(result.contains("outcome="), "{result}");
    assert!(
        !result.contains("setup_error") && !result.contains("forfeit"),
        "no technical outcome in a healthy match: {result}"
    );
    assert!(result.contains("rounds_completed=500"), "{result}");
    let replay_path = match_dir.join("replay.iarp");
    let replay_bytes = std::fs::read(&replay_path).expect("replay.iarp written");
    assert!(replay_bytes.len() > 10_000, "a real 500-round replay");

    // The journal recorded the full parity sequence and metrics exist.
    let journal = std::fs::read_to_string(match_dir.join("journal.txt")).expect("journal");
    assert_eq!(
        journal
            .lines()
            .filter(|l| l.contains("round_start="))
            .count(),
        500
    );
    assert!(result.contains("enl_submissions=") && result.contains("res_buckets="));

    // Wall-clock budget (debug builds): generous headroom over the
    // observed ~3 s.
    assert!(
        elapsed < Duration::from_secs(120),
        "live match took {elapsed:?}"
    );
}

/// Checklist (b) part 1: the server's IARP decodes and replays through
/// arena-replay — every per-round hash, event, and intake fact re-verifies.
#[test]
fn server_replay_file_replays_identically() {
    maybe_skip();
    let m = run_quick_live_match("replay-check", 60);
    let replay_bytes = std::fs::read(m.replay_path()).expect("replay");
    let file = format::decode(&replay_bytes).expect("IARP decodes");
    assert_eq!(file.round_records().count(), 60);
    let outcome = replay::replay(&file).expect("replay verifies every round");
    match file.trailer() {
        Some(EndOfMatch::Normal {
            rounds_completed, ..
        }) => assert_eq!(*rounds_completed, 60),
        other => panic!("normal trailer expected, got {other:?}"),
    }
    let _ = outcome;
}

/// Checklist (b) part 2: independent in-process re-execution — a fresh
/// `GameState` from the IARP header plus `Resolver::with_seed`, fed the
/// recorded `RoundReport.submissions` VERBATIM (receive metadata included)
/// reproduces every per-round state hash and the final result. External
/// runtime ≡ local simulator through the shared Resolver (P3-GATE).
#[test]
fn independent_reexecution_matches_all_round_hashes() {
    maybe_skip();
    let m = run_quick_live_match("equivalence", 60);
    let replay_bytes = std::fs::read(m.replay_path()).expect("replay");
    let file = format::decode(&replay_bytes).expect("decodes");

    let header = &file.header;
    let mut cfg = header.config;
    cfg.total_rounds = header.total_rounds;
    let map = header.map.clone();
    let enl_spawns = arena_core::spawn::validate_initial_spawn_set(
        &GameState::initial_roster(&cfg).0,
        &header.enl_spawns,
        &cfg,
    )
    .expect("header enl spawns");
    let res_spawns = arena_core::spawn::validate_initial_spawn_set(
        &GameState::initial_roster(&cfg).1,
        &header.res_spawns,
        &cfg,
    )
    .expect("header res spawns");
    let mut state = GameState::new(cfg, &map, &enl_spawns, &res_spawns).expect("state");
    let resolver = Resolver::with_seed(header.game_seed);

    let mut hashes: Vec<[u8; 32]> = Vec::new();
    for block in &file.blocks {
        if let Block::Round(record) = block {
            let report = resolver.run_round(&mut state, &record.submissions);
            // The recorded execution order (fixed before any command ran)
            // must equal the independent re-execution's order.
            let replay_order: Vec<arena_core::types::AgentId> =
                report.execution_order.iter().map(|s| s.agent_id).collect();
            assert_eq!(
                replay_order, record.execution_order,
                "round {}",
                record.round
            );
            // The round's slots (intake verdicts) re-derive identically.
            let mut replay_slots: Vec<_> = report.slots.iter().collect();
            let mut record_slots: Vec<_> = record.slots.iter().collect();
            replay_slots.sort_by_key(|(id, _)| **id);
            record_slots.sort_by_key(|(id, _)| **id);
            assert_eq!(replay_slots, record_slots, "round {}", record.round);
            state.advance_round();
            hashes.push(arena_replay::canon::state_hash(&state));
        }
    }
    let recorded: Vec<[u8; 32]> = file
        .round_records()
        .map(|record| record.state_hash)
        .collect();
    assert_eq!(hashes.len(), recorded.len());
    for (index, (ours, theirs)) in hashes.iter().zip(recorded.iter()).enumerate() {
        assert_eq!(ours, theirs, "state hash diverged at round {}", index + 1);
    }

    match file.trailer() {
        Some(EndOfMatch::Normal {
            final_state_hash,
            result,
            ..
        }) => {
            assert_eq!(&hashes[hashes.len() - 1], final_state_hash);
            assert_eq!(state.final_result(), *result);
        }
        other => panic!("normal trailer expected, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct QuickMatch {
    #[allow(dead_code)]
    out_dir: PathBuf,
    match_dir: PathBuf,
    #[allow(dead_code)]
    socket: PathBuf,
}

impl QuickMatch {
    fn replay_path(&self) -> PathBuf {
        self.match_dir.join("replay.iarp")
    }
}

fn run_quick_live_match(tag: &str, rounds: u32) -> QuickMatch {
    let dir = std::env::temp_dir().join("ingress-m7").join(format!(
        "{tag}-{}-{}",
        std::process::id(),
        rounds
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let socket = dir.join("arena.sock");
    let out_dir = dir.join("out");
    let match_id = format!("{tag}-{}", std::process::id());
    let mut server = Command::new(target_bin("arena-server"))
        .arg("--socket")
        .arg(&socket)
        .arg("--seed")
        .arg("0x51D5")
        .arg("--map-seed")
        .arg("11")
        .arg("--config")
        .arg("fast")
        .arg("--rounds")
        .arg(rounds.to_string())
        .arg("--match-id")
        .arg(&match_id)
        .arg("--out")
        .arg(&out_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("server spawns");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !socket.exists() {
        assert!(Instant::now() < deadline, "socket never appeared");
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut c1 = spawn_client("probe-client", &socket, 1);
    let mut c2 = spawn_client("probe-client", &socket, 2);
    let _ = c1.wait().expect("client 1 exits");
    let _ = c2.wait().expect("client 2 exits");
    let status = server.wait().expect("server exits");
    assert!(status.success(), "server status {status:?}");
    let match_dir = out_dir.join(&match_id);
    assert!(match_dir.join("replay.iarp").exists(), "replay written");
    QuickMatch {
        out_dir,
        match_dir,
        socket,
    }
}

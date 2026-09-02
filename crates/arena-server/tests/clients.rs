//! Child-process client tests (M7 checklist i and the process-level slow
//! client): real client binaries against the real server binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use arena_replay::format::{EndOfMatch, decode as decode_replay};

fn target_bin(name: &str) -> PathBuf {
    if let Ok(path) = std::env::var(format!("CARGO_BIN_EXE_{name}")) {
        return PathBuf::from(path);
    }
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target");
    for profile in ["debug", "release"] {
        let candidate = target.join(profile).join(name);
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("binary {name} not built (run cargo build --workspace)");
}

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m7-clients")
        .join(format!("{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn spawn_server(tag: &str, rounds: u32) -> (std::process::Child, PathBuf, PathBuf) {
    let dir = temp(tag);
    let socket = dir.join("arena.sock");
    let out_dir = dir.join("out");
    let child = Command::new(target_bin("arena-server"))
        .arg("--socket")
        .arg(&socket)
        .arg("--seed")
        .arg("0x11")
        .arg("--map-seed")
        .arg("2")
        .arg("--config")
        .arg("fast")
        .arg("--rounds")
        .arg(rounds.to_string())
        .arg("--match-id")
        .arg(tag)
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
    (child, socket, out_dir)
}

fn spawn_probe(socket: &Path, mode: &str, arg: u64) -> std::process::Child {
    Command::new(target_bin("probe-client"))
        .arg(socket)
        .env("PROBE_MODE", mode)
        .env("PROBE_ARG", arg.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("probe spawns")
}

/// (i) A client SIGKILLed mid-match: the surviving side wins by
/// CLIENT_FORFEIT, the server exits cleanly (6/7), the journal records
/// the forfeit, and the IARP ends with an ABNORMAL trailer.
#[test]
fn killed_client_forfeits_and_server_finishes_cleanly() {
    let (mut server, socket, out_dir) = spawn_server("kill", 10);
    let mut killer = spawn_probe(&socket, "self-kill", 2);
    let mut survivor = spawn_probe(&socket, "valid", 0);

    let _ = survivor.wait().expect("survivor exits");
    let _ = killer.wait().expect("killer exits (9)");
    let status = server.wait().expect("server exits");
    let code = status.code().expect("exit code");
    assert!(
        code == 6 || code == 7,
        "client-forfeit exit code, got {code}"
    );

    let journal = std::fs::read_to_string(out_dir.join("kill/journal.txt")).expect("journal");
    assert!(journal.contains("client_forfeit"), "{journal}");
    assert!(
        journal.contains("round_start=2"),
        "the kill landed at round 2"
    );

    // The replay ends with an abnormal trailer naming the last completed
    // round (D46) — the interrupted round settles, no phantom rounds.
    let bytes = std::fs::read(out_dir.join("kill/replay.iarp")).expect("replay written");
    let file = decode_replay(&bytes).expect("replay decodes");
    match file.trailer() {
        Some(EndOfMatch::Abnormal {
            last_completed_round,
            outcome_tag,
            ..
        }) => {
            assert_eq!(*outcome_tag, arena_replay::format::OUTCOME_CLIENT_FORFEIT);
            assert!(*last_completed_round >= 2);
            assert_eq!(file.round_records().count() as u32, *last_completed_round);
        }
        other => panic!("expected abnormal trailer, got {other:?}"),
    }
}

/// (k, process level) A slow client process never blocks the server: the
/// match completes normally with every round waiting the full window for
/// the slow side, whose slots are all NotSubmitted.
#[test]
fn slow_probe_client_does_not_block_the_match() {
    let (mut server, socket, out_dir) = spawn_server("slowprobe", 3);
    let started = Instant::now();
    let mut slow = spawn_probe(&socket, "slow-rounds", 0);
    let mut prompt = spawn_probe(&socket, "valid", 0);
    let _ = slow.wait();
    let _ = prompt.wait();
    let status = server.wait().expect("server exits");
    let elapsed = started.elapsed();
    assert!(status.success(), "normal completion");

    let result = std::fs::read_to_string(out_dir.join("slowprobe/result.txt")).expect("result");
    assert!(result.contains("rounds_completed=3"), "{result}");
    // 3 rounds x (40 ms window + probe sleep) — the slow side forced the
    // deadline every round.
    assert!(
        elapsed >= Duration::from_millis(120),
        "slow rounds each waited the window: {elapsed:?}"
    );
    assert!(
        result.contains("_timeout_rate_permille=1000"),
        "the slow side timed out every slot: {result}"
    );
    assert!(
        result.contains("_timeout_rate_permille=0"),
        "the prompt side never timed out: {result}"
    );
}

/// The probe's giant-name mode: a CLIENT_HELLO whose padded name grows
/// the frame payload to exactly 65,535 bytes — in a real process — is
/// processed normally (the match completes).
#[test]
fn probe_giant_name_frame_is_processed() {
    let (mut server, socket, out_dir) = spawn_server("giant", 2);
    let mut giant = spawn_probe(&socket, "giant-name", 0);
    let mut other = spawn_probe(&socket, "valid", 0);
    let _ = giant.wait().expect("giant client exits cleanly");
    let _ = other.wait();
    let status = server.wait().expect("server exits");
    assert!(status.success());
    let result = std::fs::read_to_string(out_dir.join("giant/result.txt")).expect("result");
    assert!(result.contains("rounds_completed=2"), "{result}");
}

use arena_runner::tournament::{archive::parse_json, monitor::ArchiveMonitor};
use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "arena-monitor-{}",
            arena_runner::tournament::exec_external::execution_id().unwrap()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn plan(&self) -> PathBuf {
        self.plan_client("iteration-client")
    }
    fn plan_client(&self, client: &str) -> PathBuf {
        let root = self.0.join("event");
        let bot = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug")
            .join(client)
            .canonicalize()
            .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_arena-runner"))
            .args(["plan", "--out"])
            .arg(&root)
            .arg("--client")
            .arg(&bot)
            .arg("--client")
            .arg(&bot)
            .args([
                "--name",
                "A <星际>",
                "--name",
                "B",
                "--bot-version",
                "v1",
                "--bot-version",
                "v2",
                "--rounds",
                "2",
                "--fast",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        root
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn oversized_uncommitted_text_is_rejected() {
    let f = Fixture::new();
    let root = f.plan();
    let dir = root.join("matches/m0/attempts/0001");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("state"), "running").unwrap();
    fs::write(dir.join("reason.txt"), vec![b'x'; 64 * 1024 + 1]).unwrap();
    let mut reader = ArchiveMonitor::open(&root).unwrap();
    let view = reader.snapshot().unwrap();
    assert_eq!(view.matches[0].state(), "corrupt");
    assert!(view.matches[0].attempts[0].attempt_reason.is_none());
}

#[test]
fn non_ascii_hash_is_corrupt_without_stopping_other_matches() {
    let f = Fixture::new();
    let root = f.plan();
    assert!(
        Command::new(env!("CARGO_BIN_EXE_arena-runner"))
            .args(["resume", "--out"])
            .arg(&root)
            .output()
            .unwrap()
            .status
            .success()
    );
    let path = root.join("matches/m0/attempts/0001/result.tsv");
    let original = fs::read_to_string(&path).unwrap();
    let mut rows = original.lines();
    let header = rows.next().unwrap();
    let hash_column = header
        .split('\t')
        .position(|c| c == "final_state_hash")
        .unwrap();
    let mut fields: Vec<String> = rows
        .next()
        .unwrap()
        .split('\t')
        .map(str::to_owned)
        .collect();
    fields[hash_column] = format!("中{}", "0".repeat(61));
    fs::write(path, format!("{header}\n{}\n", fields.join("\t"))).unwrap();
    let mut reader = ArchiveMonitor::open(&root).unwrap();
    let view = reader.snapshot().unwrap();
    assert_eq!(view.matches[0].state(), "corrupt");
    assert_eq!(view.matches[1].state(), "completed");
}

#[cfg(unix)]
#[test]
fn temporary_directories_cannot_hide_links_from_warm_cache() {
    let f = Fixture::new();
    let root = f.plan();
    assert!(
        Command::new(env!("CARGO_BIN_EXE_arena-runner"))
            .args(["resume", "--out"])
            .arg(&root)
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut reader = ArchiveMonitor::open(&root).unwrap();
    assert_eq!(reader.snapshot().unwrap().matches[0].state(), "completed");
    let hidden = root.join("matches/m0/attempts/0001/extra.tmp");
    fs::create_dir(&hidden).unwrap();
    fs::write(f.0.join("private"), "OUTSIDE").unwrap();
    std::os::unix::fs::symlink(f.0.join("private"), hidden.join("linked.tmp")).unwrap();
    assert_eq!(reader.snapshot().unwrap().matches[0].state(), "corrupt");
    assert!(reader.replay(0, "0001").is_err());
}

#[cfg(unix)]
#[test]
fn linked_attempt_never_exposes_outside_reason_or_replay() {
    let f = Fixture::new();
    let root = f.plan();
    let outside = f.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("reason.txt"), "PRIVATE OUTSIDE DATA").unwrap();
    fs::create_dir_all(root.join("matches/m0/attempts")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("matches/m0/attempts/0001")).unwrap();
    let mut reader = ArchiveMonitor::open(&root).unwrap();
    let json = reader.snapshot().unwrap().to_json();
    assert!(json.contains("corrupt"));
    assert!(!json.contains("PRIVATE OUTSIDE DATA"));
    assert!(reader.replay(0, "0001").is_err());
}

#[test]
fn identical_plan_republication_is_revalidated_without_restart() {
    let f = Fixture::new();
    let root = f.plan();
    let mut reader = ArchiveMonitor::open(&root).unwrap();
    let manifest = root.join("plan/manifest");
    fs::write(&manifest, fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(reader.snapshot().unwrap().matches.len(), 2);
    fs::write(root.join("plan/schedule.tsv"), "corrupt").unwrap();
    assert!(reader.snapshot().is_err());
}

#[test]
fn committed_match_is_visible_before_tournament_finishes_without_summary_writes() {
    use std::time::{Duration, Instant};
    struct Process(std::process::Child);
    impl Drop for Process {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let f = Fixture::new();
    let root = f.plan();
    let mut reader = ArchiveMonitor::open(&root).unwrap();
    let mut process = Process(
        Command::new(env!("CARGO_BIN_EXE_arena-runner"))
            .args(["resume", "--out"])
            .arg(&root)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let view = reader.snapshot().unwrap();
        if view.matches[0].state() == "completed" && view.matches[1].state() != "completed" {
            assert!(process.0.try_wait().unwrap().is_none());
            assert!(reader.replay(0, "0001").is_ok());
            assert!(
                !root.join("index.tsv").exists(),
                "reader must not rebuild summary during execution"
            );
            break;
        }
        assert!(
            Instant::now() < deadline && process.0.try_wait().unwrap().is_none(),
            "missed incremental completion"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(process.0.wait().unwrap().success());
    assert_eq!(reader.snapshot().unwrap().matches[1].state(), "completed");
}

#[test]
fn spawn_failures_are_final_without_replays_or_normal_ratings() {
    let f = Fixture::new();
    let root = f.plan_client("probe-client");
    let result = Command::new(env!("CARGO_BIN_EXE_arena-runner"))
        .args(["resume", "--out"])
        .arg(&root)
        .env("PROBE_MODE", "no-spawn")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut reader = ArchiveMonitor::open(&root).unwrap();
    let snapshot = reader.snapshot().unwrap();
    assert!(snapshot.matches.iter().all(|m| m.state() == "completed"));
    assert_eq!(snapshot.standings[0].rating, 1500);
    assert_eq!(snapshot.standings[0].draws, 0);
    assert_eq!(snapshot.standings[0].unscored, 2);
    assert!(!snapshot.provisional);
    assert_eq!(snapshot.rating_method, "batch_elo_v1");
    assert!(reader.replay(0, "0001").is_err());
}

#[test]
fn observes_plan_then_finished_results_without_runner_or_rebuild() {
    let f = Fixture::new();
    let root = f.plan();
    let mut monitor = ArchiveMonitor::open(&root).unwrap();
    let plan = parse_json(&monitor.snapshot().unwrap().to_json()).unwrap();
    assert_eq!(plan.get("matches").unwrap().as_array().unwrap().len(), 2);
    assert_eq!(plan.get("completed").unwrap().as_int(), Some(0));
    assert_eq!(
        plan.get("provisional"),
        Some(&arena_runner::tournament::archive::Json::Bool(true))
    );
    assert_eq!(
        plan.get("rating_method").unwrap().as_str(),
        Some("online_elo_v1")
    );
    assert!(
        !root.join("execution.lock").exists(),
        "reader must not create or acquire execution lock"
    );
    let o = Command::new(env!("CARGO_BIN_EXE_arena-runner"))
        .args(["resume", "--out"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let standings = fs::read_to_string(root.join("standings.txt")).unwrap();
    assert!(standings.contains("method=batch_elo_v1 status=final"));
    // These are disposable: a reader must neither need nor regenerate them.
    for file in ["index.tsv", "ratings.tsv", "standings.txt"] {
        fs::remove_file(root.join(file)).unwrap();
    }
    let snapshot = monitor.snapshot().unwrap();
    let json = parse_json(&snapshot.to_json()).unwrap();
    assert_eq!(json.get("completed").unwrap().as_int(), Some(2));
    assert!(!snapshot.provisional);
    assert_eq!(snapshot.rating_method, "batch_elo_v1");
    let rows = json.get("matches").unwrap().as_array().unwrap();
    assert_eq!(rows[0].get("state").unwrap().as_str(), Some("completed"));
    assert_eq!(
        json.get("entrants").unwrap().as_array().unwrap()[0]
            .get("version")
            .unwrap()
            .as_str(),
        Some("v1")
    );
    let bytes = monitor.replay(0, "0001").unwrap();
    let replay = arena_replay::replay::decode_log(&bytes).unwrap();
    assert_eq!(
        arena_replay::replay::replay(&replay)
            .unwrap()
            .rounds_completed,
        2
    );
    assert!(!root.join("index.tsv").exists());
    assert!(monitor.replay(0, "../../plan").is_err());
}

#[test]
fn incomplete_attempts_are_not_replays_and_committed_tampering_invalidates_cache() {
    let f = Fixture::new();
    let root = f.plan();
    let dir = root.join("matches/m0/attempts/0001");
    fs::create_dir_all(dir.join("server")).unwrap();
    fs::write(dir.join("state"), "running").unwrap();
    fs::write(dir.join("reason.txt"), "initial execution").unwrap();
    fs::write(dir.join("server/replay.iarp"), b"incomplete").unwrap();
    let mut monitor = ArchiveMonitor::open(&root).unwrap();
    let initial = parse_json(&monitor.snapshot().unwrap().to_json()).unwrap();
    let a = &initial.get("matches").unwrap().as_array().unwrap()[0]
        .get("attempts")
        .unwrap()
        .as_array()
        .unwrap()[0];
    assert_eq!(a.get("state").unwrap().as_str(), Some("recorded_running"));
    assert_eq!(
        a.get("attempt_reason").unwrap().as_str(),
        Some("initial execution")
    );
    assert!(monitor.replay(0, "0001").is_err());
    let o = Command::new(env!("CARGO_BIN_EXE_arena-runner"))
        .args(["resume", "--out"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(o.status.success());
    let completed = monitor.snapshot().unwrap();
    assert_eq!(completed.matches[0].attempts.len(), 2);
    assert_eq!(completed.matches[0].state(), "completed");
    assert_eq!(
        completed.standings[0].draws, 2,
        "attempts must not be scored twice"
    );
    assert!(monitor.replay(0, "0002").is_ok());
    fs::write(
        root.join("matches/m0/attempts/0002/server/replay.iarp"),
        b"broken",
    )
    .unwrap();
    let changed = monitor.snapshot().unwrap();
    assert_eq!(changed.matches[0].state(), "corrupt");
    assert_eq!(changed.standings[0].draws, 1);
    assert!(changed.provisional);
    assert_eq!(changed.rating_method, "online_elo_v1");
    assert!(monitor.replay(0, "0002").is_err());
    assert!(!root.join("matches/m0/attempts/0003").exists());
}

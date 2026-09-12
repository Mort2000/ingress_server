//! CLI validation must fail before launching clients or creating an archive.
use std::path::PathBuf;
use std::process::{Command, Output};

fn output_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("arena-cli-{tag}-{}", std::process::id()))
}

fn invoke(out: &std::path::Path, options: &[&str]) -> Output {
    let binary = env!("CARGO_BIN_EXE_arena-runner");
    Command::new(binary)
        .args(["tournament", "--out"])
        .arg(out)
        .args(["--client", binary, "--client", binary])
        .args(options)
        .output()
        .unwrap()
}

#[test]
fn invalid_schedule_creates_no_archive() {
    for (flag, value) in [
        ("--matches", "3"),
        ("--matches", "0"),
        ("--maps", "0"),
        ("--workers", "0"),
        ("--rounds", "0"),
    ] {
        let out = output_path(&format!("{flag}-{value}"));
        assert!(!out.exists());
        let result = invoke(&out, &[flag, value]);
        assert_eq!(
            result.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!out.exists());
    }
}

#[test]
fn existing_archive_is_never_overwritten() {
    let out = output_path("existing");
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("keep.txt"), "previous experiment").unwrap();
    let result = invoke(&out, &["--rounds", "1"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        std::fs::read_to_string(out.join("keep.txt")).unwrap(),
        "previous experiment"
    );
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn help_and_missing_clients_have_distinct_exit_codes() {
    let binary = env!("CARGO_BIN_EXE_arena-runner");
    let help = Command::new(binary).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--client"));
    assert!(String::from_utf8_lossy(&help.stdout).contains(&format!(
        "Rounds per match (default: {}).",
        arena_core::config::GameConfig::default().total_rounds
    )));
    let out = output_path("missing");
    let missing = Command::new(binary)
        .args(["tournament", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(!out.exists());
}

#[test]
fn plan_is_saved_before_execution_and_status_is_read_only() {
    let out = output_path("plan");
    let binary = env!("CARGO_BIN_EXE_arena-runner");
    let planned = Command::new(binary)
        .args(["plan", "--out"])
        .arg(&out)
        .args(["--client", binary, "--client", binary])
        .output()
        .unwrap();
    assert!(
        planned.status.success(),
        "{}",
        String::from_utf8_lossy(&planned.stderr)
    );
    assert!(out.join("plan/tournament.json").is_file());
    assert!(out.join("plan/config.bin").is_file());
    assert!(out.join("plan/maps/0.bin").is_file());
    let plan = arena_runner::tournament::durable::load(&out).unwrap();
    let default_rounds = arena_core::config::GameConfig::default().total_rounds;
    assert_eq!(plan.config.config.total_rounds, default_rounds);
    assert_eq!(plan.config.rounds_per_match, default_rounds);
    let status = Command::new(binary)
        .args(["status", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).contains("pending=2"));
    assert!(!out.join("matches").exists());
    std::fs::remove_dir_all(out).unwrap();
}

fn probe_binary() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("probe-client")
}

#[test]
fn completed_matches_are_reused_and_retry_cannot_select_a_game_result() {
    let out = output_path("resume");
    let binary = env!("CARGO_BIN_EXE_arena-runner");
    let probe = probe_binary();
    let result = Command::new(binary)
        .args(["tournament", "--out"])
        .arg(&out)
        .arg("--client")
        .arg(&probe)
        .arg("--client")
        .arg(&probe)
        .args([
            "--rounds",
            "2",
            "--fast",
            "--bot-version",
            "v1",
            "--bot-version",
            "v2",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("completed=2"));
    let original = std::fs::read(out.join("index.tsv")).unwrap();
    let replay = out.join("matches/m0/attempts/0001/server/replay.iarp");
    let log = arena_replay::replay::decode_log(&std::fs::read(replay).unwrap()).unwrap();
    let identity = log.header.identity.unwrap();
    assert!(
        identity
            .participants
            .iter()
            .all(|p| p.binding_verified && !p.binary_hash.is_empty())
    );
    let mut versions: Vec<_> = identity
        .participants
        .iter()
        .map(|p| p.registered_version.as_str())
        .collect();
    versions.sort();
    assert_eq!(versions, ["v1", "v2"]);
    for verb in ["resume", "rebuild"] {
        // Crash after the artifact commit point, before the state marker.
        std::fs::remove_file(out.join("matches/m0/attempts/0001/state")).unwrap();
        std::fs::write(out.join("matches/m1/attempts/0001/state"), "running").unwrap();
        std::fs::write(
            out.join("matches/m1/attempts/0001/storage-error.txt"),
            "simulated post-commit state write failure",
        )
        .unwrap();
        let result = Command::new(binary)
            .args([verb, "--out"])
            .arg(&out)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(std::fs::read(out.join("index.tsv")).unwrap(), original);
        assert_eq!(
            std::fs::read_dir(out.join("matches/m0/attempts"))
                .unwrap()
                .count(),
            1
        );
    }
    let retry = Command::new(binary)
        .args(["retry", "--out"])
        .arg(&out)
        .args(["--item", "0"])
        .output()
        .unwrap();
    assert!(!retry.status.success());
    let replay = out.join("matches/m0/attempts/0001/server/replay.iarp");
    let mut corrupt = std::fs::read(&replay).unwrap();
    corrupt[0] ^= 1;
    std::fs::write(&replay, corrupt).unwrap();
    let resume = Command::new(binary)
        .args(["resume", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        !resume.status.success(),
        "corrupt evidence must not be reused or silently rerun"
    );
    assert_eq!(
        std::fs::read_dir(out.join("matches/m0/attempts"))
            .unwrap()
            .count(),
        1
    );
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn partially_created_attempt_is_recoverable() {
    let out = output_path("partial-attempt");
    let binary = env!("CARGO_BIN_EXE_arena-runner");
    let probe = probe_binary();
    let planned = Command::new(binary)
        .args(["plan", "--out"])
        .arg(&out)
        .arg("--client")
        .arg(&probe)
        .arg("--client")
        .arg(&probe)
        .args(["--rounds", "1", "--fast"])
        .output()
        .unwrap();
    assert!(planned.status.success());
    std::fs::create_dir_all(out.join("matches/m0/attempts/0001")).unwrap();
    let status = Command::new(binary)
        .args(["status", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).contains("interrupted=1"));
    let resumed = Command::new(binary)
        .args(["resume", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert!(
        out.join("matches/m0/attempts/0002/server/replay.iarp")
            .exists()
    );
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn high_output_clients_do_not_block_and_truncation_is_explicit() {
    for sandboxed in [false, true] {
        let out = output_path(if sandboxed {
            "logs-sandbox"
        } else {
            "logs-direct"
        });
        let binary = env!("CARGO_BIN_EXE_arena-runner");
        let probe = probe_binary();
        let mut cmd = Command::new(binary);
        cmd.args(["tournament", "--out"])
            .arg(&out)
            .arg("--client")
            .arg(&probe)
            .arg("--client")
            .arg(&probe)
            .args(["--rounds", "1", "--fast"])
            .env("PROBE_LOG_BYTES", "2097152");
        if sandboxed {
            cmd.arg("--sandbox")
                .arg(probe.parent().unwrap().join("sandbox-runner"));
        }
        let result = cmd.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("completed=2"));
        let attempt = out.join("matches/m0/attempts/0001");
        if sandboxed {
            let report = std::fs::read_to_string(attempt.join("sandbox_a.json")).unwrap();
            assert!(report.contains("truncated"));
        } else {
            assert_eq!(
                std::fs::metadata(attempt.join("client_a.stdout"))
                    .unwrap()
                    .len(),
                1048576
            );
            let meta = std::fs::read_to_string(attempt.join("client_a.stdout.meta.json")).unwrap();
            assert!(meta.contains("\"truncated\":true"));
        }
        std::fs::remove_dir_all(out).unwrap();
    }
}

#[test]
fn same_named_batches_have_distinct_observation_resources() {
    let base = output_path("parallel");
    let outputs = [base.join("one/same"), base.join("two/same")];
    let probe = probe_binary();
    let binary = env!("CARGO_BIN_EXE_arena-runner");
    let children: Vec<_> = outputs
        .iter()
        .map(|out| {
            Command::new(binary)
                .args(["tournament", "--out"])
                .arg(out)
                .arg("--client")
                .arg(&probe)
                .arg("--client")
                .arg(&probe)
                .args(["--rounds", "4", "--fast"])
                .env("PROBE_OBSERVATION_LOG", "1")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("completed=2"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for root in &outputs {
        for index in 0..2 {
            let dir = root.join(format!("matches/m{index}/attempts/0001"));
            let id = std::fs::read_to_string(dir.join("execution-id")).unwrap();
            assert!(ids.insert(id.clone()));
            let log = arena_replay::replay::decode_log(
                &std::fs::read(dir.join("server/replay.iarp")).unwrap(),
            )
            .unwrap();
            assert_eq!(log.header.identity.as_ref().unwrap().execution_id, id);
            for side in ["a", "b"] {
                let text =
                    std::fs::read_to_string(dir.join(format!("client_{side}.stdout"))).unwrap();
                assert!(text.contains(&format!("match_id={id}")));
                assert!(text.contains(&format!("cwd=/tmp/ia-{id}")));
                assert!(text.contains(&format!("public=/ingress-{id}-public")));
                let entrant = log
                    .header
                    .identity
                    .as_ref()
                    .unwrap()
                    .participants
                    .iter()
                    .position(|p| p.registered_id == if side == "a" { "0" } else { "1" })
                    .unwrap();
                assert!(text.contains(if entrant == 0 {
                    "faction=Enl"
                } else {
                    "faction=Res"
                }));
                assert!(text.contains(&format!(
                    "private=/ingress-{id}-{}",
                    if entrant == 0 { "enl" } else { "res" }
                )));
            }
        }
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn interrupted_batch_reuses_completed_items_and_preserves_attempt_history() {
    use std::time::{Duration, Instant};
    let out = output_path("interrupt");
    let probe = probe_binary();
    let binary = env!("CARGO_BIN_EXE_arena-runner");
    let mut child = Command::new(binary)
        .args(["tournament", "--out"])
        .arg(&out)
        .arg("--client")
        .arg(&probe)
        .arg("--client")
        .arg(&probe)
        .args(["--matches", "4", "--rounds", "20", "--fast"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !(out.join("matches/m0/adopted").exists()
        && out.join("matches/m1/attempts/0001/state").exists())
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let first = std::fs::read(out.join("matches/m0/attempts/0001/artifacts")).unwrap();
    let competing = Command::new(binary)
        .args(["resume", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        !competing.status.success(),
        "a second supervisor must not own this archive"
    );
    child.kill().unwrap();
    child.wait().unwrap();
    let status = Command::new(binary)
        .args(["status", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&status.stdout).contains("interrupted=1"));
    let resumed = Command::new(binary)
        .args(["resume", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert!(String::from_utf8_lossy(&resumed.stdout).contains("completed=4"));
    assert_eq!(
        first,
        std::fs::read(out.join("matches/m0/attempts/0001/artifacts")).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(out.join("matches/m1/attempts/0001/state")).unwrap(),
        "interrupted"
    );
    assert!(
        out.join("matches/m1/attempts/0002/server/replay.iarp")
            .exists()
    );
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn infrastructure_retry_keeps_original_evidence_and_records_adoption_reason() {
    use std::os::unix::fs::PermissionsExt;
    let base = output_path("retry");
    std::fs::create_dir(&base).unwrap();
    let client = base.join("probe");
    std::fs::copy(probe_binary(), &client).unwrap();
    let out = base.join("run");
    let binary = env!("CARGO_BIN_EXE_arena-runner");
    let plan = Command::new(binary)
        .args(["plan", "--out"])
        .arg(&out)
        .arg("--client")
        .arg(&client)
        .arg("--client")
        .arg(&client)
        .args(["--rounds", "1", "--fast", "--attempt-limit", "2"])
        .output()
        .unwrap();
    assert!(plan.status.success());
    // An execution-environment failure without changing frozen binary bytes.
    std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o600)).unwrap();
    let first = Command::new(binary)
        .args(["resume", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(String::from_utf8_lossy(&first.stdout).contains("infrastructure_fault=2"));
    let original = std::fs::read(out.join("matches/m0/attempts/0001/artifacts")).unwrap();
    std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();
    let retry = Command::new(binary)
        .args(["retry", "--out"])
        .arg(&out)
        .args(["--item", "0"])
        .output()
        .unwrap();
    assert!(
        retry.status.success(),
        "{}",
        String::from_utf8_lossy(&retry.stderr)
    );
    assert_eq!(
        original,
        std::fs::read(out.join("matches/m0/attempts/0001/artifacts")).unwrap()
    );
    assert!(
        out.join("matches/m0/attempts/0002/server/replay.iarp")
            .is_file()
    );
    assert!(
        std::fs::read_to_string(out.join("adoptions.tsv"))
            .unwrap()
            .contains("explicit bounded infrastructure retry")
    );
    assert!(!out.join("matches/m1/attempts/0002").exists());
    std::fs::remove_dir_all(base).unwrap();
}

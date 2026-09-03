//! Resource-isolation ENFORCEMENT tests (P4-GATE, D74–D76): the
//! sb-fixture binaries run as children of the real sandbox-runner
//! binary, under real rlimits, landlock rules, and the wall watchdog.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Runs `sandbox-runner <flags> -- <bin> <args>` and parses the JSON
/// result plus the runner's exit code.
fn sandbox(flags: &[&str], bin: &str, args: &[&str]) -> (String, u32) {
    let runner = runner_bin();
    let mut cmd = Command::new(&runner);
    for flag in flags {
        cmd.arg(flag);
    }
    cmd.arg("--");
    cmd.arg(bin);
    for arg in args {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("sandbox-runner runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let code = output.status.code().unwrap_or(255);
    // The runner exits 2 for any non-Completed classification; the
    // JSON carries the real verdict.
    (stdout, code as u32)
}

fn runner_bin() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_sandbox-runner") {
        return PathBuf::from(path);
    }
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target");
    for profile in ["debug", "release"] {
        let candidate = target.join(profile).join("sandbox-runner");
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("sandbox-runner binary not built (run cargo build --workspace)");
}

fn fixture_bin(name: &str) -> String {
    // This package's own bins are directly addressable.
    let var = format!("CARGO_BIN_EXE_{name}");
    std::env::var(&var).unwrap_or_else(|_| panic!("{var} not set"))
}

/// Extracts and un-escapes the `stdout_tail` field of a runner JSON
/// result (the child's captured stdout).
fn stdout_tail(runner_json: &str) -> String {
    let start = runner_json
        .find("\"stdout_tail\": \"")
        .expect("stdout_tail present")
        + "\"stdout_tail\": \"".len();
    let end = runner_json[start..]
        .find("\",\n")
        .or_else(|| runner_json[start..].find("\"\n}"))
        .map(|offset| start + offset)
        .expect("closing quote");
    runner_json[start..end]
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m8-sbfix")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// RLIMIT_AS enforced: the malloc bomb dies with an allocation-failure
/// abort under a 64 MiB cap, classified mem_limit, never exit 42 (the
/// "limit never enforced" marker), and never hangs.
#[test]
fn address_space_limit_aborts_the_malloc_bomb() {
    let (json, _) = sandbox(
        &["--as", "64MiB", "--wall-ms", "8000"],
        &fixture_bin("sb-malloc-bomb"),
        &[],
    );
    assert!(json.contains("\"classification\": \"mem_limit\""), "{json}");
    assert!(!json.contains("exit_code\": 42"), "{json}");
}

/// The DEFAULT 2 GiB address space already enforces the spec's memory
/// row (P4 table) without any opt-in. Wall budget 30 s (not 8): under
/// a 2 GiB cap the abort takes ~5–8 s on this host — glibc's malloc
/// retry path once mmap starts failing near the limit (measured; the
/// 64 MiB variant above aborts in ~0.2 s, unsandboxed 8 GiB VA-only
/// growth takes 33 ms) — so an 8 s budget raced the watchdog under
/// parallel-suite load and misclassified as wall_timeout. The
/// assertion itself is unchanged: mem_limit vs exit 42 is the signal.
#[test]
fn default_two_gib_limit_enforced() {
    let (json, _) = sandbox(&["--wall-ms", "30000"], &fixture_bin("sb-malloc-bomb"), &[]);
    assert!(
        json.contains("\"classification\": \"mem_limit\""),
        "default AS cap bites: {json}"
    );
}

/// RLIMIT_CPU enforced: the spin loop dies at ~1 s by SIGXCPU,
/// classified cpu_limit.
#[test]
fn cpu_limit_kills_the_spin_loop() {
    let started = std::time::Instant::now();
    let (json, _) = sandbox(
        &["--cpu", "1", "--wall-ms", "8000"],
        &fixture_bin("sb-spin"),
        &[],
    );
    let elapsed = started.elapsed();
    assert!(json.contains("\"classification\": \"cpu_limit\""), "{json}");
    assert!(
        elapsed < Duration::from_secs(5),
        "killed at the 1 s CPU cap: {elapsed:?}"
    );
}

/// Wall-clock watchdog: with the CPU cap wide, the watchdog SIGKILLs
/// at 400 ms, classified wall_timeout.
#[test]
fn wall_clock_watchdog_sigkills() {
    let started = std::time::Instant::now();
    let (json, _) = sandbox(
        &["--cpu", "30", "--wall-ms", "400"],
        &fixture_bin("sb-spin"),
        &[],
    );
    let elapsed = started.elapsed();
    assert!(
        json.contains("\"classification\": \"wall_timeout\""),
        "{json}"
    );
    assert!(elapsed < Duration::from_secs(4), "{elapsed:?}");
}

/// Landlock fs policy (ABI 7 live): reads under RO roots succeed,
/// writes there are denied, writes in the scratch dir succeed, and
/// NoNewPrivs is set in the child (D74).
#[test]
fn landlock_fs_and_no_new_privs_enforced() {
    let dir = scratch("fs");
    let (json, _) = sandbox(
        &["--rw", dir.to_str().unwrap(), "--wall-ms", "5000"],
        &fixture_bin("sb-fs-probe"),
        &["/usr", "/etc/hostname"],
    );
    assert!(json.contains("RO_READ=ok"), "{json}");
    assert!(json.contains("RO_WRITE=denied"), "{json}");
    assert!(json.contains("RW_WRITE=ok"), "{json}");
    assert!(json.contains("NOPRIVS=1"), "prctl visible: {json}");
}

/// Landlock TCP deny (ABI >= 4): connecting to a LIVE local listener
/// is refused by the kernel inside the sandbox; the same fixture
/// connects fine unsandboxed (control).
#[test]
fn landlock_denies_tcp_connect_to_a_live_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let target = format!("127.0.0.1:{port}");

    // Control: unsandboxed connect succeeds.
    let control = Command::new(fixture_bin("sb-net-probe"))
        .arg(&target)
        .output()
        .expect("control runs");
    assert!(
        String::from_utf8_lossy(&control.stdout).contains("CONNECT=ok"),
        "control must connect (listener is live)"
    );

    // Sandboxed: denied by landlock.
    let (json, _) = sandbox(
        &["--wall-ms", "5000"],
        &fixture_bin("sb-net-probe"),
        &[&target],
    );
    assert!(
        json.contains("CONNECT=denied kind=PermissionDenied"),
        "kernel-level denial: {json}"
    );
}

/// Environment scrub + cwd: the child sees only PATH, HOME (the
/// scratch dir), and allowlisted PROBE_* variables, and starts in the
/// scratch dir. Uses the fork-free sb-env-report fixture — fork
/// behavior under RLIMIT_NPROC is ambient-load-dependent (uid-wide)
/// and must not gate this test.
#[test]
fn environment_scrubbed_and_cwd_is_the_scratch_dir() {
    let dir = scratch("env");
    let output = Command::new(runner_bin())
        .arg("--rw")
        .arg(&dir)
        .arg("--wall-ms")
        .arg("5000")
        .arg("--env-prefix")
        .arg("PROBE_")
        .arg("--")
        .arg(fixture_bin("sb-env-report"))
        .env("PROBE_TOKEN", "42")
        .env("SECRET_SHOULD_VANISH", "leak")
        .output()
        .expect("run");
    let json = String::from_utf8_lossy(&output.stdout).into_owned();
    let stdout = stdout_tail(&json);
    let first_line = stdout.lines().next().unwrap_or_default();
    assert!(
        Path::new(first_line).canonicalize().ok().as_deref() == dir.canonicalize().ok().as_deref(),
        "cwd is the scratch dir: {first_line} vs {}",
        dir.display()
    );
    assert!(stdout.contains("PROBE_TOKEN=42"), "{stdout}");
    assert!(!stdout.contains("SECRET_SHOULD_VANISH"), "{stdout}");
    assert!(!stdout.contains("CARGO_MANIFEST_DIR"), "{stdout}");
    let home = stdout.lines().find(|l| l.starts_with("HOME=")).unwrap();
    assert!(
        Path::new(&home[5..]).canonicalize().ok().as_deref() == dir.canonicalize().ok().as_deref(),
        "HOME is the scratch dir"
    );
    let keys: Vec<&str> = stdout
        .lines()
        .skip(1)
        .map(|l| l.split('=').next().unwrap_or(""))
        .collect();
    assert_eq!(
        keys,
        vec!["HOME", "PATH", "PROBE_TOKEN"],
        "exactly these keys"
    );
}

/// The runner survives children that die in every classified way and
/// always produces exactly one JSON object (supervision robustness).
#[test]
fn runner_reports_one_json_object_per_run() {
    for (flags, bin, args) in [
        (
            vec!["--as", "64MiB", "--wall-ms", "6000"],
            "sb-malloc-bomb",
            vec![""],
        ),
        (vec!["--wall-ms", "400", "--cpu", "30"], "sb-spin", vec![""]),
    ] {
        let args: Vec<&str> = if args[0].is_empty() { vec![] } else { args };
        let (json, _) = sandbox(&flags, &fixture_bin(bin), &args);
        assert_eq!(json.matches('{').count(), 1, "exactly one object: {json}");
        assert!(json.contains("\"duration_ms\""), "{json}");
    }
}

/// Writes inside the scratch dir stay legal under a 1 MiB fsize cap
/// (the probe writes one small file) — the fsize row of the limit
/// matrix, fork-free.
#[test]
fn scratch_writes_fit_under_the_file_size_cap() {
    let dir = scratch("fsize");
    let (json, _) = sandbox(
        &[
            "--rw",
            dir.to_str().unwrap(),
            "--fsize",
            "1MiB",
            "--wall-ms",
            "5000",
        ],
        &fixture_bin("sb-fs-probe"),
        &["/usr", "/etc/hostname"],
    );
    assert!(json.contains("RW_WRITE=ok"), "{json}");
}

/// REVIEW FIX (M8 review, findings 3+4): the runner CLI used to
/// hard-default `--nproc` to 64 (the frozen D74 default is 256;
/// `/bin/sh: Cannot fork` under the default) and the post-wait output
/// drain was an unbounded blocking read — a grandchild inheriting
/// stdout (plain safe Rust `Command`, nothing the audit chain forbids)
/// wedged the runner for the orphan's full lifetime: measured live,
/// `--wall-ms 1500 -- sh -c 'sleep 20 & exec /bin/true'` reported
/// `completed(0)` with `duration_ms` 11 while the runner returned
/// after 20.004 s; `sleep infinity` hung forever. Now the child runs
/// in its own process group, the group is SIGKILLed (watchdog at the
/// deadline, containment after the reap), and the drain is
/// deadline-bounded.
#[test]
fn orphaned_grandchild_cannot_wedge_the_runner() {
    let started = std::time::Instant::now();
    let (json, code) = sandbox(
        &["--wall-ms", "1500"],
        "/bin/sh",
        &["-c", "sleep 20 & exec /bin/true"],
    );
    let elapsed = started.elapsed();
    assert_eq!(code, 0, "runner exits 0 on completed(0): {json}");
    assert!(
        json.contains("\"classification\": \"completed(0)\""),
        "{json}"
    );
    assert!(
        !json.contains("Cannot fork"),
        "the frozen D74 default nproc (256) applies to CLI runs too: {json}"
    );
    assert!(
        elapsed < Duration::from_secs(8),
        "the runner must not wait for the orphaned grandchild's 20 s \
         lifetime (pre-fix: 20.0 s; the watchdog+containment kill the \
         group at ~1.5 s): {elapsed:?}"
    );
    // duration_ms reports the real supervision wall time (pre-fix it
    // measured only the direct child's lifetime: 11).
    let duration_ms: u128 = json
        .lines()
        .find_map(|l| l.trim().strip_prefix("\"duration_ms\": "))
        .and_then(|v| v.trim_end_matches(',').parse().ok())
        .expect("duration_ms parses");
    assert!(
        duration_ms < 8_000,
        "duration_ms reflects the supervised run, not just the child: {json}"
    );
}

/// Fixture self-check: each fixture binary is `#![forbid(unsafe_code)]`
/// (they model audited participants; P4).
#[test]
fn fixtures_are_safe_rust() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin");
    for name in [
        "sb-malloc-bomb.rs",
        "sb-spin.rs",
        "sb-net-probe.rs",
        "sb-fs-probe.rs",
        "sb-env-report.rs",
    ] {
        let text = std::fs::read_to_string(dir.join(name)).expect(name);
        assert!(text.contains("#![forbid(unsafe_code)]"), "{name}");
    }
}

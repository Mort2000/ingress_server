//! sandbox-runner library tests (D74–D76): the cgroup fake-root
//! writer, degradation recording, JSON shape, spawn errors, and the
//! unsafe-policy source shape (exactly one sanctioned module).
//! Child-process ENFORCEMENT tests live in tools/sb-fixtures/tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sandbox_runner::{Classification, SandboxConfig, cgroup, run_command};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m8-runner")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

/// The cgroup writer is fully tested against a FAKE root (D75): the
/// scope directory appears with the three limit files, the pid is
/// written into cgroup.procs, and Drop removes the scope.
#[test]
fn cgroup_writer_against_a_fake_root() {
    let root = scratch("cgroup-fake");
    let scope = cgroup::create_scope(
        &root,
        "ingress-sandbox-test",
        "200000 100000",
        "2147483648",
        64,
    )
    .expect("scope on fake root");
    let dir = root.join("ingress-sandbox-test");
    assert_eq!(
        fs::read_to_string(dir.join("cpu.max")).unwrap(),
        "200000 100000"
    );
    assert_eq!(
        fs::read_to_string(dir.join("memory.max")).unwrap(),
        "2147483648"
    );
    assert_eq!(fs::read_to_string(dir.join("pids.max")).unwrap(), "64");
    cgroup::attach(&scope, 4242).expect("attach");
    assert_eq!(
        fs::read_to_string(dir.join("cgroup.procs")).unwrap(),
        "4242"
    );
    drop(scope);
    assert!(!dir.exists(), "scope removed on drop");
}

/// The real (non-delegated) cgroup root records EACCES as a
/// degradation and the run continues (never fatal; D75).
#[test]
fn real_cgroup_root_eacces_is_a_recorded_degradation() {
    let dir = scratch("cgroup-real");
    let mut config = SandboxConfig::new(Duration::from_millis(5000), &dir);
    config.cgroup_root = Some(PathBuf::from("/sys/fs/cgroup"));
    let result = run_command(&config, Path::new("/bin/true"), &[]);
    assert_eq!(
        result.classification,
        Classification::Completed(0),
        "{}",
        result.json()
    );
    assert!(
        result
            .degraded
            .iter()
            .any(|d| d.contains("cgroup not enforced")),
        "the EACCES fact is recorded: {}",
        result.json()
    );
}

/// Disabling the cgroup layer produces no cgroup degradation.
#[test]
fn cgroup_layer_can_be_disabled() {
    let dir = scratch("cgroup-off");
    let mut config = SandboxConfig::new(Duration::from_millis(5000), &dir);
    config.cgroup_root = None;
    let result = run_command(&config, Path::new("/bin/true"), &[]);
    assert_eq!(result.classification, Classification::Completed(0));
    assert!(
        !result.degraded.iter().any(|d| d.contains("cgroup")),
        "{:?}",
        result.degraded
    );
}

/// A missing program is a Spawn classification with detail, not a
/// panic; the JSON carries the message.
#[test]
fn missing_program_is_a_spawn_error() {
    let dir = scratch("spawn-err");
    let config = SandboxConfig::new(Duration::from_millis(1000), &dir);
    let result = run_command(&config, Path::new("/nonexistent/definitely-missing"), &[]);
    assert!(matches!(result.classification, Classification::Spawn(_)));
    let json = result.json();
    assert!(json.contains("spawn_error"), "{json}");
    assert!(json.contains("nonexistent"), "{json}");
}

/// A plain child completes with its exit code and the landlock ABI of
/// this host is recorded (D76: the probe result travels with the
/// result JSON).
#[test]
fn completed_child_records_exit_code_and_abi() {
    let dir = scratch("completed");
    let config = SandboxConfig::new(Duration::from_millis(5000), &dir);
    let result = run_command(
        &config,
        Path::new("/bin/sh"),
        &["-c".into(), "exit 7".into()],
    );
    assert_eq!(result.classification, Classification::Completed(7));
    assert!(result.landlock_abi.is_some(), "landlock live on this host");
    let abi = result.landlock_abi.unwrap();
    if abi < 4 {
        assert!(
            result
                .degraded
                .iter()
                .any(|d| d.contains("TCP deny not enforceable")),
            "pre-ABI-4 kernels record the net degradation"
        );
    }
}

/// Classification labels and JSON key order stay stable (the
/// arena-server tests parse this output).
#[test]
fn classification_labels_and_json_shape() {
    assert_eq!(Classification::Completed(0).label(), "completed(0)");
    assert_eq!(Classification::MemLimit.label(), "mem_limit");
    assert_eq!(Classification::CpuLimit.label(), "cpu_limit");
    assert_eq!(Classification::WallTimeout.label(), "wall_timeout");
    assert_eq!(Classification::Signal(15).label(), "signal(15)");
    let result = sandbox_runner::SandboxResult {
        classification: Classification::Completed(3),
        duration_ms: 12,
        degraded: vec!["x".into()],
        landlock_abi: Some(7),
        stdout_tail: "out".into(),
        stderr_tail: String::new(),
    };
    let json = result.json();
    let c_at = json.find("\"classification\"").unwrap();
    let exit_at = json.find("\"exit_code\"").unwrap();
    let dur_at = json.find("\"duration_ms\"").unwrap();
    let abi_at = json.find("\"landlock_abi\"").unwrap();
    let deg_at = json.find("\"degraded\"").unwrap();
    let out_at = json.find("\"stdout_tail\"").unwrap();
    assert!(
        c_at < exit_at && exit_at < dur_at && dur_at < abi_at && abi_at < deg_at && deg_at < out_at
    );
}

/// Default config is the frozen D74 limit set.
#[test]
fn default_limits_are_the_frozen_set() {
    let dir = scratch("defaults");
    let config = SandboxConfig::new(Duration::from_secs(10), &dir);
    assert_eq!(config.address_space, 2 << 30, "2 GiB");
    assert_eq!(config.cpu_secs, 20, "2x wall");
    assert_eq!(
        config.nproc, 256,
        "uid-wide NPROC needs headroom (see D74 note)"
    );
    assert_eq!(config.fsize, 64 << 20);
    assert_eq!(config.nofile, 256);
    assert_eq!(config.core, 0);
    assert_eq!(config.net, sandbox_runner::NetPolicy::DenyTcp);
    assert!(config.ro_roots.contains(&PathBuf::from("/usr")));
    assert!(config.ro_roots.contains(&PathBuf::from("/dev/shm")));
}

/// REVIEW FIX (M8 review, finding 3): the CLI used to hard-default
/// `nproc = 64` and unconditionally overwrite the config, so every
/// real binary invocation ran with 64 — contradicting the frozen D74
/// default (256, uid-wide) and docs/design/sandbox.md §3, and breaking even
/// the runbook's own `sh -c '... & ...'` fork on this host (measured:
/// `/bin/sh: Cannot fork`). The constructor default must be the only
/// default; `--nproc` only overrides when passed.
#[test]
fn cli_has_no_own_nproc_default() {
    let main_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"))
        .expect("main.rs");
    assert!(
        !main_rs.contains("nproc = 64"),
        "no CLI-side nproc default; the frozen 256 lives in SandboxConfig::new:\n{main_rs}"
    );
    assert!(
        main_rs.contains("nproc: Option<u64>") && main_rs.contains("if let Some(nproc) = nproc"),
        "the flag overrides only when passed:\n{main_rs}"
    );
}

/// REVIEW FIX (M8 review, finding 6): D76 and docs/design/sandbox.md §5
/// record the landlock handled set as growing REFER 2 / TRUNCATE 3 /
/// IOCTL_DEV 5, but `handled_fs` never included IOCTL_DEV — on this
/// ABI-7 host device ioctls were unhandled (⇒ permitted) while the
/// docs presented them as kernel-enforced. Pinned from both sides:
/// the bit appears exactly at ABI 5 and is granted NOWHERE
/// (handled-not-granted ⇒ denied).
#[test]
fn handled_fs_grows_refer_2_truncate_3_ioctl_dev_5() {
    use sandbox_runner::sys;
    assert!(sys::handled_fs(1) & sys::ACCESS_REFER == 0);
    assert!(sys::handled_fs(2) & sys::ACCESS_REFER != 0);
    assert!(sys::handled_fs(2) & sys::ACCESS_TRUNCATE == 0);
    assert!(sys::handled_fs(3) & sys::ACCESS_TRUNCATE != 0);
    assert!(sys::handled_fs(4) & sys::ACCESS_IOCTL_DEV == 0);
    assert!(sys::handled_fs(5) & sys::ACCESS_IOCTL_DEV != 0);
    assert_eq!(sys::handled_fs(7), sys::handled_fs(5));
    assert_eq!(sys::ACCESS_IOCTL_DEV, 1 << 15);
    // Handled-not-granted: neither access set includes the right.
    assert!(sys::RO_ACCESS & sys::ACCESS_IOCTL_DEV == 0);
    assert!(sys::RW_ACCESS & sys::ACCESS_IOCTL_DEV == 0);
}

/// Unsafe policy source shape (P4-CAPABILITY): the crate root denies
/// unsafe code and exactly ONE module carries the scoped allow — the
/// documented syscall module. This is the CI pin for "the runner is
/// the only new unsafe-bearing code".
#[test]
fn exactly_one_sanctioned_unsafe_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let lib = fs::read_to_string(src.join("lib.rs")).expect("lib.rs");
    assert!(lib.contains("#![deny(unsafe_code)]"));
    let sys = fs::read_to_string(src.join("sys.rs")).expect("sys.rs");
    let allow_count =
        lib.matches("#[allow(unsafe_code)]").count() + sys.matches("#[allow(unsafe_code)]").count();
    assert_eq!(
        allow_count, 1,
        "exactly one scoped allow: the sys module declaration in lib.rs"
    );
    assert!(lib.contains("#[allow(unsafe_code)]\npub mod sys;"));
}

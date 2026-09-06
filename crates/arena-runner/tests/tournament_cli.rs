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
    let out = output_path("missing");
    let missing = Command::new(binary)
        .args(["tournament", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(!out.exists());
}

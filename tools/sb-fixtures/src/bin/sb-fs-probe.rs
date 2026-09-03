//! sb-fs-probe <ro_dir> <ro_file> : probes the landlock filesystem
//! policy from inside the sandbox (cwd = the rw scratch dir).
//! Reports: RO_READ, RO_WRITE, RW_WRITE, NOPRIVS. Exits 0 always —
//! callers assert the markers.

#![forbid(unsafe_code)]

use std::io::Write as _;

fn main() {
    let mut args = std::env::args().skip(1);
    let ro_dir = args.next().unwrap_or_else(|| "/usr".into());
    let ro_file = args.next().unwrap_or_else(|| "/etc/hostname".into());

    // Read from a read-only root: allowed.
    let ro_read = std::fs::read_to_string(&ro_file).is_ok();
    println!("RO_READ={}", if ro_read { "ok" } else { "denied" });

    // Write inside a read-only root: must be denied.
    let target = format!("{}/sb-fs-probe-write-{}", ro_dir, std::process::id());
    let ro_write = std::fs::File::create(&target)
        .and_then(|mut f| f.write_all(b"x"))
        .is_ok();
    let _ = std::fs::remove_file(&target);
    println!("RO_WRITE={}", if ro_write { "granted" } else { "denied" });

    // Write into the scratch dir (cwd): must succeed.
    let rw_path = format!("sb-fs-probe-{}.txt", std::process::id());
    let rw_write = std::fs::File::create(&rw_path)
        .and_then(|mut f| f.write_all(b"x"))
        .is_ok();
    let _ = std::fs::remove_file(&rw_path);
    println!("RW_WRITE={}", if rw_write { "ok" } else { "failed" });

    // PR_SET_NO_NEW_PRIVS visibility: /proc/self/status carries it.
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let no_privs = status
        .lines()
        .find(|l| l.starts_with("NoNewPrivs:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("?")
        .to_string();
    println!("NOPRIVS={no_privs}");
}

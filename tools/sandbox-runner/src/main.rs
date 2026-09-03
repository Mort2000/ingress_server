//! sandbox-runner CLI: run one command under the resource sandbox and
//! print the JSON result (D74–D76).
//!
//! ```text
//! sandbox-runner [--as BYTES] [--cpu SECS] [--nproc N] [--fsize BYTES]
//!                [--nofile N] [--wall-ms MS] [--ro PATH]... [--rw PATH]
//!                [--env-prefix P]... [--no-landlock] [--no-cgroup]
//!                -- <program> [args...]
//! sandbox-runner --kill-group PGID     # SIGKILL one process group, exit
//! ```
//!
//! `--kill-group` is the containment helper for supervisors (M9): the
//! tournament runner must be able to kill a sandboxed client's process
//! GROUP after SIGKILLing this runner (the group's own watchdog dies
//! with it), and `arena-runner` is `#![forbid(unsafe_code)]` — the
//! sanctioned `kill(2)` lives HERE, in `sys` (D74).

use std::path::PathBuf;
use std::time::Duration;

fn parse_bytes(text: &str) -> Option<u64> {
    let text = text.trim();
    let (number, unit) = text.split_at(
        text.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len()),
    );
    let value: u64 = number.parse().ok()?;
    match unit.trim() {
        "" | "B" => Some(value),
        "KiB" | "K" | "k" => Some(value << 10),
        "MiB" | "M" => Some(value << 20),
        "GiB" | "G" => Some(value << 30),
        _ => None,
    }
}

fn main() {
    let mut rw_dir = std::env::temp_dir().join(format!("ingress-sandbox-{}", std::process::id()));
    let mut wall_ms = 10_000u64;
    let mut address_space = 2 << 30;
    let mut cpu_secs: Option<u64> = None;
    // No CLI-side default: the frozen D74 set (nproc = 256, uid-wide;
    // docs/sandbox.md §3) lives in `SandboxConfig::new` — a CLI default
    // of 64 silently overrode it and broke even an `sh` pipeline fork
    // on this host (REVIEW FIX, M8 review finding 3).
    let mut nproc: Option<u64> = None;
    let mut fsize = 64 << 20;
    let mut nofile = 256u64;
    let mut ro_roots: Vec<PathBuf> = Vec::new();
    let mut env_prefixes: Vec<String> = Vec::new();
    let mut landlock = true;
    let mut cgroup = true;
    let mut command: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match flag.as_str() {
            "--kill-group" => {
                // Containment helper (M9, D74): SIGKILL one process group
                // (the sandboxed client's — pgid == its pid because the
                // supervised child is spawned with process_group(0)) and
                // exit. Nothing to supervise, no JSON, exit 0 whether or
                // not the group still existed (ESRCH is success).
                let pgid: u32 = match value().parse() {
                    Ok(pgid) if pgid > 0 => pgid,
                    _ => {
                        eprintln!("--kill-group needs a positive pgid");
                        std::process::exit(1);
                    }
                };
                sandbox_runner::sys::kill_process_group(pgid, 9);
                return;
            }
            "--as" => address_space = parse_bytes(&value()).unwrap_or(address_space),
            "--cpu" => cpu_secs = value().parse().ok(),
            "--nproc" => nproc = value().parse().ok(),
            "--fsize" => fsize = parse_bytes(&value()).unwrap_or(fsize),
            "--nofile" => nofile = value().parse().unwrap_or(nofile),
            "--wall-ms" => wall_ms = value().parse().unwrap_or(wall_ms),
            "--ro" => ro_roots.push(PathBuf::from(value())),
            "--rw" => rw_dir = PathBuf::from(value()),
            "--env-prefix" => env_prefixes.push(value()),
            "--no-landlock" => landlock = false,
            "--no-cgroup" => cgroup = false,
            "--" => {
                command = args.collect();
                break;
            }
            other => {
                eprintln!("unknown flag {other:?}");
                std::process::exit(1);
            }
        }
    }
    if command.is_empty() {
        eprintln!("usage: sandbox-runner [flags] -- <program> [args...]");
        std::process::exit(1);
    }

    let mut config = sandbox_runner::SandboxConfig::new(Duration::from_millis(wall_ms), &rw_dir);
    config.address_space = address_space;
    if let Some(cpu) = cpu_secs {
        config.cpu_secs = cpu;
    }
    if let Some(nproc) = nproc {
        config.nproc = nproc;
    }
    config.fsize = fsize;
    config.nofile = nofile;
    if !ro_roots.is_empty() {
        config.ro_roots.extend(ro_roots);
    }
    if !env_prefixes.is_empty() {
        config.env_prefixes = env_prefixes;
    }
    config.landlock = landlock;
    if !cgroup {
        config.cgroup_root = None;
    }

    let program = PathBuf::from(&command[0]);
    let rest: Vec<String> = command[1..].to_vec();
    let result = sandbox_runner::run_command(&config, &program, &rest);
    print!("{}", result.json());
    let ok = matches!(
        result.classification,
        sandbox_runner::Classification::Completed(_)
    );
    std::process::exit(if ok { 0 } else { 2 });
}

//! Resource isolation for audited client processes (P4, D74–D76).
//!
//! The runner spawns one child with, in the frozen order (D74):
//! rlimits (AS/CPU/NPROC/FSIZE/NOFILE/CORE) → cwd + scrubbed
//! environment (on the `Command`) → `PR_SET_NO_NEW_PRIVS` → landlock
//! path rules (read+exec on RO roots, read+write on the scratch dir,
//! TCP bind/connect denied when ABI ≥ 4) → `restrict_self` → `exec`.
//! The child runs in its OWN process group; a wall-clock watchdog
//! SIGKILLs the whole GROUP at the deadline, surviving group members
//! are killed again after the direct child is reaped (containment),
//! and the output drain is deadline-bounded — a client's timeout or
//! crash must never wedge the runner (P4-GATE). The result (exit
//! classification + any degradations) is reported as JSON.
//!
//! cgroup v2 enforcement is probed, never assumed (D75): this host's
//! cgroup root is not delegated (`EACCES`), which is RECORDED as a
//! degradation and the run continues under rlimits + landlock; the
//! writer logic itself is tested against a fake root.
//!
//! UDP is not landlock-coverable (D76): it is denied by the AUDIT
//! chain (R4 net-absence + `forbid(unsafe_code)`), not by this runner.

#![deny(unsafe_code)]
// The sole sanctioned unsafe module in this crate (see above): raw
// Linux syscalls via extern "C", no libc crate.
#[allow(unsafe_code)]
pub mod sys;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Network policy (D76): landlock (ABI ≥ 4) can deny TCP bind/connect;
/// UDP is audit-enforced, never kernel-enforced here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetPolicy {
    DenyTcp,
    /// No landlock net layer (kernel without ABI 4, or operator opt-out).
    Inherit,
}

/// Frozen default limit set (D74): 2 GiB address space, CPU = 2× the
/// wall allowance, 64 processes, 64 MiB max file, 256 fds, no core.
#[derive(Clone, Debug)]
pub struct SandboxConfig {
    pub address_space: u64,
    pub cpu_secs: u64,
    pub nproc: u64,
    pub fsize: u64,
    pub nofile: u64,
    pub core: u64,
    pub wall_timeout: Duration,
    /// Read-only roots (read + exec). The child binary's own directory
    /// is always added.
    pub ro_roots: Vec<PathBuf>,
    /// The one writable directory; also the child's working directory.
    pub rw_dir: PathBuf,
    /// Environment variable prefixes carried across the scrub.
    pub env_prefixes: Vec<String>,
    pub net: NetPolicy,
    /// Disable landlock entirely (degradation testing).
    pub landlock: bool,
    /// cgroup v2 root to probe (default `/sys/fs/cgroup`); `None`
    /// disables the cgroup layer.
    pub cgroup_root: Option<PathBuf>,
}

impl SandboxConfig {
    /// Defaults per D74 with a given wall allowance and scratch dir;
    /// `cpu_secs = 2 × wall`. NOTE: `nproc` defaults to 256, not the
    /// plan's 64 — `RLIMIT_NPROC` is UID-WIDE, and this host's ambient
    /// per-uid count already sits near 64, which made even an `sh`
    /// pipeline's fork fail under a 64 cap (measured live); 256 stays
    /// a bounded cap with headroom. Documented in docs/design/sandbox.md.
    pub fn new(wall_timeout: Duration, rw_dir: impl Into<PathBuf>) -> Self {
        let rw_dir = rw_dir.into();
        let mut ro_roots = default_ro_roots();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                ro_roots.push(dir.to_path_buf());
            }
        }
        SandboxConfig {
            address_space: 2 << 30,
            cpu_secs: wall_timeout.as_secs().max(1) * 2,
            nproc: 256,
            fsize: 64 << 20,
            nofile: 256,
            core: 0,
            wall_timeout,
            ro_roots,
            rw_dir,
            env_prefixes: vec!["ARENA_".into(), "PROBE_".into()],
            net: NetPolicy::DenyTcp,
            landlock: true,
            cgroup_root: Some(PathBuf::from("/sys/fs/cgroup")),
        }
    }
}

/// Read-only roots every child needs: the dynamic loader and libc
/// under /usr + /lib*, config and /proc for std, and the SHM mount for
/// observation regions (clients open SHM read-only).
pub fn default_ro_roots() -> Vec<PathBuf> {
    [
        "/usr", "/lib", "/lib64", "/etc", "/proc", "/dev", "/dev/shm",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Classification {
    Completed(i32),
    /// Allocation-failure abort under `RLIMIT_AS` (SIGABRT/SIGSEGV/
    /// SIGBUS) — the documented heuristic.
    MemLimit,
    /// `RLIMIT_CPU` expiry (SIGXCPU, or the hard-limit SIGKILL).
    CpuLimit,
    /// The wall-clock watchdog SIGKILLed the child.
    WallTimeout,
    /// Some other fatal signal.
    Signal(i32),
    Spawn(String),
}

impl Classification {
    pub fn label(&self) -> String {
        match self {
            Classification::Completed(code) => format!("completed({code})"),
            Classification::MemLimit => "mem_limit".to_string(),
            Classification::CpuLimit => "cpu_limit".to_string(),
            Classification::WallTimeout => "wall_timeout".to_string(),
            Classification::Signal(sig) => format!("signal({sig})"),
            Classification::Spawn(_) => "spawn_error".to_string(),
        }
    }
}

/// Cap on captured child output (fixtures print a few lines; a child
/// flooding stdout would otherwise wedge the pipe before the watchdog
/// fires — the cap keeps our post-wait read bounded).
const OUTPUT_CAP: usize = 64 * 1024;

/// Grace added to the wall deadline for the post-wait output drain:
/// EOF arrives once every pipe holder is dead (the group SIGKILL), and
/// this caps the wait even if a descendant that `setsid`-escaped the
/// group survives with an inherited pipe (P4-GATE: a client's timeout
/// or crash must not wedge the runner).
const DRAIN_GRACE: Duration = Duration::from_millis(1000);

#[derive(Clone, Debug)]
pub struct SandboxResult {
    pub classification: Classification,
    pub duration_ms: u128,
    pub degraded: Vec<String>,
    pub landlock_abi: Option<u32>,
    pub stdout_tail: String,
    pub stderr_tail: String,
}

impl SandboxResult {
    /// Deterministic JSON body (hand-rolled, no serde).
    pub fn json(&self) -> String {
        let mut out = String::from("{\n");
        out.push_str(&format!(
            "  \"classification\": {},\n",
            json_str(&self.classification.label())
        ));
        if let Classification::Completed(code) = self.classification {
            out.push_str(&format!("  \"exit_code\": {code},\n"));
        }
        if let Classification::Signal(sig) = self.classification {
            out.push_str(&format!("  \"signal\": {sig},\n"));
        }
        if let Classification::Spawn(detail) = &self.classification {
            out.push_str(&format!("  \"spawn_error\": {},\n", json_str(detail)));
        }
        out.push_str(&format!("  \"duration_ms\": {},\n", self.duration_ms));
        out.push_str(&format!(
            "  \"landlock_abi\": {},\n",
            self.landlock_abi
                .map(|a| a.to_string())
                .unwrap_or_else(|| "null".into())
        ));
        out.push_str("  \"degraded\": [");
        let items: Vec<String> = self.degraded.iter().map(|d| json_str(d)).collect();
        out.push_str(&items.join(", "));
        out.push_str("],\n");
        out.push_str(&format!(
            "  \"stdout_tail\": {},\n",
            json_str(&self.stdout_tail)
        ));
        out.push_str(&format!(
            "  \"stderr_tail\": {}\n",
            json_str(&self.stderr_tail)
        ));
        out.push_str("}\n");
        out
    }
}

pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Runs one sandboxed child command and supervises it (D74/D75/D76).
pub fn run_command(config: &SandboxConfig, program: &Path, args: &[String]) -> SandboxResult {
    let started = Instant::now();
    let mut degraded: Vec<String> = Vec::new();

    // The scratch dir must exist before it can be opened for the
    // landlock rule and used as cwd.
    std::fs::create_dir_all(&config.rw_dir).ok();

    // ---- landlock setup (parent side): ruleset + O_PATH fds ---------
    let abi = if config.landlock {
        sys::landlock_abi()
    } else {
        None
    };
    let landlock_active = abi.is_some_and(|a| a >= 1);
    if config.landlock && !landlock_active {
        degraded
            .push("landlock unavailable (kernel/module absent): fs+net rules NOT enforced".into());
    }
    let mut open_fds: Vec<i32> = Vec::new();
    let mut ruleset_fd: Option<i32> = None;
    if landlock_active {
        let abi = abi.unwrap();
        let handled_net = match (abi >= 4, config.net) {
            (true, NetPolicy::DenyTcp) => {
                Some(sys::ACCESS_NET_BIND_TCP | sys::ACCESS_NET_CONNECT_TCP)
            }
            (false, NetPolicy::DenyTcp) => {
                degraded.push(format!(
                    "landlock ABI {abi} < 4: TCP deny not enforceable (kernel); audit chain only"
                ));
                None
            }
            (_, NetPolicy::Inherit) => None,
        };
        let handled = sys::handled_fs(abi);
        match sys::create_ruleset(handled, handled_net) {
            Ok(fd) => {
                let mut ok = true;
                for root in &config.ro_roots {
                    match sys::open_path(root) {
                        Ok(path_fd) => {
                            if let Err(error) =
                                sys::add_path_rule(fd, path_fd, sys::RO_ACCESS & handled)
                            {
                                ok = false;
                                degraded.push(format!("landlock rule {root:?}: {error}"));
                            }
                            open_fds.push(path_fd);
                        }
                        Err(error) => {
                            ok = false;
                            degraded.push(format!("landlock open {root:?}: {error}"));
                        }
                    }
                }
                match sys::open_path(&config.rw_dir) {
                    Ok(path_fd) => {
                        if let Err(error) =
                            sys::add_path_rule(fd, path_fd, sys::RW_ACCESS & handled)
                        {
                            ok = false;
                            degraded.push(format!("landlock rw rule {:?}: {error}", config.rw_dir));
                        }
                        open_fds.push(path_fd);
                    }
                    Err(error) => {
                        ok = false;
                        degraded.push(format!("landlock open rw dir {:?}: {error}", config.rw_dir));
                    }
                }
                if ok {
                    ruleset_fd = Some(fd);
                } else {
                    sys::close_fd(fd);
                }
            }
            Err(error) => degraded.push(format!("landlock ruleset: {error}")),
        }
    }

    // ---- cgroup layer (D75): probe the root BEFORE spawning; attach
    //      the CHILD pid after; EACCES is a recorded degradation. -----
    let scope = config.cgroup_root.as_ref().and_then(|root| {
        match cgroup::create_scope(
            root,
            &format!("ingress-sandbox-{}", std::process::id()),
            "200000 100000", // 2 CPUs (spec table)
            "2147483648",    // 2 GiB
            64,              // pids
        ) {
            Ok(scope) => Some(scope),
            Err(error) => {
                degraded.push(format!(
                    "cgroup not enforced ({error}): recorded degradation"
                ));
                None
            }
        }
    });

    // ---- child environment + cwd (on the Command) -------------------
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&config.rw_dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &config.rw_dir);
    for (key, value) in std::env::vars() {
        if config
            .env_prefixes
            .iter()
            .any(|prefix| key.starts_with(prefix))
        {
            command.env(key, value);
        }
    }

    // ---- pre_exec (in the forked child): the D74 order ---------------
    // rlimits → NO_NEW_PRIVS → landlock restrict_self → exec. The
    // callback (direct syscalls only, async-signal-safe) lives in the
    // sanctioned sys module.
    let limits = [
        (sys::RLIMIT_AS, config.address_space),
        (sys::RLIMIT_CPU, config.cpu_secs),
        (sys::RLIMIT_NPROC, config.nproc),
        (sys::RLIMIT_FSIZE, config.fsize),
        (sys::RLIMIT_NOFILE, config.nofile),
        (sys::RLIMIT_CORE, config.core),
    ];
    sys::install_pre_exec(&mut command, &limits, ruleset_fd, &open_fds);

    // Own process group (pgid == child pid): the watchdog and the
    // post-wait containment can then kill the WHOLE tree — a
    // grandchild inheriting the pipes would otherwise outlive the wall
    // budget wedging our output drain (REVIEW FIX, M8 review finding
    // 4; plain safe Rust in the child, nothing the audit chain
    // forbids).
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }

    let spawn_result = command.spawn();
    for fd in open_fds {
        sys::close_fd(fd);
    }

    let mut child = match spawn_result {
        Ok(child) => child,
        Err(error) => {
            return SandboxResult {
                classification: Classification::Spawn(format!("{}: {error}", program.display())),
                duration_ms: started.elapsed().as_millis(),
                degraded,
                landlock_abi: abi,
                stdout_tail: String::new(),
                stderr_tail: String::new(),
            };
        }
    };

    // Attach the CHILD (not the runner) to the cgroup scope.
    if let Some(scope) = scope.as_ref() {
        if let Err(error) = cgroup::attach(scope, child.id()) {
            degraded.push(format!("cgroup attach child: {error}"));
        }
    }

    // ---- wall-clock watchdog (D74): SIGKILL the GROUP past the timeout
    let deadline = Instant::now() + config.wall_timeout;
    let child_pid = child.id();
    let expired = Arc::new(AtomicBool::new(false));
    let watchdog_flag = Arc::clone(&expired);
    let watchdog = std::thread::spawn(move || {
        while Instant::now() < deadline {
            if watchdog_flag.load(Ordering::Relaxed) {
                return; // child finished first
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !watchdog_flag.load(Ordering::Relaxed) {
            sys::kill_process_group(child_pid, 9); // SIGKILL, whole group
        }
    });

    let status = child.wait();
    let waited_ms = started.elapsed().as_millis();
    // Containment (REVIEW FIX, M8 review finding 4): the direct child
    // is reaped; kill any surviving GROUP members so no descendant
    // outlives the wall budget holding an inherited pipe open (which
    // would stall the drain below) or burning CPU outside every
    // budget. ESRCH when the group is already empty.
    sys::kill_process_group(child_pid, 9);
    expired.store(true, Ordering::Relaxed);
    let _ = watchdog.join();

    // Drain output (capped) with a hard deadline: EOF once every pipe
    // holder is dead; a holder that somehow survived the group SIGKILL
    // can stall us no longer than the wall budget + grace.
    let drain_deadline = deadline + DRAIN_GRACE;
    let stdout_tail = drain_capped(child.stdout.take(), drain_deadline);
    let stderr_tail = drain_capped(child.stderr.take(), drain_deadline);
    // Report the REAL supervision wall time (direct child + drain),
    // not just the child's lifetime: pre-fix a lingering grandchild
    // made the runner return 15 s after a run it reported as 11 ms.
    let duration_ms = started.elapsed().as_millis();
    drop(scope); // after the child exited: removal succeeds on fake roots

    let classification = match status {
        Ok(status) => {
            use std::os::unix::process::ExitStatusExt;
            if let Some(code) = status.code() {
                Classification::Completed(code)
            } else {
                match status.signal() {
                    Some(24) => Classification::CpuLimit, // SIGXCPU
                    Some(9) => {
                        // SIGKILL: the watchdog when the wall clock ran
                        // out, else the hard CPU limit. `waited_ms` (the
                        // instant the direct child was reaped, BEFORE
                        // the bounded drain) disambiguates even when a
                        // lingering grandchild stretches the drain to
                        // the deadline.
                        if waited_ms >= config.wall_timeout.as_millis() {
                            Classification::WallTimeout
                        } else {
                            Classification::CpuLimit
                        }
                    }
                    Some(6 | 7 | 11) => Classification::MemLimit, // ABRT/BUS/SEGV
                    Some(other) => Classification::Signal(other),
                    None => Classification::Spawn("no exit status".into()),
                }
            }
        }
        Err(error) => Classification::Spawn(format!("wait: {error}")),
    };
    SandboxResult {
        classification,
        duration_ms,
        degraded,
        landlock_abi: abi,
        stdout_tail,
        stderr_tail,
    }
}

/// Deadline-bounded output drain (REVIEW FIX, M8 review finding 4):
/// reads on a helper thread and abandons it at `deadline`. The old
/// blocking read could never return while ANY descendant held the pipe
/// open — a plain `sh -c 'sleep 20 & exec /bin/true'` made the runner
/// block for the grandchild's full lifetime (sleep infinity ⇒ hang)
/// long after the wall watchdog killed the direct child.
fn drain_capped<R>(reader: Option<R>, deadline: Instant) -> String
where
    R: std::io::Read + Send + 'static,
{
    let Some(reader) = reader else {
        return String::new();
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // The thread owns the reader; if we abandon it below it stays
        // blocked on the pipe until process exit (one bounded leak per
        // setsid-escaped straggler, nothing more it can do safely).
        let _ = tx.send(read_capped(Some(reader)));
    });
    match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(text) => text,
        Err(_) => String::new(),
    }
}

fn read_capped<R: std::io::Read>(mut reader: Option<R>) -> String {
    let Some(reader) = reader.as_mut() else {
        return String::new();
    };
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.len() >= OUTPUT_CAP {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    bytes.truncate(OUTPUT_CAP);
    String::from_utf8_lossy(&bytes).into_owned()
}

// ---------------------------------------------------------------------------
// cgroup v2 writer (D75): probe + fake-root-testable
// ---------------------------------------------------------------------------

pub mod cgroup {
    use std::fs;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};

    /// One created cgroup scope. Dropping it removes the directory
    /// (best effort; the attached processes must have exited).
    pub struct CgroupScope {
        pub dir: PathBuf,
    }

    impl Drop for CgroupScope {
        fn drop(&mut self) {
            // On a real cgroupfs the control files are virtual: rmdir
            // alone works once processes are gone. On a fake (plain
            // directory) root the written files must be removed too.
            if fs::remove_dir(&self.dir).is_err() {
                let _ = fs::remove_dir_all(&self.dir);
            }
        }
    }

    /// Creates `<root>/<name>` and writes cpu/memory/pids limits.
    /// Errors carry the OS text (e.g. `EACCES` on a non-delegated
    /// root — the recorded degradation path on this host).
    pub fn create_scope(
        root: &Path,
        name: &str,
        cpu_max: &str,
        memory_max: &str,
        pids_max: u32,
    ) -> Result<CgroupScope, String> {
        let dir = root.join(name);
        fs::create_dir(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        let write = |file: &str, value: &str| -> Result<(), String> {
            let path = dir.join(file);
            let mut handle =
                fs::File::create(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
            handle
                .write_all(value.as_bytes())
                .map_err(|e| format!("write {file}: {e}"))
        };
        write("cpu.max", cpu_max)?;
        write("memory.max", memory_max)?;
        write("pids.max", &pids_max.to_string())?;
        Ok(CgroupScope { dir })
    }

    /// Moves `pid` into the scope's `cgroup.procs`.
    pub fn attach(scope: &CgroupScope, pid: u32) -> Result<(), String> {
        let path = scope.dir.join("cgroup.procs");
        let mut handle =
            fs::File::create(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
        handle
            .write_all(pid.to_string().as_bytes())
            .map_err(|e| format!("write cgroup.procs: {e}"))
    }
}

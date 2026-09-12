//! External orchestration (D81): one item = one in-process `serve_match`
//! (real protocol, real SHM, real IARP) plus two REAL client binaries,
//! optionally under `tools/sandbox-runner`. A per-match wall budget bounds
//! everything: clients that have not exited at the deadline are SIGKILLed
//! and the server settles a deterministic technical outcome through the
//! existing D65 disconnect path — no wedged worker, no crash of the run.
//! Every runner-side fault (a `serve_match` panic or io error, an
//! undecodable replay of a normal match) lands in
//! `Unscored { stage: RunnerFault }`.
//!
//! FACTION FIDELITY is PROVEN, not assumed: the server assigns ENL to
//! the FIRST accepted connection (D61), so the ENL-scheduled client
//! spawns first and the runner holds the RES client until the ENL
//! client's connection has COMPLETED; the served assignment is then
//! verified against the server's own record — `MatchReport::
//! accept_peer_pids` (accept order + `SO_PEERCRED` peer pids) matched
//! against the pids observed in each side's process tree. A match whose
//! attribution cannot be proven NEVER reaches the ratings
//! (`Unscored { RunnerFault }`, D81). (Review fix: the original
//! verification compared the socket PATH's filesystem inode against the
//! `socket:[N]` SOCKFS inodes under `/proc/<pid>/fd` — two disjoint
//! kernel inode spaces that never match, so it always failed after a
//! 3 s spin while leaving attribution resting on spawn order alone.)

use std::collections::BTreeSet;
use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use arena_replay::format::EndOfMatch;
use arena_server::MatchSetup;
use arena_server::driver::serve_match_observed;

use crate::tournament::config::{ClientSandbox, TournamentConfig};
use crate::tournament::entrant::EntrantId;
use crate::tournament::outcome::{Stage, TournamentOutcome, from_server};
use crate::tournament::schedule::TournamentItem;

/// Where the match's server artifacts land, relative to the match dir.
pub const SERVER_SUBDIR: &str = "server";
/// The scratch dir holding the UDS socket (the sandbox rw dir).
pub const SCRATCH_SUBDIR: &str = "scratch";

/// Kernel entropy, independent of schedule, directory and process lifetime.
pub fn execution_id() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Keep server artifact writes inside the attempt lifetime on every error path.
struct ServerGuard(Option<std::thread::JoinHandle<std::io::Result<arena_server::MatchReport>>>);
impl ServerGuard {
    fn is_finished(&self) -> bool {
        self.0.as_ref().unwrap().is_finished()
    }
    fn join(mut self) -> std::thread::Result<std::io::Result<arena_server::MatchReport>> {
        self.0.take().unwrap().join()
    }
}
impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            let _ = handle.join();
        }
    }
}

/// One executed external item.
pub struct ExternalResult {
    pub outcome: TournamentOutcome,
    /// Path of the archived replay relative to the match dir
    /// (`server/replay.iarp`) when one exists.
    pub replay_rel: Option<String>,
    /// `(side, json)` sandbox-runner reports that were captured.
    pub sandbox_reports: Vec<(String, String)>,
    /// True when the per-match budget expired and children were killed.
    pub budget_exceeded: bool,
    /// True when the ENL-scheduled client's socket connection was
    /// OBSERVED before the RES client spawned (the D61 accept-order
    /// assignment then provably matches the schedule; recorded in
    /// `meta.json`).
    pub connect_order_verified: bool,
    /// METADATA ONLY.
    pub elapsed_ms: u64,
}

/// The spawned clients: `(child, side, sandboxed)` plus the
/// sandbox-runner binary (when any). Kills and reaps everything on Drop,
/// so a panicking worker never leaks a client process.
struct ChildGuard {
    archive: Option<PathBuf>,
    children: Vec<(Child, String, bool)>,
    /// The `tools/sandbox-runner` binary when clients run sandboxed —
    /// needed for containment: the supervised client is a GRANDCHILD in
    /// its own process group (D74), std `Child::kill` reaches only the
    /// sandbox-runner, and this crate is `#![forbid(unsafe_code)]`, so
    /// the sanctioned `kill(2)` is invoked through the runner tool's
    /// `--kill-group` mode.
    sandbox_runner: Option<PathBuf>,
}

impl ChildGuard {
    /// Whether any child is still running.
    fn any_alive(&mut self) -> bool {
        self.children
            .iter_mut()
            .any(|(child, _, _)| matches!(child.try_wait(), Ok(None)))
    }

    /// SIGKILL every child — and, for sandboxed children, every
    /// GRANDCHILD's whole process group too (D74 containment: killing
    /// the sandbox-runner alone kills its group watchdog with it and
    /// would leave the client running orphaned, still holding the match
    /// socket). Grandchildren are snapshotted BEFORE the runner dies
    /// (they reparent at its death and the scan would come up empty);
    /// the group kill goes through `sandbox-runner --kill-group`
    /// because the supervised client leads its own process group
    /// (pgid == its pid) and raw `kill(2)` needs the sanctioned `sys`
    /// module in that tool.
    fn kill_all(&mut self) {
        for (child, _, sandboxed) in self.children.iter_mut() {
            if *sandboxed {
                let grandchildren = child_pids(child.id());
                let _ = child.kill();
                if let Some(runner) = self.sandbox_runner.clone() {
                    for pid in grandchildren {
                        kill_process_group(&runner, pid);
                    }
                }
            } else {
                // Unsandboxed clients are DIRECT children; std kill
                // reaches them. (An unsandboxed client that forked is out
                // of scope: no sandbox ⇒ no containment contract.)
                let _ = child.kill();
            }
        }
    }

    fn wait_all(&mut self) {
        for (child, _, _) in self.children.iter_mut() {
            let _ = child.wait();
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        // Only kill unreaped processes; don't signal a pid recycled after wait.
        for (child, side, sandboxed) in &mut self.children {
            if !matches!(child.try_wait(), Ok(Some(_))) {
                let grandchildren = if *sandboxed {
                    child_pids(child.id())
                } else {
                    Vec::new()
                };
                let _ = child.kill();
                if let Some(runner) = &self.sandbox_runner {
                    for pid in grandchildren {
                        kill_process_group(runner, pid);
                    }
                }
            }
            if let (Some(dir), Ok(status)) = (&self.archive, child.wait()) {
                // Normal paths write explicitly and propagate IO errors. Drop
                // is the fallback for already-failed attempts only.
                let _ = super::durable::atomic_write(
                    &dir.join(format!("exit_{side}.txt")),
                    status.to_string().as_bytes(),
                );
            }
        }
        self.wait_all();
    }
}

/// `sandbox-runner --kill-group <pgid>`: SIGKILL one process group via
/// the runner tool's sanctioned `kill(2)` binding. Bounded by
/// construction (one syscall, then exit); errors are ignored — a group
/// that already exited is the desired state anyway.
fn kill_process_group(runner: &Path, pgid: u32) {
    let _ = Command::new(runner)
        .arg("--kill-group")
        .arg(pgid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Runs one item. `match_dir` is the item's archive directory
/// (`<root>/matches/m<index>`); the UDS socket lives in
/// `match_dir/scratch` and the server artifacts in `match_dir/server`.
pub fn run_item_external(
    item: &TournamentItem,
    cfg: &TournamentConfig,
    match_dir: &Path,
) -> ExternalResult {
    run_item_external_on_map(item, cfg, match_dir, None)
}

pub fn run_item_external_on_map(
    item: &TournamentItem,
    cfg: &TournamentConfig,
    match_dir: &Path,
    map: Option<arena_core::map::MapDefinition>,
) -> ExternalResult {
    let start = Instant::now();
    let budget = Duration::from_millis(cfg.per_match_budget_ms);
    let played = catch_unwind(AssertUnwindSafe(|| {
        play(item, cfg, match_dir, start + budget, map)
    }));
    match played {
        Ok(result) => result,
        Err(_) => ExternalResult {
            outcome: TournamentOutcome::Unscored {
                stage: Stage::RunnerFault,
                reason: "external match orchestration panicked".to_string(),
            },
            replay_rel: None,
            sandbox_reports: Vec::new(),
            budget_exceeded: false,
            connect_order_verified: false,
            elapsed_ms: start.elapsed().as_millis() as u64,
        },
    }
}

/// Kept between the sandbox wall clock and the remaining match budget:
/// the sandbox-runner's own GROUP watchdog — the real containment (D74)
/// — must fire strictly BEFORE the tournament deadline, while the
/// sandbox-runner stays alive long enough to write its JSON report (a
/// deadline-path std kill of a live sandbox-runner killed its watchdog
/// and lost the report).
const SANDBOX_WALL_MARGIN_MS: u64 = 500;

fn fault(reason: String, elapsed_ms: u64) -> ExternalResult {
    ExternalResult {
        outcome: TournamentOutcome::Unscored {
            stage: Stage::RunnerFault,
            reason,
        },
        replay_rel: None,
        sandbox_reports: Vec::new(),
        budget_exceeded: false,
        connect_order_verified: false,
        elapsed_ms,
    }
}

/// Pids of `pid`'s direct children, from the kernel's
/// `/proc/<pid>/task/<pid>/children` (same-uid reads, std-only, Linux —
/// the WSL2 host; the M8 sandbox evidence already assumes /proc).
fn child_pids(pid: u32) -> Vec<u32> {
    let mut pids = Vec::new();
    if let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) {
        for token in text.split_whitespace() {
            if let Ok(child_pid) = token.parse::<u32>() {
                pids.push(child_pid);
            }
        }
    }
    pids
}

/// `pid` plus its direct children: the full process tree of one spawned
/// side (unsandboxed clients ARE the child; sandboxed clients are one
/// level down, D74 — the sandbox-runner forks the client into its own
/// process group). Deeper levels do not occur (clients do not fork).
fn tree_pids(pid: u32) -> Vec<u32> {
    let mut tree = vec![pid];
    tree.extend(child_pids(pid));
    tree
}

/// SOCKFS inodes of the unix sockets `pid` holds open, from the
/// `socket:[N]` fd links under `/proc/<pid>/fd`.
#[cfg(test)]
fn fd_socket_inodes(pid: u32) -> Vec<u64> {
    let mut inodes = Vec::new();
    let fd_dir = std::path::PathBuf::from(format!("/proc/{pid}/fd"));
    if let Ok(entries) = std::fs::read_dir(&fd_dir) {
        for entry in entries.flatten() {
            if let Ok(target) = std::fs::read_link(entry.path()) {
                let target = target.to_string_lossy();
                if let Some(stripped) = target.strip_prefix("socket:[") {
                    if let Ok(inode) = stripped.trim_end_matches(']').parse::<u64>() {
                        inodes.push(inode);
                    }
                }
            }
        }
    }
    inodes
}

/// SOCKFS inodes of every CONNECTED unix socket on the host, from
/// `/proc/net/unix` (`None` when the file is unreadable). Column `St` is
/// the socket state: `01` = unconnected (a fresh socket, or a listener),
/// `03` = connected — verified live on this kernel (6.18 WSL2): both
/// endpoints of an accepted stream connection flip 01 → 03 and a bound
/// listener stays 01. A client fd in state 03 therefore proves its
/// `connect()` COMPLETED, which (backlog is FIFO) queued it ahead of any
/// connection made later — exactly what the connect-order gate needs.
#[cfg(test)]
fn connected_unix_socket_inodes() -> Option<BTreeSet<u64>> {
    let text = std::fs::read_to_string("/proc/net/unix").ok()?;
    let mut inodes = BTreeSet::new();
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        // Num RefCount Protocol Flags Type St Inode [Path]
        if fields.len() >= 7 && fields[5] == "03" {
            if let Ok(inode) = fields[6].parse::<u64>() {
                inodes.insert(inode);
            }
        }
    }
    Some(inodes)
}

/// True when any process of `pid`'s tree holds a CONNECTED unix socket:
/// the observable sign that the ENL client's `connect()` finished. If
/// `/proc/net/unix` cannot be read the weaker fallback is "any open
/// socket fd" (a freshly created, not-yet-connected socket still counts);
/// the gate is an ordering HINT — the authoritative proof is
/// [`verify_accept_order`], so a weaker gate can only cost a match, never
/// misattribute one.
#[cfg(test)]
fn tree_holds_connected_socket(pid: u32) -> bool {
    let connected = connected_unix_socket_inodes();
    for tree_pid in tree_pids(pid) {
        for inode in fd_socket_inodes(tree_pid) {
            let connected_now = match &connected {
                Some(set) => set.contains(&inode),
                None => true,
            };
            if connected_now {
                return true;
            }
        }
    }
    false
}

/// The server-side proof of the served faction assignment. The driver
/// records each accepted connection's `SO_PEERCRED` pid IN ACCEPT ORDER
/// (`MatchReport::accept_peer_pids`; slot 0 = first accept = ENL, D61).
/// The schedule's assignment holds iff the first-accepted peer belongs to
/// the ENL-SCHEDULED side's process tree and the second to the RES
/// side's — with both connections present and made by distinct
/// processes (a client that connected twice would otherwise be
/// indistinguishable from a real opponent and MUST fail the proof).
pub fn verify_accept_order(
    accept_peer_pids: [Option<u32>; 2],
    enl_tree: &BTreeSet<u32>,
    res_tree: &BTreeSet<u32>,
) -> bool {
    match accept_peer_pids {
        [Some(first), Some(second)] => {
            first != second && enl_tree.contains(&first) && res_tree.contains(&second)
        }
        _ => false,
    }
}

/// A scoreable or attributed outcome whose faction assignment could NOT
/// be proven is demoted to `Unscored { RunnerFault }` — an unproven
/// attribution must never move ratings, W/D/L, or forfeit-win counters
/// (D81/D82). Already-unscored outcomes keep their (more precise) stage.
pub fn enforce_connect_proof(
    outcome: TournamentOutcome,
    connect_order_verified: bool,
) -> TournamentOutcome {
    if connect_order_verified {
        return outcome;
    }
    match outcome {
        TournamentOutcome::Normal { .. } | TournamentOutcome::TechnicalWin { .. } => {
            TournamentOutcome::Unscored {
                stage: Stage::RunnerFault,
                reason: "connect order not proven: the first accepted connection could not be \
                         verified as the ENL-scheduled client"
                    .to_string(),
            }
        }
        unscored => unscored,
    }
}

fn play(
    item: &TournamentItem,
    cfg: &TournamentConfig,
    match_dir: &Path,
    deadline: Instant,
    map: Option<arena_core::map::MapDefinition>,
) -> ExternalResult {
    let start = Instant::now();
    if let Err(error) = std::fs::create_dir_all(match_dir) {
        return fault(format!("match dir: {error}"), 0);
    }
    // ABSOLUTE paths: the sandbox sets the client's cwd to the scratch
    // dir, so a relative socket path (or --rw dir) would resolve wrong
    // inside the sandbox — the r1 release run caught exactly this.
    let match_dir = match match_dir.canonicalize() {
        Ok(absolute) => absolute,
        Err(error) => return fault(format!("match dir canonicalize: {error}"), 0),
    };
    let match_id = match execution_id() {
        Ok(id) => id,
        Err(error) => return fault(format!("execution identity: {error}"), 0),
    };
    // Keep UDS paths below Linux's 108-byte limit regardless of archive path.
    let scratch = PathBuf::from("/tmp").join(format!("ia-{match_id}"));
    if let Err(error) = std::fs::create_dir(&scratch) {
        return fault(format!("runtime directory: {error}"), 0);
    }
    struct RuntimeDir(PathBuf);
    impl Drop for RuntimeDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("arena.sock"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let _runtime = RuntimeDir(scratch.clone());
    let server_dir = match_dir.join(SERVER_SUBDIR);
    let scratch_archive = match_dir.join(SCRATCH_SUBDIR);
    if let Err(error) = std::fs::create_dir_all(&scratch)
        .and_then(|_| std::fs::create_dir_all(&server_dir))
        .and_then(|_| std::fs::create_dir_all(&scratch_archive))
        .and_then(|_| {
            super::durable::atomic_write(
                &scratch_archive.join("runtime-path.txt"),
                scratch.to_string_lossy().as_bytes(),
            )
        })
    {
        return fault(format!("match dirs: {error}"), 0);
    }
    let socket = scratch.join("arena.sock");
    // Unique per item: keeps /dev/shm/ingress-<id>-* regions collision-free
    // under parallelism (D63).
    if let Err(error) =
        super::durable::atomic_write(&match_dir.join("execution-id"), match_id.as_bytes())
    {
        return fault(format!("execution metadata: {error}"), 0);
    }
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();

    // 1. The server thread (in-process serve_match: real protocol, real
    //    SHM, real journal/result/IARP under server_dir).
    let setup = MatchSetup {
        game_seed: item.game_seed,
        map_seed: item.map_seed,
        config: cfg.config.clone(),
        socket_path: socket.clone(),
        match_id: match_id.clone(),
        out_dir: server_dir.clone(),
    };
    let server = match std::thread::Builder::new()
        .name(format!("serve-{match_id}"))
        .spawn(move || serve_match_observed(setup, Some(accepted_tx), map))
    {
        Ok(handle) => ServerGuard(Some(handle)),
        Err(error) => return fault(format!("server thread: {error}"), 0),
    };

    // 2. Wait for the socket to exist (bounded by the budget).
    let mut socket_seen = false;
    while Instant::now() < deadline {
        if socket.exists() {
            socket_seen = true;
            break;
        }
        if server.is_finished() {
            break; // failed fast; the join below reports the error
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    if !socket_seen {
        // serve_match always returns on its own (handshake window + round
        // deadlines + write timeouts), so this join is bounded.
        let reason = match server.join() {
            Ok(Ok(report)) => format!(
                "server finished without binding the socket: {}",
                report.outcome.tag()
            ),
            Ok(Err(error)) => format!("serve_match failed: {error}"),
            Err(_) => "serve_match panicked".to_string(),
        };
        return fault(reason, start.elapsed().as_millis() as u64);
    }

    // 3. Spawn both clients (argv <socket> <client_seed>; entrant env
    //    vars pass through the sandbox allowlist). The ENL-SCHEDULED
    //    entrant spawns FIRST and the RES client is held until the ENL
    //    client's connection has COMPLETED; after the match the served
    //    assignment is PROVEN against the server's accept record (the
    //    server assigns ENL to the first accepted connection, D61, so
    //    the schedule's `a_is_enl` must match the proven accept order).
    // Drop order: kill/reap clients before joining their output drainers.
    let mut captures = Vec::new();
    let mut guard = ChildGuard {
        archive: Some(match_dir.clone()),
        children: Vec::new(),
        sandbox_runner: match &cfg.sandbox {
            ClientSandbox::SandboxRunner { runner, .. } => Some(runner.clone()),
            ClientSandbox::Off => None,
        },
    };
    let enl_first: [(String, EntrantId); 2] = if item.a_is_enl {
        [("a".to_string(), item.a), ("b".to_string(), item.b)]
    } else {
        [("b".to_string(), item.b), ("a".to_string(), item.a)]
    };
    // Per-side process-tree snapshots, accumulated from spawn through
    // the poll loop below: a sandboxed client is a GRANDCHILD that
    // appears a few ms after its sandbox-runner and is reaped by it
    // before the match ends — the union keeps every pid ever observable
    // in each side's tree for the post-match proof.
    let mut enl_pids: BTreeSet<u32> = BTreeSet::new();
    let mut res_pids: BTreeSet<u32> = BTreeSet::new();
    let mut enl_child_pid: Option<u32> = None;
    let mut res_child_pid: Option<u32> = None;
    for (side, entrant_id) in enl_first {
        let entrant = cfg.entrant(entrant_id);
        let crate::tournament::entrant::EntrantKind::External { binary, env } = &entrant.kind;
        if let Some(expected) = &entrant.provenance.binary_blake3 {
            if super::entrant::compute_binary_hash(binary).as_ref().ok() != Some(expected) {
                return fault(
                    format!("client {side} binary identity changed"),
                    start.elapsed().as_millis() as u64,
                );
            }
        }
        let seed = item.client_seed.to_string();
        let mut sandboxed = false;
        let mut command = match &cfg.sandbox {
            ClientSandbox::Off => {
                let mut command = Command::new(binary);
                command.arg(&socket).arg(&seed);
                command.current_dir(&scratch);
                command.stdout(Stdio::piped()).stderr(Stdio::piped());
                command
            }
            ClientSandbox::SandboxRunner {
                runner,
                wall_ms,
                address_space_bytes,
            } => {
                // The sandbox rw dir IS the scratch dir (the socket lives
                // there); the runner's own ro-roots cover its own binary
                // directory — add the client's explicitly (it may live
                // elsewhere). Wall clock = the remaining budget MINUS a
                // margin, so the sandbox-runner's own GROUP watchdog (the
                // real containment, D74) fires strictly before the
                // tournament deadline and its JSON report still gets
                // written.
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u64;
                let wall = (*wall_ms).min(remaining.saturating_sub(SANDBOX_WALL_MARGIN_MS).max(1));
                let mut command = Command::new(runner);
                command
                    .arg("--as")
                    .arg(address_space_bytes.to_string())
                    .arg("--wall-ms")
                    .arg(wall.to_string())
                    .arg("--rw")
                    .arg(&scratch)
                    .arg("--ro")
                    .arg(binary.parent().unwrap_or(Path::new(".")))
                    .arg("--")
                    .arg(binary)
                    .arg(&socket)
                    .arg(&seed);
                command.stdout(Stdio::piped()).stderr(Stdio::piped());
                sandboxed = true;
                command
            }
        };
        for (key, value) in env {
            command.env(key, value);
        }
        match command.spawn() {
            Ok(child) => {
                guard.children.push((child, side.clone(), sandboxed));
                let child = &mut guard.children.last_mut().unwrap().0;
                let prefix = if sandboxed { "sandbox" } else { "client" };
                let stdout = match super::logs::Capture::start(
                    child.stdout.take().unwrap(),
                    match_dir.join(format!("{prefix}_{side}.stdout")),
                ) {
                    Ok(capture) => capture,
                    Err(e) => return fault(format!("stdout capture: {e}"), 0),
                };
                let stderr = match super::logs::Capture::start(
                    child.stderr.take().unwrap(),
                    match_dir.join(format!("{prefix}_{side}.stderr")),
                ) {
                    Ok(capture) => capture,
                    Err(e) => {
                        guard.kill_all();
                        return fault(format!("stderr capture: {e}"), 0);
                    }
                };
                captures.push((side.clone(), sandboxed, stdout, stderr));
                let pid = child.id();
                if enl_child_pid.is_none() {
                    // The ENL side goes first: hold the RES client until
                    // this connection has COMPLETED (bounded by the
                    // REMAINING budget, never a fixed constant).
                    // A direct client may finish before the first poll after
                    // both spawns. Its Child pid is already authoritative evidence.
                    enl_pids.insert(pid);
                    let confirmed = loop {
                        enl_pids.extend(tree_pids(pid));
                        match accepted_rx.recv_timeout(Duration::from_millis(3)) {
                            Ok((0, Some(peer))) => {
                                // The sandbox may have forked between our tree
                                // snapshot and recv. Refresh after the event.
                                enl_pids.extend(tree_pids(pid));
                                break enl_pids.contains(&peer);
                            }
                            Ok(_) => break false,
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break false,
                            Err(_) => {}
                        }
                        if Instant::now() >= deadline || !matches!(child.try_wait(), Ok(None)) {
                            break false;
                        }
                    };
                    if !confirmed {
                        drop(guard);
                        let _ = server.join();
                        return fault(
                            "server did not confirm scheduled ENL peer".into(),
                            start.elapsed().as_millis() as u64,
                        );
                    }
                    enl_child_pid = Some(pid);
                } else {
                    res_pids.insert(pid);
                    res_pids.extend(tree_pids(pid));
                    res_child_pid = Some(pid);
                }
            }
            // Drop of `guard` kills the already-spawned peer (and, when
            // sandboxed, its client's whole process group).
            Err(error) => {
                return fault(
                    format!("client {side} spawn failed: {error}"),
                    start.elapsed().as_millis() as u64,
                );
            }
        }
    }

    // 4. Poll both children until they exit or the budget expires; at the
    //    deadline kill them — std SIGKILL for direct children, and for
    //    sandboxed clients the grandchild's whole process GROUP via the
    //    sandbox-runner's `--kill-group` (the sandbox-runner's own group
    //    watchdog died with it) — the server then settles CLIENT_FORFEIT
    //    through the disconnect path (deterministic technical outcome, no
    //    wedged worker). Each pass also refreshes the per-side pid trees
    //    (the connect-order proof needs pids that are gone by match end).
    let mut budget_exceeded = false;
    while guard.any_alive() {
        if let Some(pid) = enl_child_pid {
            enl_pids.extend(tree_pids(pid));
        }
        if let Some(pid) = res_child_pid {
            res_pids.extend(tree_pids(pid));
        }
        if Instant::now() >= deadline {
            budget_exceeded = true;
            guard.kill_all();
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut sandbox_reports = Vec::new();
    let mut log_error = None;
    for (side, sandboxed, stdout, stderr) in captures {
        let output = stdout.finish();
        let errors = stderr.finish();
        match (output, errors) {
            (Ok(bytes), Ok(_)) if sandboxed => match String::from_utf8(bytes) {
                Ok(json) => sandbox_reports.push((side, json)),
                Err(error) => log_error = Some(error.to_string()),
            },
            (Err(error), _) | (_, Err(error)) => log_error = Some(error.to_string()),
            _ => {}
        }
    }
    for (child, side, _) in &mut guard.children {
        let status = child.wait().map(|s| s.to_string());
        match status {
            Ok(status) => {
                if let Err(error) = super::durable::atomic_write(
                    &match_dir.join(format!("exit_{side}.txt")),
                    status.as_bytes(),
                ) {
                    log_error = Some(error.to_string());
                }
            }
            Err(error) => log_error = Some(error.to_string()),
        }
    }
    guard.wait_all();
    drop(guard);
    let _ = std::fs::remove_file(&socket);

    // 5. Join the server and normalize its outcome.
    let report = match server.join() {
        Ok(Ok(report)) => report,
        Ok(Err(error)) => {
            return fault(
                format!("serve_match failed: {error}"),
                start.elapsed().as_millis() as u64,
            );
        }
        Err(_) => {
            return fault(
                "serve_match panicked".to_string(),
                start.elapsed().as_millis() as u64,
            );
        }
    };
    if let Some(error) = log_error {
        return fault(
            format!("client logs: {error}"),
            start.elapsed().as_millis() as u64,
        );
    }
    let mut outcome = from_server(&report.outcome, item.a, item.b, item.a_is_enl);
    let replay_rel = report
        .replay_path
        .as_ref()
        .and_then(|path| path.strip_prefix(match_dir).ok())
        .map(|relative| relative.to_string_lossy().to_string());
    if outcome.elo_relevant() {
        // A Normal match MUST have a decodable replay whose final hash and
        // round count agree with the server report (traceability + the
        // replay-verification gate).
        let decoded = report
            .replay_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| arena_replay::replay::decode_log(&bytes).ok());
        match decoded {
            Some(file) => match file.trailer() {
                Some(EndOfMatch::Normal {
                    final_state_hash,
                    rounds_completed,
                    ..
                }) => {
                    if *rounds_completed != report.rounds_completed {
                        return fault(
                            format!(
                                "replay rounds {rounds_completed} != report rounds {}",
                                report.rounds_completed
                            ),
                            start.elapsed().as_millis() as u64,
                        );
                    }
                    outcome = outcome.with_final_hash(*final_state_hash);
                }
                trailer => {
                    return fault(
                        format!("normal match replay trailer is {trailer:?}"),
                        start.elapsed().as_millis() as u64,
                    );
                }
            },
            None => {
                return fault(
                    "normal match replay missing or undecodable".to_string(),
                    start.elapsed().as_millis() as u64,
                );
            }
        }
    }
    // 6. FACTION FIDELITY (D81): prove the served assignment from the
    //    server's own accept record (accept order + SO_PEERCRED pids,
    //    D61) against the spawned sides' process trees. An attribution
    //    that cannot be proven is demoted to Unscored — it must never
    //    move ratings, W/D/L, or forfeit-win counters.
    let connect_order_verified = verify_accept_order(report.accept_peer_pids, &enl_pids, &res_pids);
    if let Some(path) = &report.replay_path {
        let attach = || -> Result<(), String> {
            let mut log =
                arena_replay::replay::decode_log(&std::fs::read(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let identity = log
                .header
                .identity
                .as_mut()
                .ok_or("missing server identity")?;
            for (slot, entrant_id) in [item.enl_entrant(), item.res_entrant()]
                .into_iter()
                .enumerate()
            {
                let entrant = cfg.entrant(entrant_id);
                let participant = &mut identity.participants[slot];
                // Only attach registered identity after attribution is proven;
                // unproven matches retain server-collected client claims.
                if connect_order_verified {
                    participant.registered_id = entrant_id.to_string();
                    participant.display_name = entrant.name.clone();
                    participant.registered_version =
                        entrant.provenance.bot_version.clone().unwrap_or_default();
                    participant.binary_hash =
                        entrant.provenance.binary_blake3.clone().unwrap_or_default();
                    participant.binding_verified = true;
                }
            }
            super::durable::atomic_write(path, &log.encode()).map_err(|e| e.to_string())
        };
        if let Err(error) = attach() {
            return fault(
                format!("replay identity: {error}"),
                start.elapsed().as_millis() as u64,
            );
        }
    }
    let outcome = enforce_connect_proof(outcome, connect_order_verified);
    ExternalResult {
        outcome,
        replay_rel,
        sandbox_reports,
        budget_exceeded,
        connect_order_verified,
        elapsed_ms: start.elapsed().as_millis() as u64,
    }
}

/// Locates a workspace binary (client fixtures, sandbox runner) next to
/// the running executable, falling back to `target/{debug,release}` —
/// the M7 test convention (binaries must be built: `cargo build
/// --workspace`). `ARENA_BIN_DIR` overrides the search.
pub fn find_workspace_binary(name: &str) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("ARENA_BIN_DIR") {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
            if let Some(target) = dir.parent() {
                for profile in ["debug", "release"] {
                    let candidate = target.join(profile).join(name);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skip_live() -> bool {
        std::env::var("ARENA_SKIP_LIVE")
            .map(|value| value == "1")
            .unwrap_or(false)
    }

    /// Test-target binary lookup: the unit-test executable lives in
    /// `target/<profile>/deps/`, where the binary-oriented
    /// [`find_workspace_binary`] (current_exe sits in `target/<profile>/`)
    /// cannot see the tools — resolve them from this test executable's
    /// Cargo target directory and profile instead.
    fn test_bin(name: &str) -> Option<PathBuf> {
        let executable = std::env::current_exe().ok()?;
        let candidate = executable.parent()?.parent()?.join(name);
        candidate.is_file().then_some(candidate)
    }

    /// Any live process whose command line mentions `needle` (NUL-separated
    /// argv read as lossy text). Used to prove containment: after a kill,
    /// NO process of the killed side may survive.
    fn any_process_cmdline_contains(needle: &str) -> bool {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return false;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if let Ok(bytes) = std::fs::read(entry.path().join("cmdline")) {
                if String::from_utf8_lossy(&bytes).contains(needle) {
                    return true;
                }
            }
        }
        false
    }

    /// REVIEW FIX (M9, D74/D81 containment): `ChildGuard::kill_all` must
    /// kill a sandboxed client's whole process GROUP, not just the
    /// sandbox-runner. Reproduces the confirmed defect live: the client is
    /// a GRANDCHILD in its own process group, so a std kill of the runner
    /// leaves the client running orphaned (before the fix it survived for
    /// its full 30 s stall + 60 s wall). Requires the workspace binaries
    /// (`cargo build --workspace`); `ARENA_SKIP_LIVE=1` skips it.
    #[test]
    fn kill_all_contains_sandboxed_grandchildren() {
        if skip_live() {
            return;
        }
        let Some(runner) = test_bin("sandbox-runner") else {
            panic!("sandbox-runner not built (run cargo build --workspace)");
        };
        let Some(client) = test_bin("probe-client") else {
            panic!("probe-client not built (run cargo build --workspace)");
        };
        let scratch = std::env::temp_dir().join(format!(
            "ingress-m9-containment-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).expect("scratch dir");
        let socket = scratch.join("arena.sock");
        // A listener so the stall client's connect() completes and it
        // settles into its 30 s sleep while HOLDING the socket.
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        let needle = scratch.to_string_lossy().to_string();

        let mut command = Command::new(&runner);
        command
            .arg("--wall-ms")
            .arg("60000")
            .arg("--rw")
            .arg(&scratch)
            .arg("--ro")
            .arg(client.parent().unwrap_or(Path::new(".")))
            .arg("--")
            .arg(&client)
            .arg(&socket)
            .arg("7")
            .env("PROBE_MODE", "stall")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let child = command.spawn().expect("sandbox-runner spawns");

        // Wait until the client grandchild's connection has completed
        // (the same gate `play` uses), bounded.
        let gate_deadline = Instant::now() + Duration::from_secs(20);
        let mut child = child;
        while !tree_holds_connected_socket(child.id()) {
            assert!(
                Instant::now() < gate_deadline && matches!(child.try_wait(), Ok(None)),
                "the sandboxed stall client never connected"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(any_process_cmdline_contains(&needle), "client is alive");

        let mut guard = ChildGuard {
            archive: None,
            children: vec![(child, "a".to_string(), true)],
            sandbox_runner: Some(runner),
        };
        guard.kill_all();
        guard.wait_all();
        drop(guard);
        drop(listener);

        // Containment: no process of the killed side survives. SIGKILL is
        // synchronous, but allow the scheduler a moment to reap.
        let deadline = Instant::now() + Duration::from_secs(5);
        while any_process_cmdline_contains(&needle) {
            assert!(
                Instant::now() < deadline,
                "a sandboxed client process survived kill_all (D74 containment broken)"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = std::fs::remove_dir_all(&scratch);
    }

    /// The gate's socket-state discriminator: a connected unix socket fd
    /// is state 03 in /proc/net/unix on this kernel (the live check the
    /// gate depends on; if a kernel ever changes the encoding this fails
    /// loudly instead of silently weakening the gate to the fallback).
    #[test]
    fn connected_unix_socket_inodes_sees_this_process_socket() {
        let dir = std::env::temp_dir().join(format!(
            "ingress-m9-gatecheck-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let socket = dir.join("gate.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        let client = std::os::unix::net::UnixStream::connect(&socket).expect("connect");
        let _accepted = listener.accept().expect("accept");
        let Some(connected) = connected_unix_socket_inodes() else {
            panic!("/proc/net/unix is unreadable — the gate falls back to the weaker check");
        };
        let our_pid = std::process::id();
        let held = fd_socket_inodes(our_pid);
        assert!(
            !held.is_empty(),
            "this process holds the listener/client/accepted sockets"
        );
        assert!(
            held.iter().any(|inode| connected.contains(inode)),
            "a connected socket fd must appear as connected in /proc/net/unix"
        );
        drop((client, listener, _accepted));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

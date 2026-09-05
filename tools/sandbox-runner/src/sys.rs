//! THE ONLY MODULE WITH `unsafe` in sandbox-runner (P4-CAPABILITY):
//! raw Linux syscall bindings via `extern "C"` (no libc crate,
//!) — `setrlimit`, `prctl(PR_SET_NO_NEW_PRIVS)`, `open` with
//! `O_PATH`, and the three landlock syscalls (444/445/446). Everything
//! else in the crate is safe Rust and a stray `unsafe` fails the build.
//!
//! # Invariants
//!
//! * The `pre_exec` callbacks built on these bindings perform ONLY
//!   direct syscalls (async-signal-safe by construction: no
//!   allocation, no locks, no libc state) and run in the forked child
//!   before `exec`.
//! * `RLIMIT_*` / `PR_*` / `O_*` / landlock constants are stable
//!   Linux x86_64 ABI facts, written out and cross-checked below.
//! * Landlock fds are closed (or CLOEXEC-closed at exec) so nothing
//!   leaks into the sandboxed program.

use std::ffi::{c_char, c_int, c_long, c_uint, c_void};

// ---------------------------------------------------------------------------
// extern declarations
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn setrlimit(resource: c_int, rlim: *const RLimit) -> c_int;
    fn prctl(option: c_int, ...) -> c_int;
    fn syscall(number: c_long, ...) -> c_long;
    fn open(path: *const c_char, oflag: c_int, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
}

// ---------------------------------------------------------------------------
// rlimits (x86_64 numbering)
// ---------------------------------------------------------------------------

pub const RLIMIT_CPU: c_int = 0;
pub const RLIMIT_FSIZE: c_int = 1;
pub const RLIMIT_CORE: c_int = 4;
pub const RLIMIT_NPROC: c_int = 6;
pub const RLIMIT_NOFILE: c_int = 7;
pub const RLIMIT_AS: c_int = 9;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RLimit {
    pub cur: u64,
    pub max: u64,
}
const _: () = assert!(std::mem::size_of::<RLimit>() == 16);

/// Sets `cur == max == limit`. Returns the errno text on failure.
pub fn set_limit(resource: c_int, limit: u64) -> Result<(), String> {
    let rlim = RLimit {
        cur: limit,
        max: limit,
    };
    // SAFETY: `rlim` is a valid, fully initialized repr(C) value for the
    // duration of the call; setrlimit only reads it.
    if unsafe { setrlimit(resource, &rlim) } != 0 {
        return Err(format!(
            "setrlimit({resource}) failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// prctl
// ---------------------------------------------------------------------------

/// `PR_SET_NO_NEW_PRIVS`. NOTE: this host's kernel headers (WSL2
/// 6.18) define it as 38 (verified live: `prctl(38, 1)` sets
/// `NoNewPrivs: 1` in `/proc/self/status`); some older references
/// say 39, which returns EINVAL here.
pub const PR_SET_NO_NEW_PRIVS: c_int = 38;

/// `PR_SET_NO_NEW_PRIVS = 1` — required before landlock
/// `restrict_self`, and independently blocks setuid/sudo style
/// privilege escalations through the sandboxed exec.
pub fn set_no_new_privs() -> Result<(), String> {
    // SAFETY: plain integer arguments.
    if unsafe {
        prctl(
            PR_SET_NO_NEW_PRIVS,
            1 as usize,
            0 as usize,
            0 as usize,
            0 as usize,
        )
    } != 0
    {
        return Err(format!(
            "prctl(NO_NEW_PRIVS) failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// kill(2)
// ---------------------------------------------------------------------------

/// Raw `kill(2)` (syscall 62 on x86_64). Used by the wall-clock
/// watchdog to SIGKILL the supervised child.
pub fn kill(pid: u32, sig: c_int) {
    // SAFETY: plain integers.
    unsafe {
        kill_c(pid as c_int, sig);
    }
}

/// `kill(2)` on a process GROUP (negative pid). The supervised child
/// is spawned with `process_group(0)`, so pgid == child pid and every
/// descendant that did not `setsid(2)` away is in the group: the wall
/// watchdog and the post-wait containment both kill the WHOLE group —
/// a grandchild that inherited the child's stdout would otherwise
/// outlive the wall budget holding the output pipe open (REVIEW FIX,
/// M8 review finding 4). After the direct child is reaped this cannot
/// address a live unrelated process unless the group is empty AND the
/// pid wrapped around within microseconds (standard supervisor
/// practice; not observable here).
pub fn kill_process_group(pgid: u32, sig: c_int) {
    // SAFETY: plain integers; the negated pgid addresses the group.
    unsafe {
        kill_c((pgid as c_int).wrapping_neg(), sig);
    }
}

unsafe extern "C" {
    #[link_name = "kill"]
    fn kill_c(pid: c_int, sig: c_int) -> c_int;
}

// ---------------------------------------------------------------------------
// O_PATH open
// ---------------------------------------------------------------------------

pub const O_PATH: c_int = 0o10000000;
pub const O_CLOEXEC: c_int = 0o2000000;

/// Opens a path without any access intent (landlock rule parent).
/// The fd carries `O_CLOEXEC` so it closes at exec.
pub fn open_path(path: &std::path::Path) -> Result<c_int, String> {
    let Ok(cstring) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
        return Err("path contains NUL".to_string());
    };
    // SAFETY: valid C string; open only opens.
    let fd = unsafe { open(cstring.as_ptr(), O_PATH | O_CLOEXEC, 0 as c_uint) };
    if fd < 0 {
        return Err(format!(
            "open({}) failed: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(fd)
}

/// Closes an fd, ignoring errors (best-effort cleanup).
pub fn close_fd(fd: c_int) {
    // SAFETY: plain fd.
    unsafe { close(fd) };
}

// ---------------------------------------------------------------------------
// landlock (syscalls 444/445/446 on x86_64)
// ---------------------------------------------------------------------------

pub const SYS_LANDLOCK_CREATE_RULESET: c_long = 444;
pub const SYS_LANDLOCK_ADD_RULE: c_long = 445;
pub const SYS_LANDLOCK_RESTRICT_SELF: c_long = 446;

pub const LANDLOCK_CREATE_RULESET_VERSION: c_long = 1;
pub const LANDLOCK_RULE_PATH_BENEATH: c_long = 1;

/// FS access rights (ABI 1 unless noted).
pub const ACCESS_EXECUTE: u64 = 1 << 0;
pub const ACCESS_WRITE_FILE: u64 = 1 << 1;
pub const ACCESS_READ_FILE: u64 = 1 << 2;
pub const ACCESS_READ_DIR: u64 = 1 << 3;
pub const ACCESS_REMOVE_DIR: u64 = 1 << 4;
pub const ACCESS_REMOVE_FILE: u64 = 1 << 5;
pub const ACCESS_MAKE_CHAR: u64 = 1 << 6;
pub const ACCESS_MAKE_DIR: u64 = 1 << 7;
pub const ACCESS_MAKE_REG: u64 = 1 << 8;
pub const ACCESS_MAKE_SOCK: u64 = 1 << 9;
pub const ACCESS_MAKE_FIFO: u64 = 1 << 10;
pub const ACCESS_MAKE_BLOCK: u64 = 1 << 11;
pub const ACCESS_MAKE_SYM: u64 = 1 << 12;
/// ABI 2.
pub const ACCESS_REFER: u64 = 1 << 13;
/// ABI 3.
pub const ACCESS_TRUNCATE: u64 = 1 << 14;
/// ABI 5. Handled but granted NOWHERE (neither RO nor RW access sets
/// include it): device ioctls on opened char/block device nodes are
/// denied for the sandboxed child (D76).
pub const ACCESS_IOCTL_DEV: u64 = 1 << 15;

/// Network access rights (ABI 4): handled-but-not-granted = denied.
pub const ACCESS_NET_BIND_TCP: u64 = 1 << 0;
pub const ACCESS_NET_CONNECT_TCP: u64 = 1 << 1;

/// The read-only allowance for RO roots.
pub const RO_ACCESS: u64 = ACCESS_EXECUTE | ACCESS_READ_FILE | ACCESS_READ_DIR;
/// The full scratch-dir allowance (read + every write flavor).
pub const RW_ACCESS: u64 = ACCESS_READ_FILE
    | ACCESS_READ_DIR
    | ACCESS_WRITE_FILE
    | ACCESS_REMOVE_DIR
    | ACCESS_REMOVE_FILE
    | ACCESS_MAKE_CHAR
    | ACCESS_MAKE_DIR
    | ACCESS_MAKE_REG
    | ACCESS_MAKE_SOCK
    | ACCESS_MAKE_FIFO
    | ACCESS_MAKE_BLOCK
    | ACCESS_MAKE_SYM
    | ACCESS_REFER
    | ACCESS_TRUNCATE;

/// The handled FS set for a given ABI (everything the kernel can
/// enforce for us, up to what it knows). Growth per D76: REFER at ABI
/// 2, TRUNCATE at 3, IOCTL_DEV at 5.
pub fn handled_fs(abi: u32) -> u64 {
    let mut set = ACCESS_EXECUTE
        | ACCESS_WRITE_FILE
        | ACCESS_READ_FILE
        | ACCESS_READ_DIR
        | ACCESS_REMOVE_DIR
        | ACCESS_REMOVE_FILE
        | ACCESS_MAKE_CHAR
        | ACCESS_MAKE_DIR
        | ACCESS_MAKE_REG
        | ACCESS_MAKE_SOCK
        | ACCESS_MAKE_FIFO
        | ACCESS_MAKE_BLOCK
        | ACCESS_MAKE_SYM;
    if abi >= 2 {
        set |= ACCESS_REFER;
    }
    if abi >= 3 {
        set |= ACCESS_TRUNCATE;
    }
    if abi >= 5 {
        set |= ACCESS_IOCTL_DEV;
    }
    set
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LandlockPathBeneathAttr {
    allowed_access: u64,
    parent_fd: c_int,
}
const _: () = assert!(std::mem::size_of::<LandlockPathBeneathAttr>() == 16);

/// Probes the kernel landlock ABI (`create_ruleset(NULL, 0, VERSION)`).
/// `None` when unsupported (ENOSYS/EOPNOTSUPP/EPERM).
pub fn landlock_abi() -> Option<u32> {
    // SAFETY: the VERSION probe takes no pointers; NULL, 0.
    let ret = unsafe {
        syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            std::ptr::null::<c_void>(),
            0 as usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if ret < 0 {
        return None;
    }
    Some(ret as u32)
}

/// Creates a ruleset handling the given fs/net access sets. `abi >= 4`
/// is required to pass the net field.
pub fn create_ruleset(handled_fs: u64, handled_net: Option<u64>) -> Result<c_int, String> {
    let attr = LandlockRulesetAttr {
        handled_access_fs: handled_fs,
        handled_access_net: handled_net.unwrap_or(0),
    };
    let size = if handled_net.is_some() {
        std::mem::size_of::<LandlockRulesetAttr>()
    } else {
        std::mem::size_of::<u64>()
    };
    // SAFETY: attr is fully initialized for the passed size.
    let fd = unsafe {
        syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            &attr as *const LandlockRulesetAttr,
            size,
            0 as c_long,
        )
    };
    if fd < 0 {
        return Err(format!(
            "landlock_create_ruleset failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(fd as c_int)
}

/// Adds one path-beneath rule. `allowed` is masked to the handled set
/// by the caller (the kernel rejects out-of-handled bits with EINVAL).
pub fn add_path_rule(ruleset_fd: c_int, parent_fd: c_int, allowed: u64) -> Result<(), String> {
    let attr = LandlockPathBeneathAttr {
        allowed_access: allowed,
        parent_fd,
    };
    // SAFETY: attr fully initialized; rule type is a plain constant.
    let ret = unsafe {
        syscall(
            SYS_LANDLOCK_ADD_RULE,
            ruleset_fd as c_long,
            LANDLOCK_RULE_PATH_BENEATH,
            &attr as *const LandlockPathBeneathAttr,
            0 as u32,
        )
    };
    if ret != 0 {
        return Err(format!(
            "landlock_add_rule failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Applies the ruleset to the calling thread. MUST run after
/// `PR_SET_NO_NEW_PRIVS` (EPERM otherwise) — in `pre_exec`, before
/// `exec`.
pub fn restrict_self(ruleset_fd: c_int) -> Result<(), String> {
    // SAFETY: plain fd.
    let ret = unsafe { syscall(SYS_LANDLOCK_RESTRICT_SELF, ruleset_fd as c_long) };
    if ret != 0 {
        return Err(format!(
            "landlock_restrict_self failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// pre_exec assembly (the D74 order, in the forked child)
// ---------------------------------------------------------------------------

/// Installs the child-side callback: rlimits → `PR_SET_NO_NEW_PRIVS`
/// → landlock `restrict_self` → close helper fds → (std execs next).
///
/// `pre_exec` is a safe std API with an async-signal-safety CONTRACT;
/// this closure satisfies it by construction (direct syscalls only:
/// no allocation, no locks, no libc locale state).
pub fn install_pre_exec(
    command: &mut std::process::Command,
    limits: &[(c_int, u64)],
    ruleset_fd: Option<c_int>,
    close_fds: &[c_int],
) {
    use std::os::unix::process::CommandExt as _;
    let limits = limits.to_vec();
    let close_fds = close_fds.to_vec();
    // SAFETY (pre_exec contract, edition 2024 marks the call unsafe):
    // the closure only performs direct syscalls — no allocation, no
    // locks, no signal-unsafe libc state — so it is
    // async-signal-safe between fork and exec.
    unsafe {
        command.pre_exec(move || {
            for (resource, limit) in &limits {
                set_limit(*resource, *limit).map_err(std::io::Error::other)?;
            }
            set_no_new_privs().map_err(std::io::Error::other)?;
            if let Some(fd) = ruleset_fd {
                restrict_self(fd).map_err(std::io::Error::other)?;
                close_fd(fd);
            }
            for fd in &close_fds {
                close_fd(*fd);
            }
            Ok(())
        });
    }
}

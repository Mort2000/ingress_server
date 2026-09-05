//! POSIX shared-memory regions for the zero-copy observation path
//! (P3-SHM, D63). THE ONLY MODULE WITH `unsafe` IN arena-server
//! (P4-CAPABILITY): the syscalls (`shm_open`/`shm_unlink`/`ftruncate`/
//! `mmap`/`munmap`/`close`, plus `getsockopt(SO_PEERCRED)` for the
//! connection-identity probe below) are declared via `extern "C"` — no
//! libc crate— and wrapped in safe types with the
//! invariants written below. Everything else in this crate stays safe.
//!
//! # Region layout
//!
//! ```text
//! [ metadata page: 4096 bytes ][ buffer A: capacity ][ buffer B: capacity ]
//! ```
//!
//! `MetadataBlock` is `#[repr(C)]` at offset 0 with compile-time size and
//! alignment assertions. `magic`/`layout_version`/`region_kind`/
//! `schema_version`/`capacity` are IMMUTABLE after creation (written once,
//! before the first publish); everything else is seqlock-protected.
//!
//! # Publish protocol (single writer = the server)
//!
//! 1. Copy the payload into the INACTIVE buffer (plain stores), set its
//!    `payload_len`.
//! 2. `seq.fetch_add(1, AcqRel)` — the value becomes ODD (write in
//!    progress). The acquire half of the RMW forbids the later metadata
//!    stores from being hoisted above it, so no observer can see the
//!    `active` flip (or any other metadata change) while `seq` still reads
//!    as the previous even value.
//! 3. Update `active`, `snapshot_version`, `current_round`,
//!    `round_start_ms`, `deadline_ms`.
//! 4. `seq.store(seq+1, Release)` — EVEN again (published). The release
//!    store makes every prior data and metadata store visible to any
//!    reader whose Acquire load observes that even value.
//!
//! Readers (see the SDK twin of this module) load `seq` with Acquire until
//! even, snapshot the metadata plus the ACTIVE payload slice, decode it in
//! place, and re-load `seq` with Acquire; on inequality they retry. Equal
//! values prove zero completed publications during the read span
//! (`seq` is monotonic), so the decoded generation was never written: a
//! buffer is only ever written while it is INACTIVE, and `active` cannot
//! point at the buffer being written while `seq` is even. A/B double
//! buffering therefore guarantees the currently-active buffer is never the
//! write target; a reader at most two generations stale is detected and
//! retried, never served a torn payload.
//!
//! # Invariants
//!
//! * Exactly one `ShmPublisher` per region per match: the single-writer
//!   half of the seqlock contract. Regions are created `O_EXCL` and
//!   unlinked on drop (and any same-name stale leftover is unlinked at
//!   creation), so a leftover file cannot join a live match.
//! * `base` points at `len` readable-writable `MAP_SHARED` bytes owned by
//!   this handle; `munmap` runs exactly once, in `Drop`, before
//!   `shm_unlink` (unmap-before-unlink: the mapping stays valid even if
//!   another process re-creates the name).
//! * The metadata page is page-aligned (mmap guarantees it), so the
//!   `AtomicU64` at its natural offset is properly aligned for lock-free
//!   cross-process access.
//! * Clients map the region PROT_READ only; the filesystem mode is 0600.

use std::ffi::{CString, c_char, c_int, c_long, c_uint, c_void};
use std::sync::atomic::{AtomicU64, Ordering, fence};

use arena_protocol::LAYOUT_VERSION;

unsafe extern "C" {
    fn shm_open(name: *const c_char, oflag: c_int, mode: c_uint) -> c_int;
    fn shm_unlink(name: *const c_char) -> c_int;
    fn ftruncate(fd: c_int, length: c_long) -> c_int;
    fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: c_long,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, length: usize) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn getsockopt(
        sockfd: c_int,
        level: c_int,
        optname: c_int,
        optval: *mut c_void,
        optlen: *mut u32,
    ) -> c_int;
}

// Linux x86_64 constants (this milestone targets Linux only; values are
// stable ABI facts, not libc-crate imports).
const O_RDWR: c_int = 0o2;
const O_CREAT: c_int = 0o100;
const O_EXCL: c_int = 0o200;
const O_RDONLY: c_int = 0o0;
const PROT_READ: c_int = 0x1;
const PROT_WRITE: c_int = 0x2;
const MAP_SHARED: c_int = 0x01;

// ---------------------------------------------------------------------------
// Connection identity (SO_PEERCRED)
// ---------------------------------------------------------------------------

const SOL_SOCKET: c_int = 1;
const SO_PEERCRED: c_int = 17;
/// `sizeof(struct ucred)` = pid_t + uid_t + gid_t on Linux x86_64.
const UCRED_SIZE: usize = 12;

/// The pid of the process that CONNECTED this accepted stream socket to
/// the listener (`getsockopt(SOL_SOCKET, SO_PEERCRED)`; credentials are
/// latched at connect time, so the answer survives the peer's later
/// disconnect). `None` when the syscall fails (not a stream socket, fd
/// closed, ...) — callers treat that as "identity unknown", never as a
/// guess.
///
/// WHY THIS EXISTS (D81 review fix): the tournament runner must PROVE
/// that the server's first ACCEPT (assigned ENL, D61) was the
/// ENL-scheduled client process. The kernel's `ucred.pid` is the only
/// std-adjacent way to identify the accepted peer — `/proc/<pid>/fd`
/// socket links expose SOCKFS inodes, which can never be matched
/// against the socket PATH's filesystem inode (two disjoint kernel
/// inode spaces; the pre-fix verification compared them and could
/// never succeed).
pub fn peer_pid(stream: &std::os::unix::net::UnixStream) -> Option<u32> {
    use std::os::unix::io::AsRawFd;
    #[repr(C)]
    struct Ucred {
        pid: i32,
        uid: u32,
        gid: u32,
    }
    const _: () = assert!(std::mem::size_of::<Ucred>() == UCRED_SIZE);
    let mut cred = Ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = UCRED_SIZE as u32;
    // SAFETY: `cred` is a valid, fully initialized repr(C) value of the
    // advertised `optlen` for the duration of the call; getsockopt only
    // writes into it. The fd is borrowed from the live `stream`.
    let result = unsafe {
        getsockopt(
            stream.as_raw_fd(),
            SOL_SOCKET,
            SO_PEERCRED,
            &mut cred as *mut Ucred as *mut c_void,
            &mut length,
        )
    };
    (result == 0 && length as usize == UCRED_SIZE && cred.pid > 0).then_some(cred.pid as u32)
}

/// Bytes of the metadata page preceding buffer A.
pub const METADATA_PAGE_BYTES: usize = 4096;
/// Public region capacity per buffer (8 MiB, D63).
pub const PUBLIC_CAPACITY: usize = 8 << 20;
/// Private region capacity per buffer (2 MiB per faction, D63).
pub const PRIVATE_CAPACITY: usize = 2 << 20;

/// `region_kind` value: the public snapshot region.
pub const REGION_KIND_PUBLIC: u32 = 0;
/// `region_kind` value: the ENL private snapshot region.
pub const REGION_KIND_ENL: u32 = 1;
/// `region_kind` value: the RES private snapshot region.
pub const REGION_KIND_RES: u32 = 2;

/// Region magic: `"IASN"` little-endian.
pub const MAGIC: u32 = u32::from_le_bytes(*b"IASN");

/// Seqlock-protected region header at offset 0 of the mapping.
/// Field offsets are pinned by the compile-time assertions below; the SDK
/// carries a byte-identical twin definition and an integration test pins
/// their agreement.
#[repr(C)]
pub struct MetadataBlock {
    /// Always [`MAGIC`]; immutable after creation.
    pub magic: u32,
    /// Always [`LAYOUT_VERSION`]; immutable after creation.
    pub layout_version: u32,
    /// One of `REGION_KIND_*`; immutable after creation.
    pub region_kind: u32,
    /// Public/Private snapshot schema version of the payload; immutable.
    pub schema_version: u32,
    /// Seqlock counter: even = published, odd = publication in progress.
    pub seq: AtomicU64,
    /// Generation counter of the active snapshot (monotonic).
    pub snapshot_version: u64,
    /// Round the active snapshot was taken for (0 = pre-match).
    pub current_round: u32,
    /// Which buffer is active: 0 = A, 1 = B.
    pub active: u32,
    /// Server-monotonic timing fields (informational; SDKs measure
    /// deadlines locally against their ROUND_START_SIGNAL receipt).
    pub round_start_ms: u64,
    pub deadline_ms: u64,
    /// Payload lengths of buffers A and B.
    pub payload_len_a: u32,
    pub payload_len_b: u32,
    /// Per-buffer capacity in bytes; immutable after creation.
    pub capacity: u32,
}

// Compile-time layout pins (D63): sizes, alignment, and the 8-byte
// alignment of the seqlock word for lock-free cross-process access.
const _: () = assert!(std::mem::size_of::<MetadataBlock>() == 72);
const _: () = assert!(std::mem::align_of::<MetadataBlock>() == 8);
const _: () = assert!(std::mem::offset_of!(MetadataBlock, seq) == 16);
const _: () = assert!(std::mem::offset_of!(MetadataBlock, snapshot_version) == 24);
const _: () = assert!(std::mem::offset_of!(MetadataBlock, capacity) == 64);
const _: () = assert!(METADATA_PAGE_BYTES >= std::mem::size_of::<MetadataBlock>());

/// Errors of the SHM layer.
#[derive(Debug)]
pub enum ShmError {
    Io(std::io::Error),
    /// Region name contains a NUL byte.
    BadName,
    /// The region exists but its immutable header does not match the
    /// expected magic / layout / kind / schema / capacity.
    HeaderMismatch {
        field: &'static str,
    },
    /// A payload exceeded the region capacity (an infrastructure abort —
    /// never a truncation; D63).
    PayloadTooLarge {
        len: usize,
        capacity: usize,
    },
    /// The reader retried the seqlock more than [`MAX_SEQLOCK_RETRIES`]
    /// times without seeing a stable generation.
    SeqlockBusy,
    /// A seqlock-consistent metadata block still carried an impossible
    /// value (D78: `payload_len` beyond the region capacity — never
    /// trusted, never used to build a slice).
    Corrupt {
        field: &'static str,
    },
    /// `snapshot_version` went backwards or the payload failed decode.
    InvalidSnapshot,
}

impl core::fmt::Display for ShmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ShmError::Io(e) => write!(f, "shared memory io error: {e}"),
            ShmError::BadName => write!(f, "region name contains a NUL byte"),
            ShmError::HeaderMismatch { field } => {
                write!(f, "shared memory header mismatch: {field}")
            }
            ShmError::PayloadTooLarge { len, capacity } => write!(
                f,
                "snapshot of {len} bytes exceeds region capacity {capacity}"
            ),
            ShmError::SeqlockBusy => write!(f, "seqlock never settled"),
            ShmError::Corrupt { field } => {
                write!(f, "region metadata corrupt: {field}")
            }
            ShmError::InvalidSnapshot => write!(f, "snapshot payload failed validation"),
        }
    }
}

impl std::error::Error for ShmError {}

impl From<std::io::Error> for ShmError {
    fn from(value: std::io::Error) -> Self {
        ShmError::Io(value)
    }
}

/// How many seqlock retries a reader attempts before failing.
pub const MAX_SEQLOCK_RETRIES: u32 = 64;

/// One coherent snapshot read out of a region: metadata plus the payload
/// COPIED out of the mapping (server-side reads are diagnostics; the SDK's
/// zero-copy path is the real consumer).
#[derive(Clone, Debug)]
pub struct SnapshotRead {
    pub snapshot_version: u64,
    pub current_round: u32,
    pub round_start_ms: u64,
    pub deadline_ms: u64,
    pub payload: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Publisher (server side, sole writer)
// ---------------------------------------------------------------------------

/// Create/write handle over one POSIX SHM region. Dropping it unmaps and
/// unlinks the name.
#[derive(Debug)]
pub struct ShmPublisher {
    name: CString,
    base: *mut u8,
    len: usize,
    capacity: usize,
    /// Monotonic generation counter this publisher has published.
    published: u64,
}

// The mapping is plain bytes owned solely by this handle; the AtomicU64
// inside participates in the documented cross-process protocol.
unsafe impl Send for ShmPublisher {}
unsafe impl Sync for ShmPublisher {}

impl ShmPublisher {
    /// Creates the region (0600) and writes the immutable header fields.
    /// A stale leftover of the same name is unlinked first.
    pub fn create(
        name: &str,
        region_kind: u32,
        schema_version: u32,
        capacity: usize,
    ) -> Result<Self, ShmError> {
        let cname = CString::new(name).map_err(|_| ShmError::BadName)?;
        let len = METADATA_PAGE_BYTES + 2 * capacity;
        // SAFETY: `cname` is a valid NUL-terminated C string; shm_open
        // only reads it. Flags/mode are plain integers.
        let fd = unsafe { shm_open(cname.as_ptr(), O_RDWR | O_CREAT | O_EXCL, 0o600) };
        let fd = if fd >= 0 {
            fd
        } else {
            let first = std::io::Error::last_os_error();
            if first.kind() == std::io::ErrorKind::AlreadyExists {
                // Stale leftover from a crashed run: unlink and retry once.
                // SAFETY: as above.
                unsafe { shm_unlink(cname.as_ptr()) };
                let retry = unsafe { shm_open(cname.as_ptr(), O_RDWR | O_CREAT | O_EXCL, 0o600) };
                if retry < 0 {
                    return Err(ShmError::Io(std::io::Error::last_os_error()));
                }
                retry
            } else {
                return Err(ShmError::Io(first));
            }
        };
        // Set the size BEFORE mapping; failure must still close the fd and
        // unlink the name so nothing leaks for the next run.
        // SAFETY: valid fd from shm_open; length fits isize.
        let truncate = unsafe { ftruncate(fd, len as c_long) };
        if truncate != 0 {
            let error = std::io::Error::last_os_error();
            // SAFETY: syscall on a valid fd / valid name.
            unsafe {
                close(fd);
                shm_unlink(cname.as_ptr());
            }
            return Err(ShmError::Io(error));
        }
        // SAFETY: fresh mapping request (addr = null), length matches the
        // ftruncate, valid fd, zero offset; MAP_SHARED + PROT_WRITE gives
        // us the documented single-writer mapping.
        let mapping = unsafe {
            mmap(
                std::ptr::null_mut(),
                len,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd,
                0,
            )
        };
        // SAFETY: the fd is no longer needed once the mapping exists.
        unsafe { close(fd) };
        if mapping as isize == -1 || mapping.is_null() {
            // SAFETY: valid name.
            unsafe { shm_unlink(cname.as_ptr()) };
            return Err(ShmError::Io(std::io::Error::last_os_error()));
        }
        let base = mapping as *mut u8;
        let publisher = ShmPublisher {
            name: cname,
            base,
            len,
            capacity,
            published: 0,
        };
        // Zero the metadata page, then write the immutable fields (plain
        // stores are fine: no reader exists before the first publish's
        // Release store makes even-seq visible).
        // SAFETY: base..base+len is our valid mapping; the metadata page is
        // METADATA_PAGE_BYTES >= size_of::<MetadataBlock>() bytes.
        unsafe {
            std::ptr::write_bytes(base, 0, METADATA_PAGE_BYTES);
            let metadata = &mut *(base as *mut MetadataBlock);
            metadata.magic = MAGIC;
            metadata.layout_version = LAYOUT_VERSION;
            metadata.region_kind = region_kind;
            metadata.schema_version = schema_version;
            metadata.capacity = capacity as u32;
            metadata.active = 0;
            metadata.seq = AtomicU64::new(0);
            metadata.snapshot_version = 0;
            metadata.current_round = 0;
            metadata.round_start_ms = 0;
            metadata.deadline_ms = 0;
            metadata.payload_len_a = 0;
            metadata.payload_len_b = 0;
        }
        Ok(publisher)
    }

    /// Publishes one generation (the seqlock protocol; see module docs).
    /// `snapshot_version` is assigned by the publisher (monotonic, +1 per
    /// publish); generation 0 must be published first so readers can pin
    /// the starting version.
    pub fn publish(
        &mut self,
        payload: &[u8],
        current_round: u32,
        round_start_ms: u64,
        deadline_ms: u64,
    ) -> Result<(), ShmError> {
        if payload.len() > self.capacity {
            return Err(ShmError::PayloadTooLarge {
                len: payload.len(),
                capacity: self.capacity,
            });
        }
        // SAFETY: metadata lives in our valid mapping; no other writer
        // exists (single-writer invariant).
        let metadata = unsafe { &mut *(self.base as *mut MetadataBlock) };
        let target = 1 - metadata.active; // the inactive buffer
        let target_offset = METADATA_PAGE_BYTES + target as usize * self.capacity;
        // 1. Copy the payload into the inactive buffer (plain stores).
        // SAFETY: target_offset + payload.len() stays inside the mapping
        // (capacity checked above).
        unsafe {
            std::ptr::copy_nonoverlapping(
                payload.as_ptr(),
                self.base.add(target_offset),
                payload.len(),
            );
            match target {
                0 => metadata.payload_len_a = payload.len() as u32,
                _ => metadata.payload_len_b = payload.len() as u32,
            }
        }
        self.published += 1;
        // 2. Odd transition: AcqRel so the metadata stores below cannot be
        // hoisted above it (module docs, step 2).
        let odd = metadata.seq.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        // 3. Metadata flip.
        metadata.active = target;
        metadata.snapshot_version = self.published;
        metadata.current_round = current_round;
        metadata.round_start_ms = round_start_ms;
        metadata.deadline_ms = deadline_ms;
        // 4. Even publish fence: every store above is visible to any
        //    reader that Acquire-loads this value.
        fence(Ordering::Release);
        metadata.seq.store(odd.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// The generation counter this publisher has published so far.
    pub fn published_generations(&self) -> u64 {
        self.published
    }

    /// Which buffer is currently active (0 = A, 1 = B) — for the
    /// never-write-the-active-buffer invariant test.
    pub fn active_buffer(&self) -> u32 {
        // SAFETY: valid mapping, read-only use.
        unsafe { (*(self.base as *const MetadataBlock)).active }
    }

    /// The seqlock counter's current value (diagnostics).
    pub fn seq_value(&self) -> u64 {
        // SAFETY: valid mapping, atomic read.
        unsafe {
            (*(self.base as *const MetadataBlock))
                .seq
                .load(Ordering::Relaxed)
        }
    }
}

impl Drop for ShmPublisher {
    fn drop(&mut self) {
        // Unmap BEFORE unlink: the mapping stays valid for the lifetime of
        // this handle even if another process re-creates the name.
        // SAFETY: base/len are exactly what mmap returned and nothing else
        // dereferences them after this.
        unsafe {
            munmap(self.base as *mut c_void, self.len);
            shm_unlink(self.name.as_ptr());
        }
    }
}

// ---------------------------------------------------------------------------
// Reader (server-side diagnostics; the SDK has the zero-copy twin)
// ---------------------------------------------------------------------------

/// The immutable header a reader must accept before trusting a region.
#[derive(Clone, Copy, Debug)]
pub struct RegionHeader {
    pub magic: u32,
    pub layout_version: u32,
    pub region_kind: u32,
    pub schema_version: u32,
    pub capacity: u32,
}

/// Maps an existing region read-only and reads one coherent generation
/// (Acquire double-check seqlock read; see module docs). The payload is
/// copied out and validated with the given root check.
pub fn read_snapshot(
    name: &str,
    expected: RegionHeader,
    validate_payload: &dyn Fn(&[u8]) -> bool,
) -> Result<SnapshotRead, ShmError> {
    let cname = CString::new(name).map_err(|_| ShmError::BadName)?;
    // SAFETY: valid C string; O_RDONLY open only.
    let fd = unsafe { shm_open(cname.as_ptr(), O_RDONLY, 0) };
    if fd < 0 {
        return Err(ShmError::Io(std::io::Error::last_os_error()));
    }
    // Map the metadata page first to learn the capacity, then remap the
    // whole region (the capacity field is immutable, so the two-step read
    // is race-free).
    // SAFETY: single page mapping of a valid fd.
    let page = unsafe {
        mmap(
            std::ptr::null_mut(),
            METADATA_PAGE_BYTES,
            PROT_READ,
            MAP_SHARED,
            fd,
            0,
        )
    };
    if page as isize == -1 || page.is_null() {
        let error = std::io::Error::last_os_error();
        // SAFETY: valid fd.
        unsafe { close(fd) };
        return Err(ShmError::Io(error));
    }
    // SAFETY: the page is a valid read-only mapping of a MetadataBlock
    // (size asserted <= page). MetadataBlock is not Copy (AtomicU64), so
    // the fields are read individually.
    let (magic, layout_version, region_kind, schema_version, capacity) = unsafe {
        let metadata = &*(page as *const MetadataBlock);
        (
            metadata.magic,
            metadata.layout_version,
            metadata.region_kind,
            metadata.schema_version,
            metadata.capacity,
        )
    };
    // SAFETY: unmap the temporary page.
    unsafe { munmap(page, METADATA_PAGE_BYTES) };

    for (field, ok) in [
        ("magic", magic == expected.magic),
        ("layout_version", layout_version == expected.layout_version),
        ("region_kind", region_kind == expected.region_kind),
        ("schema_version", schema_version == expected.schema_version),
        ("capacity", capacity == expected.capacity),
    ] {
        if !ok {
            // SAFETY: valid fd.
            unsafe { close(fd) };
            return Err(ShmError::HeaderMismatch { field });
        }
    }

    let capacity = capacity as usize;
    let len = METADATA_PAGE_BYTES + 2 * capacity;
    // SAFETY: full-region read-only mapping of the same fd.
    let mapping = unsafe { mmap(std::ptr::null_mut(), len, PROT_READ, MAP_SHARED, fd, 0) };
    // SAFETY: mapping no longer needs the fd.
    unsafe { close(fd) };
    if mapping as isize == -1 || mapping.is_null() {
        return Err(ShmError::Io(std::io::Error::last_os_error()));
    }
    let base = mapping as *const u8;
    // SAFETY: base holds `len` readable bytes for the scope of this
    // function; the borrow of the metadata ends before munmap.
    let result = (|| -> Result<SnapshotRead, ShmError> {
        let metadata = unsafe { &*(base as *const MetadataBlock) };
        for _attempt in 0..MAX_SEQLOCK_RETRIES {
            let first = metadata.seq.load(Ordering::Acquire);
            if first & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let snapshot_version = metadata.snapshot_version;
            let current_round = metadata.current_round;
            let active = metadata.active;
            let round_start_ms = metadata.round_start_ms;
            let deadline_ms = metadata.deadline_ms;
            let payload_len = if active == 0 {
                metadata.payload_len_a
            } else {
                metadata.payload_len_b
            } as usize;
            // D78: the length is SHM-DERIVED — bound it against the
            // immutable capacity before any slice is built from it
            // (defense in depth on top of the single-writer invariant;
            // an impossible length is reported, never trusted).
            if payload_len > capacity {
                return Err(ShmError::Corrupt {
                    field: "payload_len",
                });
            }
            let offset = METADATA_PAGE_BYTES + active as usize * capacity;
            // SAFETY: offset + payload_len <= region end by construction
            // (capacity-checked above; the writer only publishes
            // in-capacity payloads).
            let payload =
                unsafe { std::slice::from_raw_parts(base.add(offset), payload_len) }.to_vec();
            // D78: read-side Acquire fence between the payload snapshot
            // and the confirming seq load — the publisher's publish
            // protocol is Release-ordered, and x86-TSO made the missing
            // fence unobservable; this closes the formal gap for
            // weaker orderings and against compiler reordering.
            fence(Ordering::Acquire);
            if metadata.seq.load(Ordering::Acquire) != first {
                continue; // spanned a publication: retry
            }
            if !validate_payload(&payload) {
                return Err(ShmError::InvalidSnapshot);
            }
            return Ok(SnapshotRead {
                snapshot_version,
                current_round,
                round_start_ms,
                deadline_ms,
                payload,
            });
        }
        Err(ShmError::SeqlockBusy)
    })();
    // SAFETY: unmap exactly what we mapped, after all borrows ended.
    unsafe { munmap(mapping, len) };
    result
}

/// Builds the region name for one match (D63):
/// `/ingress-<match_id>-{public|enl|res}`.
pub fn region_names(match_id: &str) -> (String, String, String) {
    (
        format!("/ingress-{match_id}-public"),
        format!("/ingress-{match_id}-enl"),
        format!("/ingress-{match_id}-res"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_protocol::{PRIVATE_SNAPSHOT_VERSION, PUBLIC_SNAPSHOT_VERSION};
    use std::sync::Arc;

    fn unique_name(tag: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        format!("/ingress-test-{}-{}", std::process::id(), nanos)
            .replace("ingress-test", &format!("ingress-test-{tag}"))
    }

    fn public_header(capacity: u32) -> RegionHeader {
        RegionHeader {
            magic: MAGIC,
            layout_version: LAYOUT_VERSION,
            region_kind: REGION_KIND_PUBLIC,
            schema_version: PUBLIC_SNAPSHOT_VERSION,
            capacity,
        }
    }

    #[test]
    fn publish_then_read_round_trips_every_generation() {
        let name = unique_name("rt");
        let capacity = 64 * 1024;
        let mut publisher =
            ShmPublisher::create(&name, REGION_KIND_PUBLIC, PUBLIC_SNAPSHOT_VERSION, capacity)
                .expect("create");
        let header = public_header(capacity as u32);
        // Publish 5 generations with distinct payloads.
        for generation in 0..5u64 {
            let payload = vec![generation as u8; 64 + generation as usize];
            publisher
                .publish(
                    &payload,
                    generation as u32 + 1,
                    generation * 10,
                    generation * 10 + 5,
                )
                .expect("publish");
            let read = read_snapshot(&name, header, &|p| !p.is_empty()).expect("read");
            assert_eq!(read.snapshot_version, generation + 1);
            assert_eq!(read.current_round, generation as u32 + 1);
            assert_eq!(read.payload.len(), 64 + generation as usize);
            assert!(read.payload.iter().all(|&b| b == generation as u8));
            assert_eq!(publisher.seq_value() % 2, 0, "published state is even");
        }
        assert_eq!(publisher.published_generations(), 5);
    }

    #[test]
    fn active_buffer_alternates_and_is_never_the_write_target() {
        let name = unique_name("ab");
        let mut publisher =
            ShmPublisher::create(&name, REGION_KIND_ENL, PRIVATE_SNAPSHOT_VERSION, 4096)
                .expect("create");
        let mut seen = Vec::new();
        for generation in 0..6u64 {
            let before = publisher.active_buffer();
            seen.push(before);
            publisher
                .publish(&[generation as u8; 16], 0, 0, 0)
                .expect("publish");
            assert_eq!(
                publisher.active_buffer(),
                1 - before,
                "roles swap every generation"
            );
        }
        // Strict alternation 0,1,0,1,...
        assert_eq!(seen, vec![0, 1, 0, 1, 0, 1]);
    }

    #[test]
    fn oversize_payload_is_an_error_never_a_truncation() {
        let name = unique_name("big");
        let mut publisher =
            ShmPublisher::create(&name, REGION_KIND_RES, PRIVATE_SNAPSHOT_VERSION, 128)
                .expect("create");
        assert!(matches!(
            publisher.publish(&[0u8; 129], 1, 0, 0),
            Err(ShmError::PayloadTooLarge {
                len: 129,
                capacity: 128
            })
        ));
        // The region still holds the previous (generation 0 unpublished)
        // state: no partial write landed.
        assert_eq!(publisher.published_generations(), 0);
    }

    #[test]
    fn reader_rejects_mismatched_headers_and_missing_regions() {
        let name = unique_name("hdr");
        let capacity = 4096u32;
        let mut publisher = ShmPublisher::create(
            &name,
            REGION_KIND_PUBLIC,
            PUBLIC_SNAPSHOT_VERSION,
            capacity as usize,
        )
        .expect("create");
        publisher.publish(&[1, 2, 3], 0, 0, 0).expect("publish");

        let good = public_header(capacity);
        assert!(read_snapshot(&name, good, &|_| true).is_ok());
        // Wrong kind.
        let mut wrong_kind = good;
        wrong_kind.region_kind = REGION_KIND_ENL;
        assert!(matches!(
            read_snapshot(&name, wrong_kind, &|_| true),
            Err(ShmError::HeaderMismatch {
                field: "region_kind"
            })
        ));
        // Wrong layout version.
        let mut wrong_layout = good;
        wrong_layout.layout_version = LAYOUT_VERSION + 1;
        assert!(matches!(
            read_snapshot(&name, wrong_layout, &|_| true),
            Err(ShmError::HeaderMismatch {
                field: "layout_version"
            })
        ));
        // Wrong capacity.
        let mut wrong_capacity = good;
        wrong_capacity.capacity = capacity + 1;
        assert!(matches!(
            read_snapshot(&name, wrong_capacity, &|_| true),
            Err(ShmError::HeaderMismatch { field: "capacity" })
        ));
        // Payload validation failure.
        assert!(matches!(
            read_snapshot(&name, good, &|_| false),
            Err(ShmError::InvalidSnapshot)
        ));
        // A region that does not exist errors (not panics).
        assert!(matches!(
            read_snapshot("/ingress-test-does-not-exist-xyz", good, &|_| true),
            Err(ShmError::Io(_))
        ));
    }

    /// D78 (M8): an SHM-DERIVED `payload_len` beyond the immutable
    /// capacity — written into the live region from OUTSIDE the
    /// mapping — is refused as `Corrupt`, never trusted for a slice;
    /// repairing the field restores service. The read path also
    /// carries the D78 read-side Acquire fence (see read_snapshot).
    #[test]
    fn oversize_shm_derived_payload_len_is_refused_as_corrupt() {
        use std::os::unix::fs::FileExt;
        let name = unique_name("corrupt");
        let capacity = 512usize;
        let mut publisher =
            ShmPublisher::create(&name, REGION_KIND_PUBLIC, PUBLIC_SNAPSHOT_VERSION, capacity)
                .expect("create");
        publisher.publish(&[7u8; 16], 1, 0, 0).expect("publish");
        let header = public_header(capacity as u32);
        assert!(read_snapshot(&name, header, &|_| true).is_ok());

        // One publish => active buffer is B; corrupt its length field
        // through the raw region file (the mapping is MAP_SHARED, so
        // the write is immediately visible).
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(format!("/dev/shm{name}"))
            .expect("raw region file");
        let offset = std::mem::offset_of!(MetadataBlock, payload_len_b);
        file.write_all_at(&((capacity as u32 + 1).to_le_bytes()), offset as u64)
            .expect("corrupt");
        match read_snapshot(&name, header, &|_| true) {
            Err(ShmError::Corrupt {
                field: "payload_len",
            }) => {}
            other => panic!("expected Corrupt, got {other:?}"),
        }
        // Repair: the healthy generation serves again.
        file.write_all_at(&(16u32.to_le_bytes()), offset as u64)
            .expect("repair");
        let read = read_snapshot(&name, header, &|_| true).expect("repaired");
        assert_eq!(read.payload.len(), 16);
    }

    #[test]
    fn drop_unlinks_the_region() {
        let name = unique_name("drop");
        let header;
        {
            let mut publisher =
                ShmPublisher::create(&name, REGION_KIND_PUBLIC, PUBLIC_SNAPSHOT_VERSION, 512)
                    .expect("create");
            publisher.publish(&[9], 0, 0, 0).expect("publish");
            header = public_header(512);
            assert!(read_snapshot(&name, header, &|_| true).is_ok());
        }
        match read_snapshot(&name, header, &|_| true) {
            Err(ShmError::Io(_)) => {}
            other => panic!("the name is gone after drop, got {other:?}"),
        }
    }

    #[test]
    fn stale_leftover_is_replaced_at_creation() {
        let name = unique_name("stale");
        let orphan = ShmPublisher::create(&name, REGION_KIND_PUBLIC, PUBLIC_SNAPSHOT_VERSION, 512)
            .expect("orphan");
        drop(orphan);
        // Simulate a crash leftover: create without dropping.
        let leftover =
            ShmPublisher::create(&name, REGION_KIND_PUBLIC, PUBLIC_SNAPSHOT_VERSION, 512)
                .expect("leftover");
        std::mem::forget(leftover);
        // A fresh publisher takes the name over.
        let mut fresh =
            ShmPublisher::create(&name, REGION_KIND_PUBLIC, PUBLIC_SNAPSHOT_VERSION, 512)
                .expect("stale name replaced");
        fresh.publish(&[1], 1, 0, 0).expect("publish");
        assert_eq!(fresh.seq_value(), 2);
    }

    /// Concurrent publish/read: the reader never observes a torn payload —
    /// every read either decodes a fully consistent generation or retries;
    /// a torn copy would fail the self-consistency check and panic.
    #[test]
    fn concurrent_publish_read_never_tears() {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
        let name = unique_name("conc");
        let capacity = 256 * 1024;
        let stop = Arc::new(AtomicBool::new(false));
        let max_seen = Arc::new(AtomicU64::new(0));
        let header = public_header(capacity as u32);

        let mut publisher =
            ShmPublisher::create(&name, REGION_KIND_PUBLIC, PUBLIC_SNAPSHOT_VERSION, capacity)
                .expect("create");

        // Payload grammar: [generation u32 LE][bytes where byte i ==
        // (generation ^ i) & 0xFF]. A torn copy mixing two generations
        // fails the recompute and MUST be caught as InvalidSnapshot.
        let payload_for = |generation: u32| -> Vec<u8> {
            let mut bytes = generation.to_le_bytes().to_vec();
            bytes.extend((0..100_000u32).map(|i| (generation ^ i) as u8));
            bytes
        };
        const PAYLOAD_LEN: usize = 4 + 100_000;
        let self_consistent = |bytes: &[u8]| -> bool {
            if bytes.len() != PAYLOAD_LEN {
                return false;
            }
            let generation = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            bytes[4..]
                .iter()
                .enumerate()
                .all(|(i, &b)| b == (generation ^ i as u32) as u8)
        };
        publisher.publish(&payload_for(0), 0, 0, 0).expect("gen0");

        let reader_name = name.clone();
        let stop_reader = Arc::clone(&stop);
        let seen_reader = Arc::clone(&max_seen);
        let reader = std::thread::spawn(move || {
            let mut observations = 0u64;
            while !stop_reader.load(AtomicOrdering::Relaxed) {
                match read_snapshot(&reader_name, header, &self_consistent) {
                    Ok(read) => {
                        let previous = seen_reader.load(AtomicOrdering::Relaxed);
                        assert!(
                            read.snapshot_version >= previous,
                            "snapshot_version went backwards: {} < {previous}",
                            read.snapshot_version
                        );
                        seen_reader.store(read.snapshot_version, AtomicOrdering::Relaxed);
                        observations += 1;
                    }
                    Err(ShmError::SeqlockBusy) => {}
                    Err(ShmError::InvalidSnapshot) => {
                        panic!("torn payload observed across a stable seqlock read");
                    }
                    Err(other) => panic!("unexpected shm error: {other:?}"),
                }
            }
            observations
        });

        for generation in 1..=200u32 {
            publisher
                .publish(&payload_for(generation), generation, 0, 0)
                .expect("publish");
        }
        stop.store(true, AtomicOrdering::Relaxed);
        let observations = reader.join().expect("reader thread");
        assert!(observations > 0, "the reader observed at least once");
        assert!(
            max_seen.load(AtomicOrdering::Relaxed) <= 201,
            "no future generation leaked"
        );
        // The final generation is readable and self-consistent.
        let read = read_snapshot(&name, header, &self_consistent).expect("final read");
        assert_eq!(read.snapshot_version, 201);
        assert!(self_consistent(&read.payload));
    }
}

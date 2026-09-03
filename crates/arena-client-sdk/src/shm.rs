//! Read-only POSIX SHM mappings for SDK clients (P3-SHM, D63). THE ONLY
//! MODULE WITH `unsafe` IN arena-client-sdk (P4-CAPABILITY): same syscall
//! surface as the server's twin (`shm_open`/`mmap`/`munmap`/`close` via
//! `extern "C"`, no libc crate), read-only mappings, and the Acquire
//! double-check seqlock read. Participants never see raw mappings — the
//! safe wrappers below are the only entry points.
//!
//! The `MetadataBlock` layout here is byte-identical to the server's
//! (compile-time pins; an integration test reads regions the server
//! published to prove the two definitions agree).
//!
//! # Lending invariant
//!
//! [`ShmReader::read`] hands out `&'a [u8]` over the ACTIVE buffer after
//! the seqlock double-check proves no publication spanned the read. The
//! server (trusted infrastructure in v0.1 — P4 isolates untrusted
//! CLIENTS, not the server) writes a buffer only while it is INACTIVE, so
//! a lent generation becomes write-eligible only after two further
//! publications; the `snapshot_version >= last_seen` guard refuses stale
//! lends. A compliant client that stops reading at ROUND_CLOSED never
//! races a publication at all.

use std::ffi::{CString, c_char, c_int, c_long, c_uint, c_void};
use std::sync::atomic::{Ordering, fence};

use arena_protocol::LAYOUT_VERSION;

unsafe extern "C" {
    fn shm_open(name: *const c_char, oflag: c_int, mode: c_uint) -> c_int;
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
}

const O_RDONLY: c_int = 0o0;
const PROT_READ: c_int = 0x1;
const MAP_SHARED: c_int = 0x01;

/// Bytes of the metadata page (must match the server twin).
pub const METADATA_PAGE_BYTES: usize = 4096;
/// Public region capacity (must match the server twin).
pub const PUBLIC_CAPACITY: usize = 8 << 20;
/// Private region capacity per faction (must match the server twin).
pub const PRIVATE_CAPACITY: usize = 2 << 20;

/// Region magic (must match the server twin).
pub const MAGIC: u32 = u32::from_le_bytes(*b"IASN");

/// `region_kind` values (must match the server twin).
pub const REGION_KIND_PUBLIC: u32 = 0;
pub const REGION_KIND_ENL: u32 = 1;
pub const REGION_KIND_RES: u32 = 2;

/// The seqlock header, byte-identical to the server's definition.
#[repr(C)]
pub struct MetadataBlock {
    pub magic: u32,
    pub layout_version: u32,
    pub region_kind: u32,
    pub schema_version: u32,
    pub seq: std::sync::atomic::AtomicU64,
    pub snapshot_version: u64,
    pub current_round: u32,
    pub active: u32,
    pub round_start_ms: u64,
    pub deadline_ms: u64,
    pub payload_len_a: u32,
    pub payload_len_b: u32,
    pub capacity: u32,
}

const _: () = assert!(std::mem::size_of::<MetadataBlock>() == 72);
const _: () = assert!(std::mem::align_of::<MetadataBlock>() == 8);
const _: () = assert!(std::mem::offset_of!(MetadataBlock, seq) == 16);
const _: () = assert!(METADATA_PAGE_BYTES >= std::mem::size_of::<MetadataBlock>());

/// Maximum seqlock retries before giving up (must be prompt: the SDK is
/// deadline-bound).
pub const MAX_SEQLOCK_RETRIES: u32 = 64;

/// SDK shared-memory errors.
#[derive(Debug)]
pub enum ShmError {
    Io(std::io::Error),
    BadName,
    /// Immutable header mismatch (magic/layout/kind/schema/capacity).
    HeaderMismatch {
        field: &'static str,
    },
    /// The seqlock never settled.
    SeqlockBusy,
    /// A seqlock-consistent metadata block still carried an impossible
    /// value (D78: `payload_len` beyond the region capacity — never
    /// trusted, never used to build a slice).
    Corrupt {
        field: &'static str,
    },
    /// `snapshot_version` moved backwards.
    StaleGeneration {
        seen: u64,
        offered: u64,
    },
}

impl core::fmt::Display for ShmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ShmError::Io(e) => write!(f, "shared memory io error: {e}"),
            ShmError::BadName => write!(f, "region name contains a NUL byte"),
            ShmError::HeaderMismatch { field } => {
                write!(f, "shared memory header mismatch: {field}")
            }
            ShmError::SeqlockBusy => write!(f, "seqlock never settled"),
            ShmError::Corrupt { field } => {
                write!(f, "region metadata corrupt: {field}")
            }
            ShmError::StaleGeneration { seen, offered } => {
                write!(f, "snapshot_version moved backwards: {offered} < {seen}")
            }
        }
    }
}

impl std::error::Error for ShmError {}

/// The immutable header a reader pins at open time.
#[derive(Clone, Copy, Debug)]
pub struct RegionHeader {
    pub magic: u32,
    pub layout_version: u32,
    pub region_kind: u32,
    pub schema_version: u32,
    pub capacity: u32,
}

impl RegionHeader {
    /// Expected header of the match's public region.
    pub fn public() -> Self {
        RegionHeader {
            magic: MAGIC,
            layout_version: LAYOUT_VERSION,
            region_kind: REGION_KIND_PUBLIC,
            schema_version: arena_protocol::PUBLIC_SNAPSHOT_VERSION,
            capacity: PUBLIC_CAPACITY as u32,
        }
    }

    /// Expected header of one faction's private region.
    pub fn private(region_kind: u32) -> Self {
        RegionHeader {
            magic: MAGIC,
            layout_version: LAYOUT_VERSION,
            region_kind,
            schema_version: arena_protocol::PRIVATE_SNAPSHOT_VERSION,
            capacity: PRIVATE_CAPACITY as u32,
        }
    }
}

/// One stable (seqlock-verified) generation lent to the caller. The
/// payload borrows the reader, so views cannot outlive the session that
/// owns it (ADR-0006).
pub struct StableSnapshot<'a> {
    pub snapshot_version: u64,
    pub current_round: u32,
    pub round_start_ms: u64,
    pub deadline_ms: u64,
    pub payload: &'a [u8],
}

/// Read-only handle over one region. Opened after SERVER_HELLO carries the
/// `match_id` (region names are derived from it, D63).
#[derive(Debug)]
pub struct ShmReader {
    name: CString,
    base: *const u8,
    len: usize,
    capacity: usize,
    header: RegionHeader,
    /// Last generation this reader lent (monotonic guard). Atomic so a
    /// shared `&ShmReader` can read — views borrow the session, and
    /// scoped worker threads may read concurrently (ADR-0006).
    last_seen: std::sync::atomic::AtomicU64,
}

// The mapping is read-only plain bytes; the only writer is the server.
unsafe impl Send for ShmReader {}
unsafe impl Sync for ShmReader {}

impl ShmReader {
    /// Maps an existing region read-only and verifies the immutable header.
    pub fn open(name: &str, expected: RegionHeader) -> Result<Self, ShmError> {
        let cname = CString::new(name).map_err(|_| ShmError::BadName)?;
        // SAFETY: valid C string; read-only open.
        let fd = unsafe { shm_open(cname.as_ptr(), O_RDONLY, 0) };
        if fd < 0 {
            return Err(ShmError::Io(std::io::Error::last_os_error()));
        }
        // Map the metadata page first; the capacity field then tells us the
        // full region length (immutable, so no race).
        // SAFETY: one page of a valid fd.
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
        // SAFETY: readable page over a MetadataBlock; fields read
        // individually (not Copy).
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
        // SAFETY: unmap the probe page.
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
        // SAFETY: full read-only mapping of the same fd.
        let mapping = unsafe { mmap(std::ptr::null_mut(), len, PROT_READ, MAP_SHARED, fd, 0) };
        // SAFETY: mapping no longer needs the fd.
        unsafe { close(fd) };
        if mapping as isize == -1 || mapping.is_null() {
            return Err(ShmError::Io(std::io::Error::last_os_error()));
        }
        Ok(ShmReader {
            name: cname,
            base: mapping as *const u8,
            len,
            capacity,
            header: expected,
            last_seen: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// The seqlock read: Acquire-load until even, snapshot metadata plus
    /// the active payload slice, re-load and compare; retry on inequality
    /// (at most [`MAX_SEQLOCK_RETRIES`]). Refuses generations older than
    /// the last one this reader lent. Takes `&self` — the slice borrows
    /// the reader, so views cannot outlive the session that owns it.
    pub fn read(&self) -> Result<StableSnapshot<'_>, ShmError> {
        // SAFETY: base..base+len is our valid read-only mapping; the
        // metadata borrow ends before returning.
        let metadata = unsafe { &*(self.base as *const MetadataBlock) };
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
            // immutable capacity BEFORE the confirming load and before
            // any slice is lent from it (defense in depth on top of the
            // single-writer invariant).
            if payload_len > self.capacity {
                return Err(ShmError::Corrupt {
                    field: "payload_len",
                });
            }
            let offset = METADATA_PAGE_BYTES + active as usize * self.capacity;
            // D78: read-side Acquire fence between the metadata/payload
            // snapshot and the confirming seq load (the publisher's
            // publish is Release-ordered; x86-TSO made the fence
            // unobservable — it closes the formal gap and pins the
            // snapshot against compiler reordering).
            fence(Ordering::Acquire);
            let second = metadata.seq.load(Ordering::Acquire);
            if second != first {
                continue;
            }
            let seen = self.last_seen.load(std::sync::atomic::Ordering::Acquire);
            if snapshot_version < seen {
                return Err(ShmError::StaleGeneration {
                    seen,
                    offered: snapshot_version,
                });
            }
            self.last_seen
                .store(snapshot_version, std::sync::atomic::Ordering::Release);
            // SAFETY: offset + payload_len stays inside the mapping (the
            // server never publishes over-capacity payloads); the slice
            // borrows self, so it cannot outlive the mapping.
            let payload = unsafe { std::slice::from_raw_parts(self.base.add(offset), payload_len) };
            return Ok(StableSnapshot {
                snapshot_version,
                current_round,
                round_start_ms,
                deadline_ms,
                payload,
            });
        }
        Err(ShmError::SeqlockBusy)
    }

    /// Header this reader pinned (diagnostics).
    pub fn pinned_header(&self) -> RegionHeader {
        self.header
    }
}

impl Drop for ShmReader {
    fn drop(&mut self) {
        // SAFETY: unmap exactly what was mapped, after every borrow ended
        // (Drop only runs with no outstanding references).
        unsafe {
            munmap(self.base as *mut c_void, self.len);
            let _ = self.name;
        }
    }
}

/// Region names for one match (twin of the server helper, D63):
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

    #[test]
    fn layout_twin_matches_the_server_module() {
        // The compile-time pins above are the contract; assert the twin
        // facts explicitly so a drift is a test failure, not a review
        // note.
        assert_eq!(std::mem::size_of::<MetadataBlock>(), 72);
        assert_eq!(METADATA_PAGE_BYTES, 4096);
        assert_eq!(PUBLIC_CAPACITY, 8 << 20);
        assert_eq!(PRIVATE_CAPACITY, 2 << 20);
        assert_eq!(MAGIC, u32::from_le_bytes(*b"IASN"));
    }

    #[test]
    fn opening_a_missing_region_errors_without_panicking() {
        let error = ShmReader::open("/ingress-sdk-test-missing-region", RegionHeader::public());
        assert!(matches!(error, Err(ShmError::Io(_))));
    }
}

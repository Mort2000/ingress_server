//! arena-protocol: the M7 wire contract (P3-RPC, P3-SHM; ADR-0008/0009).
//!
//! Three committed FlatBuffers schemas (`schemas/*.fbs`) with their
//! generated Rust committed under `src/generated/` (D60: builds never
//! require planus). This crate adds, on top of the raw bindings:
//!
//! * [`frame`] — the u32 big-endian length-prefixed framing with the
//!   legal-maximum check performed BEFORE any payload allocation and a
//!   poisoning `OverLimit` result (P3-RPC, D62).
//! * [`decode`] — eager full-field decoding of one envelope buffer into
//!   owned types plus conversion to arena-core command types. planus
//!   validates lazily; converting the root `*Ref` into the owned type
//!   walks every field of the matched variant, which is our Verifier gate:
//!   any corrupt, truncated, or junk buffer yields an error, never a panic.
//! * [`build`] — marshalling of `arena-core` observations/maps into
//!   Public/Private snapshot buffers (pure field mapping, zero rule math).
//! * [`versions`] — the version triple checked at handshake (D61).
//!
//! The crate contains no rule semantics: authoritative math stays in
//! arena-core, and the server drives arena-core's Resolver instead of
//! re-implementing anything here.
//!
//! Unsafe policy: everything hand-written here is safe; the crate uses
//! `deny(unsafe_code)` (not `forbid`) solely so the generated planus
//! modules — planus-runtime table writers contain audited `unsafe` — can
//! carry a scoped `allow`. No hand-written code may add `unsafe`.

#![deny(unsafe_code)]

pub mod build;
pub mod decode;
pub mod frame;
pub mod versions;

/// Generated planus bindings (committed; see `scripts/regen-protocol.sh`).
/// The scoped `allow(unsafe_code)` covers only planus-runtime glue inside
/// the generated files; see the crate-level unsafe-policy note.
pub mod generated {
    #[allow(unsafe_code)]
    pub mod protocol;
    #[allow(unsafe_code)]
    pub mod snapshot_private;
    #[allow(unsafe_code)]
    pub mod snapshot_public;

    /// The `ingress.protocol` namespace.
    pub mod proto {
        pub use super::protocol::ingress::protocol::*;
    }

    /// The `ingress.snapshot.public` namespace.
    pub mod snap_public {
        pub use super::snapshot_public::ingress::snapshot::public::*;
    }

    /// The `ingress.snapshot.private` namespace.
    pub mod snap_private {
        pub use super::snapshot_private::ingress::snapshot::private::*;
    }
}

pub use decode::{Envelope, ProtocolError};
pub use frame::{FramePoll, FrameReader, MAX_FRAME_PAYLOAD_BYTES, encode_frame};
pub use versions::{
    LAYOUT_VERSION, PRIVATE_SNAPSHOT_VERSION, PROTOCOL_VERSION, PUBLIC_SNAPSHOT_VERSION,
};

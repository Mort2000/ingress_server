//! Version constants (D61). The CLIENT_HELLO version triple must match all
//! three EXACTLY — no forward/backward/capability negotiation; a mismatch
//! aborts startup as an unscored infrastructure error before any map
//! publish (P3-RPC).

/// Wire protocol version (IARP `PROTOCOL_VERSION` placeholder retired: the
/// server now stamps this constant and `arena-replay` records it). The IARP
/// header constant `arena_replay::format::PROTOCOL_VERSION` must equal this
/// value; arena-replay's `protocol_version_tie` test (dev-dep on this
/// crate) fails the build on drift.
pub const PROTOCOL_VERSION: u32 = 1;

/// Public snapshot schema version (public SHM root `schema_version`).
pub const PUBLIC_SNAPSHOT_VERSION: u32 = 1;

/// Private snapshot schema version (private SHM root `schema_version`).
pub const PRIVATE_SNAPSHOT_VERSION: u32 = 1;

/// SHM region layout version (metadata page layout, D63).
pub const LAYOUT_VERSION: u32 = 1;

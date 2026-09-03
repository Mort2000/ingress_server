//! arena-replay: canonical state encoding + BLAKE3-256 state hash, the IARP
//! replay file format, the match recorder, and the deterministic replay
//! engine (M5; P1-HASH, P1-REPLAY, P5-REPLAY, ADR-0010). Rule semantics
//! live in `arena-core`; this crate never re-implements them — it records,
//! verifies, and replays.

#![forbid(unsafe_code)]

pub mod analysis;
pub mod canon;
pub mod format;
pub mod recorder;
pub mod replay;

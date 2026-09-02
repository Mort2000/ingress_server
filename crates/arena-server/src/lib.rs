//! arena-server: the M7 external runtime (P3). One process per match:
//! an accept loop, one reader thread per client connection, and a single
//! driver thread that owns the authoritative `GameState` and settles every
//! round through arena-core's `Resolver` — the server NEVER re-implements
//! rules. Observations reach the clients through zero-copy POSIX SHM
//! snapshots (seqlock + A/B double buffering, D63); commands travel over
//! one persistent ordered UDS stream per client (ADR-0008).
//!
//! Unsafe policy: the crate root uses `deny(unsafe_code)` (not `forbid`)
//! solely so the ONE sanctioned module — `shm.rs`, the POSIX SHM syscalls
//! via `extern "C"` with no libc crate — can carry a scoped, loudly
//! documented `allow` (P4-CAPABILITY). Every other module is unsafe-free;
//! a stray `unsafe` anywhere else still fails the build, and any new
//! `allow(unsafe_code)` is review-visible.

#![deny(unsafe_code)]
// The sole sanctioned unsafe module in this crate (see above).
#[allow(unsafe_code)]
pub mod shm;

pub mod conn;
pub mod driver;
pub mod journal;
pub mod outcomes;
pub mod testutil;

pub use driver::{MatchReport, MatchSetup, serve_match};
pub use outcomes::{FactionMetrics, MatchOutcome};

/// A fast small-window config for tests (spawn 400 ms, decision 40 ms,
/// bucket 5 ms) with a chosen round count — keeps live-match wall-clock
/// modest while exercising the same protocol.
pub fn fast_config(total_rounds: u32) -> arena_core::config::GameConfig {
    let mut cfg = arena_core::config::GameConfig::default();
    cfg.total_rounds = total_rounds;
    cfg.initial_spawn_window_ms = 400;
    cfg.decision_window_ms = 40;
    cfg
}

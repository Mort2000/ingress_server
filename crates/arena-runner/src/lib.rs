//! Batch self-play runner (P2.5, M6): builds the frozen experiment
//! schedule, executes matches in parallel over scoped worker threads,
//! folds per-match metrics incrementally from round reports, and writes
//! hand-rolled std-only text artifacts (`manifest.txt`, `matches.tsv`,
//! `summary.txt`). Wall-clock time is recorded as METADATA only — it never
//! enters a policy decision, a seed, or a result.

#![forbid(unsafe_code)]

pub mod batch;
pub mod conflict;
pub mod drops;
pub mod metrics;
pub mod policy;
pub mod report;
pub mod run;
pub mod schedule;
pub mod tournament;

pub use batch::{BatchConfig, MatchDescriptor, Matchup};
pub use metrics::{FactionMatchStats, MatchRecord, MatchStats, Outcome};
pub use policy::BucketPolicy;
pub use report::{BatchSummary, MatchupSummary, SnowballCheckpoint};
pub use run::{BatchError, BatchRun, run_batch, run_match};

/// `LE-u64(blake3(label || payload))` — the frozen seed-derivation helper
/// (D58, the M5 gate pattern): one distinct stream per domain label.
pub fn derive_seed(label: &[u8], payload: &[u8]) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label);
    hasher.update(payload);
    let hash = hasher.finalize();
    let mut words = [0u8; 8];
    words.copy_from_slice(&hash.as_bytes()[0..8]);
    u64::from_le_bytes(words)
}

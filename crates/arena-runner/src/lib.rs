//! External-client tournament scheduling, execution, replay archives, and ratings.
#![forbid(unsafe_code)]

pub mod schedule;
pub mod tournament;

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

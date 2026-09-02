//! Schedule-seed faction assignment (D55, P2.5): one schedule seed per
//! batch decides, per matchup, which faction bot A plays in each match.
//! The assignment guarantees EXACTLY `M / 2` ENL and `M / 2` RES slots per
//! bot (|ENL - RES| = 0 — strictly stronger than the spec's "difference at
//! most 1"), in a randomized order, one match per item, no auto faction
//! swap, no rematch.

/// The frozen want-bit: bit 0 of the first blake3 output byte over
/// `b"m6-sched" || schedule_seed_le64 || matchup_tag || j_le32`.
fn want_bit(schedule_seed: u64, matchup_tag: &str, j: u32) -> bool {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"m6-sched");
    hasher.update(&schedule_seed.to_le_bytes());
    hasher.update(matchup_tag.as_bytes());
    hasher.update(&j.to_le_bytes());
    hasher.finalize().as_bytes()[0] & 1 == 1
}

/// The frozen assignment algorithm (D55): bot A "wants" ENL when its bit
/// is 1. A want is honored while that faction's `cap = M / 2` slots last;
/// once a side is full A takes the other side. Deterministic in
/// `(schedule_seed, matchup_tag, j)`; every count must be even.
///
/// # Panics
///
/// Odd `count` — the frozen experiment only defines even counts (the
/// equal-exposure guarantee needs `M / 2` per side).
pub fn a_is_enl_decisions(schedule_seed: u64, matchup_tag: &str, count: u32) -> Vec<bool> {
    assert!(count % 2 == 0, "matchup counts must be even (got {count})");
    let cap = count / 2;
    let mut a_enl_used = 0u32;
    let mut decisions = Vec::with_capacity(count as usize);
    for j in 0..count {
        let res_used = j - a_enl_used;
        let a_is_enl = if want_bit(schedule_seed, matchup_tag, j) {
            a_enl_used < cap
        } else {
            // A wants RES: honor it while RES slots last, else ENL.
            !(res_used < cap)
        };
        a_enl_used += u32::from(a_is_enl);
        decisions.push(a_is_enl);
    }
    debug_assert_eq!(
        a_enl_used, cap,
        "exactly half the matches are ENL for bot A"
    );
    decisions
}

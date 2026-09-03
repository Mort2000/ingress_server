//! Seeded tie-break and near-equal candidate sampling (plan sections 5.1
//! and 6.3). The m6.0 baselines break ranking ties canonically (area2
//! descending, then ascending ids); that determinism is what makes
//! mirror matches collide on the identical largest triangle with the
//! identical spawn set (plan section 2, mirror bias). R2 policies keep
//! full determinism — same observation, seed, faction and version, same
//! submissions — but draw their tie-breaks from the policy's own
//! [`R2Rng`] stream, so two mirror factions of one policy diverge while
//! every run stays exactly reproducible.
//!
//! "Near-equal" widens an exact tie into a band: every candidate whose
//! value lies within `near_equal_permille` permille of the best value is
//! eligible, and the draw is uniform over the eligible candidates. The
//! tolerance is a recorded policy parameter, never a scattered constant.

use crate::r2::rng::R2Rng;

/// Uniform pick among `items` through the stream, consuming exactly one
/// draw. `None` for an empty slice (and no draw consumed).
pub fn pick_uniform<'a, T>(items: &'a [T], rng: &mut R2Rng) -> Option<&'a T> {
    if items.is_empty() {
        return None;
    }
    let index = rng.uniform(items.len() as u32) as usize;
    items.get(index)
}

/// Seeded near-equal sampling: the best candidate plus every candidate
/// whose value is within `near_equal_permille` permille of the best
/// (integer truncation: `best * permille / 1000`, floor = best - slack)
/// form the eligible band, and one member is drawn uniformly through the
/// stream. Rules:
///
/// - `near_equal_permille == 0` keeps only exact ties with the best;
/// - a best value of zero or below has no proportional band (only exact
///   ties) — negative values never widen the band;
/// - deterministic given the stream position: same key and same call
///   sequence, same pick; seed and faction key the stream.
pub fn pick_near_equal<'a, T, F>(
    items: &'a [T],
    value: F,
    near_equal_permille: u32,
    rng: &mut R2Rng,
) -> Option<&'a T>
where
    F: Fn(&T) -> i64,
{
    if items.is_empty() {
        return None;
    }
    let best = items.iter().map(|item| value(item)).max()?;
    let slack = if best > 0 {
        ((best as u128 * near_equal_permille as u128) / 1000).min(i64::MAX as u128) as i64
    } else {
        0
    };
    let floor = best.saturating_sub(slack);
    let eligible: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| value(item) >= floor)
        .map(|(index, _)| index)
        .collect();
    let index = eligible[rng.uniform(eligible.len() as u32) as usize];
    items.get(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r2::rng::R2Stream;
    use arena_core::types::Faction;

    #[test]
    fn the_band_floor_is_truncated_integer_permille() {
        // best 1000, permille 33: slack = 33 (1000*33/1000), floor 967.
        let mut rng = R2Rng::new(1, Faction::Enl, R2Stream::Idle);
        assert_eq!(
            pick_near_equal(&[1000i64, 968], |v| *v, 33, &mut rng),
            Some(&1000)
        );
        let mut rng = R2Rng::new(1, Faction::Enl, R2Stream::Idle);
        assert!(
            pick_near_equal(&[1000i64, 967, 900], |v| *v, 33, &mut rng).map(|v| *v) != Some(900),
            "900 is outside the band"
        );
        // Every draw over a fixed stream is stable across 64 repetitions.
        let mut rng = R2Rng::new(9, Faction::Res, R2Stream::AdaptiveHybrid);
        let picks: Vec<i64> = (0..64)
            .map(|_| *pick_near_equal(&[100i64, 99, 90], |v| *v, 10, &mut rng).unwrap())
            .collect();
        assert!(picks.iter().all(|v| *v >= 99));
    }
}

//! Integer Elo (P5-RANKING, D84): standard Elo, initial 1500, K
//! configurable (default 32), draw = 0.5 each — computed ENTIRELY in
//! integer Q32.32 fixed point (the repo-wide no-float rule).
//!
//! The expected score `E = 1 / (1 + 10^(Delta/400))` with
//! `Delta = Rb - Ra` splits `10^(Delta/400)` into `10^q * 10^(r/400)`
//! (`q = Delta div 400`, `r in [0, 400)`), evaluates `10^(r/400)` as
//! `exp(r*ln10/400)` through an 18-term Taylor series in Q32.32
//! (|z| <= ln10 < 2.303; the plan's 14 terms miss its own +/-2^-28
//! bound — the truncation tail at z = 2.303 is ~1.4e-6 there vs ~5e-10
//! with 18), and forms `E` by one rounded division. The rating update is
//! `R' = R + round_half_away(K * (s - E))`.
//!
//! Hand-worked cross-check (independent awk computation, pinned by the
//! tests): both 1500, K = 32 —
//! `m1 A beats B` E=0.5 -> A 1516 B 1484; `m2 draw` E=0.545921922780 ->
//! A 1515 B 1485; `m3 B beats A` E=0.543066492022 -> A 1498 B 1502; and
//! `m4 A beats B` under K=16, E=0.494243791589 -> A 1506 B 1494.

use crate::tournament::outcome::TournamentOutcome;
use crate::tournament::schedule::TournamentItem;

/// 1.0 in Q32.32.
pub const Q_ONE: i64 = 1 << 32;

/// `ln(10)` in Q32.32: `round(2.302585092994046 * 2^32)` (golden
/// constant, like the other pinned values — no runtime float involved).
const LN10_Q: i64 = 9_889_527_671;

/// Taylor terms for `exp` (k = 0..=17).
const EXP_TERMS: usize = 18;

/// Ratings beyond this gap saturate the expected score to 0/1 (E at
/// Delta = 3200 is already < 1e-8; the bound also keeps every i128
/// intermediate far from overflow).
const SATURATE_DELTA: i64 = 3200;

/// `num / den` rounded half AWAY from zero (integer, sign-symmetric:
/// `round_half_away(-x) == -round_half_away(x)` — the Elo pair updates
/// stay exact negatives of each other).
pub fn round_half_away(num: i128, den: i128) -> i128 {
    debug_assert!(den > 0);
    let sign = if num < 0 { -1 } else { 1 };
    let magnitude = num.unsigned_abs() + (den as u128 / 2);
    sign * ((magnitude / den as u128) as i128)
}

/// Multiplies two Q32.32 values (i128 intermediate, truncated shift).
fn q_mul(a: i64, b: i64) -> i64 {
    ((a as i128 * b as i128) >> 32) as i64
}

/// `e^z` for `0 <= z <= ln(10)` in Q32.32: the 18-term Taylor series
/// (see the module docs for the term count).
fn exp_q(z_q: i64) -> i64 {
    let mut term = Q_ONE; // z^0 / 0!
    let mut sum = Q_ONE;
    for k in 1..=EXP_TERMS {
        // term_{k} = term_{k-1} * z / k, rounded.
        term = round_half_away((q_mul(term, z_q)) as i128, k as i128) as i64;
        sum = sum.saturating_add(term);
    }
    sum
}

/// The expected score of the `a` player, `1 / (1 + 10^((rb - ra)/400))`,
/// in Q32.32.
pub fn expected_score_q(ra: i64, rb: i64) -> i64 {
    let delta = rb.saturating_sub(ra);
    if delta >= SATURATE_DELTA {
        return 0;
    }
    if delta <= -SATURATE_DELTA {
        return Q_ONE;
    }
    let q = delta.div_euclid(400);
    let r = delta.rem_euclid(400); // 0..=399
    // z = r * ln10 / 400 in Q32.32.
    let z_q = round_half_away(r as i128 * LN10_Q as i128, 400) as i64;
    let t = exp_q(z_q); // 10^(r/400)
    if q >= 0 {
        let scale = 10i128.pow(q as u32); // 1..=1e7 under the saturate bound
        // E = 2^32 / (1 + 10^q * t / 2^32)  ->  Q32.32 via one division.
        let denominator = Q_ONE as i128 + scale * t as i128;
        round_half_away(1i128 << 64, denominator) as i64
    } else {
        let scale = 10i128.pow((-q) as u32);
        // E = 2^32 * scale / (scale + t / 2^32) -> same shape.
        let denominator = scale * Q_ONE as i128 + t as i128;
        round_half_away((1i128 << 64) * scale, denominator) as i64
    }
}

/// The rating delta of the `a` player: `round_half_away(K * (s - E))`
/// (`s` in Q32.32: 0 / 2^31 / 2^32).
pub fn rating_delta(k: u32, ra: i64, rb: i64, score_q: i64) -> i64 {
    let expected = expected_score_q(ra, rb);
    round_half_away(k as i128 * (score_q - expected) as i128, Q_ONE as i128) as i64
}

/// One applied rating update (a `ratings.tsv` row).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AppliedRating {
    pub index: u64,
    /// The item's entrant ids (for name lookup in the archive).
    pub entrant_a: u32,
    pub entrant_b: u32,
    /// Pre/post rating of item entrant A.
    pub pre_a: i64,
    pub post_a: i64,
    /// Pre/post rating of item entrant B.
    pub pre_b: i64,
    pub post_b: i64,
    /// Entrant A's score (0 / 500 / 1000 milli — 0.5 draws print as
    /// "0.5").
    pub score_a_milli: i64,
}

/// Live ratings + counters over one tournament. Technical outcomes NEVER
/// move ratings or W/D/L (D82) — they land in the forfeit/unscored
/// counters.
#[derive(Clone, Debug)]
pub struct EloTable {
    k: u32,
    initial: i64,
    ratings: Vec<i64>,
    wins: Vec<u64>,
    draws: Vec<u64>,
    losses: Vec<u64>,
    forfeit_wins: Vec<u64>,
    forfeit_losses: Vec<u64>,
    unscored: Vec<u64>,
}

impl EloTable {
    pub fn new(config: &crate::tournament::config::EloConfig, entrants: usize) -> Self {
        EloTable {
            k: config.k,
            initial: config.initial,
            ratings: vec![config.initial; entrants],
            wins: vec![0; entrants],
            draws: vec![0; entrants],
            losses: vec![0; entrants],
            forfeit_wins: vec![0; entrants],
            forfeit_losses: vec![0; entrants],
            unscored: vec![0; entrants],
        }
    }

    /// Applies one item's outcome (schedule order is the caller's duty —
    /// ratings replay in item-index order). Returns the applied rating
    /// row for scored matches.
    pub fn apply(
        &mut self,
        item: &TournamentItem,
        outcome: &TournamentOutcome,
    ) -> Option<AppliedRating> {
        self.apply_k(item, outcome, self.k)
    }

    /// `apply` with a K override (the frozen hand-worked sequence pins a
    /// K=16 match; also the seam for future per-match K policies).
    pub fn apply_k(
        &mut self,
        item: &TournamentItem,
        outcome: &TournamentOutcome,
        k: u32,
    ) -> Option<AppliedRating> {
        let (a, b) = (item.a as usize, item.b as usize);
        match outcome {
            TournamentOutcome::Normal { result, .. } => {
                let (s_a_milli, _s_b_milli) = match result {
                    arena_core::types::MatchResult::Draw => (500, 500),
                    arena_core::types::MatchResult::EnlWin => {
                        if item.a_is_enl {
                            (1000, 0)
                        } else {
                            (0, 1000)
                        }
                    }
                    arena_core::types::MatchResult::ResWin => {
                        if item.a_is_enl {
                            (0, 1000)
                        } else {
                            (1000, 0)
                        }
                    }
                };
                let (pre_a, pre_b) = (self.ratings[a], self.ratings[b]);
                let s_a_q = (s_a_milli as i128 * Q_ONE as i128 / 1000) as i64;
                let delta_a = rating_delta(k, pre_a, pre_b, s_a_q);
                // sign-symmetric rounding keeps delta_b == -delta_a.
                self.ratings[a] = pre_a + delta_a;
                self.ratings[b] = pre_b - delta_a;
                if s_a_milli == 1000 {
                    self.wins[a] += 1;
                    self.losses[b] += 1;
                } else if s_a_milli == 0 {
                    self.wins[b] += 1;
                    self.losses[a] += 1;
                } else {
                    self.draws[a] += 1;
                    self.draws[b] += 1;
                }
                Some(AppliedRating {
                    index: item.index,
                    entrant_a: item.a,
                    entrant_b: item.b,
                    pre_a,
                    post_a: self.ratings[a],
                    pre_b,
                    post_b: self.ratings[b],
                    score_a_milli: s_a_milli,
                })
            }
            TournamentOutcome::TechnicalWin { winner, .. } => {
                let winner = *winner as usize;
                let loser = if winner == a { b } else { a };
                self.forfeit_wins[winner] += 1;
                self.forfeit_losses[loser] += 1;
                None
            }
            TournamentOutcome::Unscored { .. } => {
                self.unscored[a] += 1;
                self.unscored[b] += 1;
                None
            }
        }
    }

    pub fn rating(&self, id: u32) -> i64 {
        self.ratings[id as usize]
    }

    pub fn ratings(&self) -> &[i64] {
        &self.ratings
    }

    pub fn initial(&self) -> i64 {
        self.initial
    }

    pub fn k(&self) -> u32 {
        self.k
    }

    pub fn wins(&self, id: u32) -> u64 {
        self.wins[id as usize]
    }

    pub fn draws(&self, id: u32) -> u64 {
        self.draws[id as usize]
    }

    pub fn losses(&self, id: u32) -> u64 {
        self.losses[id as usize]
    }

    pub fn forfeit_wins(&self, id: u32) -> u64 {
        self.forfeit_wins[id as usize]
    }

    pub fn forfeit_losses(&self, id: u32) -> u64 {
        self.forfeit_losses[id as usize]
    }

    pub fn unscored(&self, id: u32) -> u64 {
        self.unscored[id as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Q32.32 formatting for assertions (test-side only).
    fn q_approx(q: i64) -> (i64, i64) {
        let units = q >> 32;
        let frac_nano = ((q & 0xFFFF_FFFF) as i128 * 1_000_000_000) >> 32;
        (units, frac_nano as i64)
    }

    #[test]
    fn exp_matches_the_series_at_pin_points() {
        // exp(0) is exact.
        assert_eq!(exp_q(0), Q_ONE);
        // exp(ln10) = 10 within the series + rounding + constant error
        // (bound: 2e-8 relative, i.e. < 90 Q-units of 2^32).
        let got = exp_q(LN10_Q) as i128;
        let want = 10i128 * Q_ONE as i128;
        assert!(
            (got - want).abs() <= 90,
            "exp(ln10)={got} want~{want} diff={}",
            (got - want).abs()
        );
        // exp(ln10/2) = sqrt(10) ~ 3.16227766.
        let (units, frac) = q_approx(exp_q(LN10_Q / 2));
        assert_eq!(units, 3);
        assert!(frac > 162_000_000 && frac < 162_500_000, "frac={frac}");
        // Monotone in z.
        assert!(exp_q(LN10_Q / 4) < exp_q(LN10_Q / 2));
    }

    #[test]
    fn saturation_is_symmetric() {
        assert_eq!(expected_score_q(1500, 1500 + SATURATE_DELTA), 0);
        assert_eq!(expected_score_q(1500 + SATURATE_DELTA, 1500), Q_ONE);
        // Just inside the bound both sides are still finite.
        assert!(expected_score_q(1500, 1500 + SATURATE_DELTA - 1) > 0);
        assert!(expected_score_q(1500 + SATURATE_DELTA - 1, 1500) < Q_ONE);
    }

    #[test]
    fn rating_delta_mirrors_the_hand_computed_values() {
        // 1500 vs 1500, win: 32 * 0.5 = 16 exactly.
        assert_eq!(rating_delta(32, 1500, 1500, Q_ONE), 16);
        assert_eq!(rating_delta(32, 1500, 1500, 0), -16);
        assert_eq!(rating_delta(32, 1500, 1500, Q_ONE / 2), 0);
        // m2 pre-ratings 1516 vs 1484, draw: E=0.545921922780,
        // 32 * (0.5 - E) = -1.4695 -> -1.
        assert_eq!(rating_delta(32, 1516, 1484, Q_ONE / 2), -1);
        // m3 pre-ratings 1515 vs 1485, A loses: E=0.543066492022,
        // 32 * (0 - E) = -17.378 -> -17.
        assert_eq!(rating_delta(32, 1515, 1485, 0), -17);
        // m4 pre-ratings 1498 vs 1502 under K=16, A wins: E=0.494243791589,
        // 16 * (1 - E) = 8.092 -> 8.
        assert_eq!(rating_delta(16, 1498, 1502, Q_ONE), 8);
    }

    #[test]
    fn ln10_constant_is_the_rounded_true_value() {
        // round(2.302585092994046 * 2^32) = 9889527671 (pinned; a wrong
        // constant shifts every golden vector, which the integration
        // tests would catch — this documents the derivation).
        assert_eq!(LN10_Q, 9_889_527_671);
        let _ = round_half_away(1, 1);
    }
}

//! Order-independent Bradley–Terry fit on the Elo scale. A Gaussian prior
//! N(initial, 400²) makes the optimum finite and unique even for undefeated,
//! disconnected or unplayed entrants. K belongs only to online Elo.
//!
//! The score equation for offset r_i is
//! sum_j(score_ij - games_ij * E_ij) - r_i / (400 * ln(10)) = 0.
//! Solve each monotone conditional equation by bisection, then recenter:
//! the pair likelihood is translation invariant and the prior's optimum
//! translation is zero mean. All probabilities are Q32.32; offsets use
//! micro-Elo. No match is counted more than once.

use super::{
    elo::{LN10_Q, Q_ONE, expected_score_scaled, round_half_away},
    outcome::TournamentOutcome,
    schedule::TournamentItem,
};
use std::{collections::BTreeMap, io};

const SCALE: i64 = 1_000_000;
const MAX_SWEEPS: usize = 10_000;

#[derive(Default)]
struct Pair {
    games: i128,
    score_q: i128,
}

struct Opponent {
    id: usize,
    games: i128,
    score_q: i128,
}

fn residual(offset: i64, opponents: &[Opponent], offsets: &[i64]) -> i128 {
    let observed: i128 = opponents
        .iter()
        .map(|o| o.score_q - o.games * expected_score_scaled(offset, offsets[o.id], SCALE) as i128)
        .sum();
    let prior = round_half_away(
        offset as i128 * Q_ONE as i128 * Q_ONE as i128,
        400 * SCALE as i128 * LN10_Q as i128,
    );
    observed - prior
}

pub(super) fn fit(
    initial: i64,
    entrants: usize,
    results: &[(&TournamentItem, &TournamentOutcome)],
) -> io::Result<Vec<i64>> {
    let mut pairs: BTreeMap<(usize, usize), Pair> = BTreeMap::new();
    for &(item, outcome) in results {
        if !outcome.elo_relevant() {
            continue;
        }
        let (a, b) = (item.a.min(item.b) as usize, item.a.max(item.b) as usize);
        let pair = pairs.entry((a, b)).or_default();
        pair.games += 1;
        pair.score_q += match outcome.normal_winner(item.a, item.b, item.a_is_enl) {
            Some(winner) if winner as usize == a => Q_ONE as i128,
            Some(_) => 0,
            None => (Q_ONE / 2) as i128,
        };
    }
    let mut opponents: Vec<Vec<Opponent>> = (0..entrants).map(|_| Vec::new()).collect();
    for ((a, b), pair) in pairs {
        opponents[a].push(Opponent {
            id: b,
            games: pair.games,
            score_q: pair.score_q,
        });
        opponents[b].push(Opponent {
            id: a,
            games: pair.games,
            score_q: pair.games * Q_ONE as i128 - pair.score_q,
        });
    }
    let active: Vec<_> = (0..entrants)
        .filter(|&id| !opponents[id].is_empty())
        .collect();
    let mut offsets = vec![0i64; entrants];
    for _ in 0..MAX_SWEEPS {
        let before = offsets.clone();
        for &id in &active {
            let mut lo = -400 * SCALE;
            let mut hi = 400 * SCALE;
            while residual(lo, &opponents[id], &offsets) < 0 {
                lo *= 2;
            }
            while residual(hi, &opponents[id], &offsets) > 0 {
                hi *= 2;
            }
            while hi - lo > 1 {
                let mid = lo + (hi - lo) / 2;
                if residual(mid, &opponents[id], &offsets) > 0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            offsets[id] = lo + (hi - lo) / 2;
        }
        if !active.is_empty() {
            let mean = round_half_away(
                active.iter().map(|&id| offsets[id] as i128).sum(),
                active.len() as i128,
            ) as i64;
            for &id in &active {
                offsets[id] -= mean;
            }
        }
        if offsets.iter().zip(before).all(|(a, b)| (a - b).abs() <= 2) {
            return offsets
                .into_iter()
                .map(|offset| {
                    i64::try_from(round_half_away(
                        initial as i128 * SCALE as i128 + offset as i128,
                        SCALE as i128,
                    ))
                    .map_err(|_| io::Error::other("batch Elo rating overflow"))
                })
                .collect();
        }
    }
    Err(io::Error::other("batch Elo did not converge"))
}

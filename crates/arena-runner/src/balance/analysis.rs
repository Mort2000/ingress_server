//! Plan-section-9 offline statistics (B3, ADR-0017): the frozen analysis
//! definitions behind the default-config ablation report — per-pair W-D-L
//! with Wilson 95% intervals, per-map paired score-delta quantiles
//! (nearest-rank), score share with both-zero counts, checkpoint
//! contingency tables, per-policy mechanics sums and drop counts, ENL/RES
//! faction asymmetry, mirror spawn-collision and outcome-symmetry counts,
//! the domination-decomposition ladder and the plan-section-11
//! classification rules.
//!
//! EVERYTHING here is non-authoritative offline analysis (plan 9.3):
//! integer raw counts are the record; derived permille and the Wilson
//! bounds are printed NEXT TO — never instead of — the counts (the Wilson
//! computation alone uses f64, emitted as conservatively floor/ceil-scaled
//! permille integers); nothing derived here enters any Game Result,
//! Replay Hash or rule settlement.

use std::collections::BTreeMap;

use arena_bots::policy::{PolicyId, PolicySpec};
use arena_core::types::Faction;

use crate::balance::execute::BalanceReplay;
use crate::balance::pairs::PairedResult;
use crate::balance::plan::ExperimentPlan;
use crate::balance::record::{BalanceMatchRecord, BalanceOutcome};
use crate::metrics::{CHECKPOINTS, Outcome};

// ---------------------------------------------------------------------
// Wilson 95% interval (frozen definition, ADR-0017)
// ---------------------------------------------------------------------

/// The Wilson 95% interval of `wins/(wins+losses)` in permille: z = 1.96
/// (exactly 49/25), computed in f64, emitted as `(floor(lower),
/// ceil(upper))` permille — conservative rounding only ever WIDENS the
/// interval. `None` when `decisive == 0` (no interval exists; the raw
/// W-D-L counts remain the record). Draws are excluded from the denominator
/// and reported as raw counts alongside.
pub fn wilson_permille(wins: u64, decisive: u64) -> Option<(u64, u64)> {
    if decisive == 0 {
        return None;
    }
    let z = 1.96f64;
    let z2 = z * z;
    let n = decisive as f64;
    let p = wins as f64 / n;
    let denominator = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denominator;
    let half = z / denominator * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    let lower = ((center - half) * 1000.0).floor().max(0.0) as u64;
    let upper = ((center + half) * 1000.0).ceil().min(1000.0) as u64;
    Some((lower, upper))
}

/// Whether a side's W/(W+L) meets the frozen DOMINATION standard
/// (ADR-0017, aligned with plan 10.3): Wilson 95% lower bound >= 900
/// permille.
pub fn is_domination(wins: u64, decisive: u64) -> bool {
    matches!(wilson_permille(wins, decisive), Some((low, _)) if low >= 900)
}

// ---------------------------------------------------------------------
// Nearest-rank quantiles (frozen definition, ADR-0017)
// ---------------------------------------------------------------------

/// The q-quantile (`q` in permille) of `values` under the frozen
/// nearest-rank definition: ascending order, 1-based rank
/// `ceil(q*N/1000)` clamped into `1..=N`. `None` for empty input. Pure:
/// sorts a copy, never the caller's slice.
pub fn nearest_rank_quantile(values: &[i128], permille: u64) -> Option<i128> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len() as u64;
    let rank = ((permille * n).div_ceil(1000)).clamp(1, n) as usize;
    Some(sorted[rank - 1])
}

/// The per-map paired margin distribution of one pair: raw integer
/// `margin_sum` values (one per comparable unit of `pairs.tsv`), the
/// nearest-rank quartiles alongside min/median/max.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MarginQuantiles {
    pub units: u64,
    pub min: i128,
    pub p25: i128,
    pub median: i128,
    pub p75: i128,
    pub max: i128,
}

impl MarginQuantiles {
    /// From the raw per-unit `margin_sum` integers (any order).
    pub fn of(margins: &[i128]) -> Option<MarginQuantiles> {
        let mut sorted = margins.to_vec();
        sorted.sort_unstable();
        let min = *sorted.first()?;
        let max = *sorted.last()?;
        Some(MarginQuantiles {
            units: sorted.len() as u64,
            min,
            p25: nearest_rank_quantile(margins, 250)?,
            median: nearest_rank_quantile(margins, 500)?,
            p75: nearest_rank_quantile(margins, 750)?,
            max,
        })
    }
}

// ---------------------------------------------------------------------
// Per-pair summaries
// ---------------------------------------------------------------------

/// Whether a record is a played, non-crash, non-config-failure match.
fn played(record: &BalanceMatchRecord) -> bool {
    !matches!(
        record.result,
        BalanceOutcome::ConfigFailure | BalanceOutcome::Played(Outcome::Crash)
    )
}

/// One checkpoint's contingency from policy_a's perspective (raw counts
/// only; the win-share permille is derived at render time).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CheckpointContingency {
    pub round: u32,
    /// policy_a led at the checkpoint and finally won.
    pub led_won: u64,
    /// policy_a led at the checkpoint and finally lost.
    pub led_lost: u64,
    /// policy_a led at the checkpoint and the match finally drew.
    pub led_drew: u64,
    /// the factions were tied at the checkpoint (tie rate numerator).
    pub tie: u64,
}

impl CheckpointContingency {
    fn new(round: u32) -> Self {
        CheckpointContingency {
            round,
            led_won: 0,
            led_lost: 0,
            led_drew: 0,
            tie: 0,
        }
    }

    /// The frozen derived value: `P(win | leading@r)` in permille over all
    /// led matches (a final draw is not a win), `None` when nobody led.
    pub fn win_share_permille(&self) -> Option<u64> {
        let led = self.led_won + self.led_lost + self.led_drew;
        if led == 0 {
            None
        } else {
            Some(self.led_won * 1000 / led)
        }
    }
}

/// One pair's complete raw-count summary (ADR-0017 statistics 1-4, 7).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PairSummary {
    pub pair_tag: String,
    pub policy_a: PolicyId,
    pub policy_b: PolicyId,
    pub a_wins: u64,
    pub b_wins: u64,
    pub draws: u64,
    pub crashes: u64,
    pub config_failures: u64,
    /// Wilson 95% on policy_a's W/(W+L) (floor/ceil permille).
    pub wilson_a: Option<(u64, u64)>,
    pub quantiles: Option<MarginQuantiles>,
    pub a_score_sum: u128,
    pub b_score_sum: u128,
    /// policy_a's score share in permille (None when both sums are zero).
    pub a_share_permille: Option<u64>,
    /// Matches where BOTH factions scored zero (raw count).
    pub both_zero_matches: u64,
    /// Faction asymmetry: raw ENL/RES win counts of this pair.
    pub enl_wins: u64,
    pub res_wins: u64,
    /// One contingency per checkpoint, CHECKPOINTS order.
    pub checkpoints: Vec<CheckpointContingency>,
}

/// Computes every pair's summary from the run's records plus its paired
/// integer results (margins of `base_variant` rows only), in ascending
/// pair-tag order.
pub fn pair_summaries(
    records: &[BalanceMatchRecord],
    pairs: &[PairedResult],
    base_variant: &str,
) -> Vec<PairSummary> {
    let mut by_tag: BTreeMap<String, Vec<&BalanceMatchRecord>> = BTreeMap::new();
    for record in records {
        by_tag
            .entry(record.pair_tag.clone())
            .or_default()
            .push(record);
    }
    let margins_by_tag: BTreeMap<&str, Vec<i128>> = pairs
        .iter()
        .filter(|p| p.variant_id == base_variant)
        .fold(BTreeMap::new(), |mut acc, p| {
            acc.entry(p.pair_tag.as_str())
                .or_default()
                .push(p.margin_sum);
            acc
        });
    let mut out = Vec::with_capacity(by_tag.len());
    for (pair_tag, pair_records) in &by_tag {
        let first = pair_records[0];
        let mut summary = PairSummary {
            pair_tag: pair_tag.clone(),
            policy_a: first.policy_a,
            policy_b: first.policy_b,
            a_wins: 0,
            b_wins: 0,
            draws: 0,
            crashes: 0,
            config_failures: 0,
            wilson_a: None,
            quantiles: None,
            a_score_sum: 0,
            b_score_sum: 0,
            a_share_permille: None,
            both_zero_matches: 0,
            enl_wins: 0,
            res_wins: 0,
            checkpoints: CHECKPOINTS
                .iter()
                .map(|&r| CheckpointContingency::new(r))
                .collect(),
        };
        for record in pair_records {
            let a_score = if record.policy_a_is_enl {
                record.enl_score2
            } else {
                record.res_score2
            };
            let b_score = if record.policy_a_is_enl {
                record.res_score2
            } else {
                record.enl_score2
            };
            match record.result {
                BalanceOutcome::Played(Outcome::EnlWin) => {
                    summary.enl_wins += 1;
                    if record.policy_a_is_enl {
                        summary.a_wins += 1;
                    } else {
                        summary.b_wins += 1;
                    }
                }
                BalanceOutcome::Played(Outcome::ResWin) => {
                    summary.res_wins += 1;
                    if record.policy_a_is_enl {
                        summary.b_wins += 1;
                    } else {
                        summary.a_wins += 1;
                    }
                }
                BalanceOutcome::Played(Outcome::Draw) => summary.draws += 1,
                BalanceOutcome::Played(Outcome::Crash) => summary.crashes += 1,
                BalanceOutcome::ConfigFailure => summary.config_failures += 1,
            }
            if !played(record) {
                continue;
            }
            summary.a_score_sum += a_score;
            summary.b_score_sum += b_score;
            if a_score == 0 && b_score == 0 {
                summary.both_zero_matches += 1;
            }
            // Checkpoint contingency (policy_a's perspective): the
            // snapshot array is CHECKPOINTS-ordered by construction.
            for (index, snapshot) in record.stats.checkpoints.iter().enumerate() {
                let a_leads = match snapshot.leader {
                    Some(Faction::Enl) => Some(record.policy_a_is_enl),
                    Some(Faction::Res) => Some(!record.policy_a_is_enl),
                    None => None,
                };
                let contingency = &mut summary.checkpoints[index];
                match a_leads {
                    None => contingency.tie += 1,
                    Some(false) => {}
                    Some(true) => match record.result {
                        BalanceOutcome::Played(Outcome::Draw) => contingency.led_drew += 1,
                        _ => {
                            // a led and the match was decisive: a won iff
                            // the winning faction is a's faction.
                            let a_won = match record.result {
                                BalanceOutcome::Played(Outcome::EnlWin) => record.policy_a_is_enl,
                                BalanceOutcome::Played(Outcome::ResWin) => !record.policy_a_is_enl,
                                _ => unreachable!("decisive arm"),
                            };
                            if a_won {
                                contingency.led_won += 1;
                            } else {
                                contingency.led_lost += 1;
                            }
                        }
                    },
                }
            }
        }
        let decisive = summary.a_wins + summary.b_wins;
        summary.wilson_a = wilson_permille(summary.a_wins, decisive);
        summary.quantiles = margins_by_tag
            .get(pair_tag.as_str())
            .and_then(|margins| MarginQuantiles::of(margins));
        let total = summary.a_score_sum + summary.b_score_sum;
        summary.a_share_permille = if total == 0 {
            None
        } else {
            Some((summary.a_score_sum * 1000 / total) as u64)
        };
        out.push(summary);
    }
    out
}

// ---------------------------------------------------------------------
// Per-policy mechanics (ADR-0017 statistics 5-6)
// ---------------------------------------------------------------------

/// One policy's pooled raw mechanics over the matches it played.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PolicyMechanics {
    pub policy: PolicyId,
    pub matches_played: u64,
    pub first_field_round_sum: u64,
    /// Matches with a recorded first-field round (the mean's denominator).
    pub first_field_count: u64,
    pub field_area2_sum: u128,
    pub lifetime_rounds_sum: u64,
    pub fields_created: u64,
    pub commands_submitted: u64,
    pub commands_ok: u64,
    pub commands_dropped: u64,
}

/// Pools per-policy mechanics in the plan's policy declaration order
/// (participation by policy-id membership of the record's sides, never by
/// a zero score).
pub fn policy_mechanics(
    records: &[BalanceMatchRecord],
    specs: &[PolicySpec],
) -> Vec<PolicyMechanics> {
    specs
        .iter()
        .map(|spec| {
            let mut mechanics = PolicyMechanics {
                policy: spec.policy_id,
                matches_played: 0,
                first_field_round_sum: 0,
                first_field_count: 0,
                field_area2_sum: 0,
                lifetime_rounds_sum: 0,
                fields_created: 0,
                commands_submitted: 0,
                commands_ok: 0,
                commands_dropped: 0,
            };
            for record in records.iter().filter(|r| played(r)) {
                let stats = if record.policy_a == spec.policy_id {
                    if record.policy_a_is_enl {
                        &record.stats.enl
                    } else {
                        &record.stats.res
                    }
                } else if record.policy_b == spec.policy_id {
                    if record.policy_a_is_enl {
                        &record.stats.res
                    } else {
                        &record.stats.enl
                    }
                } else {
                    continue;
                };
                mechanics.matches_played += 1;
                if let Some(round) = stats.first_field_round {
                    mechanics.first_field_round_sum += round as u64;
                    mechanics.first_field_count += 1;
                }
                mechanics.field_area2_sum += stats.field_area2_sum;
                mechanics.lifetime_rounds_sum +=
                    stats.destroyed_lifetime_rounds_sum + stats.surviving_lifetime_rounds_sum;
                mechanics.fields_created += stats.fields_created;
                mechanics.commands_submitted += stats.commands_submitted;
                mechanics.commands_ok += stats.commands_ok;
                mechanics.commands_dropped += stats.commands_dropped;
            }
            mechanics
        })
        .collect()
}

// ---------------------------------------------------------------------
// Mirror statistics (ADR-0017 statistic 8)
// ---------------------------------------------------------------------

/// One mirror pair's spawn-collision and outcome-symmetry counts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MirrorPairStats {
    pub pair_tag: String,
    pub policy: PolicyId,
    /// Mirror legs whose replay was captured and decoded (both spawn sets
    /// readable from the header).
    pub legs_checked: u64,
    /// Legs where the two factions chose the identical sorted coordinate
    /// set (the collision the canonical arm exists to re-create).
    pub spawn_collisions: u64,
    /// Mirror units with both legs recorded and played.
    pub units: u64,
    /// Units where both legs' outcomes are the same tag.
    pub symmetric_outcome_units: u64,
}

/// The spawn SET of a faction as a comparable value: positions sorted
/// (agent ids differ per faction by construction — the mirror conflict is
/// two factions choosing the identical COORDINATES). Same definition as
/// the B2 mirror gate.
fn sorted_spawns(
    spawns: &[(arena_core::types::AgentId, arena_core::types::Coord)],
) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = spawns.iter().map(|(_, c)| (c.x, c.y)).collect();
    out.sort();
    out
}

/// Computes every mirror pair's stats from the records plus the captured
/// replays (spawn sets from the replay headers).
pub fn mirror_stats(
    records: &[BalanceMatchRecord],
    replays: &[BalanceReplay],
) -> Vec<MirrorPairStats> {
    let by_cell: BTreeMap<u64, &BalanceReplay> =
        replays.iter().map(|r| (r.cell_index, r)).collect();
    let mut by_tag: BTreeMap<String, Vec<&BalanceMatchRecord>> = BTreeMap::new();
    for record in records.iter().filter(|r| r.policy_a == r.policy_b) {
        by_tag
            .entry(record.pair_tag.clone())
            .or_default()
            .push(record);
    }
    let mut out = Vec::with_capacity(by_tag.len());
    for (pair_tag, pair_records) in &by_tag {
        let mut stats = MirrorPairStats {
            pair_tag: pair_tag.clone(),
            policy: pair_records[0].policy_a,
            legs_checked: 0,
            spawn_collisions: 0,
            units: 0,
            symmetric_outcome_units: 0,
        };
        // Outcome symmetry per mirror unit (legs grouped by pair_id).
        let mut units: BTreeMap<&str, Vec<&BalanceOutcome>> = BTreeMap::new();
        for record in pair_records.iter().filter(|r| played(r)) {
            units
                .entry(record.pair_id.as_str())
                .or_default()
                .push(&record.result);
        }
        for (_, outcomes) in units {
            stats.units += 1;
            if outcomes.len() == 2 && outcomes[0] == outcomes[1] {
                stats.symmetric_outcome_units += 1;
            }
        }
        // Spawn collisions from the captured mirror legs.
        for record in pair_records {
            let Some(replay) = by_cell.get(&record.index) else {
                continue;
            };
            let Ok(file) = arena_replay::replay::decode_log(&replay.bytes) else {
                continue;
            };
            stats.legs_checked += 1;
            if sorted_spawns(&file.header.enl_spawns) == sorted_spawns(&file.header.res_spawns) {
                stats.spawn_collisions += 1;
            }
        }
        out.push(stats);
    }
    out
}

// ---------------------------------------------------------------------
// The domination-decomposition ladder + plan-7.2 contrasts
// ---------------------------------------------------------------------

/// One ladder step / plan-7.2 contrast: `upper` is the side expected to
/// be stronger (the later rung); all counts are `upper`'s.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LadderStep {
    pub contrast: &'static str,
    pub pair_tag: String,
    pub upper: PolicyId,
    pub lower: PolicyId,
    pub wins: u64,
    pub losses: u64,
    pub draws: u64,
    pub wilson: Option<(u64, u64)>,
    /// upper's W/(W+L) in permille (None when no decisive match).
    pub win_share_permille: Option<u64>,
    /// The frozen domination verdict (Wilson lower bound >= 900 permille).
    pub is_domination: bool,
}

/// The played records of one policy pair, either declaration order.
fn pair_records<'a>(
    records: &'a [BalanceMatchRecord],
    a: PolicyId,
    b: PolicyId,
) -> Vec<&'a BalanceMatchRecord> {
    records
        .iter()
        .filter(|r| (r.policy_a == a && r.policy_b == b) || (r.policy_a == b && r.policy_b == a))
        .collect()
}

fn ladder_step(
    records: &[BalanceMatchRecord],
    contrast: &'static str,
    upper: PolicyId,
    lower: PolicyId,
) -> LadderStep {
    let pair = pair_records(records, upper, lower);
    let mut step = LadderStep {
        contrast,
        pair_tag: pair
            .first()
            .map(|r| r.pair_tag.clone())
            .unwrap_or_else(|| format!("{}--{}", lower.tag(), upper.tag())),
        upper,
        lower,
        wins: 0,
        losses: 0,
        draws: 0,
        wilson: None,
        win_share_permille: None,
        is_domination: false,
    };
    for record in pair.iter().filter(|r| played(r)) {
        let upper_is_enl = if record.policy_a == upper {
            record.policy_a_is_enl
        } else {
            !record.policy_a_is_enl
        };
        match record.result {
            BalanceOutcome::Played(Outcome::EnlWin) => {
                if upper_is_enl {
                    step.wins += 1;
                } else {
                    step.losses += 1;
                }
            }
            BalanceOutcome::Played(Outcome::ResWin) => {
                if upper_is_enl {
                    step.losses += 1;
                } else {
                    step.wins += 1;
                }
            }
            BalanceOutcome::Played(Outcome::Draw) => step.draws += 1,
            BalanceOutcome::Played(Outcome::Crash) | BalanceOutcome::ConfigFailure => {}
        }
    }
    let decisive = step.wins + step.losses;
    step.wilson = wilson_permille(step.wins, decisive);
    step.win_share_permille = if decisive == 0 {
        None
    } else {
        Some(step.wins * 1000 / decisive)
    };
    step.is_domination = is_domination(step.wins, decisive);
    step
}

/// The frozen seven-step domination-decomposition ladder (ADR-0017):
/// greedy-field -> gc-r2 -> hybrid-builder-only -> hybrid-builder-attack
/// -> hybrid, plus the cross-era totals hybrid vs gc-r2 and greedy-field
/// vs gc-canonical.
pub fn domination_ladder(records: &[BalanceMatchRecord]) -> Vec<LadderStep> {
    vec![
        ladder_step(
            records,
            "ladder-1-greedy-m6-vs-gc-r2",
            PolicyId::GreedyCoordinatedR2,
            PolicyId::GreedyField,
        ),
        ladder_step(
            records,
            "ladder-2-gc-r2-vs-hybrid-builder-only",
            PolicyId::GreedyCoordinatedR2,
            PolicyId::HybridBuilderOnly,
        ),
        ladder_step(
            records,
            "ladder-3-builder-only-vs-builder-attack",
            PolicyId::HybridBuilderAttack,
            PolicyId::HybridBuilderOnly,
        ),
        ladder_step(
            records,
            "ladder-4-builder-attack-vs-hybrid",
            PolicyId::Hybrid,
            PolicyId::HybridBuilderAttack,
        ),
        ladder_step(
            records,
            "cross-era-hybrid-vs-gc-r2",
            PolicyId::Hybrid,
            PolicyId::GreedyCoordinatedR2,
        ),
        ladder_step(
            records,
            "cross-era-greedy-m6-vs-gc-canonical",
            PolicyId::GreedyCoordinatedCanonical,
            PolicyId::GreedyField,
        ),
        ladder_step(
            records,
            "cross-era-hybrid-vs-greedy-m6",
            PolicyId::Hybrid,
            PolicyId::GreedyField,
        ),
    ]
}

/// The six plan-section-7.2 contrasts, each mapped onto its matrix pair.
pub fn plan72_contrasts(records: &[BalanceMatchRecord]) -> Vec<LadderStep> {
    vec![
        ladder_step(
            records,
            "7.2-row1-greedy-m6-vs-gc-r2",
            PolicyId::GreedyCoordinatedR2,
            PolicyId::GreedyField,
        ),
        ladder_step(
            records,
            "7.2-row2-gc-r2-vs-hybrid-builder-only",
            PolicyId::GreedyCoordinatedR2,
            PolicyId::HybridBuilderOnly,
        ),
        ladder_step(
            records,
            "7.2-row3-builder-only-vs-builder-attack",
            PolicyId::HybridBuilderAttack,
            PolicyId::HybridBuilderOnly,
        ),
        // Row 4 is the defense dispatch alone: the frozen Hybrid IS
        // builder+attack+defense, so builder+attack vs Hybrid isolates the
        // Recharge scheduling (the arm is byte-identical to Hybrid except
        // for that branch).
        ladder_step(
            records,
            "7.2-row4-builder-attack-vs-builder-defense(=hybrid)",
            PolicyId::Hybrid,
            PolicyId::HybridBuilderAttack,
        ),
        ladder_step(
            records,
            "7.2-row5-canonical-spawn-vs-seeded-top-k",
            PolicyId::GreedyCoordinatedCanonical,
            PolicyId::GreedyCoordinatedR2,
        ),
        ladder_step(
            records,
            "7.2-row6-fixed-roles-vs-dynamic-roles",
            PolicyId::AdaptiveHybridR2,
            PolicyId::Hybrid,
        ),
    ]
}

// ---------------------------------------------------------------------
// Plan-section-11 classification (mechanical rules, ADR-0017)
// ---------------------------------------------------------------------

/// One applied plan-section-11 rule: the classification, the follow-up
/// action and the raw-count evidence string.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Plan11Classification {
    pub rule: String,
    pub classification: String,
    pub action: String,
    pub evidence: String,
}

/// Applies the frozen mechanical plan-11 rules to the matrix records and
/// mirror statistics (ADR-0017): the domination standard is a Wilson 95%
/// lower bound >= 900 permille of W/(W+L).
pub fn plan11_classification(
    records: &[BalanceMatchRecord],
    mirrors: &[MirrorPairStats],
) -> Vec<Plan11Classification> {
    let mut out = Vec::new();
    let hybrid_over_greedy = ladder_step(
        records,
        "hybrid-vs-greedy-m6",
        PolicyId::Hybrid,
        PolicyId::GreedyField,
    );
    let hybrid_over_gc = ladder_step(
        records,
        "hybrid-vs-gc-r2",
        PolicyId::Hybrid,
        PolicyId::GreedyCoordinatedR2,
    );
    let evidence_of = |step: &LadderStep| {
        format!(
            "{} over {}: W-L-D {}/{}/{} (Wilson95 {}..{} permille of W/(W+L))",
            step.upper.tag(),
            step.lower.tag(),
            step.wins,
            step.losses,
            step.draws,
            step.wilson
                .map(|(lo, _)| lo.to_string())
                .unwrap_or("-".to_string()),
            step.wilson
                .map(|(_, hi)| hi.to_string())
                .unwrap_or("-".to_string())
        )
    };
    // Rule 1 (plan 11 row 1): the coordinated correction removes the m6
    // domination -> Bot implementation defect.
    let corrected = hybrid_over_greedy.is_domination && !hybrid_over_gc.is_domination;
    out.push(Plan11Classification {
        rule: "plan-11 row 1 (协同修正消除支配)".to_string(),
        classification: if corrected {
            "Bot 实现缺陷".to_string()
        } else if hybrid_over_gc.is_domination {
            "未消除（残差归因）".to_string()
        } else {
            "前置不成立（Hybrid 对 Greedy m6.0 已非支配）".to_string()
        },
        action: if corrected {
            "保留规则默认值，更新 Benchmark".to_string()
        } else {
            "残差按阶梯台阶归因于角色/出生，喂给 B4".to_string()
        },
        evidence: format!(
            "{}; {}",
            evidence_of(&hybrid_over_greedy),
            evidence_of(&hybrid_over_gc)
        ),
    });
    // Rule 2 (plan 11 row 7): some policy dominates >= 3 distinct
    // opponents — multiple near-best responses still single-domination.
    let mut dominating: BTreeMap<PolicyId, u64> = BTreeMap::new();
    let mut dominated_examples: BTreeMap<PolicyId, Vec<&'static str>> = BTreeMap::new();
    for pair in pair_tags_of(records) {
        for (upper, lower) in [(pair.0, pair.1), (pair.1, pair.0)] {
            let step = ladder_step(records, "rule-risk-scan", upper, lower);
            if step.is_domination {
                *dominating.entry(upper).or_insert(0) += 1;
                dominated_examples
                    .entry(upper)
                    .or_default()
                    .push(lower.tag());
            }
        }
    }
    let multi = dominating.iter().find(|(_, count)| **count >= 3);
    if let Some((policy, count)) = multi {
        let examples = dominated_examples.get(policy).unwrap().join(", ");
        out.push(Plan11Classification {
            rule: "plan-11 row 7 (多近似最佳响应仍单一支配)".to_string(),
            classification: "规则风险".to_string(),
            action: "继续参数实验（B4）；参数空间无解时再讨论机制变更".to_string(),
            evidence: format!("{} 支配 {} 个互异对手（{}）", policy.tag(), count, examples),
        });
    }
    // Rule 3 (plan 11 row 6): mirror deviation only under identical
    // deterministic spawns -> strategy symmetry artifact.
    let collisions_of = |policy: PolicyId| -> u64 {
        mirrors
            .iter()
            .filter(|m| m.policy == policy)
            .map(|m| m.spawn_collisions)
            .sum()
    };
    let canonical_collisions = collisions_of(PolicyId::GreedyCoordinatedCanonical);
    let seeded_collisions = collisions_of(PolicyId::GreedyCoordinatedR2);
    if canonical_collisions > 0 && seeded_collisions == 0 {
        out.push(Plan11Classification {
            rule: "plan-11 row 6 (mirror 偏差仅在确定性出生下出现)".to_string(),
            classification: "策略对称伪影".to_string(),
            action:
                "修正 seeded tie-break（B2 已由 spawn jitter 落实；canonical 臂为测量有意重建）"
                    .to_string(),
            evidence: format!(
                "canonical mirror 出生冲突腿数 {} vs gc-r2 {}（legs_checked 分别 {} / {}）",
                canonical_collisions,
                seeded_collisions,
                mirrors
                    .iter()
                    .filter(|m| m.policy == PolicyId::GreedyCoordinatedCanonical)
                    .map(|m| m.legs_checked)
                    .sum::<u64>(),
                mirrors
                    .iter()
                    .filter(|m| m.policy == PolicyId::GreedyCoordinatedR2)
                    .map(|m| m.legs_checked)
                    .sum::<u64>()
            ),
        });
    }
    out
}

/// The distinct policy pairs present in the records (unordered pairs,
/// first-appearance order).
fn pair_tags_of(records: &[BalanceMatchRecord]) -> Vec<(PolicyId, PolicyId)> {
    let mut seen: Vec<(PolicyId, PolicyId)> = Vec::new();
    for record in records {
        let pair = if record.policy_a.tag() < record.policy_b.tag() {
            (record.policy_a, record.policy_b)
        } else {
            (record.policy_b, record.policy_a)
        };
        if !seen.contains(&pair) {
            seen.push(pair);
        }
    }
    seen
}

// ---------------------------------------------------------------------
// The analysis document
// ---------------------------------------------------------------------

/// The idle-opponent scoring check of the arms (plan 6.3): per arm, the
/// raw count of non-crash idle matches with `score2 > 0` and
/// `fields_created >= 1`.
pub struct IdleScoring {
    pub policy: PolicyId,
    pub scored_and_fielded: u64,
    pub matches: u64,
}

pub fn idle_scoring(records: &[BalanceMatchRecord]) -> Vec<IdleScoring> {
    let mut arms: Vec<PolicyId> = Vec::new();
    for record in records {
        for policy in [record.policy_a, record.policy_b] {
            if policy != PolicyId::Idle && !arms.contains(&policy) {
                arms.push(policy);
            }
        }
    }
    arms.iter()
        .map(|&policy| {
            let mut scored_and_fielded = 0u64;
            let mut matches = 0u64;
            for record in records.iter().filter(|r| played(r)) {
                let Some((score, fields)) = policy_score_fields(record, policy) else {
                    continue;
                };
                matches += 1;
                scored_and_fielded += u64::from(score > 0 && fields >= 1);
            }
            IdleScoring {
                policy,
                scored_and_fielded,
                matches,
            }
        })
        .collect()
}

fn policy_score_fields(record: &BalanceMatchRecord, policy: PolicyId) -> Option<(u128, u64)> {
    if record.policy_a == policy {
        let stats = if record.policy_a_is_enl {
            &record.stats.enl
        } else {
            &record.stats.res
        };
        let score = if record.policy_a_is_enl {
            record.enl_score2
        } else {
            record.res_score2
        };
        Some((score, stats.fields_created))
    } else if record.policy_b == policy {
        let stats = if record.policy_a_is_enl {
            &record.stats.res
        } else {
            &record.stats.enl
        };
        let score = if record.policy_a_is_enl {
            record.res_score2
        } else {
            record.enl_score2
        };
        Some((score, stats.fields_created))
    } else {
        None
    }
}

/// Renders the complete `analysis.txt` document of one B3 run: every
/// ADR-0017 statistic with its integer raw counts (derived permille
/// printed next to, never instead of, the counts). Pure over its inputs.
pub fn analysis_text(
    plan: &ExperimentPlan,
    records: &[BalanceMatchRecord],
    pairs: &[PairedResult],
    replays: &[BalanceReplay],
) -> String {
    let played_count = records.iter().filter(|r| played(r)).count();
    let crashes = records
        .iter()
        .filter(|r| r.result == BalanceOutcome::Played(Outcome::Crash))
        .count();
    let config_failures = records
        .iter()
        .filter(|r| r.result == BalanceOutcome::ConfigFailure)
        .count();
    let mut out = String::new();
    out.push_str(&format!(
        "# B3 ablation analysis — {} (stage {})\n",
        plan.experiment_id,
        plan.stage.tag()
    ));
    out.push_str("# offline analysis only (plan 9.3): integer raw counts are the record;\n");
    out.push_str("# permille and Wilson bounds are derived and printed NEXT TO the counts;\n");
    out.push_str("# nothing here enters any Game Result, Replay Hash or rule settlement.\n\n");
    out.push_str(&format!(
        "matches total={} played={} crashes={} config_failures={} replays_captured={}\n\n",
        records.len(),
        played_count,
        crashes,
        config_failures,
        replays.len()
    ));

    // Records integrity (the ADR-0016 gate 7 check, reused over this run).
    let integrity = crate::balance::readiness::records_gate(records, plan);
    out.push_str(&format!(
        "records-integrity: {}/{} family/version/parameters rows match policies.tsv (passed={})\n\n",
        integrity.numerator, integrity.denominator, u8::from(integrity.passed)
    ));

    // Per-pair table.
    let summaries = pair_summaries(records, pairs, &plan.base_variant);
    out.push_str("## Per-pair W-D-L, Wilson 95% and paired margins (policy_a perspective)\n\n");
    out.push_str("| pair | W-D-L (a) | Wilson95 a (permille) | margin min/p25/med/p75/max | score sums a:b (share) | both-zero | ENL/RES wins |\n|---|---|---|---|---|---|---|\n");
    for summary in &summaries {
        let wilson = match summary.wilson_a {
            Some((lo, hi)) => format!("{lo}..{hi}"),
            None => "-".to_string(),
        };
        let quantiles = match &summary.quantiles {
            Some(q) => format!(
                "{}/{}/{}/{}/{} (n={})",
                q.min, q.p25, q.median, q.p75, q.max, q.units
            ),
            None => "-".to_string(),
        };
        let share = match summary.a_share_permille {
            Some(pm) => format!("{} permille", pm),
            None => "-".to_string(),
        };
        out.push_str(&format!(
            "| {} | {}-{}-{} | {} | {} | {}:{} ({}) | {} | {}/{} |\n",
            summary.pair_tag,
            summary.a_wins,
            summary.draws,
            summary.b_wins,
            wilson,
            quantiles,
            summary.a_score_sum,
            summary.b_score_sum,
            share,
            summary.both_zero_matches,
            summary.enl_wins,
            summary.res_wins
        ));
    }
    out.push('\n');

    // Checkpoint contingency.
    out.push_str(
        "## P(win | leading@R50/100/200/300) contingency (raw counts; permille derived)\n\n",
    );
    out.push_str(
        "| pair | checkpoint | led->won/lost/drew | tie | P(win\\|led) |\n|---|---|---|---|---|\n",
    );
    for summary in &summaries {
        for contingency in &summary.checkpoints {
            out.push_str(&format!(
                "| {} | R{} | {}/{}/{} | {} | {} |\n",
                summary.pair_tag,
                contingency.round,
                contingency.led_won,
                contingency.led_lost,
                contingency.led_drew,
                contingency.tie,
                contingency
                    .win_share_permille()
                    .map(|p| format!("{p} permille"))
                    .unwrap_or_else(|| "-".to_string())
            ));
        }
    }
    out.push('\n');

    // Policy mechanics.
    out.push_str("## Per-policy mechanics (pooled raw sums; means derived)\n\n");
    out.push_str("| policy | matches | first-field sum (n, mean) | field_area2_sum | lifetime rounds (mean) | fields | submitted/ok/dropped (drop permille) |\n|---|---|---|---|---|---|---|\n");
    for mechanics in policy_mechanics(records, &plan.policies) {
        let ff_mean = if mechanics.first_field_count > 0 {
            format!(
                "{}, {}",
                mechanics.first_field_count,
                mechanics.first_field_round_sum / mechanics.first_field_count
            )
        } else {
            "-".to_string()
        };
        let lifetime_mean = if mechanics.matches_played > 0 {
            (mechanics.lifetime_rounds_sum / mechanics.matches_played).to_string()
        } else {
            "-".to_string()
        };
        let drop_pm = if mechanics.commands_submitted > 0 {
            format!(
                "{}",
                mechanics.commands_dropped * 1000 / mechanics.commands_submitted
            )
        } else {
            "-".to_string()
        };
        out.push_str(&format!(
            "| {} | {} | {} ({}) | {} | {} ({}) | {} | {}/{}{} ({}) |\n",
            mechanics.policy.tag(),
            mechanics.matches_played,
            mechanics.first_field_round_sum,
            ff_mean,
            mechanics.field_area2_sum,
            mechanics.lifetime_rounds_sum,
            lifetime_mean,
            mechanics.fields_created,
            mechanics.commands_submitted,
            mechanics.commands_ok,
            mechanics.commands_dropped,
            drop_pm
        ));
    }
    out.push('\n');

    // Mirror statistics.
    let mirrors = mirror_stats(records, replays);
    if !mirrors.is_empty() {
        out.push_str("## Mirror spawn collisions and outcome symmetry\n\n");
        out.push_str("| mirror | legs_checked | spawn collisions | units | symmetric outcome units |\n|---|---|---|---|---|\n");
        for stats in &mirrors {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                stats.pair_tag,
                stats.legs_checked,
                stats.spawn_collisions,
                stats.units,
                stats.symmetric_outcome_units
            ));
        }
        out.push('\n');
    }

    // Idle scoring (plan 6.3) when the idle opponent is present.
    if records
        .iter()
        .any(|r| r.policy_a == PolicyId::Idle || r.policy_b == PolicyId::Idle)
    {
        out.push_str("## Idle-opponent scoring of the ablation arms (plan 6.3)\n\n");
        out.push_str("| arm | scored AND fielded | matches |\n|---|---|---|\n");
        for idle in idle_scoring(records) {
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                idle.policy.tag(),
                idle.scored_and_fielded,
                idle.matches
            ));
        }
        out.push('\n');
    }

    // The plan-7.2 contrasts and the ladder.
    let contrasts = plan72_contrasts(records);
    let ladder = domination_ladder(records);
    if contrasts.iter().any(|c| c.wins + c.losses + c.draws > 0) {
        out.push_str("## Plan 7.2 contrasts (upper side's counts)\n\n");
        out.push_str("| contrast | upper vs lower | W-L-D | win share | Wilson95 | domination |\n|---|---|---|---|---|---|\n");
        for step in &contrasts {
            out.push_str(&format!(
                "| {} | {} vs {} | {}-{}-{} | {} | {} | {} |\n",
                step.contrast,
                step.upper.tag(),
                step.lower.tag(),
                step.wins,
                step.losses,
                step.draws,
                step.win_share_permille
                    .map(|p| format!("{p} permille"))
                    .unwrap_or_else(|| "-".to_string()),
                step.wilson
                    .map(|(lo, hi)| format!("{lo}..{hi}"))
                    .unwrap_or_else(|| "-".to_string()),
                u8::from(step.is_domination)
            ));
        }
        out.push('\n');
        out.push_str("## Domination-decomposition ladder (greedy-field -> gc-r2 -> builder-only -> builder-attack -> hybrid + cross-era)\n\n");
        out.push_str("| step | upper vs lower | W-L-D | win share | Wilson95 | domination |\n|---|---|---|---|---|---|\n");
        for step in &ladder {
            out.push_str(&format!(
                "| {} | {} vs {} | {}-{}-{} | {} | {} | {} |\n",
                step.contrast,
                step.upper.tag(),
                step.lower.tag(),
                step.wins,
                step.losses,
                step.draws,
                step.win_share_permille
                    .map(|p| format!("{p} permille"))
                    .unwrap_or_else(|| "-".to_string()),
                step.wilson
                    .map(|(lo, hi)| format!("{lo}..{hi}"))
                    .unwrap_or_else(|| "-".to_string()),
                u8::from(step.is_domination)
            ));
        }
        out.push('\n');
        // Plan-11 classification.
        out.push_str("## Plan-section-11 classification (mechanical rules, ADR-0017)\n\n");
        for class in plan11_classification(records, &mirrors) {
            out.push_str(&format!(
                "- rule: {}\n  classification: {}\n  action: {}\n  evidence: {}\n",
                class.rule, class.classification, class.action, class.evidence
            ));
        }
    }
    out
}

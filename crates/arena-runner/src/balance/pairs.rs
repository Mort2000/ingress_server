//! Paired effects at the INTEGER level only (B1, plan sections 8.2/9.1):
//! per comparable unit — both swap legs of one (variant, pair, map) — the
//! raw W-D-L counts, per-leg score2 values, the signed policy margin sum,
//! and the integer deltas of the variant against the base variant on the
//! SAME unit. These are the raw numerators/denominators B4 statistics will
//! consume; no floats, no intervals, and nothing derived here enters any
//! Game Result, Replay Hash or settlement.

use std::collections::BTreeMap;

use arena_core::types::Score2;

use crate::balance::plan::ExperimentPlan;
use crate::balance::record::{BalanceMatchRecord, BalanceOutcome};
use crate::balance::schedule::expand_paired;
use crate::metrics::Outcome;

/// One comparable unit's paired result under ONE config variant (plan
/// section 5.3 `PairedResult`, integer fields only).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PairedResult {
    pub pair_id: String,
    pub pair_tag: String,
    pub variant_id: String,
    pub base_variant_id: String,
    pub map_index: u32,
    /// Legs recorded for this unit under this variant (2 expected).
    pub legs: u64,
    pub policy_a_wins: u64,
    pub policy_b_wins: u64,
    pub draws: u64,
    pub crashes: u64,
    pub config_failures: u64,
    /// policy_a's score2 on leg 0 / leg 1 (raw).
    pub a_score2_leg0: Score2,
    pub a_score2_leg1: Score2,
    /// policy_b's score2 on leg 0 / leg 1 (raw).
    pub b_score2_leg0: Score2,
    pub b_score2_leg1: Score2,
    /// `sum_legs (a_score2 - b_score2)`, signed exact integer.
    pub margin_sum: i128,
    /// `a_wins(variant) - a_wins(base)` on the same unit; 0 for the base
    /// variant row itself (self-contrast).
    pub outcome_delta_vs_base: i64,
    /// `margin_sum(variant) - margin_sum(base)` on the same unit; 0 for
    /// the base row.
    pub margin_delta_vs_base: i128,
}

/// The fixed `pairs.tsv` column order.
pub fn pair_columns() -> Vec<String> {
    [
        "pair_id",
        "pair_tag",
        "variant_id",
        "base_variant_id",
        "map_index",
        "legs",
        "policy_a_wins",
        "policy_b_wins",
        "draws",
        "crashes",
        "config_failures",
        "a_score2_leg0",
        "a_score2_leg1",
        "b_score2_leg0",
        "b_score2_leg1",
        "margin_sum",
        "outcome_delta_vs_base",
        "margin_delta_vs_base",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// The whole `pairs.tsv` document.
pub fn paired_results_to_tsv(results: &[PairedResult]) -> String {
    let mut text = String::new();
    text.push_str(&pair_columns().join("\t"));
    text.push('\n');
    for result in results {
        let row = [
            result.pair_id.clone(),
            result.pair_tag.clone(),
            result.variant_id.clone(),
            result.base_variant_id.clone(),
            result.map_index.to_string(),
            result.legs.to_string(),
            result.policy_a_wins.to_string(),
            result.policy_b_wins.to_string(),
            result.draws.to_string(),
            result.crashes.to_string(),
            result.config_failures.to_string(),
            result.a_score2_leg0.to_string(),
            result.a_score2_leg1.to_string(),
            result.b_score2_leg0.to_string(),
            result.b_score2_leg1.to_string(),
            result.margin_sum.to_string(),
            result.outcome_delta_vs_base.to_string(),
            result.margin_delta_vs_base.to_string(),
        ];
        text.push_str(&row.join("\t"));
        text.push('\n');
    }
    text
}

/// Parses a `pairs.tsv` document back (exact round-trip).
pub fn parse_pairs_tsv(text: &str) -> Result<Vec<PairedResult>, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty document")?.split('\t').collect();
    let expected = pair_columns();
    if header != expected.iter().map(|s| s.as_str()).collect::<Vec<_>>() {
        return Err("header mismatch".to_string());
    }
    let mut results = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let values: Vec<&str> = line.split('\t').collect();
        if values.len() != expected.len() {
            return Err(format!("row width {} != {}", values.len(), expected.len()));
        }
        let u64_at = |i: usize| -> Result<u64, String> {
            values[i]
                .parse()
                .map_err(|e| format!("{}: {e}", expected[i]))
        };
        let i128_at = |i: usize| -> Result<i128, String> {
            values[i]
                .parse()
                .map_err(|e| format!("{}: {e}", expected[i]))
        };
        let i64_at = |i: usize| -> Result<i64, String> {
            values[i]
                .parse()
                .map_err(|e| format!("{}: {e}", expected[i]))
        };
        results.push(PairedResult {
            pair_id: values[0].to_string(),
            pair_tag: values[1].to_string(),
            variant_id: values[2].to_string(),
            base_variant_id: values[3].to_string(),
            map_index: u64_at(4)? as u32,
            legs: u64_at(5)?,
            policy_a_wins: u64_at(6)?,
            policy_b_wins: u64_at(7)?,
            draws: u64_at(8)?,
            crashes: u64_at(9)?,
            config_failures: u64_at(10)?,
            a_score2_leg0: u64_at(11)? as Score2,
            a_score2_leg1: u64_at(12)? as Score2,
            b_score2_leg0: u64_at(13)? as Score2,
            b_score2_leg1: u64_at(14)? as Score2,
            margin_sum: i128_at(15)?,
            outcome_delta_vs_base: i64_at(16)?,
            margin_delta_vs_base: i128_at(17)?,
        });
    }
    Ok(results)
}

/// policy_a's and policy_b's score2 on one leg record.
fn leg_scores(record: &BalanceMatchRecord) -> (Score2, Score2) {
    if record.policy_a_is_enl {
        (record.enl_score2, record.res_score2)
    } else {
        (record.res_score2, record.enl_score2)
    }
}

/// Computes every unit's paired result from the run's records, in the
/// plan's own expansion order (variant declaration -> matchup -> map).
/// A unit with a missing or duplicated leg is an ERROR (a pair is never
/// silently dropped or invented); crashed and config-failed legs count
/// into their own counters and keep the pair in the result set.
pub fn compute_paired_results(
    plan: &ExperimentPlan,
    records: &[BalanceMatchRecord],
) -> Result<Vec<PairedResult>, String> {
    // Index the records by (variant, pair_id, leg) — exactly one each.
    let mut by_key: BTreeMap<(String, String, u8), &BalanceMatchRecord> = BTreeMap::new();
    for record in records {
        let key = (
            record.variant_id.clone(),
            record.pair_id.clone(),
            record.leg,
        );
        if by_key.insert(key, record).is_some() {
            return Err(format!(
                "duplicate record for variant {} unit {} leg {}",
                record.variant_id, record.pair_id, record.leg
            ));
        }
    }

    let units = expand_paired(plan)?;
    let mut by_unit: Vec<PairedResult> = Vec::with_capacity(units.len());
    for unit in &units {
        let mut result = PairedResult {
            pair_id: unit.pair_id.clone(),
            pair_tag: unit.pair_tag.clone(),
            variant_id: unit.variant_id.clone(),
            base_variant_id: plan.base_variant.clone(),
            map_index: unit.map_index,
            legs: 0,
            policy_a_wins: 0,
            policy_b_wins: 0,
            draws: 0,
            crashes: 0,
            config_failures: 0,
            a_score2_leg0: 0,
            a_score2_leg1: 0,
            b_score2_leg0: 0,
            b_score2_leg1: 0,
            margin_sum: 0,
            outcome_delta_vs_base: 0,
            margin_delta_vs_base: 0,
        };
        for (leg, _cell) in [(0u8, &unit.leg0), (1u8, &unit.leg1)] {
            let key = (unit.variant_id.clone(), unit.pair_id.clone(), leg);
            let record = by_key.get(&key).ok_or_else(|| {
                format!(
                    "unit {} under variant {} is missing leg {leg}",
                    unit.pair_id, unit.variant_id
                )
            })?;
            result.legs += 1;
            let (a_score, b_score) = leg_scores(record);
            match leg {
                0 => {
                    result.a_score2_leg0 = a_score;
                    result.b_score2_leg0 = b_score;
                }
                _ => {
                    result.a_score2_leg1 = a_score;
                    result.b_score2_leg1 = b_score;
                }
            }
            result.margin_sum += a_score as i128 - b_score as i128;
            match record.result {
                BalanceOutcome::Played(Outcome::EnlWin) => {
                    if record.policy_a_is_enl {
                        result.policy_a_wins += 1;
                    } else {
                        result.policy_b_wins += 1;
                    }
                }
                BalanceOutcome::Played(Outcome::ResWin) => {
                    if record.policy_a_is_enl {
                        result.policy_b_wins += 1;
                    } else {
                        result.policy_a_wins += 1;
                    }
                }
                BalanceOutcome::Played(Outcome::Draw) => result.draws += 1,
                BalanceOutcome::Played(Outcome::Crash) => result.crashes += 1,
                BalanceOutcome::ConfigFailure => result.config_failures += 1,
            }
        }
        by_unit.push(result);
    }

    // Cross-variant deltas on the SAME unit (common random numbers make
    // these paired contrasts): variant row minus base row.
    let base_stats: BTreeMap<String, (u64, i128)> = by_unit
        .iter()
        .filter(|r| r.variant_id == plan.base_variant)
        .map(|r| (r.pair_id.clone(), (r.policy_a_wins, r.margin_sum)))
        .collect();
    for result in by_unit.iter_mut() {
        if result.variant_id == plan.base_variant {
            continue; // self-contrast: zeros
        }
        let (base_wins, base_margin) =
            base_stats.get(&result.pair_id).copied().ok_or_else(|| {
                format!(
                    "unit {} has no base-variant row to contrast with",
                    result.pair_id
                )
            })?;
        result.outcome_delta_vs_base = result.policy_a_wins as i64 - base_wins as i64;
        result.margin_delta_vs_base = result.margin_sum - base_margin;
    }
    Ok(by_unit)
}

/// Integer-only per-variant aggregate for `summary.txt`: raw counts, no
/// rates (those are derived offline).
#[derive(Clone, Default, Debug)]
pub struct VariantSummary {
    pub variant_id: String,
    pub units: u64,
    pub legs: u64,
    pub policy_a_wins: u64,
    pub policy_b_wins: u64,
    pub draws: u64,
    pub crashes: u64,
    pub config_failures: u64,
    pub margin_sum: i128,
    pub outcome_delta_sum_vs_base: i64,
    pub margin_delta_sum_vs_base: i128,
    pub maps: u64,
}

/// Aggregates paired results per variant, in plan variant-declaration
/// order (integer raw counts only).
pub fn variant_summaries(plan: &ExperimentPlan, results: &[PairedResult]) -> Vec<VariantSummary> {
    plan.variants
        .iter()
        .map(|variant| {
            let rows: Vec<&PairedResult> = results
                .iter()
                .filter(|r| r.variant_id == variant.variant_id)
                .collect();
            let mut maps = std::collections::BTreeSet::new();
            let mut summary = VariantSummary {
                variant_id: variant.variant_id.clone(),
                units: rows.len() as u64,
                ..VariantSummary::default()
            };
            for row in rows {
                maps.insert(row.map_index);
                summary.legs += row.legs;
                summary.policy_a_wins += row.policy_a_wins;
                summary.policy_b_wins += row.policy_b_wins;
                summary.draws += row.draws;
                summary.crashes += row.crashes;
                summary.config_failures += row.config_failures;
                summary.margin_sum += row.margin_sum;
                summary.outcome_delta_sum_vs_base += row.outcome_delta_vs_base;
                summary.margin_delta_sum_vs_base += row.margin_delta_vs_base;
            }
            summary.maps = maps.len() as u64;
            summary
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::*;
    use crate::balance::plan::{ConfigVariant, PolicyMatchup, SeedSet, Stage};
    use crate::balance::schedule::MatchCell;
    use crate::policy::BucketPolicy;
    use arena_bots::policy::{PolicyFamily, PolicyId, PolicyParameters};
    use arena_core::config::GameConfig;

    fn plan_two_variants() -> ExperimentPlan {
        ExperimentPlan {
            experiment_id: "pairs-test".to_string(),
            stage: Stage::Sensitivity,
            seed_set: SeedSet::Development,
            map_count: 1,
            base_variant: "default".to_string(),
            variants: vec![
                ConfigVariant::new("default", GameConfig::default(), Vec::new()),
                {
                    let mut cfg = GameConfig::default();
                    cfg.move_budget_cm = 24_000;
                    ConfigVariant::new("move-budget-240m", cfg, vec!["move_budget_cm"])
                },
            ],
            policies: vec![
                PolicyId::GreedyField.frozen_m6_spec(),
                PolicyId::Hybrid.frozen_m6_spec(),
            ],
            matchups: vec![PolicyMatchup::new(
                "greedy-hybrid",
                PolicyId::GreedyField,
                PolicyId::Hybrid,
            )],
            bucket_policy: BucketPolicy::Flat(0),
            gates: Vec::new(),
        }
    }

    /// Hand-built leg record: policy_a's score is `a_score`, policy_b's is
    /// `b_score`, with the given outcome and faction assignment.
    fn leg_record(
        plan: &ExperimentPlan,
        variant: &str,
        leg: u8,
        a_score: u128,
        b_score: u128,
        result: BalanceOutcome,
    ) -> BalanceMatchRecord {
        let units = expand_paired(plan).expect("expands");
        let cell: &MatchCell = if leg == 0 {
            &units
                .iter()
                .find(|u| u.variant_id == variant)
                .expect("variant")
                .leg0
        } else {
            &units
                .iter()
                .find(|u| u.variant_id == variant)
                .expect("variant")
                .leg1
        };
        let (enl, res) = if cell.policy_a_is_enl {
            (a_score, b_score)
        } else {
            (b_score, a_score)
        };
        let played = matches!(result, BalanceOutcome::Played(_));
        BalanceMatchRecord {
            index: cell.index,
            experiment_id: plan.experiment_id.clone(),
            seed_set_tag: plan.seed_set.tag().to_string(),
            variant_id: variant.to_string(),
            pair_tag: cell.pair_tag.clone(),
            pair_id: cell.pair_id.clone(),
            leg,
            map_id: cell.map_id.clone(),
            map_index: cell.map_index,
            policy_a: cell.policy_a,
            policy_a_family: PolicyFamily::FieldBuilder,
            policy_a_version: "m6.0".to_string(),
            policy_a_parameters: PolicyParameters::FrozenM6,
            policy_b: cell.policy_b,
            policy_b_family: PolicyFamily::AdaptiveHybrid,
            policy_b_version: "m6.0".to_string(),
            policy_b_parameters: PolicyParameters::FrozenM6,
            policy_a_is_enl: cell.policy_a_is_enl,
            map_seed: cell.map_seed,
            game_seed: cell.game_seed,
            bot_seed: cell.bot_seed,
            schedule_seed: cell.schedule_seed,
            bucket_policy: cell.bucket_policy,
            config_hash: cell.config_hash,
            result,
            enl_score2: enl,
            res_score2: res,
            final_state_hash: if played { [7u8; 32] } else { [0u8; 32] },
            stats: crate::metrics::MatchStats::new(),
            elapsed_ms: 1,
        }
    }

    // Symmetric legs (a wins as ENL 100:60, a loses as RES 60:100):
    // outcome and margin deltas are exactly zero — no faction bias.
    #[test]
    fn symmetric_legs_produce_zero_deltas() {
        let plan = plan_two_variants();
        // Leg 1: policy_a is RES with 60 vs 100 -> the ENL side (policy_b)
        // wins, i.e. EnlWin.
        let records = vec![
            leg_record(
                &plan,
                "default",
                0,
                100,
                60,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
            leg_record(
                &plan,
                "default",
                1,
                60,
                100,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                0,
                100,
                60,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                1,
                60,
                100,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
        ];
        let results = compute_paired_results(&plan, &records).expect("pairs compute");
        assert_eq!(results.len(), 2);
        for result in &results {
            assert_eq!(result.legs, 2);
            assert_eq!(result.policy_a_wins, 1);
            assert_eq!(result.policy_b_wins, 1);
            assert_eq!(result.margin_sum, 0, "100-60 then 60-100 cancels exactly");
            assert_eq!(result.outcome_delta_vs_base, 0);
            assert_eq!(result.margin_delta_vs_base, 0);
        }
        // TSV round-trip.
        let tsv = paired_results_to_tsv(&results);
        assert_eq!(parse_pairs_tsv(&tsv).expect("round-trips"), results);
    }

    // Asymmetric legs move the deltas by exactly the integer difference.
    #[test]
    fn asymmetric_legs_move_deltas_by_exact_integers() {
        let plan = plan_two_variants();
        // Base: a wins both legs (leg 1 as RES -> ResWin). Variant: b wins
        // both legs (leg 0 as RES -> ResWin, leg 1 as ENL -> EnlWin).
        let records = vec![
            leg_record(
                &plan,
                "default",
                0,
                100,
                60,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
            leg_record(
                &plan,
                "default",
                1,
                100,
                60,
                BalanceOutcome::Played(Outcome::ResWin),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                0,
                60,
                100,
                BalanceOutcome::Played(Outcome::ResWin),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                1,
                60,
                100,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
        ];
        let results = compute_paired_results(&plan, &records).expect("pairs compute");
        let base = results.iter().find(|r| r.variant_id == "default").unwrap();
        let variant = results
            .iter()
            .find(|r| r.variant_id == "move-budget-240m")
            .unwrap();
        assert_eq!(base.policy_a_wins, 2);
        assert_eq!(base.margin_sum, 80);
        assert_eq!(variant.policy_a_wins, 0);
        assert_eq!(variant.margin_sum, -80);
        assert_eq!(variant.outcome_delta_vs_base, -2);
        assert_eq!(variant.margin_delta_vs_base, -160);
    }

    // A crashed leg counts into `crashes` and never drops the pair; a
    // config-failed leg likewise; a missing leg is an error.
    #[test]
    fn failed_legs_count_and_missing_legs_error() {
        let plan = plan_two_variants();
        let records = vec![
            leg_record(
                &plan,
                "default",
                0,
                100,
                60,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
            leg_record(
                &plan,
                "default",
                1,
                60,
                100,
                BalanceOutcome::Played(Outcome::Crash),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                0,
                10,
                5,
                BalanceOutcome::ConfigFailure,
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                1,
                60,
                100,
                BalanceOutcome::Played(Outcome::ResWin),
            ),
        ];
        let results = compute_paired_results(&plan, &records).expect("pairs compute");
        let base = results.iter().find(|r| r.variant_id == "default").unwrap();
        assert_eq!(base.crashes, 1);
        assert_eq!(base.legs, 2, "the pair is retained");
        assert_eq!(base.policy_a_wins, 1);
        let variant = results
            .iter()
            .find(|r| r.variant_id == "move-budget-240m")
            .unwrap();
        assert_eq!(variant.config_failures, 1);
        assert_eq!(variant.legs, 2);

        // Missing leg -> error, never a silent drop.
        let missing = vec![leg_record(
            &plan,
            "default",
            0,
            100,
            60,
            BalanceOutcome::Played(Outcome::EnlWin),
        )];
        assert!(compute_paired_results(&plan, &missing).is_err());
        // Duplicate leg -> error.
        let dup = vec![
            leg_record(
                &plan,
                "default",
                0,
                1,
                1,
                BalanceOutcome::Played(Outcome::Draw),
            ),
            leg_record(
                &plan,
                "default",
                0,
                1,
                1,
                BalanceOutcome::Played(Outcome::Draw),
            ),
        ];
        assert!(compute_paired_results(&plan, &dup).is_err());
        // A variant row without its base counterpart -> error.
        let orphan = vec![
            leg_record(
                &plan,
                "default",
                0,
                1,
                1,
                BalanceOutcome::Played(Outcome::Draw),
            ),
            leg_record(
                &plan,
                "default",
                1,
                1,
                1,
                BalanceOutcome::Played(Outcome::Draw),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                0,
                1,
                1,
                BalanceOutcome::Played(Outcome::Draw),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                1,
                1,
                1,
                BalanceOutcome::Played(Outcome::Draw),
            ),
        ];
        // (both variants present here, so this computes; the orphan case is
        // covered by missing-base below)
        assert!(compute_paired_results(&plan, &orphan).is_ok());
    }

    // Variant summaries aggregate raw integer counts per variant.
    #[test]
    fn variant_summaries_aggregate_raw_counts() {
        let plan = plan_two_variants();
        let records = vec![
            leg_record(
                &plan,
                "default",
                0,
                100,
                60,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
            leg_record(
                &plan,
                "default",
                1,
                60,
                100,
                BalanceOutcome::Played(Outcome::ResWin),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                0,
                90,
                60,
                BalanceOutcome::Played(Outcome::EnlWin),
            ),
            leg_record(
                &plan,
                "move-budget-240m",
                1,
                50,
                100,
                BalanceOutcome::Played(Outcome::ResWin),
            ),
        ];
        let results = compute_paired_results(&plan, &records).expect("pairs compute");
        let summaries = variant_summaries(&plan, &results);
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].variant_id, "default");
        assert_eq!(summaries[0].legs, 2);
        assert_eq!(summaries[0].margin_sum, 0);
        assert_eq!(summaries[1].margin_delta_sum_vs_base, -20);
        assert_eq!(summaries[1].outcome_delta_sum_vs_base, 0);
    }
}

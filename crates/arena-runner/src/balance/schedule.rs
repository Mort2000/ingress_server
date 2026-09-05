//! Paired scheduling (B1, `docs/balance/ai-balance-plan.md` section 8.2,
//! ADR-0014): expands an [`ExperimentPlan`] into the deterministic list of
//! match cells. The comparable unit is `(map, game_seed, bot_seed, policy
//! pair, bucket policy)`; every unit appears EXACTLY TWICE per variant
//! with opposite faction assignments (leg 0 = policy_a as ENL, leg 1 =
//! policy_a as RES — an explicit deterministic swap, not the D55 want-bit
//! schedule), and config contrasts reuse the IDENTICAL unit seeds.

use arena_bots::policy::PolicyId;

use crate::balance::plan::{ExperimentPlan, SeedSet};
use crate::policy::BucketPolicy;

/// The stable map identity inside artifacts: `<seed-set-tag>-<k:03>`.
pub fn map_id(seed_set: SeedSet, k: u32) -> String {
    format!("{}-{:03}", seed_set.tag(), k)
}

/// The stable comparable-unit identity inside artifacts:
/// `<pair-tag>#m<k:03>` — shared by BOTH legs and ALL config variants of
/// that unit (the pairing key of `pairs.tsv`).
pub fn pair_id(pair_tag: &str, k: u32) -> String {
    format!("{pair_tag}#m{k:03}")
}

/// Everything one balance match needs, derived up front (the balance
/// counterpart of the frozen m6 `MatchDescriptor`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MatchCell {
    /// Global 0-based expansion index (row order of `matches.tsv`).
    pub index: u64,
    pub experiment_id: String,
    pub seed_set_tag: String,
    pub variant_id: String,
    /// The matchup (policy pair) tag.
    pub pair_tag: String,
    /// The comparable-unit id, shared across legs and variants.
    pub pair_id: String,
    /// 0 = policy_a plays ENL; 1 = policy_a plays RES.
    pub leg: u8,
    pub map_id: String,
    pub map_index: u32,
    pub policy_a: PolicyId,
    pub policy_b: PolicyId,
    /// Leg 0 -> true, leg 1 -> false (by construction).
    pub policy_a_is_enl: bool,
    pub map_seed: u64,
    pub game_seed: u64,
    pub bot_seed: u64,
    pub schedule_seed: u64,
    pub bucket_policy: BucketPolicy,
    pub config_hash: u64,
}

impl MatchCell {
    /// `(a_policy, b_policy)` ordered ENL-first for the executor.
    pub fn faction_policies(&self) -> (PolicyId, PolicyId) {
        if self.policy_a_is_enl {
            (self.policy_a, self.policy_b)
        } else {
            (self.policy_b, self.policy_a)
        }
    }
}

/// One comparable unit under ONE config variant: both swap legs plus the
/// unit's shared seeds (the pairing input of `pairs.tsv`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PairedCell {
    pub variant_id: String,
    pub pair_tag: String,
    pub pair_id: String,
    pub map_index: u32,
    pub map_seed: u64,
    pub game_seed: u64,
    pub bot_seed: u64,
    pub policy_a: PolicyId,
    pub policy_b: PolicyId,
    /// policy_a as ENL.
    pub leg0: MatchCell,
    /// policy_a as RES.
    pub leg1: MatchCell,
}

/// Expands the plan into its match cells. Pure and deterministic: the
/// order is variant (declaration) -> matchup (declaration) -> map index
/// ascending -> leg 0, leg 1; no HashMap iteration anywhere. Refuses an
/// invalid plan (plan-build-time rules apply to the schedule too).
pub fn expand(plan: &ExperimentPlan) -> Result<Vec<MatchCell>, String> {
    plan.validate()?;
    let schedule_seed = plan.schedule_seed();
    let mut cells =
        Vec::with_capacity(plan.variants.len() * plan.matchups.len() * plan.map_count as usize * 2);
    let mut index = 0u64;
    for variant in &plan.variants {
        for matchup in &plan.matchups {
            for k in 0..plan.map_count {
                let game_seed = crate::balance::plan::game_seed(plan.seed_set, &matchup.tag, k);
                let bot_seed = crate::balance::plan::bot_seed(plan.seed_set, &matchup.tag, k);
                let map_seed = crate::balance::plan::map_seed(plan.seed_set, k);
                for leg in [0u8, 1u8] {
                    cells.push(MatchCell {
                        index,
                        experiment_id: plan.experiment_id.clone(),
                        seed_set_tag: plan.seed_set.tag().to_string(),
                        variant_id: variant.variant_id.clone(),
                        pair_tag: matchup.tag.clone(),
                        pair_id: pair_id(&matchup.tag, k),
                        leg,
                        map_id: map_id(plan.seed_set, k),
                        map_index: k,
                        policy_a: matchup.a,
                        policy_b: matchup.b,
                        policy_a_is_enl: leg == 0,
                        map_seed,
                        game_seed,
                        bot_seed,
                        schedule_seed,
                        bucket_policy: plan.bucket_policy,
                        config_hash: variant.config_hash(),
                    });
                    index += 1;
                }
            }
        }
    }
    Ok(cells)
}

/// Groups [`expand`] into per-(variant, unit) `PairedCell`s, in the same
/// total order.
pub fn expand_paired(plan: &ExperimentPlan) -> Result<Vec<PairedCell>, String> {
    let cells = expand(plan)?;
    let mut paired = Vec::with_capacity(cells.len() / 2);
    let mut cells = cells.into_iter().peekable();
    while let Some(leg0) = cells.next() {
        let leg1 = cells.next().expect("legs come in pairs");
        assert_eq!(leg0.leg, 0, "leg 0 first (total order)");
        assert_eq!(leg1.leg, 1, "leg 1 second (total order)");
        assert_eq!(leg0.variant_id, leg1.variant_id);
        assert_eq!(leg0.pair_id, leg1.pair_id);
        assert!(leg0.policy_a_is_enl && !leg1.policy_a_is_enl);
        paired.push(PairedCell {
            variant_id: leg0.variant_id.clone(),
            pair_tag: leg0.pair_tag.clone(),
            pair_id: leg0.pair_id.clone(),
            map_index: leg0.map_index,
            map_seed: leg0.map_seed,
            game_seed: leg0.game_seed,
            bot_seed: leg0.bot_seed,
            policy_a: leg0.policy_a,
            policy_b: leg0.policy_b,
            leg0,
            leg1,
        });
    }
    Ok(paired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_bots::policy::PolicyId;
    use arena_core::config::GameConfig;

    fn two_variant_plan(maps: u32) -> ExperimentPlan {
        ExperimentPlan {
            experiment_id: "sched-test".to_string(),
            stage: crate::balance::plan::Stage::Sensitivity,
            seed_set: SeedSet::Development,
            map_count: maps,
            base_variant: "default".to_string(),
            variants: vec![
                crate::balance::plan::ConfigVariant::new(
                    "default",
                    GameConfig::default(),
                    Vec::new(),
                ),
                {
                    let mut cfg = GameConfig::default();
                    cfg.move_budget_cm = 24_000;
                    crate::balance::plan::ConfigVariant::new(
                        "move-budget-240m",
                        cfg,
                        vec!["move_budget_cm"],
                    )
                },
            ],
            policies: vec![
                PolicyId::GreedyField.frozen_m6_spec(),
                PolicyId::Hybrid.frozen_m6_spec(),
            ],
            matchups: vec![crate::balance::plan::PolicyMatchup::new(
                "greedy-hybrid",
                PolicyId::GreedyField,
                PolicyId::Hybrid,
            )],
            bucket_policy: BucketPolicy::Flat(0),
            gates: Vec::new(),
        }
    }

    // Every unit yields exactly leg0 (policy_a ENL) + leg1 (policy_a RES),
    // the leg pair shares game_seed/bot_seed/map_seed, and the SAME unit
    // seeds appear under both variant ids (common random numbers).
    #[test]
    fn every_unit_swaps_factions_with_shared_seeds_across_variants() {
        let plan = two_variant_plan(3);
        let cells = expand(&plan).expect("plan expands");
        assert_eq!(
            cells.len(),
            2 * 1 * 3 * 2,
            "variants x matchups x maps x legs"
        );
        for variant in ["default", "move-budget-240m"] {
            for k in 0..3u32 {
                let unit: Vec<&MatchCell> = cells
                    .iter()
                    .filter(|c| c.variant_id == variant && c.map_index == k)
                    .collect();
                assert_eq!(unit.len(), 2, "exactly two legs per unit");
                let (leg0, leg1) = (unit[0], unit[1]);
                assert_eq!(leg0.leg, 0);
                assert!(leg0.policy_a_is_enl, "leg 0: policy_a is ENL");
                assert_eq!(leg1.leg, 1);
                assert!(!leg1.policy_a_is_enl, "leg 1: policy_a is RES");
                assert_eq!(leg0.game_seed, leg1.game_seed);
                assert_eq!(leg0.bot_seed, leg1.bot_seed);
                assert_eq!(leg0.map_seed, leg1.map_seed);
                assert_eq!(leg0.pair_id, leg1.pair_id);
                // Cross-variant common random numbers: the same unit seeds
                // under the OTHER variant id.
                let other: Vec<&MatchCell> = cells
                    .iter()
                    .filter(|c| {
                        c.variant_id != variant && c.map_index == k && c.pair_tag == leg0.pair_tag
                    })
                    .collect();
                assert_eq!(other.len(), 2);
                assert!(other.iter().any(|c| c.game_seed == leg0.game_seed));
                assert!(other.iter().any(|c| c.bot_seed == leg0.bot_seed));
            }
        }
    }

    // No duplicate cells; expansion is a pure function with a total,
    // stable order.
    #[test]
    fn expansion_is_pure_total_and_duplicate_free() {
        let plan = two_variant_plan(4);
        let first = expand(&plan).expect("expands");
        let second = expand(&plan).expect("expands");
        assert_eq!(first, second, "expand is pure");

        let mut keys: Vec<(String, String, u32, u8)> = first
            .iter()
            .map(|c| (c.variant_id.clone(), c.pair_tag.clone(), c.map_index, c.leg))
            .collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), total, "no duplicate cells");

        // Total order: indices are dense and cells are grouped variant ->
        // matchup -> map -> leg.
        let indices: Vec<u64> = first.iter().map(|c| c.index).collect();
        assert_eq!(indices, (0..first.len() as u64).collect::<Vec<_>>());
        let variant_order: Vec<String> = {
            let mut v: Vec<String> = first.iter().map(|c| c.variant_id.clone()).collect();
            v.dedup();
            v
        };
        assert_eq!(
            variant_order,
            vec!["default".to_string(), "move-budget-240m".to_string()],
            "variant declaration order"
        );
        let map_order: Vec<u32> = {
            let mut m: Vec<u32> = first
                .iter()
                .filter(|c| c.variant_id == "default")
                .map(|c| c.map_index)
                .collect();
            m.dedup();
            m
        };
        assert_eq!(map_order, vec![0, 1, 2, 3], "ascending map order");
    }

    // Cell seeds/config hashes are the plan's own derivations, and the
    // paired view groups legs with the shared unit fields.
    #[test]
    fn cells_carry_plan_derivations_and_pair_cells_group_legs() {
        let plan = two_variant_plan(2);
        let cells = expand(&plan).expect("expands");
        for cell in &cells {
            assert_eq!(
                cell.game_seed,
                crate::balance::plan::game_seed(plan.seed_set, &cell.pair_tag, cell.map_index)
            );
            assert_eq!(
                cell.bot_seed,
                crate::balance::plan::bot_seed(plan.seed_set, &cell.pair_tag, cell.map_index)
            );
            assert_eq!(
                cell.map_seed,
                crate::balance::plan::map_seed(plan.seed_set, cell.map_index)
            );
            let variant = plan
                .variants
                .iter()
                .find(|v| v.variant_id == cell.variant_id)
                .expect("variant exists");
            assert_eq!(cell.config_hash, variant.config_hash());
            assert_eq!(cell.schedule_seed, plan.schedule_seed());
            assert_eq!(cell.map_id, map_id(plan.seed_set, cell.map_index));
            assert_eq!(cell.pair_id, pair_id(&cell.pair_tag, cell.map_index));
            assert_eq!(cell.bucket_policy, plan.bucket_policy);
        }
        let paired = expand_paired(&plan).expect("pairs");
        assert_eq!(paired.len(), 2 * 1 * 2);
        for unit in &paired {
            assert_eq!(unit.leg0.game_seed, unit.game_seed);
            assert_eq!(unit.leg1.bot_seed, unit.bot_seed);
            assert_eq!(unit.leg0.policy_a, unit.policy_a);
            assert_eq!(unit.leg1.policy_b, unit.policy_b);
        }
    }
}

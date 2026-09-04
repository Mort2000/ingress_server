//! The B4 single-variable sensitivity experiments
//! (`docs/ai-balance-plan.md` sections 7.3/7.4/8.3, ADR-0018): the
//! frozen plan-7.3 candidate grid as DATA (13 single-field variants
//! beside the untouched default — candidates are Pilot starting points,
//! not preset conclusions), the six frozen competitive matchups at FIXED
//! r2_spec() parameters (the plan-7.3 "固定 R2 策略参数" clause), the
//! pilot/full/interaction plan builders, and the B4 replay sampling.
//! Nothing here executes matches or derives statistics.

use arena_bots::policy::PolicyId;
use arena_core::config::GameConfig;

use crate::balance::plan::{ConfigVariant, ExperimentPlan, PolicyMatchup, SeedSet, Stage};
use crate::balance::schedule::MatchCell;
use crate::policy::BucketPolicy;

/// The frozen experiment ids (archive directory names; ADR-0018). A
/// defective run is re-issued as `-r2` with the defective directory kept
/// as history, never overwritten; scratch pilots append `-pilot-r0`.
pub const B4_PILOT_EXPERIMENT_ID: &str = "b4-sensitivity-pilot-r1";
pub const B4_FULL_DEV_EXPERIMENT_ID: &str = "b4-sensitivity-full-dev-r1";
pub const B4_FULL_VAL_EXPERIMENT_ID: &str = "b4-sensitivity-full-val-r1";

/// The official map counts (plan section 8.3: Pilot >= 100 Development
/// maps with the faction swap; Full extends stable candidates to all of
/// Development + Validation).
pub const B4_PILOT_MAP_COUNT: u32 = 100;
pub const B4_FULL_DEV_MAP_COUNT: u32 = 300;
pub const B4_FULL_VAL_MAP_COUNT: u32 = 100;

/// The scratch-pilot map count (`--pilot`: shrunk maps ONLY — plan 8.3
/// never reduces swap legs or pairings).
pub const B4_PILOT_SCRATCH_MAP_COUNT: u32 = 8;

/// Replays are sampled from leg 0 of the first N maps per
/// (variant, pair) — B3's 25-map precedent (ADR-0018).
pub const B4_REPLAY_MAPS: u32 = 25;

// ---------------------------------------------------------------------
// The frozen candidate grid (plan section 7.3, ADR-0018)
// ---------------------------------------------------------------------

/// One row of the plan-7.3 grid: a single-field candidate variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct B4Candidate {
    pub variant_id: &'static str,
    /// The one declared changed field (canonical config field name).
    pub field: &'static str,
    /// The candidate's value for that field (all seven fields are u32).
    pub value: u32,
    /// The plan-7.3 主要观察 column (report rendering).
    pub focus: &'static str,
}

/// The thirteen candidate variants of plan 7.3 (seven parameters; every
/// non-default level — `RECHARGE_COST` has a single one), declaration
/// order = plan-table row order (low value first). Each changes EXACTLY
/// one field against the untouched `GameConfig::default()`.
pub const B4_CANDIDATES: [B4Candidate; 13] = [
    B4Candidate {
        variant_id: "move-budget-160m",
        field: "move_budget_cm",
        value: 16_000,
        focus: "First Field, map coverage, comeback rate, policy ordering",
    },
    B4Candidate {
        variant_id: "move-budget-240m",
        field: "move_budget_cm",
        value: 24_000,
        focus: "First Field, map coverage, comeback rate, policy ordering",
    },
    B4Candidate {
        variant_id: "max-link-1500m",
        field: "max_link_distance_cm",
        value: 150_000,
        focus: "Field area, count, Blocking, snowball",
    },
    B4Candidate {
        variant_id: "max-link-2500m",
        field: "max_link_distance_cm",
        value: 250_000,
        focus: "Field area, count, Blocking, snowball",
    },
    B4Candidate {
        variant_id: "attack-damage-15",
        field: "attack_damage",
        value: 15,
        focus: "destroy time, attack opportunity cost, Field lifetime",
    },
    B4Candidate {
        variant_id: "attack-damage-25",
        field: "attack_damage",
        value: 25,
        focus: "destroy time, attack opportunity cost, Field lifetime",
    },
    B4Candidate {
        variant_id: "recharge-amount-10",
        field: "recharge_amount",
        value: 10,
        focus: "defense trigger rate, survival payoff, Supply consumption",
    },
    B4Candidate {
        variant_id: "recharge-amount-20",
        field: "recharge_amount",
        value: 20,
        focus: "defense trigger rate, survival payoff, Supply consumption",
    },
    B4Candidate {
        variant_id: "recharge-cost-2",
        field: "recharge_cost_supply",
        value: 2,
        focus: "defense choice vs economic constraint",
    },
    B4Candidate {
        variant_id: "initial-supply-10",
        field: "initial_supply",
        value: 10,
        focus: "opening action space, Hack demand, idle resources",
    },
    B4Candidate {
        variant_id: "initial-supply-40",
        field: "initial_supply",
        value: 40,
        focus: "opening action space, Hack demand, idle resources",
    },
    B4Candidate {
        variant_id: "portal-count-80",
        field: "portal_count",
        value: 80,
        focus: "geometric opportunity, contest density, compute cost",
    },
    B4Candidate {
        variant_id: "portal-count-160",
        field: "portal_count",
        value: 160,
        focus: "geometric opportunity, contest density, compute cost",
    },
];

/// Sets one grid field on a default config (the frozen grid's own
/// vocabulary; unknown fields are a programmer error).
pub fn apply_grid_field(cfg: &mut GameConfig, field: &str, value: u32) {
    match field {
        "move_budget_cm" => cfg.move_budget_cm = value,
        "max_link_distance_cm" => cfg.max_link_distance_cm = value,
        "attack_damage" => cfg.attack_damage = value,
        "recharge_amount" => cfg.recharge_amount = value,
        "recharge_cost_supply" => cfg.recharge_cost_supply = value,
        "initial_supply" => cfg.initial_supply = value,
        "portal_count" => cfg.portal_count = value,
        other => panic!("field {other} is not in the B4 grid"),
    }
}

/// The candidate as a complete `ConfigVariant` (default config + the one
/// declared field).
pub fn candidate_variant(candidate: &B4Candidate) -> ConfigVariant {
    let mut cfg = GameConfig::default();
    apply_grid_field(&mut cfg, candidate.field, candidate.value);
    ConfigVariant::new(candidate.variant_id, cfg, vec![candidate.field])
}

/// The frozen factor short codes of interaction experiment ids
/// (ADR-0018): `b4-interaction-<a>-<b>-r1`.
pub fn factor_code(field: &str) -> &'static str {
    match field {
        "move_budget_cm" => "mb",
        "max_link_distance_cm" => "mld",
        "attack_damage" => "ad",
        "recharge_amount" => "ra",
        "recharge_cost_supply" => "rc",
        "initial_supply" => "is",
        "portal_count" => "pc",
        other => panic!("field {other} is not a B4 factor"),
    }
}

/// The frozen interaction priority order (plan 7.4): interactions run
/// only where BOTH factors have stable main effects, in this order.
pub const B4_INTERACTION_PRIORITY: [(&str, &str); 4] = [
    ("move_budget_cm", "max_link_distance_cm"),
    ("attack_damage", "recharge_amount"),
    ("attack_damage", "recharge_cost_supply"),
    ("portal_count", "max_link_distance_cm"),
];

// ---------------------------------------------------------------------
// The frozen roster (ADR-0018)
// ---------------------------------------------------------------------

/// The six frozen matchups over `PolicyId::R2_COMPETITIVE` (>= 3
/// independent pairs per plan 10.3, chosen around the B3-flagged
/// rule-risk domination; no mirrors — faction bias is covered by the
/// swap legs; no m6 baselines — the era bridge is B3's ladder).
pub fn b4_matchups() -> Vec<PolicyMatchup> {
    vec![
        PolicyMatchup::new(
            "greedy-coordinated-r2--builder-disruptor-r2",
            PolicyId::GreedyCoordinatedR2,
            PolicyId::BuilderDisruptorR2,
        ),
        PolicyMatchup::new(
            "builder-defender-r2--builder-disruptor-r2",
            PolicyId::BuilderDefenderR2,
            PolicyId::BuilderDisruptorR2,
        ),
        PolicyMatchup::new(
            "builder-disruptor-r2--adaptive-hybrid-r2",
            PolicyId::BuilderDisruptorR2,
            PolicyId::AdaptiveHybridR2,
        ),
        PolicyMatchup::new(
            "greedy-coordinated-r2--builder-defender-r2",
            PolicyId::GreedyCoordinatedR2,
            PolicyId::BuilderDefenderR2,
        ),
        PolicyMatchup::new(
            "logistics-builder-r2--blocker-builder-r2",
            PolicyId::LogisticsBuilderR2,
            PolicyId::BlockerBuilderR2,
        ),
        PolicyMatchup::new(
            "greedy-coordinated-r2--logistics-builder-r2",
            PolicyId::GreedyCoordinatedR2,
            PolicyId::LogisticsBuilderR2,
        ),
    ]
}

/// The B4 policy registry: the six competitive R2 policies at their
/// FIXED r2_spec() parameters (version r2.0, full-value records per
/// ADR-0016 — plan 7.3's fixed-R2-parameter clause).
pub fn b4_policies() -> Vec<arena_bots::policy::PolicySpec> {
    PolicyId::R2_COMPETITIVE
        .iter()
        .map(|&id| id.r2_spec())
        .collect()
}

/// The B4 acceptance gates (plan sections 10.1/10.2, ADR-0018).
pub fn b4_gates() -> Vec<crate::balance::plan::Gate> {
    vec![
        crate::balance::plan::Gate::Pending {
            gate_id: "records-integrity".to_string(),
            statement: "every match record's policy family/version/parameters match policies.tsv"
                .to_string(),
        },
        crate::balance::plan::Gate::Pending {
            gate_id: "no-crashes".to_string(),
            statement: "zero crash and zero config-failure records in the experiment".to_string(),
        },
        crate::balance::plan::Gate::Pending {
            gate_id: "command-drop".to_string(),
            statement: "pooled competitive commands_dropped/commands_submitted < 50 permille"
                .to_string(),
        },
    ]
}

/// The base variant: the untouched `GameConfig::default()` (the B4 grid
/// never modifies the default).
pub fn b4_base_variant() -> ConfigVariant {
    ConfigVariant::new("default", GameConfig::default(), Vec::new())
}

// ---------------------------------------------------------------------
// The frozen plans
// ---------------------------------------------------------------------

/// The frozen pilot plan: ONE plan carrying the base plus ALL fourteen
/// candidates (the base runs once, not once per parameter; units are
/// shared across contrasts by construction), stage `sensitivity`,
/// Development maps.
pub fn b4_pilot_plan(map_count: u32, experiment_id: &str) -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Sensitivity,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: std::iter::once(b4_base_variant())
            .chain(B4_CANDIDATES.iter().map(candidate_variant))
            .collect(),
        policies: b4_policies(),
        matchups: b4_matchups(),
        bucket_policy: BucketPolicy::Flat(0),
        gates: b4_gates(),
    }
}

/// The Full plan over one seed set: base plus the STABLE candidates
/// derived from pilot data by the frozen stability rule
/// (`effects::stable_candidates` is the only bridge — ADR-0018). One
/// seed set per plan, so Full = two runs (Development 300, Validation
/// 100). Refuses an empty or unknown stable set with the recorded
/// reason.
pub fn b4_full_plan(
    seed_set: SeedSet,
    map_count: u32,
    stable_arms: &[String],
    experiment_id: &str,
) -> Result<ExperimentPlan, String> {
    if stable_arms.is_empty() {
        return Err(format!(
            "full experiment {experiment_id}: no stable candidate survived the pilot stability \
             rule — nothing to extend (record the per-direction stops instead)"
        ));
    }
    let mut variants = vec![b4_base_variant()];
    for candidate in &B4_CANDIDATES {
        if stable_arms.iter().any(|arm| arm == candidate.variant_id) {
            variants.push(candidate_variant(candidate));
        }
    }
    if variants.len() != stable_arms.len() + 1 {
        let unknown: Vec<&String> = stable_arms
            .iter()
            .filter(|arm| !B4_CANDIDATES.iter().any(|c| c.variant_id == arm.as_str()))
            .collect();
        return Err(format!(
            "full experiment {experiment_id}: stable arms {unknown:?} are not grid variants"
        ));
    }
    Ok(ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Sensitivity,
        seed_set,
        map_count,
        base_variant: "default".to_string(),
        variants,
        policies: b4_policies(),
        matchups: b4_matchups(),
        bucket_policy: BucketPolicy::Flat(0),
        gates: b4_gates(),
    })
}

/// The frozen interaction plan of two factors (plan 7.4 small factorial
/// experiment, ADR-0018): `[base, A, B, A+B]` — the A+B arm declares
/// exactly the two factor fields. `a` and `b` are the candidates chosen
/// by the frozen arm rule (largest |pooled margin_delta sum|, tie ->
/// lexicographically smaller variant_id).
pub fn b4_interaction_plan(
    a: &B4Candidate,
    b: &B4Candidate,
    map_count: u32,
    experiment_id: &str,
) -> ExperimentPlan {
    assert_ne!(
        a.field, b.field,
        "an interaction needs two distinct factors"
    );
    let mut combined_cfg = GameConfig::default();
    apply_grid_field(&mut combined_cfg, a.field, a.value);
    apply_grid_field(&mut combined_cfg, b.field, b.value);
    let combined = ConfigVariant::new(
        &format!("{}+{}", a.variant_id, b.variant_id),
        combined_cfg,
        vec![a.field, b.field],
    );
    ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Interaction,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: vec![
            b4_base_variant(),
            candidate_variant(a),
            candidate_variant(b),
            combined,
        ],
        policies: b4_policies(),
        matchups: b4_matchups(),
        bucket_policy: BucketPolicy::Flat(0),
        gates: b4_gates(),
    }
}

/// The interaction experiment id of two factor fields (ADR-0018 short
/// codes): `b4-interaction-<a-code>-<b-code>-r1`.
pub fn interaction_experiment_id(a_field: &str, b_field: &str) -> String {
    format!(
        "b4-interaction-{}-{}-r1",
        factor_code(a_field),
        factor_code(b_field)
    )
}

/// The B4 replay sampling (ADR-0018): leg 0 of the first
/// [`B4_REPLAY_MAPS`] maps per (variant, pair) — 14 x 6 x 25 = 2,100
/// IARP replays on the official pilot.
pub fn b4_replay_sample(cell: &MatchCell) -> bool {
    cell.leg == 0 && cell.map_index < B4_REPLAY_MAPS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::balance::plan::validate_contrast;

    fn candidate(id: &str) -> B4Candidate {
        *B4_CANDIDATES
            .iter()
            .find(|c| c.variant_id == id)
            .unwrap_or_else(|| panic!("unknown candidate {id}"))
    }

    // The frozen grid, pinned field-by-field against plan section 7.3
    // (each candidate changes EXACTLY one declared field vs default and
    // the default itself is untouched).
    #[test]
    fn the_thirteen_candidate_variants_match_plan_73_and_validate() {
        assert_eq!(B4_CANDIDATES.len(), 13);
        let expected: [(&str, &str, u32); 13] = [
            ("move-budget-160m", "move_budget_cm", 16_000),
            ("move-budget-240m", "move_budget_cm", 24_000),
            ("max-link-1500m", "max_link_distance_cm", 150_000),
            ("max-link-2500m", "max_link_distance_cm", 250_000),
            ("attack-damage-15", "attack_damage", 15),
            ("attack-damage-25", "attack_damage", 25),
            ("recharge-amount-10", "recharge_amount", 10),
            ("recharge-amount-20", "recharge_amount", 20),
            ("recharge-cost-2", "recharge_cost_supply", 2),
            ("initial-supply-10", "initial_supply", 10),
            ("initial-supply-40", "initial_supply", 40),
            ("portal-count-80", "portal_count", 80),
            ("portal-count-160", "portal_count", 160),
        ]
        .into_iter()
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
        for (spec, (id, field, value)) in B4_CANDIDATES.iter().zip(expected) {
            assert_eq!(spec.variant_id, id);
            assert_eq!(spec.field, field);
            assert_eq!(spec.value, value);
            assert!(!spec.focus.is_empty(), "{id} carries its focus metrics");
        }
        // The frozen defaults the grid contrasts against (plan 7.3).
        let default = GameConfig::default();
        assert_eq!(default.move_budget_cm, 20_000);
        assert_eq!(default.max_link_distance_cm, 200_000);
        assert_eq!(default.attack_damage, 20);
        assert_eq!(default.recharge_amount, 15);
        assert_eq!(default.recharge_cost_supply, 1);
        assert_eq!(default.initial_supply, 20);
        assert_eq!(default.portal_count, 120);

        let base = b4_base_variant();
        assert_eq!(base.config, GameConfig::default(), "default untouched");
        assert_eq!(base.config_hash(), 0x9ead_0522_19fa_7ff3);
        // Every candidate: complete config validates, declared-only diff
        // against base, exactly one field, map-regime iff portal_count.
        for spec in &B4_CANDIDATES {
            let variant = candidate_variant(spec);
            assert_eq!(variant.config.validate(), Ok(()));
            assert_eq!(validate_contrast(&variant, &base), Ok(()));
            assert_eq!(variant.declared_changed_fields, vec![spec.field]);
            assert_eq!(
                variant.is_map_regime(),
                spec.field == "portal_count",
                "{}: only the portal-count arms are map-regime (ADR-0018)",
                spec.variant_id
            );
        }
        // The whole grid sits in one valid sensitivity plan.
        let plan = b4_pilot_plan(4, "grid-check");
        assert_eq!(plan.validate(), Ok(()));
        assert_eq!(plan.variants.len(), 14, "base + 13 candidates");
    }

    // The pilot plan's frozen shape: id, stage, seed set, policies at
    // fixed r2 parameters, the six matchup tags, gates.
    #[test]
    fn b4_plans_are_frozen_and_valid() {
        let plan = b4_pilot_plan(B4_PILOT_MAP_COUNT, B4_PILOT_EXPERIMENT_ID);
        assert_eq!(plan.experiment_id, B4_PILOT_EXPERIMENT_ID);
        assert_eq!(plan.stage, Stage::Sensitivity);
        assert_eq!(plan.seed_set, SeedSet::Development);
        assert_eq!(plan.map_count, 100);
        assert_eq!(plan.base_variant, "default");
        assert_eq!(plan.bucket_policy, BucketPolicy::Flat(0));
        // Fixed R2 policy parameters: the six competitive policies at
        // r2_spec(), version r2.0, full-value records.
        assert_eq!(plan.policies.len(), 6);
        for spec in &plan.policies {
            assert_eq!(spec.version, "r2.0");
            assert_eq!(spec, &spec.policy_id.r2_spec());
            assert!(!spec.family.is_diagnostic());
        }
        // The six frozen matchup tags, in order.
        let tags: Vec<&str> = plan.matchups.iter().map(|m| m.tag.as_str()).collect();
        assert_eq!(
            tags,
            vec![
                "greedy-coordinated-r2--builder-disruptor-r2",
                "builder-defender-r2--builder-disruptor-r2",
                "builder-disruptor-r2--adaptive-hybrid-r2",
                "greedy-coordinated-r2--builder-defender-r2",
                "logistics-builder-r2--blocker-builder-r2",
                "greedy-coordinated-r2--logistics-builder-r2",
            ]
        );
        let gate_ids: Vec<&str> = plan
            .gates
            .iter()
            .map(|g| match g {
                crate::balance::plan::Gate::Pending { gate_id, .. } => gate_id.as_str(),
            })
            .collect();
        assert_eq!(
            gate_ids,
            vec!["records-integrity", "no-crashes", "command-drop"]
        );
        assert_eq!(plan.validate(), Ok(()));

        // Full: base + the stable arms in grid order, on its own seed set.
        let stable = vec![
            "attack-damage-25".to_string(),
            "portal-count-160".to_string(),
        ];
        let dev = b4_full_plan(
            SeedSet::Development,
            B4_FULL_DEV_MAP_COUNT,
            &stable,
            B4_FULL_DEV_EXPERIMENT_ID,
        )
        .expect("full plan builds");
        assert_eq!(dev.map_count, 300);
        assert_eq!(dev.seed_set, SeedSet::Development);
        let ids: Vec<&str> = dev.variants.iter().map(|v| v.variant_id.as_str()).collect();
        assert_eq!(ids, vec!["default", "attack-damage-25", "portal-count-160"]);
        assert_eq!(dev.validate(), Ok(()));
        let val = b4_full_plan(
            SeedSet::Validation,
            B4_FULL_VAL_MAP_COUNT,
            &stable,
            B4_FULL_VAL_EXPERIMENT_ID,
        )
        .expect("full val builds");
        assert_eq!(val.seed_set, SeedSet::Validation);
        assert_eq!(val.map_count, 100);
        assert_eq!(
            val.variants
                .iter()
                .map(|v| v.variant_id.as_str())
                .collect::<Vec<_>>(),
            ids,
            "same variant set as the dev run"
        );
    }

    // The scratch pilot shrinks MAP COUNTS ONLY (plan 8.3: never the
    // swap legs, the pairings or the variant grid).
    #[test]
    fn pilot_flag_shrinks_map_counts_only() {
        let official = b4_pilot_plan(B4_PILOT_MAP_COUNT, B4_PILOT_EXPERIMENT_ID);
        let scratch = b4_pilot_plan(
            B4_PILOT_SCRATCH_MAP_COUNT,
            &format!("{B4_PILOT_EXPERIMENT_ID}-pilot-r0"),
        );
        assert_eq!(scratch.map_count, 8);
        assert_eq!(scratch.experiment_id, "b4-sensitivity-pilot-r1-pilot-r0");
        assert_eq!(scratch.stage, official.stage);
        assert_eq!(scratch.variants, official.variants);
        assert_eq!(scratch.policies, official.policies);
        assert_eq!(scratch.matchups, official.matchups);
        assert_eq!(scratch.gates, official.gates);
        assert_eq!(scratch.bucket_policy, official.bucket_policy);
    }

    // The full builder refuses an empty or off-grid stable set with the
    // recorded reason (the stability rule's else-branch stays honest).
    #[test]
    fn full_builder_refuses_an_empty_stable_set() {
        let error = b4_full_plan(
            SeedSet::Development,
            B4_FULL_DEV_MAP_COUNT,
            &[],
            B4_FULL_DEV_EXPERIMENT_ID,
        )
        .expect_err("empty stable set refused");
        assert!(error.contains("no stable candidate"), "{error}");
        assert!(error.contains(B4_FULL_DEV_EXPERIMENT_ID), "{error}");
        let unknown = b4_full_plan(
            SeedSet::Development,
            B4_FULL_DEV_MAP_COUNT,
            &["move-budget-320m".to_string()],
            B4_FULL_DEV_EXPERIMENT_ID,
        )
        .expect_err("off-grid arm refused");
        assert!(unknown.contains("not grid variants"), "{unknown}");
    }

    // The interaction builder declares exactly the two factor fields:
    // [base, A, B, A+B] validates at Stage::Interaction.
    #[test]
    fn interaction_builder_declares_exactly_the_two_factor_fields() {
        let a = candidate("move-budget-240m");
        let b = candidate("max-link-2500m");
        let id = interaction_experiment_id(a.field, b.field);
        assert_eq!(id, "b4-interaction-mb-mld-r1");
        let plan = b4_interaction_plan(&a, &b, 100, &id);
        assert_eq!(plan.stage, Stage::Interaction);
        assert_eq!(plan.experiment_id, "b4-interaction-mb-mld-r1");
        let ids: Vec<&str> = plan
            .variants
            .iter()
            .map(|v| v.variant_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "default",
                "move-budget-240m",
                "max-link-2500m",
                "move-budget-240m+max-link-2500m"
            ]
        );
        let base = &plan.variants[0];
        for variant in &plan.variants[1..3] {
            assert_eq!(variant.declared_changed_fields.len(), 1);
        }
        let combined = &plan.variants[3];
        let mut declared = combined.declared_changed_fields.clone();
        declared.sort_unstable();
        assert_eq!(declared, vec!["max_link_distance_cm", "move_budget_cm"]);
        assert_eq!(
            crate::balance::plan::validate_contrast(combined, base),
            Ok(())
        );
        assert_eq!(plan.validate(), Ok(()));
        // The frozen priority list covers four distinct factor pairs in
        // plan-7.4 order.
        assert_eq!(
            B4_INTERACTION_PRIORITY.first().copied(),
            Some(("move_budget_cm", "max_link_distance_cm"))
        );
        assert_eq!(B4_INTERACTION_PRIORITY.len(), 4);
    }

    // Replay sampling: leg 0 of the first 25 maps, every variant/pair.
    #[test]
    fn replay_sampling_takes_leg0_of_the_first_25_maps() {
        let plan = b4_pilot_plan(30, "replay-sample-check");
        let cells = crate::balance::schedule::expand(&plan).expect("expands");
        let sampled: Vec<&MatchCell> = cells.iter().filter(|c| b4_replay_sample(c)).collect();
        // 14 variants x 6 pairs x 25 maps.
        assert_eq!(sampled.len(), 14 * 6 * 25);
        assert!(sampled.iter().all(|c| c.leg == 0 && c.map_index < 25));
        assert!(
            cells
                .iter()
                .any(|c| c.leg == 0 && c.map_index == 25 && !b4_replay_sample(c))
        );
    }
}

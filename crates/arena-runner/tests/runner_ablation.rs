//! B3 ablation gates (`docs/balance/ai-balance-plan.md` sections 7.2, 9, 12,
//! 13.2, 16.5;): the frozen experiment plans, the plan-section-9
//! statistics on hand-built records, the plan-section-11 classification
//! rules, the verified balance-cell re-execution behind the per-reason
//! drop tally, and one end-to-end scratch-root run.

use arena_bots::policy::{PolicyId, PolicyParameters};
use arena_core::config::GameConfig;
use arena_core::types::Faction;
use arena_runner::balance::ablation::{
    B3_IDLE_EXPERIMENT_ID, B3_IDLE_MAP_COUNT, B3_MATRIX_EXPERIMENT_ID, B3_MATRIX_MAP_COUNT,
    B3_MATRIX_POLICIES, b3_idle_plan, b3_matrix_plan, b3_replay_sample, round_robin_matchups,
    run_b3_plan,
};
use arena_runner::balance::ablation::{B3_PILOT_IDLE_MAP_COUNT, B3_PILOT_MATRIX_MAP_COUNT};
use arena_runner::balance::analysis::{
    domination_ladder, is_domination, nearest_rank_quantile, pair_summaries, plan11_classification,
    plan72_contrasts, policy_mechanics, wilson_permille,
};
use arena_runner::balance::execute::run_balance_batch_with;
use arena_runner::balance::plan::{ConfigVariant, ExperimentPlan, PolicyMatchup, SeedSet, Stage};
use arena_runner::balance::record::{BalanceMatchRecord, BalanceOutcome};
use arena_runner::balance::schedule::expand;
use arena_runner::drops::{
    BalanceCellDrops, aggregate_balance_drop_reasons, balance_drop_reasons_tsv,
    reexecute_balance_cell, verify_balance_cell_drops,
};
use arena_runner::metrics::{CHECKPOINTS, FactionMatchStats, MatchStats, Outcome, ScoreSnapshot};
use arena_runner::policy::BucketPolicy;

// ---------------------------------------------------------------------
// The frozen plans
// ---------------------------------------------------------------------

#[test]
fn b3_matrix_plan_is_frozen_and_valid() {
    let plan = b3_matrix_plan(B3_MATRIX_MAP_COUNT, B3_MATRIX_EXPERIMENT_ID);
    assert_eq!(plan.experiment_id, "b3-ablation-matrix-r3");
    assert_eq!(plan.stage, Stage::Ablation);
    assert_eq!(plan.seed_set, SeedSet::Development);
    assert_eq!(plan.map_count, 100);
    assert_eq!(plan.base_variant, "default");
    assert_eq!(plan.variants.len(), 1, "single default variant");
    assert_eq!(plan.variants[0].declared_changed_fields.len(), 0);
    assert_eq!(plan.bucket_policy, BucketPolicy::Flat(0));
    // The exact eight-policy roster, in order.
    let ids: Vec<PolicyId> = plan.policies.iter().map(|s| s.policy_id).collect();
    assert_eq!(ids, B3_MATRIX_POLICIES.to_vec());
    assert_eq!(
        ids,
        vec![
            PolicyId::GreedyField,
            PolicyId::Hybrid,
            PolicyId::GreedyCoordinatedR2,
            PolicyId::HybridBuilderOnly,
            PolicyId::HybridBuilderAttack,
            PolicyId::GreedyCoordinatedCanonical,
            PolicyId::AdaptiveHybridR2,
            PolicyId::BuilderDisruptorR2,
        ]
    );
    // 28 distinct pairs + 8 mirrors = 36 matchups, every policy registered.
    assert_eq!(plan.matchups.len(), 36);
    let mirrors = plan.matchups.iter().filter(|m| m.a == m.b).count();
    assert_eq!(mirrors, 8);
    assert_eq!(plan.validate(), Ok(()));
    // The schedule expands to exactly 36 x 100 x 2 = 7,200 cells.
    assert_eq!(expand(&plan).expect("expands").len(), 7_200);

    // Mutating any config field fails validation: the ablation stage may
    // not change ANY rule parameter (belt-and-braces).
    let mut mutated = b3_matrix_plan(B3_MATRIX_MAP_COUNT, B3_MATRIX_EXPERIMENT_ID);
    let mut cfg = GameConfig::default();
    cfg.move_budget_cm = 24_000;
    mutated
        .variants
        .push(ConfigVariant::new("tuned", cfg, vec!["move_budget_cm"]));
    assert!(mutated.validate().is_err());
    // An undeclared diff is refused even without a declaration.
    let mut sneaky = b3_matrix_plan(B3_MATRIX_MAP_COUNT, B3_MATRIX_EXPERIMENT_ID);
    let mut cfg = GameConfig::default();
    cfg.portal_count = 100;
    sneaky
        .variants
        .push(ConfigVariant::new("sneaky", cfg, Vec::new()));
    assert!(sneaky.validate().is_err());
}

#[test]
fn pilot_variant_shrinks_map_counts_only() {
    let official = b3_matrix_plan(B3_MATRIX_MAP_COUNT, B3_MATRIX_EXPERIMENT_ID);
    let pilot = b3_matrix_plan(B3_PILOT_MATRIX_MAP_COUNT, "b3-ablation-matrix-r3-pilot-r0");
    assert_eq!(pilot.map_count, 8);
    assert_eq!(pilot.stage, official.stage);
    assert_eq!(pilot.variants, official.variants);
    assert_eq!(pilot.policies, official.policies);
    assert_eq!(pilot.matchups, official.matchups);
    assert_eq!(pilot.bucket_policy, official.bucket_policy);
    let idle_official = b3_idle_plan(B3_IDLE_MAP_COUNT, B3_IDLE_EXPERIMENT_ID);
    let idle_pilot = b3_idle_plan(B3_PILOT_IDLE_MAP_COUNT, "b3-ablation-idle-r3-pilot-r0");
    assert_eq!(idle_pilot.map_count, 4);
    assert_eq!(idle_pilot.matchups, idle_official.matchups);
    assert_eq!(idle_pilot.policies, idle_official.policies);
}

#[test]
fn b3_idle_plan_is_frozen_and_valid() {
    let plan = b3_idle_plan(B3_IDLE_MAP_COUNT, B3_IDLE_EXPERIMENT_ID);
    assert_eq!(plan.experiment_id, "b3-ablation-idle-r3");
    assert_eq!(plan.stage, Stage::Ablation);
    assert_eq!(plan.map_count, 50);
    assert_eq!(plan.policies.len(), 4, "three arms + idle");
    let arms: Vec<PolicyId> = plan
        .policies
        .iter()
        .map(|s| s.policy_id)
        .filter(|id| *id != PolicyId::Idle)
        .collect();
    assert_eq!(
        arms,
        vec![
            PolicyId::HybridBuilderOnly,
            PolicyId::HybridBuilderAttack,
            PolicyId::GreedyCoordinatedCanonical,
        ]
    );
    assert_eq!(plan.matchups.len(), 3);
    for matchup in &plan.matchups {
        assert_eq!(matchup.b, PolicyId::Idle);
        assert!(matchup.tag.ends_with("--idle"));
    }
    assert_eq!(plan.validate(), Ok(()));
    // The round-robin builder generalizes: 4 policies -> 6+4 matchups.
    assert_eq!(round_robin_matchups(&B3_MATRIX_POLICIES).len(), 36);
    assert_eq!(round_robin_matchups(&arms).len(), 3 + 3);
}

// The frozen replay sampling: every leg-0, plus both legs of mirrors on
// the first 25 maps only.
#[test]
fn b3_replay_sampling_takes_leg0_and_mirror_legs_on_25_maps() {
    let plan = b3_matrix_plan(30, "sample-test");
    let cells = expand(&plan).expect("expands");
    let mut leg0 = 0;
    let mut mirror_extra = 0;
    for cell in &cells {
        if cell.leg == 0 {
            leg0 += u64::from(b3_replay_sample(cell));
        } else if cell.policy_a == cell.policy_b {
            mirror_extra += u64::from(cell.map_index < 25 && b3_replay_sample(cell));
            if cell.map_index >= 25 {
                assert!(
                    !b3_replay_sample(cell),
                    "mirror legs after map 25 not sampled"
                );
            }
        } else {
            assert!(!b3_replay_sample(cell), "non-mirror leg-1 never sampled");
        }
    }
    assert_eq!(leg0, 36 * 30);
    assert_eq!(mirror_extra, 8 * 25);
}

// ---------------------------------------------------------------------
// Plan-section-9 statistics on hand-built records
// ---------------------------------------------------------------------

/// Hand-built record factory (the pairs.rs test pattern): policy_a's
/// score is `a_score`, policy_b's `b_score`, with the given outcome,
/// checkpoint leaders and stats.
#[allow(clippy::too_many_arguments)]
fn record(
    index: u64,
    pair_tag: &str,
    pair_id: &str,
    leg: u8,
    policy_a: PolicyId,
    policy_b: PolicyId,
    a_score: u128,
    b_score: u128,
    result: BalanceOutcome,
    leader_r100: Option<Faction>,
    enl_stats: FactionMatchStats,
    res_stats: FactionMatchStats,
) -> BalanceMatchRecord {
    let a_is_enl = leg == 0;
    let (enl, res) = if a_is_enl {
        (a_score, b_score)
    } else {
        (b_score, a_score)
    };
    let checkpoints = CHECKPOINTS.map(|round| ScoreSnapshot {
        round,
        enl: enl as u64 as u128,
        res,
        // Only R100 carries a leader in these fixtures; earlier
        // checkpoints tie, later ones echo R100.
        leader: if round >= 100 { leader_r100 } else { None },
    });
    BalanceMatchRecord {
        index,
        experiment_id: "hand-built".to_string(),
        seed_set_tag: "development".to_string(),
        variant_id: "default".to_string(),
        pair_tag: pair_tag.to_string(),
        pair_id: pair_id.to_string(),
        leg,
        map_id: format!("development-{:03}", leg),
        map_index: leg as u32,
        policy_a,
        policy_a_family: policy_a.family(),
        policy_a_version: version_of(policy_a).to_string(),
        policy_a_parameters: parameters_of(policy_a),
        policy_b,
        policy_b_family: policy_b.family(),
        policy_b_version: version_of(policy_b).to_string(),
        policy_b_parameters: parameters_of(policy_b),
        policy_a_is_enl: a_is_enl,
        map_seed: 1,
        game_seed: 2,
        bot_seed: 3,
        schedule_seed: 4,
        bucket_policy: BucketPolicy::Flat(0),
        config_hash: arena_runner::tournament::config::config_identity(&GameConfig::default()),
        result,
        enl_score2: enl,
        res_score2: res,
        final_state_hash: [index as u8; 32],
        stats: MatchStats {
            enl: enl_stats,
            res: res_stats,
            checkpoints,
        },
        elapsed_ms: 0,
    }
}

fn version_of(policy: PolicyId) -> &'static str {
    match policy {
        id if id.bot_kind().is_some() => "m6.0",
        PolicyId::HybridBuilderOnly | PolicyId::HybridBuilderAttack => "b3.0",
        PolicyId::Idle => "r2.0",
        _ => "r2.0",
    }
}

fn parameters_of(policy: PolicyId) -> PolicyParameters {
    match policy {
        id if id.bot_kind().is_some() => PolicyParameters::FrozenM6,
        PolicyId::HybridBuilderOnly => PolicyId::HybridBuilderOnly.ablation_spec().parameters,
        PolicyId::HybridBuilderAttack => PolicyId::HybridBuilderAttack.ablation_spec().parameters,
        other => other.r2_spec().parameters,
    }
}

fn stats_with(fields_created: u64, first_field: Option<u32>) -> FactionMatchStats {
    let mut stats = FactionMatchStats::default();
    stats.fields_created = fields_created;
    stats.first_field_round = first_field;
    stats.commands_submitted = 100;
    stats.commands_ok = 90;
    stats.commands_dropped = 10;
    stats
}

#[test]
fn wilson_bounds_match_hand_computed_values() {
    // Hand-computed Wilson 95% bounds (z = 1.96), floor/ceil to permille.
    assert_eq!(wilson_permille(10, 10), Some((722, 1000)));
    assert_eq!(wilson_permille(0, 10), Some((0, 278)));
    assert_eq!(wilson_permille(5, 10), Some((236, 764)));
    assert_eq!(wilson_permille(1, 4), Some((45, 700)));
    assert_eq!(wilson_permille(3, 4), Some((300, 955)));
    assert_eq!(wilson_permille(9, 10), Some((595, 983)));
    // No decisive match -> no interval (the raw W-D-L stays the record).
    assert_eq!(wilson_permille(0, 0), None);
    assert_eq!(wilson_permille(7, 0), None);
    // The frozen domination standard: Wilson lower bound >= 900 permille.
    // An all-wins record needs n >= 35 (30/30 -> 886, 35/35 -> 901).
    assert!(is_domination(1750, 1750));
    assert!(!is_domination(10, 10), "10/10 lower bound is 722 permille");
    assert!(!is_domination(30, 30), "30/30 lower bound is 886");
    assert!(is_domination(35, 35), "35/35 lower bound is 901");
    assert!(is_domination(70, 70));
    // A split record is never a domination.
    assert!(!is_domination(50, 100));
}

#[test]
fn quantiles_use_the_frozen_nearest_rank_definition() {
    // N=8: ranks ceil(.25*8)=2, ceil(.5*8)=4, ceil(.75*8)=6.
    let values: Vec<i128> = (1..=8).map(|v| v as i128).collect();
    assert_eq!(nearest_rank_quantile(&values, 250), Some(2));
    assert_eq!(nearest_rank_quantile(&values, 500), Some(4));
    assert_eq!(nearest_rank_quantile(&values, 750), Some(6));
    // N=7: ranks ceil(1.75)=2, ceil(3.5)=4, ceil(5.25)=6.
    let odd: Vec<i128> = vec![10, 20, 30, 40, 50, 60, 70];
    assert_eq!(nearest_rank_quantile(&odd, 250), Some(20));
    assert_eq!(nearest_rank_quantile(&odd, 500), Some(40));
    assert_eq!(nearest_rank_quantile(&odd, 750), Some(60));
    // Unsorted input is sorted by the function; single and empty edges.
    let unsorted = vec![7i128, 1, 5, 3];
    let sorted_copy = unsorted.clone();
    assert_eq!(
        nearest_rank_quantile(&unsorted, 500),
        Some(3),
        "N=4 median rank is 2"
    );
    assert_eq!(unsorted, sorted_copy, "the caller's slice is untouched");
    assert_eq!(nearest_rank_quantile(&[5i128], 250), Some(5));
    assert_eq!(nearest_rank_quantile(&[], 500), None);
    // The extreme permille clamp into 1..=N.
    assert_eq!(nearest_rank_quantile(&odd, 0), Some(10));
    assert_eq!(nearest_rank_quantile(&odd, 1000), Some(70));
}

#[test]
fn checkpoint_contingency_counts_leaders_and_winners() {
    let enl = stats_with(1, Some(40));
    let res = stats_with(0, None);
    // Four played records of the hybrid--greedy-field pair:
    // - leg0 (a=hybrid is ENL): hybrid leads at R100 and wins (EnlWin).
    // - leg1 (a=hybrid is RES): RES (hybrid = a) leads at R100 but the
    //   ENL side wins (EnlWin) -> a led and lost.
    // - a draw where a led.
    // - a checkpoint tie (both zero scores), a finally wins.
    let records = vec![
        record(
            0,
            "hybrid--greedy-field",
            "hybrid--greedy-field#m000",
            0,
            PolicyId::Hybrid,
            PolicyId::GreedyField,
            100,
            10,
            BalanceOutcome::Played(Outcome::EnlWin),
            Some(Faction::Enl),
            enl.clone(),
            res.clone(),
        ),
        record(
            1,
            "hybrid--greedy-field",
            "hybrid--greedy-field#m000",
            1,
            PolicyId::Hybrid,
            PolicyId::GreedyField,
            10,
            100,
            BalanceOutcome::Played(Outcome::EnlWin),
            Some(Faction::Res),
            enl.clone(),
            res.clone(),
        ),
        record(
            2,
            "hybrid--greedy-field",
            "hybrid--greedy-field#m001",
            0,
            PolicyId::Hybrid,
            PolicyId::GreedyField,
            50,
            50,
            BalanceOutcome::Played(Outcome::Draw),
            Some(Faction::Enl),
            enl.clone(),
            res.clone(),
        ),
        record(
            3,
            "hybrid--greedy-field",
            "hybrid--greedy-field#m001",
            1,
            PolicyId::Hybrid,
            PolicyId::GreedyField,
            0,
            0,
            BalanceOutcome::Played(Outcome::ResWin),
            None,
            enl.clone(),
            res.clone(),
        ),
    ];
    // One crash + one config failure stay in the W-D-L table but never
    // enter the checkpoint contingency or the score sums.
    let mut crashed = record(
        4,
        "hybrid--greedy-field",
        "hybrid--greedy-field#m002",
        0,
        PolicyId::Hybrid,
        PolicyId::GreedyField,
        0,
        0,
        BalanceOutcome::Played(Outcome::Crash),
        Some(Faction::Enl),
        FactionMatchStats::default(),
        FactionMatchStats::default(),
    );
    crashed.result = BalanceOutcome::Played(Outcome::Crash);
    let mut records = records;
    records.push(crashed);
    records.push(record(
        5,
        "hybrid--greedy-field",
        "hybrid--greedy-field#m002",
        1,
        PolicyId::Hybrid,
        PolicyId::GreedyField,
        0,
        0,
        BalanceOutcome::ConfigFailure,
        None,
        FactionMatchStats::default(),
        FactionMatchStats::default(),
    ));
    let summaries = pair_summaries(&records, &[], "default");
    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!((summary.a_wins, summary.b_wins, summary.draws), (2, 1, 1));
    assert_eq!(summary.crashes, 1);
    assert_eq!(summary.config_failures, 1);
    // Faction asymmetry: ENL won twice (records 0 and 1), RES once.
    assert_eq!((summary.enl_wins, summary.res_wins), (2, 1));
    // Score sums cover ONLY the played records: a = 100+10+50+0, b =
    // 10+100+50+0.
    assert_eq!(summary.a_score_sum, 160);
    assert_eq!(summary.b_score_sum, 160);
    assert_eq!(summary.a_share_permille, Some(500));
    // Record 3 is the single both-zero PLAYED match.
    assert_eq!(summary.both_zero_matches, 1);
    // R50 ties for every record; from R100 on: led-won 1 (record 0:
    // a=hybrid as ENL leads and wins), led-lost 1 (record 1: a as RES
    // leads, b wins), led-drew 1 (record 2: a leads, draw), tie 1
    // (record 3).
    let r50 = &summary.checkpoints[0];
    assert_eq!((r50.round, r50.tie), (50, 4));
    let r100 = &summary.checkpoints[1];
    assert_eq!(
        (r100.led_won, r100.led_lost, r100.led_drew, r100.tie),
        (1, 1, 1, 1)
    );
    assert_eq!(r100.win_share_permille(), Some(333));
}

#[test]
fn mechanics_pool_per_policy_with_raw_sums() {
    let enl = {
        let mut stats = stats_with(2, Some(40));
        stats.field_area2_sum = 1_000;
        stats.destroyed_lifetime_rounds_sum = 30;
        stats.surviving_lifetime_rounds_sum = 70;
        stats
    };
    let res = stats_with(0, None);
    let records = vec![
        record(
            0,
            "hybrid--idle",
            "hybrid--idle#m000",
            0,
            PolicyId::Hybrid,
            PolicyId::Idle,
            100,
            0,
            BalanceOutcome::Played(Outcome::EnlWin),
            Some(Faction::Enl),
            enl.clone(),
            res.clone(),
        ),
        record(
            1,
            "hybrid--idle",
            "hybrid--idle#m000",
            1,
            PolicyId::Hybrid,
            PolicyId::Idle,
            90,
            0,
            BalanceOutcome::Played(Outcome::ResWin),
            Some(Faction::Res),
            res.clone(),
            enl.clone(),
        ),
    ];
    let specs = vec![PolicyId::Hybrid.frozen_m6_spec(), PolicyId::Idle.r2_spec()];
    let mechanics = policy_mechanics(&records, &specs);
    assert_eq!(mechanics.len(), 2);
    let hybrid = &mechanics[0];
    // Both legs, both stats views pooled: hybrid was ENL in leg 0, RES in
    // leg 1 — its stats are the `enl` view in both records' own columns.
    assert_eq!(hybrid.matches_played, 2);
    assert_eq!(hybrid.first_field_round_sum, 80);
    assert_eq!(hybrid.first_field_count, 2);
    assert_eq!(hybrid.field_area2_sum, 2_000);
    assert_eq!(hybrid.lifetime_rounds_sum, 200);
    assert_eq!(hybrid.fields_created, 4);
    assert_eq!(hybrid.commands_submitted, 200);
    assert_eq!(hybrid.commands_dropped, 20);
    // The idle side never participated with scoring stats.
    assert_eq!(mechanics[1].matches_played, 2);
    assert_eq!(mechanics[1].fields_created, 0);

    // A MIRROR record contributes BOTH faction slots of the same policy
    // (the r2 official matrix undercounted mirrors here — one side per
    // record — until the raw matches.tsv recompute caught it; this row
    // pins both-sides pooling).
    let enl_mirror = {
        let mut stats = stats_with(3, Some(50));
        stats.commands_submitted = 80;
        stats.commands_ok = 78;
        stats.commands_dropped = 2;
        stats.field_area2_sum = 7_000;
        stats
    };
    let res_mirror = {
        let mut stats = stats_with(2, Some(60));
        stats.commands_submitted = 70;
        stats.commands_ok = 66;
        stats.commands_dropped = 4;
        stats.field_area2_sum = 5_000;
        stats
    };
    let mirror = record(
        2,
        "hybrid--hybrid",
        "hybrid--hybrid#m000",
        0,
        PolicyId::Hybrid,
        PolicyId::Hybrid,
        100,
        90,
        BalanceOutcome::Played(Outcome::EnlWin),
        Some(Faction::Enl),
        enl_mirror,
        res_mirror,
    );
    let mechanics = policy_mechanics(&[mirror], &specs);
    assert_eq!(mechanics[0].matches_played, 2, "both mirror slots");
    assert_eq!(mechanics[0].commands_submitted, 150);
    assert_eq!(mechanics[0].commands_ok, 144);
    assert_eq!(mechanics[0].commands_dropped, 6);
    assert_eq!(mechanics[0].fields_created, 5);
    assert_eq!(mechanics[0].field_area2_sum, 12_000);
    assert_eq!(mechanics[0].first_field_round_sum, 110);
    assert_eq!(mechanics[0].first_field_count, 2);
}

// ---------------------------------------------------------------------
// The ladder + plan-section-11 classification
// ---------------------------------------------------------------------

/// hybrid beats greedy-field 10-0 (a frozen-style domination) but splits
/// 5-5 with gc-r2 -> the coordinated correction removes the domination.
fn ladder_records() -> Vec<BalanceMatchRecord> {
    let stats = stats_with(1, Some(40));
    let mut records = Vec::new();
    let mut index = 0;
    let mut push = |a: PolicyId, b: PolicyId, a_wins: usize, total: usize| {
        for k in 0..total {
            let a_is_enl = k % 2 == 0;
            let a_won = k < a_wins;
            // a is ENL on even k: EnlWin means a won.
            let result = if a_won == a_is_enl {
                Outcome::EnlWin
            } else {
                Outcome::ResWin
            };
            records.push(record(
                index,
                &format!("{}--{}", a.tag(), b.tag()),
                &format!("{}--{}#m{:03}", a.tag(), b.tag(), k),
                u8::from(!a_is_enl),
                a,
                b,
                if a_won { 100 } else { 10 },
                if a_won { 10 } else { 100 },
                BalanceOutcome::Played(result),
                Some(Faction::Enl),
                stats.clone(),
                stats.clone(),
            ));
            index += 1;
        }
    };
    push(PolicyId::Hybrid, PolicyId::GreedyField, 10, 10);
    push(PolicyId::Hybrid, PolicyId::GreedyCoordinatedR2, 5, 10);
    push(PolicyId::GreedyCoordinatedR2, PolicyId::GreedyField, 8, 8);
    push(
        PolicyId::HybridBuilderAttack,
        PolicyId::HybridBuilderOnly,
        6,
        8,
    );
    records
}

#[test]
fn ladder_and_contrasts_order_the_seven_frozen_steps() {
    let records = ladder_records();
    let ladder = domination_ladder(&records);
    assert_eq!(ladder.len(), 7, "five rungs + two cross-era totals");
    assert_eq!(ladder[0].contrast, "ladder-1-greedy-m6-vs-gc-r2");
    assert_eq!(ladder[1].contrast, "ladder-2-gc-r2-vs-hybrid-builder-only");
    assert_eq!(
        ladder[2].contrast,
        "ladder-3-builder-only-vs-builder-attack"
    );
    assert_eq!(ladder[3].contrast, "ladder-4-builder-attack-vs-hybrid");
    assert_eq!(ladder[4].contrast, "cross-era-hybrid-vs-gc-r2");
    assert_eq!(ladder[5].contrast, "cross-era-greedy-m6-vs-gc-canonical");
    assert_eq!(ladder[6].contrast, "cross-era-hybrid-vs-greedy-m6");
    // The upper side carries the counts: gc-r2 beat greedy-field 8-0
    // (not a domination at n=8), hybrid split with gc-r2.
    assert_eq!(
        (ladder[0].wins, ladder[0].losses, ladder[0].draws),
        (8, 0, 0)
    );
    assert!(!ladder[0].is_domination);
    assert_eq!((ladder[4].wins, ladder[4].losses), (5, 5));
    assert_eq!(ladder[4].win_share_permille, Some(500));
    assert_eq!((ladder[6].wins, ladder[6].losses), (10, 0));
    assert_eq!(ladder[6].win_share_permille, Some(1000));
    // The six plan-7.2 contrasts cover the six table rows.
    let contrasts = plan72_contrasts(&records);
    assert_eq!(contrasts.len(), 6);
    assert!(contrasts.iter().all(|c| c.contrast.starts_with("7.2-row")));
}

#[test]
fn plan11_rules_classify_the_correction_case() {
    let records = ladder_records();
    let classes = plan11_classification(&records, &[]);
    // At n=10 the 10/10 Wilson lower bound is 722 permille — below the
    // frozen 900 standard — so the classifier must state the honest
    // precondition rather than fire.
    let rule1 = &classes[0];
    assert!(rule1.rule.contains("row 1"));
    assert!(
        rule1
            .evidence
            .contains("hybrid over greedy-field: W-L-D 10/0/0")
    );
    assert_eq!(
        rule1.classification,
        "前置不成立（Hybrid 对 Greedy m6.0 已非支配）"
    );

    // A REAL frozen-standard domination (n=100, 100/100 wins: lower bound
    // ~963 permille) that gc-r2 removes (50-50) fires the rule.
    let stats = stats_with(1, Some(40));
    let mut domination_records = Vec::new();
    let mut index = 0u64;
    for k in 0..100u32 {
        let a_is_enl = k % 2 == 0;
        domination_records.push(record(
            index,
            "hybrid--greedy-field",
            &format!("hybrid--greedy-field#m{k:03}"),
            u8::from(!a_is_enl),
            PolicyId::Hybrid,
            PolicyId::GreedyField,
            100,
            10,
            BalanceOutcome::Played(if a_is_enl {
                Outcome::EnlWin
            } else {
                Outcome::ResWin
            }),
            Some(Faction::Enl),
            stats.clone(),
            stats.clone(),
        ));
        index += 1;
    }
    for k in 0..100u32 {
        let a_is_enl = k % 2 == 0;
        let hybrid_won = k % 2 == 0;
        domination_records.push(record(
            index,
            "hybrid--greedy-coordinated-r2",
            &format!("hybrid--greedy-coordinated-r2#m{k:03}"),
            u8::from(!a_is_enl),
            PolicyId::Hybrid,
            PolicyId::GreedyCoordinatedR2,
            if hybrid_won { 100 } else { 10 },
            if hybrid_won { 10 } else { 100 },
            BalanceOutcome::Played(if hybrid_won == a_is_enl {
                Outcome::EnlWin
            } else {
                Outcome::ResWin
            }),
            Some(Faction::Enl),
            stats.clone(),
            stats.clone(),
        ));
        index += 1;
    }
    let fired = plan11_classification(&domination_records, &[]);
    let rule1 = fired
        .iter()
        .find(|c| c.rule.contains("row 1"))
        .expect("rule 1 evaluated");
    assert_eq!(rule1.classification, "Bot 实现缺陷");
    assert_eq!(rule1.action, "保留规则默认值，更新 Benchmark");
}

// The mirror-symmetry artifact rule: canonical collisions > 0 while the
// seeded arm collides 0 times.
#[test]
fn plan11_mirror_rule_fires_only_for_identical_deterministic_spawns() {
    use arena_runner::balance::analysis::MirrorPairStats;
    let records = ladder_records();
    let mirrors = vec![
        MirrorPairStats {
            pair_tag: "greedy-coordinated-canonical--greedy-coordinated-canonical".to_string(),
            policy: PolicyId::GreedyCoordinatedCanonical,
            legs_checked: 50,
            spawn_collisions: 50,
            units: 25,
            symmetric_outcome_units: 25,
        },
        MirrorPairStats {
            pair_tag: "greedy-coordinated-r2--greedy-coordinated-r2".to_string(),
            policy: PolicyId::GreedyCoordinatedR2,
            legs_checked: 50,
            spawn_collisions: 0,
            units: 25,
            symmetric_outcome_units: 20,
        },
    ];
    let classes = plan11_classification(&records, &mirrors);
    let mirror_rule = classes
        .iter()
        .find(|c| c.rule.contains("row 6"))
        .expect("the artifact rule fires");
    assert_eq!(mirror_rule.classification, "策略对称伪影");
    // Without canonical collisions the rule stays silent.
    let quiet = plan11_classification(&records, &mirrors[1..]);
    assert!(quiet.iter().all(|c| !c.rule.contains("row 6")));
}

// ---------------------------------------------------------------------
// Verified balance-cell re-execution (the per-reason drop tally base)
// ---------------------------------------------------------------------

fn tiny_plan(experiment_id: &str) -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Ablation,
        seed_set: SeedSet::Development,
        map_count: 1,
        base_variant: "default".to_string(),
        variants: vec![ConfigVariant::new(
            "default",
            GameConfig::default(),
            Vec::new(),
        )],
        policies: vec![
            PolicyId::GreedyField.frozen_m6_spec(),
            PolicyId::HybridBuilderAttack.ablation_spec(),
        ],
        matchups: vec![PolicyMatchup::new(
            "greedy-field--hybrid-builder-attack",
            PolicyId::GreedyField,
            PolicyId::HybridBuilderAttack,
        )],
        bucket_policy: BucketPolicy::Flat(0),
        gates: Vec::new(),
    }
}

#[test]
fn reexecuted_balance_cell_reproduces_hash_and_counts() {
    let plan = tiny_plan("reexec-test");
    let run = run_balance_batch_with(
        &plan,
        1,
        &arena_runner::balance::execute::default_cell_setup,
        &|_| false,
    )
    .expect("tiny run");
    assert_eq!(run.records.len(), 2);
    let cfg = GameConfig::default();
    let map = arena_core::map::generate_map(
        arena_runner::balance::plan::map_seed(SeedSet::Development, 0),
        &cfg,
    )
    .expect("map regenerates");
    for record in &run.records {
        let drops = reexecute_balance_cell(record, &cfg, &map)
            .unwrap_or_else(|e| panic!("cell {}: {e}", record.index));
        assert_eq!(verify_balance_cell_drops(record, &drops), Ok(()));
        assert_eq!(drops.pair_tag, record.pair_tag);
        assert_eq!(drops.enl.dropped, record.stats.enl.commands_dropped);
        // The per-reason sum equals the archived drop total.
        assert_eq!(
            drops.enl.reasons.values().sum::<u64>(),
            record.stats.enl.commands_dropped
        );
    }
    // A tampered row aborts verification: wrong hash...
    let mut tampered = run.records[0].clone();
    tampered.final_state_hash = [9u8; 32];
    let drops = reexecute_balance_cell(&tampered, &cfg, &map).expect("re-executes");
    assert!(verify_balance_cell_drops(&tampered, &drops).is_err());
    // ...wrong drop counts...
    let mut tampered = run.records[0].clone();
    tampered.stats.enl.commands_dropped += 1;
    let drops = reexecute_balance_cell(&tampered, &cfg, &map).expect("re-executes");
    assert!(verify_balance_cell_drops(&tampered, &drops).is_err());
    // ...and a tampered parameter set never constructs (identity
    // sensitivity: the record IS the strategy).
    let mut tampered = run.records[0].clone();
    tampered.policy_a_parameters = PolicyId::HybridBuilderOnly.ablation_spec().parameters;
    assert!(reexecute_balance_cell(&tampered, &cfg, &map).is_err());
    // Crash and config-failure rows are refused.
    let mut crashed = run.records[0].clone();
    crashed.result = BalanceOutcome::Played(Outcome::Crash);
    assert!(reexecute_balance_cell(&crashed, &cfg, &map).is_err());
    let mut failed = run.records[0].clone();
    failed.result = BalanceOutcome::ConfigFailure;
    assert!(reexecute_balance_cell(&failed, &cfg, &map).is_err());
}

// The drop mechanism table sums per policy from hand-built tallies.
#[test]
fn drop_mechanism_table_sums_per_policy_from_hand_built_stats() {
    let stats = stats_with(1, Some(40));
    let mut record_a = record(
        0,
        "greedy-field--hybrid-builder-attack",
        "greedy-field--hybrid-builder-attack#m000",
        0,
        PolicyId::GreedyField,
        PolicyId::HybridBuilderAttack,
        100,
        10,
        BalanceOutcome::Played(Outcome::EnlWin),
        Some(Faction::Enl),
        stats.clone(),
        stats.clone(),
    );
    // Give the archived row drop totals the hand-built tally must match.
    record_a.stats.enl.commands_submitted = 100;
    record_a.stats.enl.commands_ok = 90;
    record_a.stats.enl.commands_dropped = 10;
    record_a.stats.res.commands_submitted = 50;
    record_a.stats.res.commands_ok = 49;
    record_a.stats.res.commands_dropped = 1;
    let mut drops = BalanceCellDrops {
        index: 0,
        pair_tag: record_a.pair_tag.clone(),
        policy_a: record_a.policy_a,
        policy_b: record_a.policy_b,
        policy_a_is_enl: true,
        enl: arena_runner::drops::FactionDrops::default(),
        res: arena_runner::drops::FactionDrops::default(),
        final_state_hash: [0u8; 32],
    };
    drops.enl.submitted = 100;
    drops.enl.ok = 90;
    drops.enl.dropped = 10;
    drops.enl.reasons.insert("CaptureTargetNotNeutral", 7);
    drops.enl.reasons.insert("LinkAlreadyExists", 3);
    drops.res.submitted = 50;
    drops.res.ok = 49;
    drops.res.dropped = 1;
    drops.res.reasons.insert("AttackNoEnemyPortalsInRange", 1);
    assert_eq!(verify_balance_cell_drops(&record_a, &drops), Ok(()));

    // LEG 1 of the same unit (policy_a plays RES): the row's OWN faction
    // assignment must attribute BOTH the re-executed tally and the
    // archived per-faction totals to policy_a (the r1 official drop pass
    // caught the swapped pairing here — mixed tally/archived sides — via
    // its aggregation cross-check; this row pins the fix).
    let mut record_b = record(
        0,
        "greedy-field--hybrid-builder-attack",
        "greedy-field--hybrid-builder-attack#m000",
        1,
        PolicyId::GreedyField,
        PolicyId::HybridBuilderAttack,
        100,
        10,
        BalanceOutcome::Played(Outcome::EnlWin),
        Some(Faction::Enl),
        stats.clone(),
        stats.clone(),
    );
    // leg 1: policy_a (greedy-field) is RES — distinct counts per side so
    // any misattribution changes the sums.
    record_b.stats.enl.commands_submitted = 70;
    record_b.stats.enl.commands_ok = 65;
    record_b.stats.enl.commands_dropped = 5;
    record_b.stats.res.commands_submitted = 40;
    record_b.stats.res.commands_ok = 36;
    record_b.stats.res.commands_dropped = 4;
    let mut drops_b = BalanceCellDrops {
        index: 0,
        pair_tag: record_b.pair_tag.clone(),
        policy_a: record_b.policy_a,
        policy_b: record_b.policy_b,
        policy_a_is_enl: false,
        enl: arena_runner::drops::FactionDrops::default(),
        res: arena_runner::drops::FactionDrops::default(),
        final_state_hash: [0u8; 32],
    };
    drops_b.enl.submitted = 70;
    drops_b.enl.ok = 65;
    drops_b.enl.dropped = 5;
    drops_b.enl.reasons.insert("LinkAlreadyExists", 5);
    drops_b.res.submitted = 40;
    drops_b.res.ok = 36;
    drops_b.res.dropped = 4;
    drops_b.res.reasons.insert("CaptureTargetNotNeutral", 4);
    assert_eq!(verify_balance_cell_drops(&record_b, &drops_b), Ok(()));

    let summaries =
        aggregate_balance_drop_reasons(&[record_a, record_b], &[Some(drops), Some(drops_b)])
            .expect("aggregates");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].pair_tag, "greedy-field--hybrid-builder-attack");
    assert_eq!(summaries[0].policies.len(), 2);
    let greedy = summaries[0]
        .policies
        .iter()
        .find(|p| p.policy == PolicyId::GreedyField)
        .unwrap();
    // Leg 0 as ENL (10 drops) + leg 1 as RES (4 drops): the archived and
    // re-executed sums follow the policy across the faction swap.
    assert_eq!(greedy.matches, 2);
    assert_eq!(greedy.archived_submitted, 100 + 40);
    assert_eq!(greedy.archived_dropped, 10 + 4);
    assert_eq!(greedy.reason_sum(), 10 + 4);
    assert_eq!(greedy.reasons.get("CaptureTargetNotNeutral"), Some(&11));
    let attack_arm = summaries[0]
        .policies
        .iter()
        .find(|p| p.policy == PolicyId::HybridBuilderAttack)
        .unwrap();
    assert_eq!(attack_arm.matches, 2);
    assert_eq!(attack_arm.archived_dropped, 1 + 5);
    assert_eq!(attack_arm.reason_sum(), 1 + 5);
    // The TSV carries explicit zero rows and a TOTAL per policy.
    let tsv = balance_drop_reasons_tsv(&summaries);
    assert!(tsv.contains(
        "greedy-field--hybrid-builder-attack\tgreedy-field\tCaptureTargetNotNeutral\t11\n"
    ));
    assert!(tsv.contains("greedy-field--hybrid-builder-attack\tgreedy-field\tLinkSelfLoop\t0\n"));
    assert!(tsv.contains("greedy-field--hybrid-builder-attack\tgreedy-field\tTOTAL\t14\n"));

    // The aggregate belt-and-braces cross-check: a tally whose per-reason
    // sum cannot equal the archived drop total is refused.
    let mut consistent = record(
        1,
        "short",
        "short#m001",
        0,
        PolicyId::GreedyField,
        PolicyId::Idle,
        1,
        1,
        BalanceOutcome::Played(Outcome::Draw),
        None,
        stats.clone(),
        stats.clone(),
    );
    consistent.stats.enl.commands_submitted = 10;
    consistent.stats.enl.commands_ok = 9;
    consistent.stats.enl.commands_dropped = 1;
    consistent.stats.res = FactionMatchStats::default();
    let mut short_drops = BalanceCellDrops {
        index: 1,
        pair_tag: "short".to_string(),
        policy_a: PolicyId::GreedyField,
        policy_b: PolicyId::Idle,
        policy_a_is_enl: true,
        enl: arena_runner::drops::FactionDrops::default(),
        res: arena_runner::drops::FactionDrops::default(),
        final_state_hash: [0u8; 32],
    };
    short_drops.enl.submitted = 10;
    short_drops.enl.ok = 9;
    short_drops.enl.dropped = 1;
    short_drops.enl.reasons.insert("LinkAlreadyExists", 1);
    assert!(
        aggregate_balance_drop_reasons(&[consistent.clone()], &[Some(short_drops.clone())]).is_ok()
    );
    // The same tally against a row claiming 5 archived drops cannot
    // reconcile (the per-match verification would already have caught it;
    // the aggregate refuses regardless).
    let mut lying = consistent;
    lying.stats.enl.commands_dropped = 5;
    assert!(aggregate_balance_drop_reasons(&[lying], &[Some(short_drops)]).is_err());
}

// ---------------------------------------------------------------------
// End-to-end scratch-root run (archive + analysis + hash folding +
// existing-dir refusal + records match policies.tsv)
// ---------------------------------------------------------------------

#[test]
fn b3_scratch_run_archives_analysis_and_refuses_existing_dirs() {
    let root =
        std::env::temp_dir().join(format!("b3-scratch-run-{}-{}", std::process::id(), line!()));
    let _ = std::fs::remove_dir_all(&root);
    let plan = tiny_plan("b3-scratch-run");
    let (dir, crashes, failures) =
        run_b3_plan(&root, &plan, &|cell| cell.leg == 0, Some("test-commit")).expect("run");
    assert_eq!(crashes, 0);
    assert_eq!(failures, 0);
    for name in [
        "manifest.txt",
        "configs.tsv",
        "policies.tsv",
        "maps.tsv",
        "matches.tsv",
        "pairs.tsv",
        "summary.txt",
        "report.md",
        "hashes.txt",
        "analysis.txt",
    ] {
        assert!(dir.join(name).exists(), "{name} archived");
    }
    // The analysis hash is folded into hashes.txt.
    let analysis = std::fs::read_to_string(dir.join("analysis.txt")).expect("analysis");
    let hashes = std::fs::read_to_string(dir.join("hashes.txt")).expect("hashes");
    let analysis_hash = {
        let line = hashes
            .lines()
            .find(|l| l.ends_with("analysis.txt"))
            .expect("folded");
        line.split(':')
            .next()
            .unwrap()
            .trim_start_matches("blake3")
            .to_string()
    };
    // blake3:<hex>  analysis.txt — verify the hex against the file bytes.
    let expected = arena_replay::canon::hex(blake3::hash(analysis.as_bytes()).as_bytes());
    assert!(hashes.contains(&format!("blake3:{expected}  analysis.txt")));
    let _ = analysis_hash;
    // The analysis document carries the raw-count tables.
    assert!(analysis.contains("matches total=2 played=2"));
    assert!(analysis.contains("records-integrity:"));
    assert!(analysis.contains("Per-pair W-D-L"));
    // The per-policy mechanics table separates submitted/ok/dropped with
    // explicit slashes (regression pin: r1's analysis.txt concatenated ok
    // and dropped into one ambiguous column).
    assert!(analysis.contains("submitted/ok/dropped (drop permille)"));
    for line in analysis
        .lines()
        .filter(|l| l.starts_with("| greedy-field |"))
    {
        let last = line
            .split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .last()
            .unwrap_or_default();
        let counts = last.split_whitespace().next().unwrap_or_default();
        let parts: Vec<&str> = counts.split('/').collect();
        assert_eq!(
            parts.len(),
            3,
            "mechanics row counts '{counts}' are submitted/ok/dropped"
        );
    }
    // Records match policies.tsv (family/version/parameters per row).
    let policies_tsv = std::fs::read_to_string(dir.join("policies.tsv")).expect("policies");
    let matches_tsv = std::fs::read_to_string(dir.join("matches.tsv")).expect("matches");
    let header: Vec<&str> = matches_tsv.lines().next().unwrap().split('\t').collect();
    let a_params = header
        .iter()
        .position(|c| *c == "policy_a_parameters")
        .unwrap();
    let a_tag = header.iter().position(|c| *c == "policy_a").unwrap();
    for line in matches_tsv.lines().skip(1) {
        let values: Vec<&str> = line.split('\t').collect();
        let tag = values[a_tag];
        let parameters = values[a_params];
        let row = policies_tsv
            .lines()
            .find(|l| l.starts_with(tag))
            .unwrap_or_else(|| panic!("{tag} registered"));
        assert!(
            row.split('\t').nth(3).unwrap().contains(parameters),
            "{tag} parameters {parameters} match policies.tsv"
        );
    }
    // The manifest carries the balance-b3 reproduce line.
    let manifest = std::fs::read_to_string(dir.join("manifest.txt")).expect("manifest");
    assert!(manifest.contains("balance-b3"));
    // The report is stage-aware too (regression pin: the archived official
    // B3 runs carried the B1 smoke boilerplate — balance-b1 reproduce line,
    // "None registered" gates and the B1 doc note; the historical runs stay
    // frozen, this pins every FUTURE archive).
    let report = std::fs::read_to_string(dir.join("report.md")).expect("report");
    assert!(report.contains(
        "Reproduce: `cargo run --release -p arena-runner -- balance-b3 runs <base_commit>`"
    ));
    assert!(!report.contains("balance-b1"));
    assert!(!report.contains("None registered — B1"));
    assert!(report.contains("None registered for this experiment."));
    assert!(manifest.contains("docs/balance/balance-b3-ablation.md carries"));
    // An existing experiment directory is refused — nothing overwritten.
    let again = run_b3_plan(&root, &plan, &|_| false, Some("test-commit"));
    assert!(again.is_err(), "existing dir refused");
    std::fs::remove_dir_all(&root).ok();
}

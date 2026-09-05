//! Balance layer gates (B1, `docs/balance/ai-balance-plan.md` sections 12/13.2):
//! the frozen m6 `matches.tsv` byte-format pin (the report.rs visibility
//! refactor must change nothing), the balance record TSV round-trip with
//! tamper rejection, and — further below as the milestones land — the
//! end-to-end B1 completion condition.

use arena_bots::policy::{PolicyFamily, PolicyId, PolicyParameters};
use arena_core::types::Score2;
use arena_runner::balance::plan::{ConfigVariant, ExperimentPlan, SeedSet, Stage};
use arena_runner::balance::record::{
    BalanceMatchRecord, BalanceOutcome, balance_columns, balance_records_to_tsv, parse_balance_tsv,
};
use arena_runner::balance::schedule::{MatchCell, expand, pair_id};
use arena_runner::metrics::{CHECKPOINTS, FactionMatchStats, MatchStats, Outcome, ScoreSnapshot};
use arena_runner::policy::BucketPolicy;

fn smoke_plan(map_count: u32) -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: format!("record-test-{map_count}"),
        stage: Stage::Smoke,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: vec![
            ConfigVariant::new(
                "default",
                arena_core::config::GameConfig::default(),
                Vec::new(),
            ),
            {
                let mut cfg = arena_core::config::GameConfig::default();
                cfg.move_budget_cm = 24_000;
                ConfigVariant::new("move-budget-240m", cfg, vec!["move_budget_cm"])
            },
        ],
        policies: vec![
            PolicyId::GreedyField.frozen_m6_spec(),
            PolicyId::Hybrid.frozen_m6_spec(),
        ],
        matchups: vec![arena_runner::balance::plan::PolicyMatchup::new(
            "greedy-hybrid",
            PolicyId::GreedyField,
            PolicyId::Hybrid,
        )],
        bucket_policy: BucketPolicy::Flat(0),
        gates: Vec::new(),
    }
}

// FROZEN-FORMAT PIN: the m6 matches.tsv header is byte-identical after the
// report.rs visibility refactor (the balance layer borrows its helpers).
#[test]
fn m6_matches_tsv_format_is_unchanged() {
    let columns = arena_runner::report::columns();
    assert_eq!(
        columns.len(),
        79,
        "21 identity + 2x23 faction + 12 checkpoint"
    );
    let head: Vec<&str> = columns[..21].iter().map(|s| s.as_str()).collect();
    assert_eq!(
        head,
        vec![
            "index",
            "matchup_index",
            "matchup_tag",
            "j",
            "bot_a",
            "bot_b",
            "bot_a_is_enl",
            "bot_a_version",
            "bot_b_version",
            "game_seed",
            "map_seed",
            "map_index",
            "bot_seed",
            "schedule_seed",
            "bucket_policy",
            "config_hash",
            "result",
            "enl_score2",
            "res_score2",
            "final_state_hash",
            "elapsed_ms",
        ]
    );
    assert_eq!(columns[21], "enl_commands_submitted");
    assert_eq!(columns[43], "enl_supply_remaining");
    assert_eq!(columns[44], "res_commands_submitted");
    assert_eq!(columns[66], "res_supply_remaining");
    assert_eq!(columns[67], "enl_score_r50");
    assert_eq!(columns[78], "leader_r300");
    // The shared payload const matches the frozen order exactly.
    assert_eq!(
        arena_runner::report::FACTION_STAT_FIELDS[0],
        "commands_submitted"
    );
    assert_eq!(
        arena_runner::report::FACTION_STAT_FIELDS[22],
        "supply_remaining"
    );
    // And the m6 round-trip still holds (parse is the exact inverse).
    let mut record = arena_runner::metrics::MatchRecord {
        index: 0,
        matchup_index: 0,
        matchup_tag: "t".to_string(),
        j: 0,
        bot_a: "hybrid".to_string(),
        bot_b: "greedy-field".to_string(),
        bot_a_is_enl: true,
        game_seed: 1,
        map_seed: 2,
        map_index: 3,
        bot_seed: 4,
        schedule_seed: 5,
        bucket_policy: BucketPolicy::Flat(0),
        config_hash: 6,
        bot_a_version: "m6.0".to_string(),
        bot_b_version: "m6.0".to_string(),
        result: Outcome::Draw,
        enl_score2: 7,
        res_score2: 7,
        final_state_hash: [9u8; 32],
        stats: MatchStats::new(),
        elapsed_ms: 11,
    };
    record.stats.enl.first_field_round = Some(12);
    record.stats.res.field_area2_sum = 13;
    let tsv = arena_runner::report::records_to_tsv(&[record.clone()]);
    let parsed = arena_runner::report::parse_matches_tsv(&tsv).expect("round-trips");
    assert_eq!(parsed, vec![record]);
}

fn hand_built_record(index: u64, leg: u8, a_wins: bool) -> BalanceMatchRecord {
    let plan = smoke_plan(1);
    let cells = expand(&plan).expect("plan expands");
    let cell = cells
        .iter()
        .find(|c| c.leg == leg && c.variant_id == "default")
        .expect("cell exists");
    let spec_a = PolicyId::GreedyField.frozen_m6_spec();
    let spec_b = PolicyId::Hybrid.frozen_m6_spec();
    let mut record = BalanceMatchRecord::from_match_record(
        cell,
        &spec_a,
        &spec_b,
        &arena_runner::metrics::MatchRecord {
            index: cell.index,
            matchup_index: 0,
            matchup_tag: cell.pair_tag.clone(),
            j: leg as u32,
            bot_a: PolicyId::GreedyField.tag().to_string(),
            bot_b: PolicyId::Hybrid.tag().to_string(),
            bot_a_is_enl: cell.policy_a_is_enl,
            game_seed: cell.game_seed,
            map_seed: cell.map_seed,
            map_index: cell.map_index,
            bot_seed: cell.bot_seed,
            schedule_seed: cell.schedule_seed,
            bucket_policy: cell.bucket_policy,
            config_hash: cell.config_hash,
            bot_a_version: spec_a.version.to_string(),
            bot_b_version: spec_b.version.to_string(),
            result: if a_wins {
                Outcome::EnlWin
            } else {
                Outcome::ResWin
            },
            enl_score2: 100,
            res_score2: 60,
            final_state_hash: [0xAB; 32],
            stats: MatchStats {
                enl: FactionMatchStats {
                    commands_submitted: 10,
                    field_area2_sum: 1_000_000,
                    first_field_round: Some(30),
                    layering_depth_max: 2,
                    ..FactionMatchStats::default()
                },
                res: FactionMatchStats {
                    commands_submitted: 9,
                    ..FactionMatchStats::default()
                },
                checkpoints: CHECKPOINTS.map(|round| ScoreSnapshot {
                    round,
                    enl: 40,
                    res: 20,
                    leader: Some(arena_core::types::Faction::Enl),
                }),
            },
            elapsed_ms: 1_234,
        },
    );
    record.index = index;
    record
}

// The balance record round-trips exactly (modulo metadata-only elapsed
// time), the header width is pinned, and tampered rows are refused.
#[test]
fn balance_records_tsv_round_trips_exactly() {
    let records = vec![
        hand_built_record(0, 0, true),
        hand_built_record(1, 1, false),
    ];
    assert_eq!(balance_columns().len(), 29 + 2 * 23 + 12);
    let tsv = balance_records_to_tsv(&records);
    let parsed = parse_balance_tsv(&tsv).expect("round-trips");
    assert_eq!(parsed.len(), 2);
    for (a, b) in records.iter().zip(parsed.iter()) {
        assert!(a.same_ignoring_elapsed(b));
    }
    assert_eq!(parsed[0].policy_a, PolicyId::GreedyField);
    assert_eq!(parsed[0].policy_a_family, PolicyFamily::FieldBuilder);
    assert_eq!(parsed[0].policy_a_version, "m6.0");
    // The complete parameter sets ride in their own per-side columns
    // (plan 10.1 bullet 6; ADR-0016 gate 7 reads them back).
    assert!(balance_columns().contains(&"policy_a_parameters".to_string()));
    assert!(balance_columns().contains(&"policy_b_parameters".to_string()));
    assert_eq!(parsed[0].policy_a_parameters, PolicyParameters::FrozenM6);
    assert_eq!(parsed[0].policy_b_parameters, PolicyParameters::FrozenM6);
    assert_eq!(parsed[0].policy_b_family, PolicyFamily::AdaptiveHybrid);
    assert_eq!(parsed[0].stats.enl.first_field_round, Some(30));
    assert_eq!(parsed[0].stats.enl.field_area2_sum, 1_000_000);
    assert_eq!(parsed[0].result, BalanceOutcome::Played(Outcome::EnlWin));
    assert_eq!(parsed[1].leg, 1);
    assert!(!parsed[1].policy_a_is_enl);

    // A config-failure record round-trips too (stays in the result set).
    let plan = smoke_plan(1);
    let cells = expand(&plan).expect("expands");
    let failure = BalanceMatchRecord::config_failure(
        cells.last().expect("cell"),
        &PolicyId::GreedyField.frozen_m6_spec(),
        &PolicyId::Hybrid.frozen_m6_spec(),
    );
    let tsv = balance_records_to_tsv(&[failure.clone()]);
    let parsed = parse_balance_tsv(&tsv).expect("round-trips");
    assert_eq!(parsed[0].result, BalanceOutcome::ConfigFailure);
    assert_eq!(parsed[0].stats, MatchStats::new());
}

#[test]
fn tampered_balance_rows_are_refused() {
    let records = vec![hand_built_record(0, 0, true)];
    let tsv = balance_records_to_tsv(&records);
    // Bad policy tag.
    let tampered = tsv.replacen("greedy-field\t", "no-such-policy\t", 1);
    assert!(parse_balance_tsv(&tampered).is_err());
    // Bad family tag.
    let tampered = tsv.replacen("field-builder\t", "fieldbuilder\t", 1);
    assert!(parse_balance_tsv(&tampered).is_err());
    // Unknown parameter serialization: the full-value parameters column
    // parses strictly (an edited string never maps to a parameter set).
    let tampered = tsv.replacen("frozen-m6\t", "frozen-m6-v2\t", 1);
    assert!(parse_balance_tsv(&tampered).is_err());
    // Odd-length state hash hex.
    let odd_hash = {
        let mut row: Vec<String> =
            arena_runner::balance::record::balance_record_to_row(&hand_built_record(0, 0, true));
        let hash_at = balance_columns()
            .iter()
            .position(|c| c == "final_state_hash")
            .expect("hash column");
        row[hash_at] = "a".repeat(31); // odd nibble count -> last chunk is 1 char
        let mut text = balance_columns().join("\t");
        text.push('\n');
        text.push_str(&row.join("\t"));
        text.push('\n');
        text
    };
    assert!(parse_balance_tsv(&odd_hash).is_err(), "odd hash hex");
    // Bad result tag.
    let bad_result = {
        let mut row: Vec<String> =
            arena_runner::balance::record::balance_record_to_row(&hand_built_record(0, 0, true));
        let at = balance_columns()
            .iter()
            .position(|c| c == "result")
            .expect("result column");
        row[at] = "timeout".to_string();
        let mut text = balance_columns().join("\t");
        text.push('\n');
        text.push_str(&row.join("\t"));
        text.push('\n');
        text
    };
    assert!(parse_balance_tsv(&bad_result).is_err(), "bad result tag");
    // Header mismatch.
    assert!(parse_balance_tsv("index\tleg\n").is_err());
    assert!(parse_balance_tsv("").is_err());
    // Row width mismatch.
    let short = format!("{}\n0", balance_columns().join("\t"));
    assert!(parse_balance_tsv(&short).is_err());
}

// Compile-time-ish sanity on the cell/record vocabulary used above.
#[test]
fn pair_ids_and_map_ids_are_stable() {
    assert_eq!(pair_id("greedy-hybrid", 7), "greedy-hybrid#m007");
    let plan = smoke_plan(2);
    let cells = expand(&plan).expect("expands");
    assert_eq!(cells.len(), 2 * 2 * 2);
    let first: &MatchCell = cells.first().expect("cell");
    assert_eq!(first.map_id, "development-000");
    assert_eq!(first.pair_id, "greedy-hybrid#m000");
    let _: Score2 = 0;
}

// ---------------------------------------------------------------------
// Executor gates: worker independence, reproducibility, the crash seam,
// and sampled-replay capture (plan sections 12/13.2/13.3).
// ---------------------------------------------------------------------
mod executor_gates {
    use arena_bots::policy::PolicyId;
    use arena_core::config::GameConfig;
    use arena_core::harness::{AgentSubmission, RoundStrategy};
    use arena_core::map::MapDefinition;
    use arena_core::observe::Observation;
    use arena_runner::balance::execute::{
        CellSetup, default_cell_setup, run_balance_batch, run_balance_batch_with,
    };
    use arena_runner::balance::plan::{ConfigVariant, ExperimentPlan, SeedSet, Stage};
    use arena_runner::balance::record::BalanceOutcome;
    use arena_runner::balance::schedule::expand;
    use arena_runner::metrics::Outcome;
    use arena_runner::policy::BucketPolicy;

    fn tiny_plan() -> ExperimentPlan {
        ExperimentPlan {
            experiment_id: "exec-test".to_string(),
            stage: Stage::Sensitivity,
            seed_set: SeedSet::Development,
            map_count: 2,
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
            matchups: vec![arena_runner::balance::plan::PolicyMatchup::new(
                "greedy-hybrid",
                PolicyId::GreedyField,
                PolicyId::Hybrid,
            )],
            bucket_policy: BucketPolicy::Flat(0),
            gates: Vec::new(),
        }
    }

    /// A strategy that panics on its first round (the crash seam).
    struct PanicStrategy;
    impl RoundStrategy for PanicStrategy {
        fn submit_round(&mut self, _obs: &Observation) -> Vec<AgentSubmission> {
            panic!("crash seam: strategy panicked");
        }
    }

    fn setup_with_panicking_enl(
        cell: &arena_runner::balance::schedule::MatchCell,
        spec_enl: &arena_bots::policy::PolicySpec,
        spec_res: &arena_bots::policy::PolicySpec,
        cfg: &GameConfig,
        map: &MapDefinition,
    ) -> CellSetup {
        let mut setup = default_cell_setup(cell, spec_enl, spec_res, cfg, map);
        setup.enl = Box::new(PanicStrategy);
        setup
    }

    // The completion condition in miniature: identical records at 1 and 3
    // workers, and a second run reproduces the first (modulo elapsed_ms).
    #[test]
    fn executor_is_worker_independent_and_reproducible() {
        let plan = tiny_plan();
        let cells = expand(&plan).expect("expands");
        assert_eq!(cells.len(), 8, "2 variants x 1 pair x 2 maps x 2 legs");

        let one = run_balance_batch(&plan, 1).expect("runs at 1 worker");
        let three = run_balance_batch(&plan, 3).expect("runs at 3 workers");
        let again = run_balance_batch(&plan, 3).expect("runs again");
        assert_eq!(one.workers, 1);
        assert_eq!(three.workers, 3);
        assert_eq!(one.records.len(), 8);
        assert_eq!(three.records.len(), 8);
        assert_eq!(one.crashes(), 0);
        assert_eq!(one.config_failures, 0);
        for (a, b) in one.records.iter().zip(three.records.iter()) {
            assert!(
                a.same_ignoring_elapsed(b),
                "worker count moved record {}",
                a.index
            );
        }
        for (a, b) in three.records.iter().zip(again.records.iter()) {
            assert!(
                a.same_ignoring_elapsed(b),
                "record {} not reproducible",
                a.index
            );
        }
        // Both variants played, with swapped legs on shared seeds.
        for variant in ["default", "move-budget-240m"] {
            let rows: Vec<&arena_runner::balance::record::BalanceMatchRecord> = one
                .records
                .iter()
                .filter(|r| r.variant_id == variant)
                .collect();
            assert_eq!(rows.len(), 4);
            for k in 0..2u32 {
                let unit: Vec<&&arena_runner::balance::record::BalanceMatchRecord> =
                    rows.iter().filter(|r| r.map_index == k).collect();
                assert_eq!(unit.len(), 2);
                assert_ne!(unit[0].policy_a_is_enl, unit[1].policy_a_is_enl);
                assert_eq!(unit[0].game_seed, unit[1].game_seed);
                assert_eq!(unit[0].bot_seed, unit[1].bot_seed);
            }
        }
        // The two variants' rows for one map share seeds (common random
        // numbers) and identical map seeds.
        let default_row = one
            .records
            .iter()
            .find(|r| r.variant_id == "default" && r.map_index == 0)
            .expect("row");
        let variant_row = one
            .records
            .iter()
            .find(|r| r.variant_id == "move-budget-240m" && r.map_index == 0)
            .expect("row");
        assert_eq!(default_row.game_seed, variant_row.game_seed);
        assert_eq!(default_row.bot_seed, variant_row.bot_seed);
        assert_eq!(default_row.map_seed, variant_row.map_seed);
        assert_ne!(default_row.config_hash, variant_row.config_hash);
    }

    // A panicking policy is recorded as Crash and STAYS in the result set.
    #[test]
    fn a_panicking_policy_is_recorded_as_crash() {
        let plan = tiny_plan();
        let run =
            run_balance_batch_with(&plan, 3, &setup_with_panicking_enl, &|_| false).expect("runs");
        assert_eq!(run.records.len(), 8, "nothing dropped");
        for record in &run.records {
            assert_eq!(record.result, BalanceOutcome::Played(Outcome::Crash));
            assert_eq!(record.stats.enl.commands_submitted, 0);
        }
        assert_eq!(run.crashes(), 8);
        assert!(run.replays.is_empty(), "no sampling requested");
    }

    // The crash contract covers the WHOLE cell, not just the rounds: a
    // panic while constructing a cell (strategy build, spawn validation)
    // is one failed match, recorded as Crash — it must not abort the
    // batch (the frozen m6 `run_match` crash contract, plan 5.2 "isolate
    // single-match failures").
    #[test]
    fn a_panicking_cell_setup_is_recorded_as_crash_per_match() {
        let plan = tiny_plan();
        let run = run_balance_batch_with(
            &plan,
            2,
            &|_cell, _enl, _res, _cfg, _map| -> CellSetup {
                panic!("crash seam: cell setup panicked");
            },
            &|_| false,
        )
        .expect("the batch survives per-cell setup panics");
        assert_eq!(run.records.len(), 8, "every cell stays in the result set");
        for record in &run.records {
            assert_eq!(record.result, BalanceOutcome::Played(Outcome::Crash));
            assert_eq!(record.enl_score2, 0);
            assert_eq!(record.res_score2, 0);
        }
        assert_eq!(run.crashes(), 8);
        assert!(run.replays.is_empty());
    }

    // Sampled replays decode, replay and match their records (IARP capture
    // through the shared m6 play path).
    #[test]
    fn sampled_replays_decode_and_match_their_records() {
        let plan = tiny_plan();
        let run = run_balance_batch(&plan, 2).expect("runs");
        // Default sampling: leg 0 of every pair -> one per (variant, map).
        assert_eq!(run.replays.len(), 4);
        for replay in &run.replays {
            let record = run
                .records
                .iter()
                .find(|r| r.index == replay.cell_index)
                .expect("replay maps to a record");
            assert_eq!(record.leg, 0);
            let outcome = arena_replay::replay::replay_bytes(&replay.bytes).expect("replays");
            assert_eq!(outcome.final_state_hash, record.final_state_hash);
            assert_eq!(outcome.final_scores.0, record.enl_score2);
            assert_eq!(outcome.final_scores.1, record.res_score2);
            assert_eq!(outcome.rounds_completed as u64, 500);
            // Cross-variant shared map: same map -> same replay header map.
            let _ = arena_replay::replay::decode_log(&replay.bytes).expect("decodes");
        }
        // Both variants sampled the SAME map ids (the shared Map
        // Definition across a config contrast; the archive test pins the
        // map CONTENT hash below).
        let map_ids_of = |variant: &str| -> Vec<String> {
            let mut ids: Vec<String> = run
                .replays
                .iter()
                .filter(|r| r.variant_id == variant)
                .map(|r| r.map_id.clone())
                .collect();
            ids.sort();
            ids
        };
        assert_eq!(map_ids_of("default"), map_ids_of("move-budget-240m"));
        assert_eq!(map_ids_of("default").len(), 2);
    }
}

// ---------------------------------------------------------------------
// The B1 completion condition as a permanent gate: the frozen smoke plan
// definition, run twice, reproduces byte-identical matches.tsv/pairs.tsv
// modulo elapsed_ms; and the full section-12 artifact set.
// ---------------------------------------------------------------------
mod smoke_gates {
    use arena_runner::balance::archive::{ArchiveError, decode_map, write_balance_artifacts};
    use arena_runner::balance::execute::run_balance_batch;
    use arena_runner::balance::pairs::{compute_paired_results, variant_summaries};
    use arena_runner::balance::record::{balance_records_to_tsv, parse_balance_tsv};
    use arena_runner::balance::schedule::expand;
    use arena_runner::balance::smoke::{B1_SMOKE_EXPERIMENT_ID, B1_SMOKE_MAP_COUNT, b1_smoke_plan};

    fn zeroed_elapsed_tsv(records: &[arena_runner::balance::record::BalanceMatchRecord]) -> String {
        let mut parsed = records.to_vec();
        for record in &mut parsed {
            record.elapsed_ms = 0;
        }
        balance_records_to_tsv(&parsed)
    }

    // The frozen plan's shape: 2 variants x 8 maps x 2 legs = 32 cells.
    #[test]
    fn frozen_smoke_plan_shape_is_pinned() {
        let plan = b1_smoke_plan(B1_SMOKE_MAP_COUNT);
        assert_eq!(plan.experiment_id, B1_SMOKE_EXPERIMENT_ID);
        assert_eq!(plan.stage.tag(), "smoke");
        assert_eq!(plan.seed_set.tag(), "development");
        assert_eq!(plan.variants.len(), 2);
        assert_eq!(plan.base_variant, "default");
        let cells = expand(&plan).expect("expands");
        assert_eq!(cells.len(), 32);
        assert_eq!(
            plan.variants[1].declared_changed_fields,
            vec!["move_budget_cm"]
        );
        // The default variant IS the untouched GameConfig::default().
        assert_eq!(
            plan.variants[0].config,
            arena_core::config::GameConfig::default()
        );
        assert!(plan.policies.iter().all(|s| s.version == "m6.0"));
    }

    // The completion condition: two configs x two policies x same-map
    // faction swap reproduces (small map count for suite speed; the real
    // 8-map run is the CLI batch recorded in docs/balance/balance-b1-smoke.md).
    #[test]
    fn frozen_smoke_plan_reproduces_end_to_end() {
        let plan = b1_smoke_plan(2);
        let run_a = run_balance_batch(&plan, 3).expect("runs");
        let run_b = run_balance_batch(&plan, 3).expect("runs again");
        // matches.tsv: byte-identical modulo elapsed_ms.
        assert_eq!(
            zeroed_elapsed_tsv(&run_a.records),
            zeroed_elapsed_tsv(&run_b.records)
        );
        // pairs.tsv: byte-identical (integers only, no time columns).
        let pairs_a = compute_paired_results(&plan, &run_a.records).expect("pairs");
        let pairs_b = compute_paired_results(&plan, &run_b.records).expect("pairs");
        assert_eq!(
            arena_runner::balance::pairs::paired_results_to_tsv(&pairs_a),
            arena_runner::balance::pairs::paired_results_to_tsv(&pairs_b)
        );
        // The frozen plan's vocabulary is present: both variants, both
        // policies, swapped legs, shared seeds and shared map content.
        let tsv = balance_records_to_tsv(&run_a.records);
        for needle in ["default", "move-budget-240m", "greedy-field", "hybrid"] {
            assert!(tsv.contains(needle), "missing {needle}");
        }
        let parsed = parse_balance_tsv(&tsv).expect("round-trips");
        assert!(parsed.iter().any(|r| r.leg == 0 && r.policy_a_is_enl));
        assert!(parsed.iter().any(|r| r.leg == 1 && !r.policy_a_is_enl));
        let default_row = parsed
            .iter()
            .find(|r| r.variant_id == "default" && r.map_index == 0)
            .expect("row");
        let variant_row = parsed
            .iter()
            .find(|r| r.variant_id == "move-budget-240m" && r.map_index == 0)
            .expect("row");
        assert_eq!(default_row.game_seed, variant_row.game_seed);
        assert_eq!(default_row.bot_seed, variant_row.bot_seed);
        assert_eq!(default_row.map_seed, variant_row.map_seed);
        assert_ne!(default_row.config_hash, variant_row.config_hash);
        assert_eq!(default_row.policy_a_version, "m6.0");
    }

    // The full section-12 artifact set, written once into a NEW directory.
    #[test]
    fn archive_writes_the_complete_artifact_set_and_refuses_overwrites() {
        let plan = b1_smoke_plan(2);
        let run = run_balance_batch(&plan, 2).expect("runs");
        let pairs = compute_paired_results(&plan, &run.records).expect("pairs");
        let summaries = variant_summaries(&plan, &pairs);
        let root = std::env::temp_dir().join(format!(
            "b1-archive-test-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let dir =
            write_balance_artifacts(&root, &plan, &run, &pairs, &summaries, Some("testcommit"))
                .expect("archives");
        assert_eq!(dir, root.join("balance").join(B1_SMOKE_EXPERIMENT_ID));

        // All 9 artifact kinds exist (+ the replays dir and files).
        for name in [
            "manifest.txt",
            "configs.tsv",
            "policies.tsv",
            "maps.tsv",
            "matches.tsv",
            "pairs.tsv",
            "summary.txt",
            "report.md",
            "replays.tsv",
            "hashes.txt",
        ] {
            assert!(dir.join(name).is_file(), "missing {name}");
        }
        assert!(dir.join("maps").is_dir());
        assert!(dir.join("replays").is_dir());

        // configs.tsv: variant rows carry the complete 31-word config, the
        // changed fields and the config hash.
        let configs = std::fs::read_to_string(dir.join("configs.tsv")).expect("read");
        let header: Vec<&str> = configs
            .lines()
            .next()
            .expect("header")
            .split('\t')
            .collect();
        assert_eq!(header.len(), 1 + 31 + 2);
        assert_eq!(header[15], "move_budget_cm");
        let rows: Vec<Vec<&str>> = configs
            .lines()
            .skip(1)
            .map(|l| l.split('\t').collect())
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "default");
        assert_eq!(rows[0][33], "-");
        assert_eq!(rows[1][0], "move-budget-240m");
        assert_eq!(rows[1][15], "24000");
        assert_eq!(rows[0][15], "20000");
        assert_eq!(rows[1][33], "move_budget_cm");
        assert_ne!(rows[0][32], rows[1][32], "config hashes differ");

        // maps.tsv: one row per base map (the smoke plan has no map-regime
        // variant, so every row belongs to the base variant — ADR-0018
        // added the trailing variant column); map files are
        // content-addressed and decode back to the identical MapDefinition.
        let maps_text = std::fs::read_to_string(dir.join("maps.tsv")).expect("read");
        assert_eq!(
            maps_text.lines().next().expect("header"),
            "map_id\tmap_seed\tcontent_hash\tfile\tvariant_id"
        );
        let map_rows: Vec<Vec<&str>> = maps_text
            .lines()
            .skip(1)
            .map(|l| l.split('\t').collect())
            .collect();
        assert_eq!(map_rows.len(), 2);
        let mut seen_hashes = std::collections::BTreeSet::new();
        for row in &map_rows {
            assert_eq!(row.len(), 5);
            assert_eq!(row[4], "default", "base rows carry the base variant id");
            assert_eq!(row[3], format!("maps/map-{}.bin", row[2]));
            let bytes = std::fs::read(dir.join(row[3])).expect("map file");
            assert_eq!(
                arena_replay::canon::hex(blake3::hash(&bytes).as_bytes()),
                row[2],
                "content-addressed name matches an independent hash"
            );
            let decoded = decode_map(&bytes).expect("decodes");
            let seed = u64::from_str_radix(row[1].trim_start_matches("0x"), 16).expect("seed");
            let regenerated =
                arena_core::map::generate_map(seed, &arena_core::config::GameConfig::default())
                    .expect("regenerates");
            assert_eq!(decoded, regenerated, "map round-trips through the archive");
            seen_hashes.insert(row[2].to_string());
        }
        assert_eq!(seen_hashes.len(), 2, "two distinct maps");

        // Shared-Map-Definition proof: both variants' rows for one map id
        // reference the SAME map content hash (one maps.tsv row, one file).
        let matches_text = std::fs::read_to_string(dir.join("matches.tsv")).expect("read");
        let map_ids_in_matches: std::collections::BTreeSet<&str> = matches_text
            .lines()
            .skip(1)
            .map(|l| l.split('\t').collect::<Vec<_>>())
            .map(|row| row[7]) // map_id column
            .collect();
        assert_eq!(map_ids_in_matches.len(), 2);
        let hash_of = |map_id: &str| -> String {
            map_rows
                .iter()
                .find(|r| r[0] == map_id)
                .map(|r| r[2].to_string())
                .expect("map row")
        };
        let first_map_id = map_ids_in_matches.iter().next().copied().expect("id");
        let _ = hash_of(first_map_id);

        // replays.tsv lists the sampled cells; files exist and decode.
        let replays_text = std::fs::read_to_string(dir.join("replays.tsv")).expect("read");
        let replay_rows: Vec<Vec<&str>> = replays_text
            .lines()
            .skip(1)
            .map(|l| l.split('\t').collect())
            .collect();
        assert_eq!(
            replay_rows.len(),
            run.replays.len(),
            "one row per captured replay"
        );
        for row in &replay_rows {
            assert_eq!(row[5], "captured");
            let bytes = std::fs::read(dir.join(row[6])).expect("replay file");
            arena_replay::replay::decode_log(&bytes).expect("decodes");
        }

        // hashes.txt: every artifact's blake3 matches an independent
        // re-hash, and hashes.txt itself is not listed.
        let hashes_text = std::fs::read_to_string(dir.join("hashes.txt")).expect("read");
        let mut hashed_paths = 0usize;
        for line in hashes_text.lines() {
            let (hash, relative) = line.split_once("  ").expect("format");
            let bytes = std::fs::read(dir.join(relative)).expect("artifact");
            assert_eq!(
                hash.trim_start_matches("blake3:"),
                arena_replay::canon::hex(blake3::hash(&bytes).as_bytes()),
                "hash mismatch for {relative}"
            );
            hashed_paths += 1;
        }
        // 9 top-level artifacts (hashes.txt never lists itself) + one map
        // file per distinct map + one file per captured replay.
        assert!(hashed_paths >= 9 + run.replays.len() + 2);
        assert!(!hashes_text.contains("hashes.txt"));

        // An existing experiment directory is refused (never overwrite).
        match write_balance_artifacts(&root, &plan, &run, &pairs, &summaries, Some("testcommit")) {
            Err(ArchiveError::ExperimentDirExists(_)) => {}
            other => panic!("expected refusal, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}

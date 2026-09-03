//! Balance layer gates (B1, `docs/ai-balance-plan.md` sections 12/13.2):
//! the frozen m6 `matches.tsv` byte-format pin (the report.rs visibility
//! refactor must change nothing), the balance record TSV round-trip with
//! tamper rejection, and — further below as the milestones land — the
//! end-to-end B1 completion condition.

use arena_bots::policy::{PolicyFamily, PolicyId};
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
        bot_a: arena_bots::bot::BotKind::Hybrid,
        bot_b: arena_bots::bot::BotKind::GreedyField,
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
            bot_a: PolicyId::GreedyField.bot_kind(),
            bot_b: PolicyId::Hybrid.bot_kind(),
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
    assert_eq!(balance_columns().len(), 27 + 2 * 23 + 12);
    let tsv = balance_records_to_tsv(&records);
    let parsed = parse_balance_tsv(&tsv).expect("round-trips");
    assert_eq!(parsed.len(), 2);
    for (a, b) in records.iter().zip(parsed.iter()) {
        assert!(a.same_ignoring_elapsed(b));
    }
    assert_eq!(parsed[0].policy_a, PolicyId::GreedyField);
    assert_eq!(parsed[0].policy_a_family, PolicyFamily::FieldBuilder);
    assert_eq!(parsed[0].policy_a_version, "m6.0");
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

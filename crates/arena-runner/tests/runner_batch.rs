//! Runner batch integration gates (checklist 14, 15, 18, 19, 20): the
//! MatchRecord metadata contract, the crash path, TSV round-trips,
//! sub-batch reproducibility with worker independence, and M6-bot replay
//! spot-checks.

use arena_bots::bot::{BotKind, make_bot};
use arena_core::config::GameConfig;
use arena_core::harness::{MatchHarness, RoundStrategy};
use arena_core::map::generate_map;
use arena_core::observe::Observation;
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::{Coord, Faction};
use arena_runner::batch::{BatchConfig, Matchup};
use arena_runner::metrics::Outcome;
use arena_runner::policy::BucketPolicy;
use arena_runner::report::{BatchSummary, parse_matches_tsv, records_to_tsv, write_artifacts};
use arena_runner::run::{run_batch, run_match, run_match_with};

fn integration_batch() -> BatchConfig {
    BatchConfig {
        batch_id: "runner-batch".to_string(),
        matchups: vec![
            Matchup::new(
                "Nearest-Nearest",
                BotKind::NearestPortal,
                BotKind::NearestPortal,
                4,
            ),
            Matchup::new("Greedy-Hybrid", BotKind::GreedyField, BotKind::Hybrid, 4),
            Matchup::new("Random-Random", BotKind::Random, BotKind::Random, 4),
        ],
        map_count: 3,
        master_seed: 0xBACC_0002,
        bucket_policy: BucketPolicy::Flat(0),
        config: GameConfig::default(),
    }
}

// Checklist 14: MatchRecords carry the full metadata contract.
#[test]
fn records_carry_the_full_metadata_contract() {
    let batch = integration_batch();
    let run = run_batch(&batch, 4).expect("batch runs");
    assert_eq!(run.records.len(), 12);
    let decisions: Vec<bool> =
        arena_runner::schedule::a_is_enl_decisions(batch.schedule_seed(), "Greedy-Hybrid", 4);
    for record in &run.records {
        assert_eq!(record.config_hash, batch.config_hash());
        assert_eq!(
            record.game_seed,
            batch.game_seed(&record.matchup_tag, record.j)
        );
        assert_eq!(record.map_seed, batch.map_seed(record.map_index));
        assert_eq!(
            record.bot_seed,
            batch.bot_seed(&record.matchup_tag, record.j)
        );
        assert_eq!(record.schedule_seed, batch.schedule_seed());
        assert_eq!(record.bucket_policy, batch.bucket_policy);
        assert_eq!(record.bot_a_version, "m6.0");
        assert_eq!(record.bot_b_version, "m6.0");
        // The schedule decision is echoed per match.
        if record.matchup_tag == "Greedy-Hybrid" {
            assert_eq!(record.bot_a_is_enl, decisions[record.j as usize]);
        }
        assert!(
            matches!(
                record.result,
                Outcome::EnlWin | Outcome::ResWin | Outcome::Draw | Outcome::Crash
            ),
            "legal outcome"
        );
        assert_ne!(record.result, Outcome::Crash);
        assert!(record.elapsed_ms < 60_000, "elapsed recorded");
        assert_ne!(record.final_state_hash, [0u8; 32], "final hash recorded");
        // The recorded result agrees with the recorded scores.
        match record.result {
            Outcome::EnlWin => assert!(record.enl_score2 > record.res_score2),
            Outcome::ResWin => assert!(record.res_score2 > record.enl_score2),
            Outcome::Draw => assert_eq!(record.enl_score2, record.res_score2),
            Outcome::Crash => unreachable!("no crashes in this batch"),
        }
    }
}

// Checklist 15: a panicking stub bot is recorded as Outcome::Crash and
// stays in the result set; other matches still complete.
#[test]
fn panicking_bot_is_recorded_as_crash_and_batch_completes() {
    struct PanicStrategy;
    impl RoundStrategy for PanicStrategy {
        fn submit_round(
            &mut self,
            _obs: &Observation,
        ) -> Vec<arena_core::harness::AgentSubmission> {
            panic!("stub bot failure");
        }
    }
    struct IdleStrategy;
    impl RoundStrategy for IdleStrategy {
        fn submit_round(
            &mut self,
            _obs: &Observation,
        ) -> Vec<arena_core::harness::AgentSubmission> {
            Vec::new()
        }
    }

    let batch = integration_batch();
    let descriptor = &batch.descriptors()[0];
    let cfg = &batch.config;
    let map = generate_map(descriptor.map_seed, cfg).expect("map");
    let (enl_roster, res_roster) = GameState::initial_roster(cfg);
    let spawns = |roster: &[arena_core::types::AgentId]| {
        let entries: Vec<_> = roster.iter().map(|&id| (id, Coord::new(0, 0))).collect();
        validate_initial_spawn_set(roster, &entries, cfg).expect("spawns")
    };
    let record = run_match_with(
        descriptor,
        cfg,
        &map,
        &spawns(&enl_roster),
        &spawns(&res_roster),
        Box::new(PanicStrategy),
        Box::new(IdleStrategy),
    );
    assert_eq!(record.result, Outcome::Crash);
    assert_eq!(record.index, descriptor.index);
    assert_eq!(record.matchup_tag, descriptor.matchup_tag);
    assert_eq!(record.game_seed, descriptor.game_seed);
    assert_eq!(record.enl_score2, 0);

    // The rest of the batch still completes around the crashed match.
    let run = run_batch(&batch, 3).expect("other matches complete");
    assert_eq!(run.records.len(), 12);
    assert!(run.records.iter().all(|r| r.result != Outcome::Crash));
    // And the crashed record stays in a result set alongside them.
    let mut all = run.records.clone();
    all.push(record);
    assert_eq!(all.len(), 13);
    assert_eq!(all.iter().filter(|r| r.result == Outcome::Crash).count(), 1);
}

// B2 (ADR-0016): MatchRecord bot fields are POLICY TAG strings — a record
// carrying an R2 tag no BotKind maps to round-trips through matches.tsv
// verbatim, so balance cells are recorded honestly.
#[test]
fn a_record_with_a_non_bot_kind_policy_tag_round_trips() {
    let batch = integration_batch();
    let run = run_batch(&batch, 2).expect("batch runs");
    let mut record = run.records[0].clone();
    record.bot_a = "greedy-coordinated-r2".to_string();
    record.bot_b = "idle".to_string();
    let text = records_to_tsv(&[record.clone()]);
    let parsed = parse_matches_tsv(&text).expect("parse");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].bot_a, "greedy-coordinated-r2");
    assert_eq!(parsed[0].bot_b, "idle");
    assert_eq!(parsed[0], record, "full record round-trips");
}

// Checklist 18: manifest/matches.tsv/summary.txt round-trip — re-parsing
// matches.tsv reproduces the in-memory MatchRecords exactly.
#[test]
fn matches_tsv_round_trips_exactly() {
    let batch = integration_batch();
    let run = run_batch(&batch, 4).expect("batch runs");
    let text = records_to_tsv(&run.records);
    let parsed = parse_matches_tsv(&text).expect("parse");
    assert_eq!(parsed.len(), run.records.len());
    for (original, reparsed) in run.records.iter().zip(parsed.iter()) {
        assert_eq!(original, reparsed, "record {} round-trips", original.index);
    }

    // Artifacts write and re-read.
    let dir = std::env::temp_dir().join("arena-runner-test-artifacts");
    let _ = std::fs::remove_dir_all(&dir);
    write_artifacts(&dir, &batch, &run.records, run.workers, run.elapsed_ms)
        .expect("write artifacts");
    let manifest = std::fs::read_to_string(dir.join("manifest.txt")).expect("manifest");
    assert!(manifest.contains("batch_id=runner-batch"));
    assert!(manifest.contains("bucket_policy=flat:0"));
    assert!(manifest.contains(&format!("schedule_seed={:#018x}", batch.schedule_seed())));
    let from_disk =
        parse_matches_tsv(&std::fs::read_to_string(dir.join("matches.tsv")).expect("tsv"))
            .expect("parse disk");
    assert_eq!(from_disk, run.records);
    let summary_text = std::fs::read_to_string(dir.join("summary.txt")).expect("summary");
    assert!(summary_text.contains("Nearest-Nearest"));
    assert!(summary_text.contains("snowball_r100"));
    let _ = std::fs::remove_dir_all(&dir);
}

// Checklist 19: a 32-match sub-batch re-run reproduces identical final
// hashes and MatchRecords; worker count (1 vs 12) cannot move a match.
#[test]
fn sub_batch_is_reproducible_and_worker_independent() {
    let batch = BatchConfig {
        batch_id: "runner-repro".to_string(),
        matchups: vec![
            Matchup::new(
                "Nearest-Nearest",
                BotKind::NearestPortal,
                BotKind::NearestPortal,
                6,
            ),
            Matchup::new("Greedy-Hybrid", BotKind::GreedyField, BotKind::Hybrid, 6),
            Matchup::new("Hybrid-Hybrid", BotKind::Hybrid, BotKind::Hybrid, 6),
            Matchup::new("Random-Random", BotKind::Random, BotKind::Random, 6),
            Matchup::new(
                "Greedy-Greedy",
                BotKind::GreedyField,
                BotKind::GreedyField,
                4,
            ),
            Matchup::new(
                "Nearest-Destroy",
                BotKind::NearestPortal,
                BotKind::DestroyAnchor,
                4,
            ),
        ],
        map_count: 6,
        master_seed: 0x3E90_0BAD,
        bucket_policy: BucketPolicy::Flat(0),
        config: GameConfig::default(),
    };
    assert_eq!(batch.total_matches(), 32);

    let first = run_batch(&batch, 12).expect("12-thread run");
    let second = run_batch(&batch, 12).expect("re-run");
    let serial = run_batch(&batch, 1).expect("serial run");

    assert_eq!(first.records.len(), 32);
    for (a, b) in first.records.iter().zip(second.records.iter()) {
        assert!(
            a.same_ignoring_elapsed(b),
            "re-run reproduces match {}",
            a.index
        );
    }
    for (a, b) in first.records.iter().zip(serial.records.iter()) {
        assert!(
            a.same_ignoring_elapsed(b),
            "worker count cannot move match {} (final hash {:?} vs {:?})",
            a.index,
            a.final_state_hash,
            b.final_state_hash
        );
    }

    // And single matches re-run identically on their own.
    let descriptors = batch.descriptors();
    for descriptor in [
        descriptors[0].clone(),
        descriptors[13].clone(),
        descriptors[31].clone(),
    ] {
        let map = generate_map(descriptor.map_seed, &batch.config).expect("map");
        let record = run_match(&descriptor, &batch.config, &map);
        let batch_record = &first.records[descriptor.index as usize];
        assert!(
            record.same_ignoring_elapsed(batch_record),
            "match {} reproduces standalone",
            descriptor.index
        );
    }
}

// Checklist 20: 8 recorded M6-bot matches decode and replay to identical
// per-round and final hashes (the M5 property extended to M6 bots).
#[test]
fn recorded_matches_replay_identically() {
    use arena_replay::recorder::ReplayWriter;
    use arena_replay::replay::{decode_log, replay};

    let batch = integration_batch();
    let descriptors = batch.descriptors();
    // Spread across the three matchups and both faction assignments.
    let picks = [0usize, 1, 4, 5, 8, 9, 10, 11];
    for pick in picks {
        let descriptor = &descriptors[pick];
        let cfg = &batch.config;
        let map = generate_map(descriptor.map_seed, cfg).expect("map");
        let (enl_kind, res_kind) = descriptor.faction_kinds();
        let mut enl_bot = make_bot(enl_kind, Faction::Enl, descriptor.bot_seed, cfg, &map);
        let mut res_bot = make_bot(res_kind, Faction::Res, descriptor.bot_seed, cfg, &map);
        let (enl_roster, res_roster) = GameState::initial_roster(cfg);
        let enl_entries = enl_bot.choose_spawns(cfg, &map, &enl_roster);
        let res_entries = res_bot.choose_spawns(cfg, &map, &res_roster);
        let enl_spawns =
            validate_initial_spawn_set(&enl_roster, &enl_entries, cfg).expect("spawns");
        let res_spawns =
            validate_initial_spawn_set(&res_roster, &res_entries, cfg).expect("spawns");

        // The recorded run (buckets through the policy, like run_match).
        let enl = arena_runner::policy::PolicyStrategy::new(
            Box::new(arena_bots::bot::BotStrategy(enl_bot)),
            Faction::Enl,
            descriptor.bucket_policy,
        );
        let res = arena_runner::policy::PolicyStrategy::new(
            Box::new(arena_bots::bot::BotStrategy(res_bot)),
            Faction::Res,
            descriptor.bucket_policy,
        );
        let mut writer =
            ReplayWriter::new(descriptor.game_seed, cfg, &map, &enl_spawns, &res_spawns);
        let mut harness = MatchHarness::new(
            descriptor.game_seed,
            *cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(enl),
            Box::new(res),
        )
        .expect("match builds");
        for _ in 0..cfg.total_rounds {
            let report = harness.run_round();
            writer
                .record_round(&report, harness.state())
                .expect("record");
        }
        let bytes = writer.finish(harness.state()).expect("finish");
        let decoded = decode_log(&bytes).expect("decode");
        let outcome = replay(&decoded).expect("every round replays identically");
        assert_eq!(outcome.rounds_completed, cfg.total_rounds);
        assert_eq!(
            outcome.final_state_hash,
            arena_replay::canon::state_hash(harness.state())
        );
        assert_eq!(outcome.result, harness.state().final_result());

        // The plain run of the same descriptor agrees on the final hash.
        let record = run_match(descriptor, cfg, &map);
        assert_eq!(
            record.final_state_hash, outcome.final_state_hash,
            "recorded replay and plain run agree for match {}",
            descriptor.index
        );
    }
}

// Bonus consistency: the batch summary aggregation over a real run has no
// empty metric rows. (Nearest-Nearest and Random-Random legitimately score
// zero — neither baseline ever LINKs reliably — so "someone scored" is
// asserted only for the builder matchup.)
#[test]
fn real_batch_summary_has_no_empty_rows() {
    let batch = integration_batch();
    let run = run_batch(&batch, 4).expect("batch runs");
    let summary = BatchSummary::from_records(&run.records, &batch.matchups);
    assert_eq!(summary.matchups.len(), 3);
    for row in &summary.matchups {
        assert_eq!(row.count, 4);
        assert!(row.enl.commands_submitted > 0, "ENL submitted commands");
        assert!(row.res.commands_submitted > 0, "RES submitted commands");
        assert!(row.enl.attacks + row.enl.captures + row.enl.links_created > 0);
        assert!(row.res.attacks + row.res.captures + row.res.links_created > 0);
        if row.tag == "Greedy-Hybrid" {
            assert!(
                row.enl.score2_sum > 0 || row.res.score2_sum > 0,
                "the builder matchup scores"
            );
        }
    }
}

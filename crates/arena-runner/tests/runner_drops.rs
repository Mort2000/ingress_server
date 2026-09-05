//! Drop-reason tally integration gates (B0 evidence pass): a small frozen
//! batch is archived to TSV, re-parsed, re-executed per archived record,
//! verified against each archived row, and aggregated — the exact pipeline
//! behind the `arena-runner drop-reasons` subcommand and the per-reason
//! breakdown quoted in `docs/balance/balance-report-r1.md`.

use arena_bots::bot::BotKind;
use arena_core::config::GameConfig;
use arena_core::map::generate_map;
use arena_runner::batch::{BatchConfig, Matchup};
use arena_runner::drops::{
    aggregate_drop_reasons, drop_reasons_tsv, reexecute_archived_match, verify_match_drops,
};
use arena_runner::policy::BucketPolicy;
use arena_runner::report::{parse_matches_tsv, records_to_tsv};
use arena_runner::run::run_batch;

fn drops_batch() -> BatchConfig {
    BatchConfig {
        batch_id: "drops-gate".to_string(),
        matchups: vec![
            // The matchup the report's drop note cites, plus a mirror so
            // the aggregation's pooled-slots path is exercised.
            Matchup::new(
                "Greedy-DestroyAnchor",
                BotKind::GreedyField,
                BotKind::DestroyAnchor,
                4,
            ),
            Matchup::new("Hybrid-Hybrid", BotKind::Hybrid, BotKind::Hybrid, 2),
        ],
        map_count: 2,
        master_seed: 0xD00D_0001,
        bucket_policy: BucketPolicy::Flat(0),
        config: GameConfig::default(),
    }
}

/// The full pipeline on a real batch: archive -> re-parse -> re-execute ->
/// verify -> aggregate, with the per-reason sums reconciling exactly
/// against the archived per-faction drop totals.
#[test]
fn archived_drop_reasons_reexecute_verify_and_reconcile() {
    let batch = drops_batch();
    let run = run_batch(&batch, 3).expect("batch runs");
    let text = records_to_tsv(&run.records);
    let records = parse_matches_tsv(&text).expect("round-trip parse");
    assert_eq!(records.len(), 6);

    let cfg = GameConfig::default();
    let mut tallies = Vec::with_capacity(records.len());
    for record in &records {
        let map = generate_map(record.map_seed, &cfg).expect("archived map regenerates");
        let drops = reexecute_archived_match(record, &cfg, &map)
            .unwrap_or_else(|error| panic!("match {}: {error}", record.index));
        verify_match_drops(record, &drops)
            .unwrap_or_else(|error| panic!("match {}: {error}", record.index));
        tallies.push(Some(drops));
    }

    // Determinism of the tally itself: the same archived record
    // re-executes to identical counts.
    let again = reexecute_archived_match(
        &records[0],
        &cfg,
        &generate_map(records[0].map_seed, &cfg).expect("map"),
    )
    .expect("re-execution");
    assert_eq!(tallies[0].as_ref().expect("first tally"), &again);

    let matchups = aggregate_drop_reasons(&records, &tallies).expect("aggregation cross-checks");
    assert_eq!(matchups.len(), 2);

    let gd = matchups
        .iter()
        .find(|m| m.tag == "Greedy-DestroyAnchor")
        .expect("GD matchup");
    assert_eq!(gd.matches, 4);
    assert_eq!(gd.bots.len(), 2);
    for bot in &gd.bots {
        assert_eq!(bot.matches, 4);
        // The reconciliation the report quotes: per-reason sum equals the
        // archived commands_dropped total, exactly.
        assert_eq!(bot.reason_sum(), bot.archived_dropped);
        assert!(bot.archived_dropped > 0, "greedy and destroy both drop");
    }
    // GreedyField's dominant reason in this pairing is the internal
    // capture race (the report's root-cause claim, at gate scale).
    let greedy = gd
        .bots
        .iter()
        .find(|bot| bot.bot == BotKind::GreedyField)
        .expect("greedy entry");
    let races = greedy
        .reasons
        .get("CaptureTargetNotNeutral")
        .copied()
        .unwrap_or(0);
    assert!(
        races * 10 > greedy.archived_dropped,
        "capture races dominate"
    );

    // Mirror matchup: both slots pool into ONE entry per BotKind.
    let hh = matchups
        .iter()
        .find(|m| m.tag == "Hybrid-Hybrid")
        .expect("HH matchup");
    assert_eq!(hh.matches, 2);
    assert_eq!(hh.bots.len(), 1);
    assert_eq!(hh.bots[0].bot, BotKind::Hybrid);
    assert_eq!(hh.bots[0].matches, 2, "a mirror match counts once");
    assert_eq!(hh.bots[0].reason_sum(), hh.bots[0].archived_dropped);

    // The TSV carries every reason row (zeros included) plus totals.
    let tsv = drop_reasons_tsv(&matchups);
    assert!(tsv.starts_with("matchup_tag\tbot\tdrop_reason\tcount\n"));
    assert!(tsv.contains("Greedy-DestroyAnchor\tgreedy-field\tCaptureTargetNotNeutral\t"));
    assert!(tsv.contains("Greedy-DestroyAnchor\tgreedy-field\tLinkSelfLoop\t0\n"));
    assert!(tsv.contains("Hybrid-Hybrid\thybrid\tTOTAL\t"));
}

/// Verification is not decorative: a tampered archive row (one dropped
/// count nudged) must fail the per-match comparison, so the tally can
/// never launder numbers that did not come from the frozen run.
#[test]
fn tampered_archive_row_fails_verification() {
    let batch = drops_batch();
    let run = run_batch(&batch, 2).expect("batch runs");
    let text = records_to_tsv(&run.records);
    let mut records = parse_matches_tsv(&text).expect("round-trip parse");

    let tampered_drops = records[0].stats.enl.commands_dropped + 1;
    records[0].stats.enl.commands_dropped = tampered_drops;
    let cfg = GameConfig::default();
    let map = generate_map(records[0].map_seed, &cfg).expect("archived map regenerates");
    let drops = reexecute_archived_match(&records[0], &cfg, &map).expect("re-execution runs");
    let error = verify_match_drops(&records[0], &drops)
        .expect_err("tampered drop total must fail verification");
    assert!(error.contains("command counts mismatch"), "{error}");

    // A wrong final State Hash is caught the same way.
    let mut wrong_hash = records[1].clone();
    wrong_hash.final_state_hash[0] ^= 0xFF;
    let map = generate_map(wrong_hash.map_seed, &cfg).expect("archived map regenerates");
    let drops = reexecute_archived_match(&wrong_hash, &cfg, &map).expect("re-execution runs");
    let error = verify_match_drops(&wrong_hash, &drops)
        .expect_err("wrong state hash must fail verification");
    assert!(error.contains("State Hash mismatch"), "{error}");
}

//! Scaled runner gate (checklist 21): all six frozen matchups, 20 matches
//! each on 30 shared maps, `Flat(0)`, completing green inside the test
//! budget. `ARENA_RUNNER_GATE_COUNT` scales the per-matchup count down for
//! iteration (the frozen experiment itself is the binary run, not this
//! test).

use std::time::{Duration, Instant};

use arena_bots::bot::BotKind;
use arena_core::config::GameConfig;
use arena_runner::batch::{BatchConfig, Matchup};
use arena_runner::metrics::Outcome;
use arena_runner::policy::BucketPolicy;
use arena_runner::report::BatchSummary;
use arena_runner::run::run_batch;

/// Default per-matchup count; the env var lowers it for iteration.
fn gate_count() -> u32 {
    std::env::var("ARENA_RUNNER_GATE_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20)
}

/// The frozen six-matchup table (the same one the binary runs).
fn frozen_matchups(count: u32) -> Vec<Matchup> {
    vec![
        Matchup::new("Random-Random", BotKind::Random, BotKind::Random, count),
        Matchup::new(
            "Nearest-Nearest",
            BotKind::NearestPortal,
            BotKind::NearestPortal,
            count,
        ),
        Matchup::new(
            "Nearest-Greedy",
            BotKind::NearestPortal,
            BotKind::GreedyField,
            count,
        ),
        Matchup::new(
            "Greedy-DestroyAnchor",
            BotKind::GreedyField,
            BotKind::DestroyAnchor,
            count,
        ),
        Matchup::new(
            "Greedy-Hybrid",
            BotKind::GreedyField,
            BotKind::Hybrid,
            count,
        ),
        Matchup::new("Hybrid-Hybrid", BotKind::Hybrid, BotKind::Hybrid, count),
    ]
}

#[test]
fn scaled_gate_six_matchups_complete_green() {
    let count = gate_count().max(2);
    if count % 2 == 1 {
        // Keep the schedule's equal-exposure contract intact.
        panic!("ARENA_RUNNER_GATE_COUNT must be even");
    }
    let batch = BatchConfig {
        batch_id: "runner-gate".to_string(),
        matchups: frozen_matchups(count),
        map_count: 30,
        master_seed: 0x9A7E_0003,
        bucket_policy: BucketPolicy::Flat(0),
        config: GameConfig::default(),
    };
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(arena_runner::run::MAX_WORKERS);
    let start = Instant::now();
    let run = run_batch(&batch, workers).expect("scaled gate runs");
    let elapsed = start.elapsed();

    assert_eq!(run.records.len() as u32, count * 6, "every match ran");
    let summary = BatchSummary::from_records(&run.records, &batch.matchups);

    // All six matchups present with full counts and legal outcomes.
    assert_eq!(summary.matchups.len(), 6);
    let expected_tags = [
        "Random-Random",
        "Nearest-Nearest",
        "Nearest-Greedy",
        "Greedy-DestroyAnchor",
        "Greedy-Hybrid",
        "Hybrid-Hybrid",
    ];
    for (row, tag) in summary.matchups.iter().zip(expected_tags) {
        assert_eq!(row.tag, tag);
        assert_eq!(row.count, count as u64);
        assert_eq!(row.crashes, 0, "{tag}: no crashes");
        for record in run.records.iter().filter(|r| r.matchup_tag == tag) {
            assert!(
                matches!(
                    record.result,
                    Outcome::EnlWin | Outcome::ResWin | Outcome::Draw
                ),
                "{tag}: legal outcome"
            );
        }
        // No empty metric rows: both factions played the field game
        // somewhere in the matchup.
        assert!(row.enl.commands_submitted > 0, "{tag} ENL commands");
        assert!(row.res.commands_submitted > 0, "{tag} RES commands");
    }

    // The builder matchups really build fields on the 30-map pool.
    for tag in ["Greedy-Hybrid", "Hybrid-Hybrid"] {
        let row = summary.matchups.iter().find(|r| r.tag == tag).unwrap();
        assert!(
            row.enl.fields_created + row.res.fields_created > 0,
            "{tag}: fields were created"
        );
        assert!(
            row.enl.score2_sum + row.res.score2_sum > 0,
            "{tag}: fields scored"
        );
    }

    // Faction exposure: exactly count/2 ENL per bot in every matchup.
    for row in &summary.matchups {
        let enl_a = run
            .records
            .iter()
            .filter(|r| r.matchup_tag == row.tag && r.bot_a_is_enl)
            .count();
        assert_eq!(enl_a as u64, row.count / 2, "{}: equal exposure", row.tag);
    }

    // Test-suite budget (debug build).
    assert!(
        elapsed < Duration::from_secs(300),
        "gate of {} matches took {elapsed:?}",
        count * 6
    );
    eprintln!(
        "RUNNER GATE: matches={} maps={} workers={} elapsed={:?} ({:.1} matches/s)",
        count * 6,
        batch.map_count,
        run.workers,
        elapsed,
        (count * 6) as f64 / elapsed.as_secs_f64()
    );
}

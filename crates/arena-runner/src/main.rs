//! The frozen 10,000-match M6 experiment (P2.5-GATE, D58/D59):
//!
//! ```text
//! Random x Random            1000      Greedy x DestroyAnchor  1750
//! Nearest x Nearest          1500      Greedy x Hybrid         1750
//! Nearest x Greedy           1750      Hybrid x Hybrid         2250
//! ```
//!
//! on 500 distinct generated default-config maps (shared across matchups),
//! `Flat(0)` virtual buckets, 12 threads, all seeds derived from one
//! master seed. Artifacts land in `runs/m6-r1/`. A second, explicit
//! cross-bucket batch (120 Hybrid x Hybrid matches under
//! `Factions { enl: 0, res: 1 }`) lands in `runs/m6-r1-crossbucket/` —
//! end-to-end evidence that bucket order dominates parity (D19/D20/D56).

#![forbid(unsafe_code)]

use std::time::Instant;

use arena_bots::bot::BotKind;
use arena_core::config::GameConfig;
use arena_runner::batch::{BatchConfig, Matchup};
use arena_runner::policy::BucketPolicy;
use arena_runner::report::{BatchSummary, summary_text, write_artifacts};
use arena_runner::run::{MAX_WORKERS, run_batch};

/// The frozen master seed of experiment round 1 (D58).
pub const M6_R1_MASTER_SEED: u64 = 0x006D_3600_0000_0031;

/// The frozen matchup table (all counts even; each bot gets exactly M/2
/// ENL and M/2 RES slots, D55).
pub fn frozen_matchups() -> Vec<Matchup> {
    vec![
        Matchup::new("Random-Random", BotKind::Random, BotKind::Random, 1000),
        Matchup::new(
            "Nearest-Nearest",
            BotKind::NearestPortal,
            BotKind::NearestPortal,
            1500,
        ),
        Matchup::new(
            "Nearest-Greedy",
            BotKind::NearestPortal,
            BotKind::GreedyField,
            1750,
        ),
        Matchup::new(
            "Greedy-DestroyAnchor",
            BotKind::GreedyField,
            BotKind::DestroyAnchor,
            1750,
        ),
        Matchup::new("Greedy-Hybrid", BotKind::GreedyField, BotKind::Hybrid, 1750),
        Matchup::new("Hybrid-Hybrid", BotKind::Hybrid, BotKind::Hybrid, 2250),
    ]
}

fn main() {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_WORKERS);
    let args: Vec<String> = std::env::args().collect();
    let output_root = std::path::PathBuf::from(args.get(1).map(|s| s.as_str()).unwrap_or("runs"));

    // ---------------------------------------------------------------
    // The frozen 10,000-match balance batch (Flat(0)).
    // ---------------------------------------------------------------
    let balance = BatchConfig {
        batch_id: "m6-r1".to_string(),
        matchups: frozen_matchups(),
        map_count: 500,
        master_seed: M6_R1_MASTER_SEED,
        bucket_policy: BucketPolicy::Flat(0),
        config: GameConfig::default(),
    };
    println!(
        "arena-runner: {} matches on {} maps, {workers} threads, bucket policy {}",
        balance.total_matches(),
        balance.map_count,
        balance.bucket_policy.tag()
    );
    let start = Instant::now();
    let run = run_batch(&balance, workers).expect("the frozen batch runs");
    println!(
        "arena-runner: batch m6-r1 done in {:?} ({} matches, {} crashes)",
        start.elapsed(),
        run.records.len(),
        run.records
            .iter()
            .filter(|r| r.result == arena_runner::metrics::Outcome::Crash)
            .count()
    );
    let dir = output_root.join("m6-r1");
    write_artifacts(&dir, &balance, &run.records, run.workers, run.elapsed_ms)
        .expect("write m6-r1 artifacts");
    let summary = BatchSummary::from_records(&run.records, &balance.matchups);
    println!(
        "arena-runner: artifacts in {} (manifest.txt, matches.tsv, summary.txt)",
        dir.display()
    );
    println!();
    print!(
        "{}",
        summary_text(&summary, balance.config.portal_count as u64)
    );

    // ---------------------------------------------------------------
    // The explicit cross-bucket batch (D56): bucket order dominates
    // parity — ENL always executes first. Still equal ENL/RES exposure
    // per bot through the schedule.
    // ---------------------------------------------------------------
    let cross = BatchConfig {
        batch_id: "m6-r1-crossbucket".to_string(),
        matchups: vec![Matchup::new(
            "Hybrid-Hybrid",
            BotKind::Hybrid,
            BotKind::Hybrid,
            120,
        )],
        map_count: 60,
        master_seed: M6_R1_MASTER_SEED ^ 0xC505_B0C0_0000_0000,
        bucket_policy: BucketPolicy::Factions { enl: 0, res: 1 },
        config: GameConfig::default(),
    };
    let start = Instant::now();
    let run = run_batch(&cross, workers).expect("the cross-bucket batch runs");
    println!(
        "arena-runner: batch m6-r1-crossbucket done in {:?} ({} matches)",
        start.elapsed(),
        run.records.len()
    );
    let dir = output_root.join("m6-r1-crossbucket");
    write_artifacts(&dir, &cross, &run.records, run.workers, run.elapsed_ms)
        .expect("write cross-bucket artifacts");
    let summary = BatchSummary::from_records(&run.records, &cross.matchups);
    print!(
        "{}",
        summary_text(&summary, cross.config.portal_count as u64)
    );
}

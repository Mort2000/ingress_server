//! The frozen B1 smoke experiment (plan section 14, row B1: "两配置×两策略
//! ×同图换边的小批次可复现"). Lives in the library (not main.rs) so the
//! completion-condition test runs the SAME definition the CLI runs;
//! `arena-runner balance-b1` is the operator entry.

use std::path::Path;

use arena_bots::policy::PolicyId;
use arena_core::config::GameConfig;

use crate::balance::archive::write_balance_artifacts;
use crate::balance::execute::run_balance_batch;
use crate::balance::pairs::{compute_paired_results, variant_summaries};
use crate::balance::plan::{ConfigVariant, ExperimentPlan, PolicyMatchup, SeedSet, Stage};
use crate::policy::BucketPolicy;
use crate::run::MAX_WORKERS;

/// The frozen experiment id (the archive directory name).
pub const B1_SMOKE_EXPERIMENT_ID: &str = "b1-smoke-r1";

/// The frozen maps of the real smoke run (first 8 Development maps).
pub const B1_SMOKE_MAP_COUNT: u32 = 8;

/// The frozen B1 smoke plan: stage Smoke on the Development seed set, two
/// config variants (the untouched default plus the plan section 7.3 grid's
/// 240 m `move_budget_cm` candidate — a variant that provably shares the
/// Map Definition across the contrast), the two competitive m6.0 policies
/// (GreedyField, Hybrid), one matchup, `Flat(0)` buckets. The default
/// `GameConfig` is NOT modified.
pub fn b1_smoke_plan(map_count: u32) -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: B1_SMOKE_EXPERIMENT_ID.to_string(),
        stage: Stage::Smoke,
        seed_set: SeedSet::Development,
        map_count,
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

/// Operator entry behind `arena-runner balance-b1 [output-root]
/// [base-commit]`: runs the frozen smoke plan and archives the full
/// artifact set into a NEW `<output_root>/balance/b1-smoke-r1/`
/// directory. Returns the process exit code.
pub fn run_b1_smoke(output_root: &Path, base_commit: Option<&str>) -> i32 {
    let plan = b1_smoke_plan(B1_SMOKE_MAP_COUNT);
    if let Err(error) = plan.validate() {
        eprintln!("balance-b1: {error}");
        return 1;
    }
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_WORKERS);
    println!(
        "balance-b1: {} variants x {} maps x 2 swap legs = {} matches, {workers} workers",
        plan.variants.len(),
        plan.map_count,
        plan.variants.len() * plan.matchups.len() * plan.map_count as usize * 2
    );
    let run = match run_balance_batch(&plan, workers) {
        Ok(run) => run,
        Err(error) => {
            eprintln!("balance-b1: {error}");
            return 1;
        }
    };
    println!(
        "balance-b1: {} matches in {:?} ({} crashes, {} config failures)",
        run.records.len(),
        std::time::Duration::from_millis(run.elapsed_ms as u64),
        run.crashes(),
        run.config_failures
    );
    let pairs = match compute_paired_results(&plan, &run.records) {
        Ok(pairs) => pairs,
        Err(error) => {
            eprintln!("balance-b1: pairing failed: {error}");
            return 1;
        }
    };
    let summaries = variant_summaries(&plan, &pairs);
    let dir =
        match write_balance_artifacts(output_root, &plan, &run, &pairs, &summaries, base_commit) {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("balance-b1: {error}");
                return 1;
            }
        };
    println!("balance-b1: artifacts in {}", dir.display());
    print!(
        "{}",
        std::fs::read_to_string(dir.join("summary.txt")).expect("summary written")
    );
    for line in std::fs::read_to_string(dir.join("hashes.txt"))
        .expect("hashes written")
        .lines()
    {
        println!("balance-b1: {line}");
    }
    0
}

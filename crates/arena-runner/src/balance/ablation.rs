//! The B3 default-config ablation experiments (plan sections 7.1/7.2,
//! 8.2/8.3, 16; ADR-0017): the frozen same-map faction-swap matrix over
//! the eight ablation-relevant policies and the idle benchmark of the
//! three new arms, plus the operator entry that runs, archives and
//! appends the plan-section-9 `analysis.txt` (hash folded into
//! `hashes.txt`). NO rule parameter is touched — `Stage::Ablation`
//! refuses any config diff (belt-and-braces in `plan.rs::validate`).

use arena_bots::policy::{PolicyId, PolicySpec};
use arena_core::config::GameConfig;

use crate::balance::analysis::analysis_text;
use crate::balance::archive::write_balance_artifacts;
use crate::balance::execute::{BalanceRunResult, run_balance_batch_with};
use crate::balance::pairs::{compute_paired_results, variant_summaries};
use crate::balance::plan::{ConfigVariant, ExperimentPlan, PolicyMatchup, SeedSet, Stage};
use crate::balance::schedule::MatchCell;
use crate::policy::BucketPolicy;
use crate::run::MAX_WORKERS;

/// The frozen experiment ids (archive directory names; ADR-0017). `-r1`
/// was superseded by a rendering defect in its archived `analysis.txt`
/// (ok and dropped counts printed without a separator); `-r2` was
/// superseded by a mechanics-pooling defect (mirror records contributed
/// only ONE of the policy's two faction slots). Raw per-match records
/// were correct in both; both stay on disk as history, never
/// overwritten. `-r3` is the current official run of the same frozen
/// plans (seeds derive without the experiment id, so every re-issue
/// reproduces the same trajectories exactly). Pilot runs append
/// `-pilot-r0` and are exploratory only.
pub const B3_MATRIX_EXPERIMENT_ID: &str = "b3-ablation-matrix-r3";
pub const B3_IDLE_EXPERIMENT_ID: &str = "b3-ablation-idle-r3";

/// The official map counts (frozen before the official run; plan section
/// 8.3 never reduces swap legs — every unit runs both legs).
pub const B3_MATRIX_MAP_COUNT: u32 = 100;
pub const B3_IDLE_MAP_COUNT: u32 = 50;

/// The pilot map counts (shrunk only; pairings, stage and variants
/// unchanged).
pub const B3_PILOT_MATRIX_MAP_COUNT: u32 = 8;
pub const B3_PILOT_IDLE_MAP_COUNT: u32 = 4;

/// Mirror cells on the first N maps capture BOTH legs' replays (the
/// mirror statistics read spawn sets from replay headers; ADR-0017).
pub const B3_MIRROR_REPLAY_MAPS: u32 = 25;

/// The frozen eight-policy matrix roster (ADR-0017): the two m6.0
/// baselines involved in the domination, the r2 coordinated baseline, the
/// three B3 arms, and the two r2 policies carrying the plan-7.2 role row
/// and the B2-flagged dominant (`builder-disruptor-r2`).
pub const B3_MATRIX_POLICIES: [PolicyId; 8] = [
    PolicyId::GreedyField,
    PolicyId::Hybrid,
    PolicyId::GreedyCoordinatedR2,
    PolicyId::HybridBuilderOnly,
    PolicyId::HybridBuilderAttack,
    PolicyId::GreedyCoordinatedCanonical,
    PolicyId::AdaptiveHybridR2,
    PolicyId::BuilderDisruptorR2,
];

/// The matrix policy specs in roster order (mirrored m6.0 specs for the
/// baselines, r2 specs for the r2 policies, ablation specs for the arms).
pub fn b3_matrix_specs() -> Vec<PolicySpec> {
    B3_MATRIX_POLICIES
        .iter()
        .map(|&id| match id {
            id if id.bot_kind().is_some() => id.frozen_m6_spec(),
            PolicyId::HybridBuilderOnly
            | PolicyId::HybridBuilderAttack
            | PolicyId::GreedyCoordinatedCanonical => id.ablation_spec(),
            _ => id.r2_spec(),
        })
        .collect()
}

/// The round-robin matchup list of `ids`: every distinct pair
/// (declaration order) plus every mirror — the readiness layer's
/// `r2_round_robin_matchups` generalized.
pub fn round_robin_matchups(ids: &[PolicyId]) -> Vec<PolicyMatchup> {
    let mut matchups = Vec::new();
    for (index, &a) in ids.iter().enumerate() {
        for &b in ids.iter().skip(index + 1) {
            matchups.push(PolicyMatchup::new(
                &format!("{}--{}", a.tag(), b.tag()),
                a,
                b,
            ));
        }
    }
    for &a in ids {
        matchups.push(PolicyMatchup::new(
            &format!("{}--{}", a.tag(), a.tag()),
            a,
            a,
        ));
    }
    matchups
}

/// The frozen ablation matrix plan: the eight-policy round-robin (28
/// distinct pairs + 8 mirrors = 36 matchups) over `map_count` Development
/// maps, `Flat(0)`, default config only, stage `ablation`.
pub fn b3_matrix_plan(map_count: u32, experiment_id: &str) -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Ablation,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: vec![ConfigVariant::new(
            "default",
            GameConfig::default(),
            Vec::new(),
        )],
        policies: b3_matrix_specs(),
        matchups: round_robin_matchups(&B3_MATRIX_POLICIES),
        bucket_policy: BucketPolicy::Flat(0),
        gates: vec![
            crate::balance::plan::Gate::Pending {
                gate_id: "records-integrity".to_string(),
                statement:
                    "every match record's policy family/version/parameters match policies.tsv"
                        .to_string(),
            },
            crate::balance::plan::Gate::Pending {
                gate_id: "no-crashes".to_string(),
                statement: "zero crash and zero config-failure records in the matrix".to_string(),
            },
        ],
    }
}

/// The frozen idle benchmark plan: the three B3 arms vs the diagnostic
/// `idle` opponent (the plan-6.3 unopposed-scoring check).
pub fn b3_idle_plan(map_count: u32, experiment_id: &str) -> ExperimentPlan {
    let arms = [
        PolicyId::HybridBuilderOnly,
        PolicyId::HybridBuilderAttack,
        PolicyId::GreedyCoordinatedCanonical,
    ];
    ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Ablation,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: vec![ConfigVariant::new(
            "default",
            GameConfig::default(),
            Vec::new(),
        )],
        policies: arms
            .iter()
            .map(|&id| id.ablation_spec())
            .chain([PolicyId::Idle.r2_spec()])
            .collect(),
        matchups: arms
            .iter()
            .map(|&id| PolicyMatchup::new(&format!("{}--idle", id.tag()), id, PolicyId::Idle))
            .collect(),
        bucket_policy: BucketPolicy::Flat(0),
        gates: vec![crate::balance::plan::Gate::Pending {
            gate_id: "idle-scoring".to_string(),
            statement:
                "every B3 arm: score2>0 AND fields_created>=1 in 100% of non-crash idle matches"
                    .to_string(),
        }],
    }
}

/// The matrix replay sampling (ADR-0017): every leg-0 cell plus BOTH legs
/// of mirror cells on the first [`B3_MIRROR_REPLAY_MAPS`] maps.
pub fn b3_replay_sample(cell: &MatchCell) -> bool {
    cell.leg == 0 || (cell.policy_a == cell.policy_b && cell.map_index < B3_MIRROR_REPLAY_MAPS)
}

/// Runs ONE B3 experiment end to end: executes the batch, computes the
/// paired integer results, archives the complete section-12 artifact set
/// into a NEW `runs/balance/<experiment-id>/` directory, renders the
/// plan-section-9 `analysis.txt` and folds its content hash into
/// `hashes.txt`. Returns the archive directory plus the run's crash and
/// config-failure counts (the exit-status inputs).
pub fn run_b3_plan(
    output_root: &std::path::Path,
    plan: &ExperimentPlan,
    replay_sample: &(dyn Fn(&MatchCell) -> bool + Sync),
    base_commit: Option<&str>,
) -> Result<(std::path::PathBuf, u64, u64), String> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_WORKERS);
    println!(
        "balance-b3: {} — {} matchups x {} maps x 2 legs = {} matches, {workers} workers",
        plan.experiment_id,
        plan.matchups.len(),
        plan.map_count,
        plan.matchups.len() * plan.map_count as usize * 2
    );
    let run: BalanceRunResult = run_balance_batch_with(
        plan,
        workers,
        &crate::balance::execute::default_cell_setup,
        replay_sample,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "balance-b3: {} done in {:?} ({} crashes, {} config failures, {} replays)",
        plan.experiment_id,
        std::time::Duration::from_millis(run.elapsed_ms as u64),
        run.crashes(),
        run.config_failures,
        run.replays.len()
    );
    let pairs = compute_paired_results(plan, &run.records).map_err(|e| e.to_string())?;
    let summaries = variant_summaries(plan, &pairs);
    let dir = write_balance_artifacts(output_root, plan, &run, &pairs, &summaries, base_commit)
        .map_err(|e| e.to_string())?;
    let text = analysis_text(plan, &run.records, &pairs, &run.replays);
    std::fs::write(dir.join("analysis.txt"), &text).map_err(|e| e.to_string())?;
    // Fold the analysis hash into hashes.txt (every artifact content-hashed).
    let hashes_path = dir.join("hashes.txt");
    let mut hashes = std::fs::read_to_string(&hashes_path).map_err(|e| e.to_string())?;
    hashes.push_str(&format!(
        "blake3:{}  analysis.txt\n",
        arena_replay::canon::hex(blake3::hash(text.as_bytes()).as_bytes())
    ));
    std::fs::write(&hashes_path, &hashes).map_err(|e| e.to_string())?;
    Ok((dir, run.crashes(), run.config_failures))
}

/// `arena-runner balance-b3 <output-root> [base-commit] [--pilot]`: runs
/// the two frozen B3 experiments (matrix + idle) into NEW
/// `runs/balance/<experiment-id>/` directories (pilots append
/// `-pilot-r0`) and prints the run summaries. Returns the process exit
/// code (0 iff zero crashes and zero config failures anywhere — any
/// failure keeps its records but fails the run, never silently skipped).
pub fn run_b3_ablation(
    output_root: &std::path::Path,
    base_commit: Option<&str>,
    pilot: bool,
) -> i32 {
    let suffix = if pilot { "-pilot-r0" } else { "" };
    let (matrix_maps, idle_maps) = if pilot {
        (B3_PILOT_MATRIX_MAP_COUNT, B3_PILOT_IDLE_MAP_COUNT)
    } else {
        (B3_MATRIX_MAP_COUNT, B3_IDLE_MAP_COUNT)
    };
    let matrix = b3_matrix_plan(matrix_maps, &format!("{B3_MATRIX_EXPERIMENT_ID}{suffix}"));
    let idle = b3_idle_plan(idle_maps, &format!("{B3_IDLE_EXPERIMENT_ID}{suffix}"));
    for plan in [&matrix, &idle] {
        if let Err(error) = plan.validate() {
            eprintln!("balance-b3: {error}");
            return 1;
        }
    }
    let experiments: [(&ExperimentPlan, &(dyn Fn(&MatchCell) -> bool + Sync)); 2] =
        [(&matrix, &b3_replay_sample), (&idle, &|_cell| false)];
    let mut clean = true;
    for (plan, sample) in experiments {
        match run_b3_plan(output_root, plan, sample, base_commit) {
            Ok((dir, crashes, config_failures)) => {
                println!("balance-b3: archived {}", dir.display());
                if crashes > 0 || config_failures > 0 {
                    eprintln!(
                        "balance-b3: {} recorded {crashes} crashes and {config_failures} config failures — inspect {}",
                        plan.experiment_id,
                        dir.display()
                    );
                    clean = false;
                }
            }
            Err(error) => {
                eprintln!("balance-b3: {error}");
                return 1;
            }
        }
    }
    i32::from(!clean)
}

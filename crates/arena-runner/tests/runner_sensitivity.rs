//! B4 sensitivity integration tests (`docs/ai-balance-plan.md` sections
//! 7.3/8.3, ADR-0018): map-regime config variants in the executor — a
//! variant declaring a map-defining field generates its OWN MapDefinition
//! from the same map_seed (pairing keys unchanged), an infeasible
//! variant map becomes that variant's config-failure cells (never a
//! skip, never a batch abort), and non-map-regime variants keep the
//! ADR-0014 byte-shared base map.

use std::collections::BTreeMap;
use std::sync::Mutex;

use arena_bots::policy::PolicyId;
use arena_core::config::GameConfig;

use arena_runner::balance::archive::encode_map;
use arena_runner::balance::execute::{CellSetup, default_cell_setup, run_balance_batch_with};
use arena_runner::balance::plan::{ConfigVariant, ExperimentPlan, PolicyMatchup, SeedSet, Stage};
use arena_runner::balance::record::BalanceOutcome;
use arena_runner::balance::schedule::MatchCell;
use arena_runner::metrics::Outcome;
use arena_runner::policy::BucketPolicy;

/// A tiny B4-shaped plan: default base + a map-regime candidate + a
/// rule-only control, over one competitive R2 matchup.
fn three_variant_plan(map_count: u32, regime_variant: ConfigVariant) -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: "b4-exec-test".to_string(),
        stage: Stage::Sensitivity,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: vec![
            ConfigVariant::new("default", GameConfig::default(), Vec::new()),
            regime_variant,
            {
                let mut cfg = GameConfig::default();
                cfg.move_budget_cm = 24_000;
                ConfigVariant::new("move-budget-240m", cfg, vec!["move_budget_cm"])
            },
        ],
        policies: vec![
            PolicyId::GreedyCoordinatedR2.r2_spec(),
            PolicyId::BuilderDefenderR2.r2_spec(),
        ],
        matchups: vec![PolicyMatchup::new(
            "greedy-coordinated-r2--builder-defender-r2",
            PolicyId::GreedyCoordinatedR2,
            PolicyId::BuilderDefenderR2,
        )],
        bucket_policy: BucketPolicy::Flat(0),
        gates: Vec::new(),
    }
}

fn portal_count_80() -> ConfigVariant {
    let mut cfg = GameConfig::default();
    cfg.portal_count = 80;
    ConfigVariant::new("portal-count-80", cfg, vec!["portal_count"])
}

/// A settled (non-crash, non-config-failure) result.
fn settled(record: &arena_runner::balance::record::BalanceMatchRecord) -> bool {
    matches!(
        record.result,
        BalanceOutcome::Played(Outcome::EnlWin | Outcome::ResWin | Outcome::Draw)
    )
}

/// Runs the plan while capturing the MapDefinition each variant's cells
/// actually ran on (content hash, portal count), through the same setup
/// seam the default factory uses.
fn run_capturing_maps(
    plan: &ExperimentPlan,
    workers: usize,
) -> (
    arena_runner::balance::execute::BalanceRunResult,
    BTreeMap<String, (u64, usize)>,
) {
    let captured: Mutex<BTreeMap<String, (u64, usize)>> = Mutex::new(BTreeMap::new());
    let setup = |cell: &MatchCell,
                 enl: &arena_bots::policy::PolicySpec,
                 res: &arena_bots::policy::PolicySpec,
                 cfg: &GameConfig,
                 map: &arena_core::map::MapDefinition|
     -> CellSetup {
        let hash = {
            use std::hash::Hasher;
            let mut hasher = blake3::Hasher::new();
            hasher.update(&encode_map(map));
            let mut out = [0u8; 8];
            out.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
            u64::from_le_bytes(out)
        };
        captured
            .lock()
            .expect("capture lock")
            .insert(cell.variant_id.clone(), (hash, map.portals().len()));
        default_cell_setup(cell, enl, res, cfg, map)
    };
    let run = run_balance_batch_with(plan, workers, &setup, &|_| false).expect("runs");
    let maps = captured.into_inner().expect("capture lock");
    (run, maps)
}

// (a) A map-regime variant runs every cell settled on its OWN
// regenerated map (80 portals), while the rule-only control byte-shares
// the base map (same content, 120 portals).
#[test]
fn map_regime_variants_play_on_their_own_regenerated_maps() {
    let plan = three_variant_plan(2, portal_count_80());
    let (run, maps) = run_capturing_maps(&plan, 2);
    assert_eq!(
        run.records.len(),
        3 * 2 * 2,
        "3 variants x 2 maps x 2 legs, nothing dropped"
    );
    assert_eq!(run.crashes(), 0);
    assert_eq!(run.config_failures, 0, "portal_count=80 maps generate");
    assert!(run.records.iter().all(|r| settled(r)));

    let (base_hash, base_portals) = *maps.get("default").expect("default ran");
    let (control_hash, control_portals) = *maps.get("move-budget-240m").expect("control ran");
    let (regime_hash, regime_portals) = *maps.get("portal-count-80").expect("regime ran");
    assert_eq!(base_portals, 120);
    assert_eq!(control_portals, 120);
    assert_eq!(
        regime_portals, 80,
        "the map-regime variant runs on its own 80-portal map"
    );
    assert_eq!(
        base_hash, control_hash,
        "rule-only variants byte-share the base map (ADR-0014)"
    );
    assert_ne!(
        base_hash, regime_hash,
        "the map-regime map differs from the base map (same seed, own config)"
    );

    // Pairing keys unchanged (ADR-0018): same unit seeds and pair_id
    // across the contrast — the paired delta stays well defined.
    let base_row = run
        .records
        .iter()
        .find(|r| r.variant_id == "default" && r.map_index == 0)
        .expect("base row");
    let regime_row = run
        .records
        .iter()
        .find(|r| r.variant_id == "portal-count-80" && r.map_index == 0)
        .expect("regime row");
    assert_eq!(base_row.map_seed, regime_row.map_seed);
    assert_eq!(base_row.game_seed, regime_row.game_seed);
    assert_eq!(base_row.pair_id, regime_row.pair_id);
}

// (b) An infeasible map-regime variant: portal_min_distance 600 km on a
// 600 km map cannot host 120 pairwise-spaced portals — that variant's
// cells are recorded as config failures and the batch COMPLETES (base
// infeasibility would abort; a variant's never does).
#[test]
fn an_infeasible_map_regime_map_becomes_config_failure_cells() {
    let mut cfg = GameConfig::default();
    cfg.portal_min_distance_cm = 600_000;
    let infeasible = ConfigVariant::new("min-distance-6km", cfg, vec!["portal_min_distance_cm"]);
    let plan = three_variant_plan(2, infeasible);
    let (run, maps) = run_capturing_maps(&plan, 2);
    assert_eq!(run.records.len(), 12, "every cell stays in the result set");
    assert_eq!(run.config_failures, 4, "the variant's 2 maps x 2 legs");
    assert_eq!(run.crashes(), 0);
    for record in &run.records {
        if record.variant_id == "min-distance-6km" {
            assert_eq!(record.result, BalanceOutcome::ConfigFailure);
            assert_eq!(record.enl_score2, 0);
            assert_eq!(record.res_score2, 0);
        } else {
            assert!(settled(record), "{}", record.variant_id);
            assert_eq!(
                maps.get(&record.variant_id).unwrap_or(&(0, 0)).1,
                120,
                "{} byte-shares the base map",
                record.variant_id
            );
        }
    }
    assert!(
        maps.get("min-distance-6km").is_none(),
        "the infeasible variant never ran a cell"
    );
}

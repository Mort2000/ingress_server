//! Balance batch execution (B1, plan section 5.2): runs the plan's cells
//! through the SHARED m6 match path (`crate::run::run_match_recorded` — a
//! behavior-neutral recorder hook on the frozen `play` loop), on scoped
//! worker threads with a stride partition (worker count cannot move a
//! match), catching panics as `Crash` records that stay in the result set.
//! Variants that cannot honor the unit's shared Map Definition are
//! recorded as `config-failure` cells, never silently skipped (plan
//! section 7.3, ADR-0014 caveat).

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use arena_bots::policy::{PolicyId, PolicySpec};
use arena_core::config::GameConfig;
use arena_core::harness::RoundStrategy;
use arena_core::map::{MapDefinition, generate_map};
use arena_core::spawn::InitialSpawns;
use arena_core::types::Faction;

use crate::balance::plan::ExperimentPlan;
use crate::balance::record::{BalanceMatchRecord, BalanceOutcome};
use crate::balance::schedule::{MatchCell, expand};
use crate::batch::MatchDescriptor;
use crate::metrics::Outcome;
use crate::policy::PolicyStrategy;
use crate::run::MAX_WORKERS;

/// Batch-level failures that abort a balance run (never match outcomes).
#[derive(Debug, PartialEq, Eq)]
pub enum BalanceError {
    /// The plan itself is invalid (plan-build-time rules).
    PlanInvalid(String),
    /// A BASE-variant map seed does not generate: a config defect (no map
    /// skip rule, the m6 D58 convention).
    MapInfeasible { map_index: u32 },
}

impl fmt::Display for BalanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BalanceError::PlanInvalid(why) => write!(f, "invalid experiment plan: {why}"),
            BalanceError::MapInfeasible { map_index } => write!(
                f,
                "base-variant map {map_index} is infeasible (config defect)"
            ),
        }
    }
}

impl std::error::Error for BalanceError {}

/// One captured IARP replay (plan section 12: pre-sampled matches).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BalanceReplay {
    /// The cell's global index (`matches.tsv` row).
    pub cell_index: u64,
    pub variant_id: String,
    pub pair_id: String,
    pub leg: u8,
    pub map_id: String,
    pub bytes: Vec<u8>,
}

/// One completed balance run: records in cell order, captured replays,
/// and run metadata (wall clock — metadata only).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BalanceRunResult {
    pub records: Vec<BalanceMatchRecord>,
    pub replays: Vec<BalanceReplay>,
    /// Cells recorded as `config-failure` (count; the records carry the
    /// detail).
    pub config_failures: u64,
    pub elapsed_ms: u128,
    pub workers: usize,
}

impl BalanceRunResult {
    /// Non-time summary: played/crashed/failed counts.
    pub fn played(&self) -> u64 {
        self.records
            .iter()
            .filter(|r| {
                !matches!(
                    r.result,
                    BalanceOutcome::ConfigFailure | BalanceOutcome::Played(Outcome::Crash)
                )
            })
            .count() as u64
    }

    pub fn crashes(&self) -> u64 {
        self.records
            .iter()
            .filter(|r| r.result == BalanceOutcome::Played(Outcome::Crash))
            .count() as u64
    }
}

/// One cell's strategies and validated spawn sets (the seam the default
/// policy factory and the crash tests plug into).
pub struct CellSetup {
    pub enl_spawns: InitialSpawns,
    pub res_spawns: InitialSpawns,
    pub enl: Box<dyn RoundStrategy>,
    pub res: Box<dyn RoundStrategy>,
}

/// Builds a cell's setup from the plan's policy specs (the default
/// factory): one `make_policy_bot` per side, ENL spawns chosen first, the
/// bucket policy applied AFTER the bot returns (D56, m6 construction
/// order).
pub fn default_cell_setup(
    cell: &MatchCell,
    spec_enl: &PolicySpec,
    spec_res: &PolicySpec,
    cfg: &GameConfig,
    map: &MapDefinition,
) -> CellSetup {
    use arena_core::spawn::validate_initial_spawn_set;
    use arena_core::state::GameState;

    use arena_bots::bot::BotStrategy;
    use arena_bots::policy::make_policy_bot;

    let mut enl_bot = make_policy_bot(spec_enl, Faction::Enl, cell.bot_seed, cfg, map);
    let mut res_bot = make_policy_bot(spec_res, Faction::Res, cell.bot_seed, cfg, map);
    let (enl_roster, res_roster) = GameState::initial_roster(cfg);
    let enl_entries = enl_bot.choose_spawns(cfg, map, &enl_roster);
    let res_entries = res_bot.choose_spawns(cfg, map, &res_roster);
    let enl_spawns =
        validate_initial_spawn_set(&enl_roster, &enl_entries, cfg).expect("bot spawn set");
    let res_spawns =
        validate_initial_spawn_set(&res_roster, &res_entries, cfg).expect("bot spawn set");
    CellSetup {
        enl_spawns,
        res_spawns,
        enl: Box::new(PolicyStrategy::new(
            Box::new(BotStrategy(enl_bot)),
            Faction::Enl,
            cell.bucket_policy,
        )),
        res: Box::new(PolicyStrategy::new(
            Box::new(BotStrategy(res_bot)),
            Faction::Res,
            cell.bucket_policy,
        )),
    }
}

/// The default replay sampling (plan section 12): leg 0 of every pair.
/// Crashed cells are listed in the archive's replay index as unavailable —
/// a panic unwinds past the recorder, leaving no settled state to trailer.
pub fn sample_leg0(cell: &MatchCell) -> bool {
    cell.leg == 0
}

/// The plan's spec of `id` (validation guarantees presence).
fn spec_of(plan: &ExperimentPlan, id: PolicyId) -> PolicySpec {
    plan.policies
        .iter()
        .find(|s| s.policy_id == id)
        .expect("plan validation registers every matchup policy")
        .clone()
}

/// The m6-shaped descriptor of a balance cell (the m6 `MatchRecord`
/// machinery — fold, crash record — is reused, not forked).
fn cell_descriptor(cell: &MatchCell, spec_a: &PolicySpec, spec_b: &PolicySpec) -> MatchDescriptor {
    MatchDescriptor {
        index: cell.index,
        matchup_index: 0,
        matchup_tag: cell.pair_tag.clone(),
        // Unique inside the pair: `k * 2 + leg`.
        j: cell.map_index * 2 + cell.leg as u32,
        bot_a: cell.policy_a.bot_kind(),
        bot_b: cell.policy_b.bot_kind(),
        bot_a_is_enl: cell.policy_a_is_enl,
        game_seed: cell.game_seed,
        map_seed: cell.map_seed,
        map_index: cell.map_index,
        bot_seed: cell.bot_seed,
        schedule_seed: cell.schedule_seed,
        bucket_policy: cell.bucket_policy,
        config_hash: cell.config_hash,
        bot_a_version: spec_a.version,
        bot_b_version: spec_b.version,
    }
}

/// Runs the whole plan with the default policy factory and the leg-0
/// replay sampling on at most [`MAX_WORKERS`] scoped threads.
pub fn run_balance_batch(
    plan: &ExperimentPlan,
    workers: usize,
) -> Result<BalanceRunResult, BalanceError> {
    run_balance_batch_with(plan, workers, &default_cell_setup, &sample_leg0)
}

/// The full executor: expands the plan, generates the base-variant map
/// pool, verifies every other variant SHARES each map (else records
/// `config-failure` cells — ADR-0014 caveat), then runs every cell on a
/// stride-partitioned worker pool and merges results in cell order.
pub fn run_balance_batch_with(
    plan: &ExperimentPlan,
    workers: usize,
    setup: &(
         dyn Fn(&MatchCell, &PolicySpec, &PolicySpec, &GameConfig, &MapDefinition) -> CellSetup
             + Sync
     ),
    replay_sample: &(dyn Fn(&MatchCell) -> bool + Sync),
) -> Result<BalanceRunResult, BalanceError> {
    let start = Instant::now();
    plan.validate().map_err(BalanceError::PlanInvalid)?;
    let cells = expand(plan).map_err(BalanceError::PlanInvalid)?;
    let base = plan.base();

    // Base-variant map pool (shared across variants and matchups).
    let mut pool: Vec<Arc<MapDefinition>> = Vec::with_capacity(plan.map_count as usize);
    for k in 0..plan.map_count {
        let seed = crate::balance::plan::map_seed(plan.seed_set, k);
        match generate_map(seed, &base.config) {
            Ok(map) => pool.push(Arc::new(map)),
            Err(_) => return Err(BalanceError::MapInfeasible { map_index: k }),
        }
    }

    // Shared-map verification: every non-base variant must produce the
    // SAME MapDefinition from the same map seed, or its cells on that map
    // become config failures (recorded, not skipped).
    let mut shared_map_failures: BTreeSet<(String, u32)> = BTreeSet::new();
    for variant in &plan.variants {
        if variant.variant_id == plan.base_variant {
            continue;
        }
        for k in 0..plan.map_count {
            let seed = crate::balance::plan::map_seed(plan.seed_set, k);
            let shares =
                generate_map(seed, &variant.config).is_ok_and(|map| map == *pool[k as usize]);
            if !shares {
                shared_map_failures.insert((variant.variant_id.clone(), k));
            }
        }
    }

    let workers = workers.clamp(1, MAX_WORKERS).min(cells.len().max(1));
    let variant_cfg: Vec<(String, GameConfig)> = plan
        .variants
        .iter()
        .map(|v| (v.variant_id.clone(), v.config))
        .collect();

    // One output slot per cell; each worker owns a stride of cells.
    enum Slot {
        Played(BalanceMatchRecord, Option<BalanceReplay>),
        Failure(BalanceMatchRecord),
    }
    let mut slots: Vec<Option<Slot>> = cells.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for worker in 0..workers {
            let cells = &cells;
            let pool = &pool;
            let variant_cfg = &variant_cfg;
            let failures = &shared_map_failures;
            handles.push(scope.spawn(move || {
                let mut ours = Vec::new();
                let mut index = worker;
                while index < cells.len() {
                    let cell = &cells[index];
                    let spec_a = spec_of(plan, cell.policy_a);
                    let spec_b = spec_of(plan, cell.policy_b);
                    let cfg = &variant_cfg
                        .iter()
                        .find(|(id, _)| *id == cell.variant_id)
                        .expect("variant exists")
                        .1;
                    let slot = if failures.contains(&(cell.variant_id.clone(), cell.map_index)) {
                        // Config failure: identity echo, empty stats,
                        // stays in the result set.
                        Slot::Failure(BalanceMatchRecord::config_failure(cell, &spec_a, &spec_b))
                    } else {
                        let (enl_spec, res_spec) = if cell.policy_a_is_enl {
                            (spec_a.clone(), spec_b.clone())
                        } else {
                            (spec_b.clone(), spec_a.clone())
                        };
                        let cell_setup = setup(
                            cell,
                            &enl_spec,
                            &res_spec,
                            cfg,
                            &pool[cell.map_index as usize],
                        );
                        let descriptor = cell_descriptor(cell, &spec_a, &spec_b);
                        let played = crate::run::run_match_recorded(
                            &descriptor,
                            cfg,
                            &pool[cell.map_index as usize],
                            &cell_setup.enl_spawns,
                            &cell_setup.res_spawns,
                            cell_setup.enl,
                            cell_setup.res,
                            replay_sample(cell),
                        );
                        let replay = played.replay.map(|bytes| BalanceReplay {
                            cell_index: cell.index,
                            variant_id: cell.variant_id.clone(),
                            pair_id: cell.pair_id.clone(),
                            leg: cell.leg,
                            map_id: cell.map_id.clone(),
                            bytes,
                        });
                        Slot::Played(
                            BalanceMatchRecord::from_match_record(
                                cell,
                                &spec_a,
                                &spec_b,
                                &played.record,
                            ),
                            replay,
                        )
                    };
                    ours.push((index, slot));
                    index += workers;
                }
                ours
            }));
        }
        for handle in handles {
            for (slot_index, slot) in handle.join().expect("balance worker panicked") {
                slots[slot_index] = Some(slot);
            }
        }
    });

    let mut records = Vec::with_capacity(cells.len());
    let mut replays = Vec::new();
    let mut config_failures = 0u64;
    for slot in slots {
        match slot.expect("every cell ran exactly once") {
            Slot::Played(record, replay) => {
                if let Some(replay) = replay {
                    replays.push(replay);
                }
                records.push(record);
            }
            Slot::Failure(record) => {
                config_failures += 1;
                records.push(record);
            }
        }
    }
    Ok(BalanceRunResult {
        records,
        replays,
        config_failures,
        elapsed_ms: start.elapsed().as_millis(),
        workers,
    })
}

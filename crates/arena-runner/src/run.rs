//! Match and batch execution (D57): each descriptor maps to one
//! `MatchRecord`. Matches run on scoped worker threads (stride partition
//! over the descriptor indices, `available_parallelism().min(12)` like the
//! M5 gate); a panicking bot or harness is caught with `catch_unwind`,
//! recorded as `Outcome::Crash`, and stays in the result set. Reports are
//! never retained — each round folds into the stats and is dropped.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Instant;

use arena_bots::bot::{BotStrategy, make_bot};
use arena_core::config::GameConfig;
use arena_core::harness::MatchHarness;
use arena_core::map::MapDefinition;
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::Faction;

use crate::batch::{BatchConfig, MatchDescriptor};
use crate::metrics::{MatchFold, MatchRecord};
use crate::policy::PolicyStrategy;

/// Maximum worker threads (12-core machine, M5 gate convention).
pub const MAX_WORKERS: usize = 12;

/// Batch-level failures that abort the run (never match outcomes).
#[derive(Debug, PartialEq, Eq)]
pub enum BatchError {
    /// A generated map was infeasible: a config defect (no map skip rule,
    /// D58).
    MapInfeasible { map_index: u32 },
}

/// One completed batch: records in descriptor order, the aggregate
/// summary, and run metadata (wall clock — metadata only).
pub struct BatchRun {
    pub records: Vec<MatchRecord>,
    pub elapsed_ms: u128,
    pub workers: usize,
}

/// Executes one match described by `descriptor` on the pool map. The whole
/// match (bot construction, spawn validation, all rounds) runs under
/// `catch_unwind`: any panic becomes `Outcome::Crash`.
pub fn run_match(
    descriptor: &MatchDescriptor,
    cfg: &GameConfig,
    map: &MapDefinition,
) -> MatchRecord {
    let start = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let (enl_kind, res_kind) = descriptor.faction_kinds();

        // One bot instance per faction: the SAME instance chooses spawns
        // and then plays (D53 — spawn draws precede round draws on its
        // stream). A rejected spawn set is a bot bug and panics here ->
        // Crash record.
        let mut enl_bot = make_bot(enl_kind, Faction::Enl, descriptor.bot_seed, cfg, map);
        let mut res_bot = make_bot(res_kind, Faction::Res, descriptor.bot_seed, cfg, map);
        let (enl_roster, res_roster) = GameState::initial_roster(cfg);
        let enl_entries = enl_bot.choose_spawns(cfg, map, &enl_roster);
        let res_entries = res_bot.choose_spawns(cfg, map, &res_roster);
        let enl_spawns =
            validate_initial_spawn_set(&enl_roster, &enl_entries, cfg).expect("bot spawn set");
        let res_spawns =
            validate_initial_spawn_set(&res_roster, &res_entries, cfg).expect("bot spawn set");

        // Bucket policy wraps each faction AFTER the bot returns (D56).
        let enl = PolicyStrategy::new(
            Box::new(BotStrategy(enl_bot)),
            Faction::Enl,
            descriptor.bucket_policy,
        );
        let res = PolicyStrategy::new(
            Box::new(BotStrategy(res_bot)),
            Faction::Res,
            descriptor.bucket_policy,
        );
        play(
            descriptor,
            cfg,
            map,
            &enl_spawns,
            &res_spawns,
            Box::new(enl),
            Box::new(res),
        )
    }));
    match outcome {
        Ok(record) => record,
        Err(_panic) => {
            // The default hook already printed the panic; the record keeps
            // the failure in the result set.
            MatchRecord::crashed(descriptor, start.elapsed().as_millis() as u64)
        }
    }
}

/// Same crash contract, but with caller-supplied strategies and spawn sets
/// (the test seam for the crash path and recorded/replayed matches).
pub fn run_match_with(
    descriptor: &MatchDescriptor,
    cfg: &GameConfig,
    map: &MapDefinition,
    enl_spawns: &arena_core::spawn::InitialSpawns,
    res_spawns: &arena_core::spawn::InitialSpawns,
    enl: Box<dyn arena_core::harness::RoundStrategy>,
    res: Box<dyn arena_core::harness::RoundStrategy>,
) -> MatchRecord {
    let start = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        play(descriptor, cfg, map, enl_spawns, res_spawns, enl, res)
    }));
    match outcome {
        Ok(record) => record,
        Err(_panic) => MatchRecord::crashed(descriptor, start.elapsed().as_millis() as u64),
    }
}

/// The shared match driver: runs every round, folding stats incrementally
/// (reports are never retained).
fn play(
    descriptor: &MatchDescriptor,
    cfg: &GameConfig,
    map: &MapDefinition,
    enl_spawns: &arena_core::spawn::InitialSpawns,
    res_spawns: &arena_core::spawn::InitialSpawns,
    enl: Box<dyn arena_core::harness::RoundStrategy>,
    res: Box<dyn arena_core::harness::RoundStrategy>,
) -> MatchRecord {
    let start = Instant::now();
    let mut harness = MatchHarness::new(
        descriptor.game_seed,
        *cfg,
        map,
        enl_spawns,
        res_spawns,
        enl,
        res,
    )
    .expect("match builds from validated inputs");
    let mut fold = MatchFold::new();
    let mut events_written = 0usize;
    for _ in 0..cfg.total_rounds {
        let report = harness.run_round();
        let state = harness.state();
        let new_events = &state.events()[events_written..];
        fold.round(state.round() - 1, &report, new_events, map, cfg);
        events_written = state.events().len();
    }
    let state = harness.state();
    fold.finish(state, cfg);
    MatchRecord::from_match(
        descriptor,
        state,
        fold.stats,
        start.elapsed().as_millis() as u64,
    )
}

/// Runs the whole batch on `workers` scoped threads (stride partition over
/// the global match indices; worker count cannot influence any match —
/// pinned by the reproducibility tests). The map pool is generated once
/// and shared; an infeasible map aborts the batch.
pub fn run_batch(config: &BatchConfig, workers: usize) -> Result<BatchRun, BatchError> {
    let start = Instant::now();
    let descriptors = config.descriptors();
    let pool = match config.map_pool() {
        Ok(pool) => pool,
        Err(_) => {
            // Surface the first infeasible map index deterministically.
            for k in 0..config.map_count {
                if arena_core::map::generate_map(config.map_seed(k), &config.config).is_err() {
                    return Err(BatchError::MapInfeasible { map_index: k });
                }
            }
            return Err(BatchError::MapInfeasible {
                map_index: u32::MAX,
            });
        }
    };
    let pool: Vec<Arc<MapDefinition>> = pool.into_iter().map(Arc::new).collect();
    let workers = workers.clamp(1, descriptors.len().max(1));

    let mut slots: Vec<Option<MatchRecord>> = descriptors.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for worker in 0..workers {
            let descriptors = &descriptors;
            let pool = &pool;
            let cfg = &config.config;
            handles.push(scope.spawn(move || {
                // Stride partition: worker w owns indices w, w+workers, ...
                let mut ours = Vec::new();
                let mut index = worker;
                while index < descriptors.len() {
                    let descriptor = &descriptors[index];
                    ours.push(run_match(
                        descriptor,
                        cfg,
                        &pool[descriptor.map_index as usize],
                    ));
                    index += workers;
                }
                ours
            }));
        }
        for (worker, handle) in handles.into_iter().enumerate() {
            let records = handle.join().expect("batch worker panicked");
            let mut index = worker;
            for record in records {
                slots[index] = Some(record);
                index += workers;
            }
        }
    });

    let records: Vec<MatchRecord> = slots
        .into_iter()
        .map(|slot| slot.expect("every descriptor ran exactly once"))
        .collect();
    Ok(BatchRun {
        records,
        elapsed_ms: start.elapsed().as_millis(),
        workers,
    })
}

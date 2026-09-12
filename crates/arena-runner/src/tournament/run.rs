//! Tournament execution drivers: expand the frozen schedule once, then
//! run every item on `concurrency` scoped worker threads (dynamic
//! task claiming; execution order
//! cannot influence records because every result lands in its indexed
//! slot). Writes per-match artifacts under
//! `<root>/matches/m<index>/` and hand the rest of the archive to
//! `archive.rs`.

use std::path::Path;
use std::time::Instant;

use crate::tournament::config::TournamentConfig;
use crate::tournament::elo::AppliedRating;
use crate::tournament::exec_external::run_item_external;
use crate::tournament::outcome::TournamentOutcome;
use crate::tournament::schedule::{TournamentError, TournamentItem, build_items};

/// Everything one completed tournament contributes (items in index
/// order; results in the matching slots).
pub struct TournamentRun {
    pub items: Vec<TournamentItem>,
    pub outcomes: Vec<TournamentOutcome>,
    /// METADATA ONLY (per match).
    pub elapsed_ms: Vec<u64>,
    /// Replay path relative to the match dir, per match.
    pub replay_rel: Vec<Option<String>>,
    /// Per-match budget kills (metadata for the report).
    pub budget_exceeded: Vec<bool>,
    /// Per match: whether the served factions provably matched the
    /// schedule (external: the ENL client's connection was observed
    /// first, D61 accept order; verified from accepted connections).
    pub connect_order_verified: Vec<bool>,
    /// Scored matches in schedule order (the Elo replay).
    pub ratings: Vec<AppliedRating>,
    /// Total wall clock — METADATA ONLY.
    pub total_elapsed_ms: u128,
    pub workers: usize,
}

fn slots<T>(count: usize) -> Vec<Option<T>> {
    (0..count).map(|_| None).collect()
}

/// Runs every EXTERNAL item under `root` (the tournament archive dir).
pub fn run_external_tournament(
    cfg: &TournamentConfig,
    root: &Path,
) -> Result<TournamentRun, TournamentError> {
    let start = Instant::now();
    let items = build_items(cfg)?;
    let matches_dir = root.join("matches");
    std::fs::create_dir_all(&matches_dir)
        .map_err(|error| TournamentError::Invalid(format!("matches dir: {error}")))?;
    let workers = cfg.concurrency.min(items.len().max(1));
    let mut outcome_slots = slots(items.len());
    let mut elapsed_slots = slots(items.len());
    let mut replay_slots = slots(items.len());
    let mut budget_slots = slots(items.len());
    let mut connect_slots = slots(items.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            // The closures move REFERENCES (the M6 run_batch pattern).
            let items = &items;
            let matches_dir = &matches_dir;
            let next = &next;
            handles.push(scope.spawn(move || {
                let mut ours = Vec::new();
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if index >= items.len() {
                        break;
                    }
                    let item = &items[index];
                    let match_dir = matches_dir.join(format!("m{}", item.index));
                    let result = run_item_external(item, cfg, &match_dir);
                    ours.push((
                        index,
                        result.outcome,
                        result.elapsed_ms,
                        result.replay_rel,
                        result.budget_exceeded,
                        result.connect_order_verified,
                        result.sandbox_reports,
                    ));
                }
                ours
            }));
        }
        for handle in handles {
            let records = handle.join().expect("tournament worker panicked");
            for (index, outcome, elapsed, replay, budget, verified, sandbox) in records {
                outcome_slots[index] = Some(outcome);
                elapsed_slots[index] = Some(elapsed);
                replay_slots[index] = replay;
                budget_slots[index] = Some(budget);
                connect_slots[index] = Some(verified);
                let item = &items[index];
                let match_dir = matches_dir.join(format!("m{}", item.index));
                for (side, json) in sandbox {
                    let _ = std::fs::write(match_dir.join(format!("sandbox_{side}.json")), json);
                }
            }
        }
    });
    finish_run(
        cfg,
        items,
        outcome_slots,
        elapsed_slots,
        replay_slots,
        budget_slots,
        connect_slots,
        start.elapsed().as_millis(),
        workers,
    )
}

/// Fills the run struct and replays the ratings IN ITEM-INDEX ORDER from
/// the frozen config (never worker order or wall clock).
fn finish_run(
    cfg: &TournamentConfig,
    items: Vec<TournamentItem>,
    outcome_slots: Vec<Option<TournamentOutcome>>,
    elapsed_slots: Vec<Option<u64>>,
    replay_slots: Vec<Option<String>>,
    budget_slots: Vec<Option<bool>>,
    connect_slots: Vec<Option<bool>>,
    total_elapsed_ms: u128,
    workers: usize,
) -> Result<TournamentRun, TournamentError> {
    let outcomes: Vec<TournamentOutcome> = outcome_slots
        .into_iter()
        .zip(items.iter())
        .map(|(slot, _)| slot.expect("every item ran exactly once"))
        .collect();
    let elapsed_ms: Vec<u64> = elapsed_slots
        .into_iter()
        .map(|slot| slot.expect("every item has elapsed metadata"))
        .collect();
    let replay_rel: Vec<Option<String>> = replay_slots;
    let budget_exceeded: Vec<bool> = budget_slots
        .into_iter()
        .map(|slot| slot.expect("every item has a budget flag"))
        .collect();
    let connect_order_verified: Vec<bool> = connect_slots
        .into_iter()
        .map(|slot| slot.expect("every item has a connect-order flag"))
        .collect();
    // Elo replay: strictly in item-index order over Normal outcomes only.
    let mut table = crate::tournament::elo::EloTable::new(&cfg.elo, cfg.entrants.len());
    let mut ratings = Vec::new();
    for (item, outcome) in items.iter().zip(outcomes.iter()) {
        if let Some(applied) = table.apply(item, outcome) {
            ratings.push(applied);
        }
    }
    Ok(TournamentRun {
        items,
        outcomes,
        elapsed_ms,
        replay_rel,
        budget_exceeded,
        connect_order_verified,
        ratings,
        total_elapsed_ms,
        workers,
    })
}

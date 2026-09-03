//! In-process execution (D86): one item through the M6 engine wrapped in
//! the M5 recorder — the recorded-match pattern of the M6 gate, lifted so
//! every in-process tournament match ALSO archives an `replay.iarp`.
//! Faction roles come from the item's `a_is_enl`; the client seed feeds
//! both `BotRng` streams (D53 disjointness via faction/kind tags). A
//! panicking bot or harness is caught with `catch_unwind` and recorded as
//! `Unscored { stage: RunnerFault }` — an in-process crash never
//! masquerades as a score result.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use arena_bots::bot::{BotKind, BotStrategy, make_bot};
use arena_core::harness::MatchHarness;
use arena_core::map::MapDefinition;
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::{Faction, Score2};
use arena_replay::canon;
use arena_replay::recorder::ReplayWriter;

use crate::policy::PolicyStrategy;
use crate::tournament::config::TournamentConfig;
use crate::tournament::outcome::{TournamentOutcome, inprocess_crash, normal_inprocess};
use crate::tournament::schedule::TournamentItem;

/// One executed in-process item.
pub struct InprocessResult {
    pub outcome: TournamentOutcome,
    /// The finished IARP file (empty iff the match never reached a
    /// recorder — a panic or a spawn-set rejection, both `Unscored`).
    pub replay_bytes: Vec<u8>,
    /// METADATA ONLY.
    pub elapsed_ms: u64,
}

/// Runs one item in-process. `map` is the Benchmark Seed Set member
/// (`item.map_index`), pre-generated and shared.
pub fn run_item_inprocess(
    item: &TournamentItem,
    cfg: &TournamentConfig,
    map: &MapDefinition,
) -> InprocessResult {
    let start = Instant::now();
    let played = catch_unwind(AssertUnwindSafe(|| play(item, cfg, map)));
    let elapsed = start.elapsed().as_millis() as u64;
    match played {
        Ok(Ok(mut result)) => {
            result.elapsed_ms = elapsed;
            result
        }
        // A panic anywhere in the engine/bots is a runner-side fault in
        // tournament mode (never a score result, never a crash of the run).
        Ok(Err(reason)) => InprocessResult {
            outcome: inprocess_crash(&reason),
            replay_bytes: Vec::new(),
            elapsed_ms: elapsed,
        },
        Err(_) => InprocessResult {
            outcome: inprocess_crash("in-process match panicked"),
            replay_bytes: Vec::new(),
            elapsed_ms: elapsed,
        },
    }
}

/// The M6 run body plus the M5 recorder. `Err(reason)` is the caught
/// non-panic fault path (a rejected spawn set is a bot bug — recorded,
/// never silenced).
fn play(
    item: &TournamentItem,
    cfg: &TournamentConfig,
    map: &MapDefinition,
) -> Result<InprocessResult, String> {
    let (enl_id, res_id) = (item.enl_entrant(), item.res_entrant());
    let kind_of = |id: u32| -> Result<BotKind, String> {
        match &cfg.entrant(id).kind {
            crate::tournament::entrant::EntrantKind::InProcess { bot } => Ok(*bot),
            crate::tournament::entrant::EntrantKind::External { .. } => Err(format!(
                "entrant {id} is external; the in-process executor takes only InProcess entrants"
            )),
        }
    };
    let enl_kind: BotKind = kind_of(enl_id)?;
    let res_kind: BotKind = kind_of(res_id)?;

    // One bot instance per faction: the SAME instance chooses spawns and
    // then plays (D53/D54). Client seed feeds both streams.
    let bot_seed = item.client_seed;
    let mut enl_bot = make_bot(enl_kind, Faction::Enl, bot_seed, &cfg.config, map);
    let mut res_bot = make_bot(res_kind, Faction::Res, bot_seed, &cfg.config, map);
    let (enl_roster, res_roster) = GameState::initial_roster(&cfg.config);
    let enl_entries = enl_bot.choose_spawns(&cfg.config, map, &enl_roster);
    let res_entries = res_bot.choose_spawns(&cfg.config, map, &res_roster);
    let enl_spawns = validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg.config)
        .map_err(|error| format!("ENL spawn set rejected: {error:?}"))?;
    let res_spawns = validate_initial_spawn_set(&res_roster, &res_entries, &cfg.config)
        .map_err(|error| format!("RES spawn set rejected: {error:?}"))?;

    // Flat(0) buckets — the P2.5 balance default (arrival = collection
    // order; parity alternation D19/D20).
    let enl = PolicyStrategy::new(
        Box::new(BotStrategy(enl_bot)),
        Faction::Enl,
        crate::policy::BucketPolicy::Flat(0),
    );
    let res = PolicyStrategy::new(
        Box::new(BotStrategy(res_bot)),
        Faction::Res,
        crate::policy::BucketPolicy::Flat(0),
    );
    play_core(
        item,
        cfg,
        map,
        &enl_spawns,
        &res_spawns,
        Box::new(enl),
        Box::new(res),
    )
}

/// The same crash contract, but with caller-supplied strategies and spawn
/// sets (the M6 `run_match_with` seam: the crash-path test injects a
/// panicking strategy through the SAME public wrapper).
pub fn run_item_inprocess_with(
    item: &TournamentItem,
    cfg: &TournamentConfig,
    map: &MapDefinition,
    enl_spawns: &arena_core::spawn::InitialSpawns,
    res_spawns: &arena_core::spawn::InitialSpawns,
    enl: Box<dyn arena_core::harness::RoundStrategy>,
    res: Box<dyn arena_core::harness::RoundStrategy>,
) -> InprocessResult {
    let start = Instant::now();
    let played = catch_unwind(AssertUnwindSafe(|| {
        play_core(item, cfg, map, enl_spawns, res_spawns, enl, res)
    }));
    let elapsed = start.elapsed().as_millis() as u64;
    match played {
        Ok(Ok(mut result)) => {
            result.elapsed_ms = elapsed;
            result
        }
        Ok(Err(reason)) => InprocessResult {
            outcome: inprocess_crash(&reason),
            replay_bytes: Vec::new(),
            elapsed_ms: elapsed,
        },
        Err(_) => InprocessResult {
            outcome: inprocess_crash("in-process match panicked"),
            replay_bytes: Vec::new(),
            elapsed_ms: elapsed,
        },
    }
}

/// The recorded match loop shared by both entries.
fn play_core(
    item: &TournamentItem,
    cfg: &TournamentConfig,
    map: &MapDefinition,
    enl_spawns: &arena_core::spawn::InitialSpawns,
    res_spawns: &arena_core::spawn::InitialSpawns,
    enl: Box<dyn arena_core::harness::RoundStrategy>,
    res: Box<dyn arena_core::harness::RoundStrategy>,
) -> Result<InprocessResult, String> {
    let mut writer = ReplayWriter::new(item.game_seed, &cfg.config, map, enl_spawns, res_spawns);
    let mut harness = MatchHarness::new(
        item.game_seed,
        cfg.config.clone(),
        map,
        enl_spawns,
        res_spawns,
        enl,
        res,
    )
    .map_err(|error| format!("harness build failed: {error:?}"))?;
    for _ in 0..cfg.config.total_rounds {
        let report = harness.run_round();
        writer
            .record_round(&report, harness.state())
            .map_err(|error| format!("replay record failed: {error:?}"))?;
    }
    let state = harness.state();
    let replay_bytes = writer
        .finish(state)
        .map_err(|error| format!("replay finish failed: {error:?}"))?;
    let outcome = normal_inprocess(
        state.final_result(),
        state.score(Faction::Enl),
        state.score(Faction::Res),
        cfg.config.total_rounds,
        canon::state_hash(state),
    );
    Ok(InprocessResult {
        outcome,
        replay_bytes,
        elapsed_ms: 0, // filled by the caller (metadata)
    })
}

/// Convenience for callers that want the score pair of a `Normal`
/// outcome.
pub fn score_pair(outcome: &TournamentOutcome) -> Option<(Score2, Score2)> {
    match outcome {
        TournamentOutcome::Normal {
            score_enl,
            score_res,
            ..
        } => Some((*score_enl, *score_res)),
        _ => None,
    }
}

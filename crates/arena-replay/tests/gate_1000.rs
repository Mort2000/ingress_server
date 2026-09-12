//! M5 acceptance gate (D50): 1000 recorded matches — every one replays to
//! identical per-round and final hashes from the log alone, re-encodes
//! byte-exactly, and the whole batch fits the debug-build time budget on
//! scoped worker threads. Seeds are blake3-derived per match index; the
//! strategy mix rotates FieldBot/AggroBot (FF, FA, AF, AA).

mod support;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use arena_core::config::GameConfig;
use arena_core::harness::MatchHarness;
use arena_core::types::{Coord, Faction, MatchResult};
use arena_replay::recorder::ReplayWriter;
use arena_replay::replay::{decode_log, replay};
use support::{AggroBot, FieldBot, gate_game_seed, gate_map, gate_map_seed, triangle_spawns};

/// Default match count; `ARENA_REPLAY_GATE_COUNT` lowers it for iteration.
const DEFAULT_COUNT: u32 = 1000;

/// Whole-gate wall-clock budget in debug builds (M4 measured 0.08 s per
/// bare match; record + serialize + decode + replay + re-encode is ~4x).
const BUDGET: Duration = Duration::from_secs(180);

/// Maximum worker threads (12-core machine).
const MAX_WORKERS: usize = 12;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Mix {
    FieldField,
    FieldAggro,
    AggroField,
    AggroAggro,
}

impl Mix {
    fn of(m: u32) -> Mix {
        match m % 4 {
            0 => Mix::FieldField,
            1 => Mix::FieldAggro,
            2 => Mix::AggroField,
            _ => Mix::AggroAggro,
        }
    }

    fn enl_is_field(self) -> bool {
        matches!(self, Mix::FieldField | Mix::FieldAggro)
    }

    fn res_is_field(self) -> bool {
        matches!(self, Mix::FieldField | Mix::AggroField)
    }
}

/// One match's contribution to the gate evidence.
struct Evidence {
    m: u32,
    map_seed_used: u64,
    skips: u32,
    result: MatchResult,
    links: usize,
    fields: usize,
    hacks: usize,
    events: usize,
    bytes: usize,
    final_hash: [u8; 32],
}

fn match_count() -> u32 {
    std::env::var("ARENA_REPLAY_GATE_COUNT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_COUNT)
}

/// Runs gate match `m`: record, serialize, decode, replay (verifying every
/// per-round hash, delta, event, and intake fact), and re-encode.
fn run_match(m: u32) -> Evidence {
    let cfg = GameConfig::default();
    let game_seed = gate_game_seed(m);
    let mix = Mix::of(m);
    let (map, map_seed_used, skips) = gate_map(m, &cfg, mix.enl_is_field() || mix.res_is_field());
    let (pair_enl, pair_res) = if mix.enl_is_field() || mix.res_is_field() {
        support::find_triangle_pair(&map)
    } else {
        ([0u32; 3], [0u32; 3])
    };

    // FieldBot factions spawn on their triangle; AggroBot factions spawn
    // all agents at the deterministic map corner.
    let corner_spawns = |faction: Faction| {
        let roster = arena_core::state::GameState::initial_roster(&cfg);
        let ids = if faction == Faction::Enl {
            roster.0
        } else {
            roster.1
        };
        let entries: Vec<_> = ids.iter().map(|&id| (id, Coord::new(0, 0))).collect();
        arena_core::spawn::validate_initial_spawn_set(&ids, &entries, &cfg).expect("corner spawns")
    };
    let enl_spawns = if mix.enl_is_field() {
        triangle_spawns(&cfg, Faction::Enl, &map, &pair_enl)
    } else {
        corner_spawns(Faction::Enl)
    };
    let res_spawns = if mix.res_is_field() {
        triangle_spawns(&cfg, Faction::Res, &map, &pair_res)
    } else {
        corner_spawns(Faction::Res)
    };

    let enl: Box<dyn arena_core::harness::RoundStrategy> = if mix.enl_is_field() {
        Box::new(FieldBot::new(Faction::Enl, pair_enl, 0))
    } else {
        Box::new(AggroBot::new(Faction::Enl, &cfg))
    };
    let res: Box<dyn arena_core::harness::RoundStrategy> = if mix.res_is_field() {
        Box::new(FieldBot::new(Faction::Res, pair_res, 3))
    } else {
        Box::new(AggroBot::new(Faction::Res, &cfg))
    };

    // Record the match (the recorder self-decodes on finish).
    let mut writer = ReplayWriter::new(game_seed, &cfg, &map, &enl_spawns, &res_spawns);
    let mut harness = MatchHarness::new(game_seed, cfg, &map, &enl_spawns, &res_spawns, enl, res)
        .expect("gate match builds");
    for _ in 0..800 {
        let report = harness.run_round();
        writer
            .record_round(&report, harness.state())
            .expect("record");
    }
    let bytes = writer.finish(harness.state()).expect("finish");

    // Decode + replay: replay() verifies EVERY per-round state hash, the
    // deltas, the events, the intake facts, the final hash, and the result
    // tag — 0 mismatches means exactly that.
    let decoded = decode_log(&bytes).expect("decode");
    let outcome = replay(&decoded).expect("every round replays identically");
    assert_eq!(outcome.rounds_completed, 800);

    // Re-encode is byte-exact for every match.
    assert_eq!(
        decoded.encode(),
        bytes,
        "match {m}: re-encode(decoded) == original bytes"
    );

    let state = &outcome.state;
    let hacks: u32 = state.hack_trackers().values().map(|t| t.hack_ordinal).sum();
    Evidence {
        m,
        map_seed_used,
        skips,
        result: outcome.result,
        links: state.link_graph().links().len(),
        fields: state.link_graph().fields().len(),
        hacks: hacks as usize,
        events: state.events().len(),
        bytes: bytes.len(),
        final_hash: outcome.final_state_hash,
    }
}

#[test]
fn gate_1000_matches_replay_identically() {
    let count = match_count();
    let start = Instant::now();
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_WORKERS);
    let workers = workers.max(1) as u32;

    // Each worker owns a disjoint m-range (stride partition).
    let mut evidence: Vec<Evidence> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                scope.spawn(move || {
                    let mut ours = Vec::new();
                    let mut m = worker;
                    while m < count {
                        ours.push(run_match(m));
                        m += workers;
                    }
                    ours
                })
            })
            .collect();
        let mut all = Vec::new();
        for handle in handles {
            all.extend(handle.join().expect("gate worker panicked"));
        }
        all
    });
    let elapsed = start.elapsed();

    // ---------------------------------------------------------------
    // Aggregates (every assert is over ALL matches that ran).
    // ---------------------------------------------------------------
    assert_eq!(evidence.len() as u32, count, "every match ran exactly once");
    evidence.sort_by_key(|e| e.m);

    // Distinct seeds really used (D50): game seeds by index, map seeds
    // actually used after skip replacement.
    let game_seeds: BTreeSet<u64> = (0..count).map(gate_game_seed).collect();
    assert_eq!(game_seeds.len() as u32, count, "1000 distinct game seeds");
    let map_seeds: BTreeSet<u64> = evidence.iter().map(|e| e.map_seed_used).collect();
    let map_seed_base: BTreeSet<u64> = (0..count).map(gate_map_seed).collect();
    assert_eq!(
        map_seed_base.len() as u32,
        count,
        "the base map seeds are distinct"
    );
    assert!(
        map_seeds.len() as u32 >= count.saturating_sub(count / 10),
        "skip replacement keeps map variety ({} distinct of {})",
        map_seeds.len(),
        count
    );

    // Skip rate under 10% of the matches that needed the triangle search.
    let search_matches: Vec<&Evidence> = evidence
        .iter()
        .filter(|e| Mix::of(e.m) != Mix::AggroAggro)
        .collect();
    let skips: u32 = search_matches.iter().map(|e| e.skips).sum();
    assert!(
        skips * 10 < search_matches.len() as u32,
        "skips {skips} must stay under 10% of {} search matches",
        search_matches.len()
    );

    // Strategy mix covers all four pairings.
    let mixes: BTreeSet<Mix> = evidence.iter().map(|e| Mix::of(e.m)).collect();
    assert!(count < 4 || mixes.len() == 4, "FF/FA/AF/AA all present");

    // Non-vacuity: the gate really exercised links, fields, hacks, events.
    let links: usize = evidence.iter().map(|e| e.links).sum();
    let fields: usize = evidence.iter().map(|e| e.fields).sum();
    let hacks: usize = evidence.iter().map(|e| e.hacks).sum();
    let events: usize = evidence.iter().map(|e| e.events).sum();
    assert!(links > 0, "aggregate links");
    assert!(fields > 0, "aggregate fields");
    assert!(hacks > 0, "aggregate hacks");
    assert!(events > 0, "aggregate events");

    // Results only ever take the three legal values.
    for e in &evidence {
        assert!(
            matches!(
                e.result,
                MatchResult::EnlWin | MatchResult::ResWin | MatchResult::Draw
            ),
            "match {} result {:?}",
            e.m,
            e.result
        );
    }

    // Parallel determinism spot-check (checklist 28): a serial rerun of a
    // subset reproduces the same final hashes — worker count cannot
    // influence a match.
    let spot = count.min(8);
    for e in &evidence[..spot as usize] {
        let serial = run_match(e.m);
        assert_eq!(
            serial.final_hash, e.final_hash,
            "match {} replays identically single-threaded",
            e.m
        );
    }

    // Budget (debug build).
    assert!(
        elapsed < BUDGET,
        "gate of {count} matches took {elapsed:?} (budget {BUDGET:?})"
    );

    let byte_total: usize = evidence.iter().map(|e| e.bytes).sum();
    eprintln!(
        "M5 GATE: matches={} skips={} (of {} search matches) distinct_maps={} threads={} elapsed={:?} matches/s={:.1} avg_replay_bytes={}",
        count,
        skips,
        search_matches.len(),
        map_seeds.len(),
        workers,
        elapsed,
        count as f64 / elapsed.as_secs_f64(),
        byte_total / count as usize,
    );
    eprintln!(
        "M5 GATE: aggregate links={} fields={} hacks={} events={}; mixes={:?}",
        links, fields, hacks, events, mixes
    );
}

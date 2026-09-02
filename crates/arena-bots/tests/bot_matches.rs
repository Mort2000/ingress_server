//! Match-level bot gates (checklist 4, 8, 11): determinism over identical
//! fixture matches, the GreedyField complexity bound on a real 500-round
//! default-map match, and the per-bot legality (drop-rate) gate against a
//! dummy opponent.

mod support;

use std::time::{Duration, Instant};

use arena_bots::bot::{BotKind, make_bot};
use arena_core::config::GameConfig;
use arena_core::harness::MatchHarness;
use arena_core::resolve::{CommandOutcome, RoundReport, SlotStatus};
use arena_core::types::{Coord, Faction, MatchResult};
use support::{IdleStrategy, fixture_cfg, fixture_map, generated_map, roster, validate_bot_spawns};

// Checklist 4: every bot, replayed twice over the same fixture match
// inputs, submits byte-identical command streams.
#[test]
fn bots_are_deterministic_given_identical_inputs() {
    let cfg = fixture_cfg();
    let map = fixture_map();
    let opponents = [
        (BotKind::Random, BotKind::Random),
        (BotKind::NearestPortal, BotKind::NearestPortal),
        (BotKind::GreedyField, BotKind::GreedyField),
        (BotKind::DestroyAnchor, BotKind::DestroyAnchor),
        (BotKind::Hybrid, BotKind::Hybrid),
        (BotKind::Hybrid, BotKind::GreedyField),
    ];
    for (enl_kind, res_kind) in opponents {
        // Identical streams <=> identical echoed submission slices in the
        // round reports (D48: submissions echo the exact input slice).
        let run = |bot_seed: u64| {
            let mut enl = make_bot(enl_kind, Faction::Enl, bot_seed, &cfg, &map);
            let mut res = make_bot(res_kind, Faction::Res, bot_seed, &cfg, &map);
            let enl_entries = enl.choose_spawns(&cfg, &map, &roster(&cfg, Faction::Enl));
            let res_entries = res.choose_spawns(&cfg, &map, &roster(&cfg, Faction::Res));
            let enl_spawns = validate_bot_spawns(&cfg, Faction::Enl, &map, &enl_entries);
            let res_spawns = validate_bot_spawns(&cfg, Faction::Res, &map, &res_entries);
            let mut harness = MatchHarness::new(
                99,
                cfg,
                &map,
                &enl_spawns,
                &res_spawns,
                Box::new(arena_bots::bot::BotStrategy(enl)),
                Box::new(arena_bots::bot::BotStrategy(res)),
            )
            .expect("match builds");
            let reports = harness.run_rounds(60);
            let enl_streams: Vec<_> = reports
                .iter()
                .map(|r| {
                    r.submissions
                        .iter()
                        .filter(|s| s.faction == Faction::Enl)
                        .map(|s| (s.agent_id, s.commands.clone()))
                        .collect::<Vec<_>>()
                })
                .collect();
            let res_streams: Vec<_> = reports
                .iter()
                .map(|r| {
                    r.submissions
                        .iter()
                        .filter(|s| s.faction == Faction::Res)
                        .map(|s| (s.agent_id, s.commands.clone()))
                        .collect::<Vec<_>>()
                })
                .collect();
            let state = harness.into_state();
            (enl_streams, res_streams, state.final_result())
        };
        let first = run(7);
        let second = run(7);
        assert_eq!(first.0, second.0, "{enl_kind:?} ENL streams identical");
        assert_eq!(first.1, second.1, "{res_kind:?} RES streams identical");
        assert_eq!(first.2, second.2);
        assert!(!first.0.is_empty());
    }

    // The deterministic bots ignore the bot seed; RandomBot's draws (and
    // therefore its streams) change with it.
    let run_random = |bot_seed: u64| recorded_submissions(bot_seed);
    assert_ne!(run_random(7), run_random(8), "RandomBot keyed by bot_seed");
    assert_eq!(run_random(9), run_random(9));
}

fn recorded_submissions(
    bot_seed: u64,
) -> Vec<Vec<(arena_core::types::AgentId, Vec<arena_core::types::Command>)>> {
    let cfg = fixture_cfg();
    let map = fixture_map();
    let mut enl = make_bot(BotKind::Random, Faction::Enl, bot_seed, &cfg, &map);
    let mut res = make_bot(BotKind::NearestPortal, Faction::Res, bot_seed, &cfg, &map);
    let enl_entries = enl.choose_spawns(&cfg, &map, &roster(&cfg, Faction::Enl));
    let res_entries = res.choose_spawns(&cfg, &map, &roster(&cfg, Faction::Res));
    let enl_spawns = validate_bot_spawns(&cfg, Faction::Enl, &map, &enl_entries);
    let res_spawns = validate_bot_spawns(&cfg, Faction::Res, &map, &res_entries);
    let mut harness = MatchHarness::new(
        5,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(arena_bots::bot::BotStrategy(enl)),
        Box::new(arena_bots::bot::BotStrategy(res)),
    )
    .expect("match builds");
    harness
        .run_rounds(40)
        .into_iter()
        .map(|r| {
            r.submissions
                .iter()
                .filter(|s| s.faction == Faction::Enl)
                .map(|s| (s.agent_id, s.commands.clone()))
                .collect()
        })
        .collect()
}

// Checklist 8: the candidate search runs only on invalidation — a
// 500-round default-map match with two GreedyField bots completes well
// inside the per-match time budget (debug build).
#[test]
fn greedy_field_500_round_match_fits_the_time_budget() {
    let start = Instant::now();
    let cfg = GameConfig::default();
    let map = generated_map();
    let mut enl = make_bot(BotKind::GreedyField, Faction::Enl, 1, &cfg, &map);
    let mut res = make_bot(BotKind::GreedyField, Faction::Res, 2, &cfg, &map);
    let enl_entries = enl.choose_spawns(&cfg, &map, &roster(&cfg, Faction::Enl));
    let res_entries = res.choose_spawns(&cfg, &map, &roster(&cfg, Faction::Res));
    let enl_spawns = validate_bot_spawns(&cfg, Faction::Enl, &map, &enl_entries);
    let res_spawns = validate_bot_spawns(&cfg, Faction::Res, &map, &res_entries);
    let mut harness = MatchHarness::new(
        0xBEEF_0006,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(arena_bots::bot::BotStrategy(enl)),
        Box::new(arena_bots::bot::BotStrategy(res)),
    )
    .expect("match builds");
    let reports = harness.run_rounds(500);
    let state = harness.into_state();
    let elapsed = start.elapsed();

    assert_eq!(state.round(), 501, "500 rounds ran");
    assert!(!state.link_graph().links().is_empty(), "links were built");
    assert!(
        !state.link_graph().fields().is_empty(),
        "at least one field completed on the generated map"
    );
    assert!(state.final_result() != MatchResult::Draw || state.score(Faction::Enl) == 0);
    let _ = &reports;
    assert!(
        elapsed < Duration::from_secs(120),
        "two GreedyField bots over 500 rounds took {elapsed:?} (budget 120 s)"
    );
}

// Checklist 11: over a 200-round default-config match against a dummy
// opponent, every bot's DROP rate stays under 5% of submitted commands —
// the baselines are mostly-legal measuring tools.
#[test]
fn per_bot_drop_rate_under_five_percent_vs_dummy() {
    let cfg = GameConfig::default();
    let map = generated_map();
    for kind in BotKind::ALL {
        let res_roster = roster(&cfg, Faction::Res);
        let entries: Vec<_> = res_roster
            .iter()
            .map(|&id| (id, Coord::new(0, 0)))
            .collect();
        let res_spawns = validate_bot_spawns(&cfg, Faction::Res, &map, &entries);
        let mut bot = make_bot(kind, Faction::Enl, 42, &cfg, &map);
        let enl_entries = bot.choose_spawns(&cfg, &map, &roster(&cfg, Faction::Enl));
        let enl_spawns = validate_bot_spawns(&cfg, Faction::Enl, &map, &enl_entries);
        let mut harness = MatchHarness::new(
            11,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(arena_bots::bot::BotStrategy(bot)),
            Box::new(IdleStrategy),
        )
        .expect("match builds");
        let reports = harness.run_rounds(200);
        let (mut submitted, mut dropped) = (0u64, 0u64);
        for report in &reports {
            count_drops(report, Faction::Enl, &mut submitted, &mut dropped);
            // Structural rejections would be a bot bug outright.
            for status in report.slots.values() {
                assert!(!matches!(status, SlotStatus::Rejected(_)), "{kind:?}");
            }
        }
        assert!(submitted > 0, "{kind:?} submitted commands");
        let rate_permille = dropped * 1000 / submitted.max(1);
        assert!(
            rate_permille < 50,
            "{kind:?} drop rate {dropped}/{submitted} = {rate_permille} permille (>= 50)"
        );
    }
}

fn count_drops(report: &RoundReport, faction: Faction, submitted: &mut u64, dropped: &mut u64) {
    for stream in &report.execution_order {
        if stream.faction != faction {
            continue;
        }
        for outcome in &stream.outcomes {
            *submitted += 1;
            if matches!(outcome, CommandOutcome::Drop(_)) {
                *dropped += 1;
            }
        }
    }
}

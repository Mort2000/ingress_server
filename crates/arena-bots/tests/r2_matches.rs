//! Match-level R2 behavior gates (`docs/ai-balance-plan.md` sections 10.1
//! and 13.3) on REAL matches through the harness: every R2 policy scores
//! stably against an idle opponent, real Transfer usage appears in a
//! competitive R2 pairing, identical inputs produce byte-identical
//! submissions, mirror factions of one policy choose different spawn sets
//! on the same map (no fixed-AgentId/spawn symmetry artifacts), and the
//! pooled Command Drop rate of a competitive R2 pairing stays far below
//! the 5% readiness threshold. Matches run 250 rounds of the default
//! config on the pinned generated map, so reinforcement agents (rounds
//! 100/200) join through the same allocation paths.

mod support;

use arena_bots::policy::{PolicyId, PolicySpec};
use arena_core::config::GameConfig;
use arena_core::event::EventKind;
use arena_core::types::{Command, Faction, Score2};
use support::{IdleStrategy, portal_spawns, run_policy_match};

/// The pinned default-config map (120 clustered portals).
fn match_map() -> arena_core::map::MapDefinition {
    support::generated_map()
}

/// Default config trimmed to 250 rounds (crosses two reinforcement
/// boundaries); every rule parameter is the frozen default.
fn match_cfg() -> GameConfig {
    let mut cfg = GameConfig::default();
    cfg.total_rounds = 250;
    cfg
}

fn competitive_specs() -> Vec<PolicySpec> {
    PolicyId::R2_COMPETITIVE
        .iter()
        .map(|id| id.r2_spec())
        .collect()
}

/// One faction's field-creation count from a played match's final public
/// event log (append-only; counts every FieldCreated of the faction).
fn faction_field_creations(state: &arena_core::state::GameState, faction: Faction) -> u64 {
    state
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::FieldCreated {
                    faction: f, ..
                } if f == faction
            )
        })
        .count() as u64
}

// Plan 10.1 idle gate: every R2 policy scores stably against the idle
// opponent — positive score2 and at least one created field.
#[test]
fn every_r2_policy_scores_against_idle() {
    let cfg = match_cfg();
    let map = match_map();
    let res_spawns = portal_spawns(&cfg, &map, Faction::Res);
    for spec in competitive_specs() {
        let (_reports, state) = run_policy_match(
            &cfg,
            &map,
            &spec,
            Box::new(IdleStrategy),
            res_spawns.clone(),
            0x6A1E_5EED_0000_0001,
            0x5EED_0000_00B2_0000 | spec.policy_id.tag().len() as u64,
            cfg.total_rounds,
        );
        let score: Score2 = state.score(Faction::Enl);
        let fields = faction_field_creations(&state, Faction::Enl);
        assert!(
            score > 0,
            "{}: score2 {} against idle must be positive",
            spec.policy_id.tag(),
            score
        );
        assert!(
            fields >= 1,
            "{}: {} field creations against idle",
            spec.policy_id.tag(),
            fields
        );
    }
}

// Plan 6.3 determinism: the same (spec, seeds, map, config) run twice
// produces IDENTICAL submission recordings, and the match outcome with it.
#[test]
fn r2_matches_are_deterministic() {
    let cfg = match_cfg();
    let map = match_map();
    let res_spawns = portal_spawns(&cfg, &map, Faction::Res);
    let run = |seed: u64| -> (Vec<Vec<Vec<Command>>>, Score2, Score2) {
        let spec = PolicyId::AdaptiveHybridR2.r2_spec();
        let mut player =
            arena_bots::policy::make_policy_player(&spec, Faction::Enl, seed, &cfg, &map);
        let enl_roster = support::roster(&cfg, Faction::Enl);
        let entries = player.choose_spawns(&cfg, &map, &enl_roster);
        let enl_spawns = support::validate_bot_spawns(&cfg, Faction::Enl, &map, &entries);
        let mut harness = arena_core::harness::MatchHarness::new(
            0x6A1E_5EED_0000_0002,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            player.into_strategy(),
            Box::new(IdleStrategy),
        )
        .expect("match builds");
        let reports = harness.run_rounds(cfg.total_rounds);
        let state = harness.into_state();
        (
            reports
                .iter()
                .map(|r| {
                    r.submissions
                        .iter()
                        .map(|s| s.commands.clone())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            state.score(Faction::Enl),
            state.score(Faction::Res),
        )
    };
    let first = run(0x11D1_7E57_0000_0001);
    let second = run(0x11D1_7E57_0000_0001);
    assert_eq!(first.0, second.0, "byte-identical submissions");
    assert_eq!((first.1, first.2), (second.1, second.2), "identical scores");
}

// Plan 10.1 mirror gate: ENL and RES instances of the same policy on the
// SAME map with the SAME bot_seed choose different spawn sets (the
// faction-keyed stream) — no identical-spawn mirror collisions.
#[test]
fn mirror_factions_of_one_policy_choose_different_spawns() {
    let cfg = GameConfig::default();
    let map = match_map();
    for spec in competitive_specs() {
        let spawns = |faction: Faction| {
            let mut player = arena_bots::policy::make_policy_player(
                &spec,
                faction,
                0x0B2A_1111_2222_3333,
                &cfg,
                &map,
            );
            let roster = support::roster(&cfg, faction);
            let mut entries = player.choose_spawns(&cfg, &map, &roster);
            entries.sort_by_key(|(agent, coord)| (agent.0, coord.x, coord.y));
            entries
        };
        let enl = spawns(Faction::Enl);
        let res = spawns(Faction::Res);
        assert_ne!(
            enl,
            res,
            "{}: mirror factions must not collide on one spawn set",
            spec.policy_id.tag()
        );
    }
}

// Plan 10.1 transfer gate at match level: a competitive R2 pairing
// (logistics-builder-r2 vs greedy-coordinated-r2) exercises real Transfer
// commands — the mechanism the frozen m6 baseline never used once in
// 10,000 matches.
// Plan 10.1 transfer gate at match level: a competitive R2 pairing
// (logistics-builder-r2 vs greedy-coordinated-r2) exercises real Transfer
// commands — the mechanism the frozen m6 baseline never used once in
// 10,000 matches.
#[test]
fn logistics_uses_transfer_in_a_competitive_match() {
    let cfg = match_cfg();
    let map = match_map();
    let enl_spec = PolicyId::LogisticsBuilderR2.r2_spec();
    let res_spec = PolicyId::GreedyCoordinatedR2.r2_spec();
    // The pilot round-robin measured transfers>0 in 94/95 of this
    // pairing's matches; a single seed asserts the mechanism without
    // betting the test on one trajectory, so scan a handful of seeds and
    // require at least one transfer-producing competitive match with both
    // sides scoring.
    let mut saw_transfer = false;
    for seed in [
        0x7A5C_0000_0000_0001u64,
        0x5EED_0000_0000_00AA,
        0x1234_5678_9ABC_DEF0,
    ] {
        let spawns = |spec: &PolicySpec, faction: Faction| {
            let mut player =
                arena_bots::policy::make_policy_player(spec, faction, seed, &cfg, &map);
            let roster = support::roster(&cfg, faction);
            let entries = player.choose_spawns(&cfg, &map, &roster);
            support::validate_bot_spawns(&cfg, faction, &map, &entries)
        };
        let enl_spawns = spawns(&enl_spec, Faction::Enl);
        let res_spawns = spawns(&res_spec, Faction::Res);
        let mut harness = arena_core::harness::MatchHarness::new(
            0x6A1E_5EED_0000_0003,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            arena_bots::policy::make_policy_player(&enl_spec, Faction::Enl, seed, &cfg, &map)
                .into_strategy(),
            arena_bots::policy::make_policy_player(&res_spec, Faction::Res, seed, &cfg, &map)
                .into_strategy(),
        )
        .expect("match builds");
        let reports = harness.run_rounds(cfg.total_rounds);
        // TRANSFER emits no public event (R-EVENT excludes it), so the
        // count comes from the echoed submission streams.
        let mut transfers = 0u64;
        for report in &reports {
            for stream in &report.submissions {
                for command in &stream.commands {
                    if matches!(command, Command::Transfer { .. }) {
                        transfers += 1;
                    }
                }
            }
        }
        let state = harness.into_state();
        let enl_scores =
            state.score(Faction::Enl) > 0 || faction_field_creations(&state, Faction::Enl) > 0;
        let res_scores =
            state.score(Faction::Res) > 0 || faction_field_creations(&state, Faction::Res) > 0;
        if transfers > 0 && enl_scores && res_scores {
            saw_transfer = true;
            break;
        }
    }
    assert!(
        saw_transfer,
        "logistics-builder-r2 must Transfer in a competitive match with both sides scoring"
    );
}

// Plan 10.1 drop gate at match level: a competitive R2 pairing's pooled
// Command Drop rate stays below the 5% readiness threshold (50 permille).
#[test]
fn competitive_r2_pairing_drop_rate_stays_low() {
    let cfg = match_cfg();
    let map = match_map();
    let (reports, _state) = {
        let enl_spec = PolicyId::BuilderDisruptorR2.r2_spec();
        let res_spec = PolicyId::BuilderDefenderR2.r2_spec();
        let enl_spawns = {
            let mut player = arena_bots::policy::make_policy_player(
                &enl_spec,
                Faction::Enl,
                0x3D10_0000_0000_0001,
                &cfg,
                &map,
            );
            let roster = support::roster(&cfg, Faction::Enl);
            let entries = player.choose_spawns(&cfg, &map, &roster);
            support::validate_bot_spawns(&cfg, Faction::Enl, &map, &entries)
        };
        let res_spawns = {
            let mut player = arena_bots::policy::make_policy_player(
                &res_spec,
                Faction::Res,
                0x3D10_0000_0000_0001,
                &cfg,
                &map,
            );
            let roster = support::roster(&cfg, Faction::Res);
            let entries = player.choose_spawns(&cfg, &map, &roster);
            support::validate_bot_spawns(&cfg, Faction::Res, &map, &entries)
        };
        let enl = arena_bots::policy::make_policy_player(
            &enl_spec,
            Faction::Enl,
            0x3D10_0000_0000_0001,
            &cfg,
            &map,
        );
        let res = arena_bots::policy::make_policy_player(
            &res_spec,
            Faction::Res,
            0x3D10_0000_0000_0001,
            &cfg,
            &map,
        );
        let mut harness = arena_core::harness::MatchHarness::new(
            0x6A1E_5EED_0000_0004,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            enl.into_strategy(),
            res.into_strategy(),
        )
        .expect("match builds");
        let reports = harness.run_rounds(cfg.total_rounds);
        let state = harness.into_state();
        (reports, state)
    };
    let mut submitted = 0u64;
    let mut dropped = 0u64;
    for report in &reports {
        for stream in &report.execution_order {
            for outcome in &stream.outcomes {
                submitted += 1;
                if matches!(outcome, arena_core::resolve::CommandOutcome::Drop(_)) {
                    dropped += 1;
                }
            }
        }
    }
    let permille = if submitted == 0 {
        0
    } else {
        dropped * 1000 / submitted
    };
    assert!(
        permille < 50,
        "pooled drop rate {permille} permille over {submitted} commands (threshold 50)"
    );
}

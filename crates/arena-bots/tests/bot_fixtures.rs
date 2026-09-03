//! Bot decision unit tests on hand-built fixture observations and fixture
//! matches (checklist 3, 5, 6, 7, 9, 10).

mod support;

use arena_bots::bot::{BotKind, make_bot};
use arena_core::config::GameConfig;
use arena_core::event::{Event, EventKind};
use arena_core::geometry::move_cost;
use arena_core::harness::AgentSubmission;
use arena_core::observe::Observation;
use arena_core::resolve::{CommandEffect, CommandOutcome, RoundReport};
use arena_core::state::GameState;
use arena_core::types::{AgentId, Command, Coord, Faction, Owner, PortalId};
use support::{
    IdleStrategy, NeutralizeAndCapture, ObsBuilder, fixture_cfg, fixture_map, generated_map,
    roster, run_bot_match, validate_bot_spawns,
};

// ---------------------------------------------------------------------
// Checklist 3: spawn sets
// ---------------------------------------------------------------------

#[test]
fn choose_spawns_validate_and_are_deterministic_on_both_maps() {
    let scenes: [(GameConfig, arena_core::map::MapDefinition); 2] = [
        (fixture_cfg(), fixture_map()),
        (GameConfig::default(), generated_map()),
    ];
    for (cfg, map) in scenes {
        for kind in BotKind::ALL {
            for faction in [Faction::Enl, Faction::Res] {
                for bot_seed in [0u64, 1, 0xDEAD_BEEF] {
                    let roster = roster(&cfg, faction);
                    let mut bot = make_bot(kind, faction, bot_seed, &cfg, &map);
                    let entries = bot.choose_spawns(&cfg, &map, &roster);
                    // Full coverage of the roster, in bounds, whole-set valid.
                    validate_bot_spawns(&cfg, faction, &map, &entries);
                    for (_, coord) in &entries {
                        assert!(coord.x >= 0 && coord.x <= cfg.map_width_cm);
                        assert!(coord.y >= 0 && coord.y <= cfg.map_height_cm);
                    }
                    // Deterministic given the same bot seed.
                    let mut again = make_bot(kind, faction, bot_seed, &cfg, &map);
                    assert_eq!(
                        again.choose_spawns(&cfg, &map, &roster),
                        entries,
                        "{kind:?} {faction:?} seed {bot_seed}"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// Checklist 5: RandomBot stream shapes
// ---------------------------------------------------------------------

/// A rich observation: a neutral portal in range, an enemy portal in range,
/// an own damaged portal in range, far portals out of range, and one legal
/// link opportunity (standing on an own portal holding a keyed target).
fn random_obs() -> Observation {
    ObsBuilder::new(1)
        .portal(0, Coord::new(10_000, 10_000), Owner::Neutral, 0)
        .portal(1, Coord::new(12_000, 10_000), Owner::Res, 100)
        .portal(2, Coord::new(10_000, 12_000), Owner::Enl, 40)
        .portal(3, Coord::new(38_000, 38_000), Owner::Neutral, 0)
        .agent(0, Coord::new(11_000, 10_900), 20)
        .agent(1, Coord::new(11_000, 11_100), 5)
        .agent_keys(1, 0, 1)
        .build()
}

#[test]
fn random_bot_emits_only_structurally_valid_streams() {
    let cfg = fixture_cfg();
    let map = fixture_map();
    let obs = random_obs();
    let mut bot = make_bot(BotKind::Random, Faction::Enl, 42, &cfg, &map);
    let mut distinct = std::collections::BTreeSet::new();
    for _ in 0..100 {
        let submissions = bot.submit_round(&obs);
        // At least one command per agent, every round (menu >= 1 entry and
        // k >= 1 draw).
        assert_eq!(submissions.len(), obs.own_agents.len());
        for submission in &submissions {
            let commands = &submission.commands;
            assert!(!commands.is_empty(), ">= 1 command");
            assert!(commands.len() <= cfg.max_commands_per_agent_round as usize);
            assert!(
                commands.iter().filter(|c| c.is_major()).count() <= 1,
                "<= 1 major"
            );
            let moves: Vec<&Command> = commands
                .iter()
                .filter(|c| matches!(c, Command::Move { .. }))
                .collect();
            assert!(moves.len() <= 1, "<= 1 move");
            if let Some(Command::Move { destination }) = moves.first() {
                let me = obs
                    .own_agents
                    .iter()
                    .find(|a| a.id == submission.agent_id)
                    .expect("own agent");
                assert!(move_cost(me.position, *destination) <= cfg.move_budget_cm as u64);
            }
        }
        distinct.insert(format!("{:?}", submissions));
    }
    // The menu draw is real randomness: 100 rounds produced more than one
    // distinct submission vector.
    assert!(distinct.len() > 1, "draws vary across rounds");
}

// ---------------------------------------------------------------------
// Checklist 6: NearestPortalBot decisions
// ---------------------------------------------------------------------

fn nearest_cfg() -> GameConfig {
    fixture_cfg()
}

#[test]
fn nearest_bot_targets_nearest_non_own_with_ascending_id_tie_break() {
    let cfg = nearest_cfg();
    let map = fixture_map();
    let mut bot = make_bot(BotKind::NearestPortal, Faction::Enl, 1, &cfg, &map);
    // Two equally distant neutral portals (P0 and P1 at dist2 400M from the
    // agent) plus a far own portal: P0 wins the tie.
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(30_000, 10_000), Owner::Neutral, 0)
        .portal(1, Coord::new(10_000, 30_000), Owner::Neutral, 0)
        .portal(2, Coord::new(39_000, 39_000), Owner::Enl, 100)
        .agent(0, Coord::new(10_000, 10_000), 20)
        .build();
    let submissions = bot.submit_round(&obs);
    assert_eq!(submissions.len(), 1);
    assert_eq!(
        submissions[0].commands,
        vec![Command::Move {
            destination: Coord::new(30_000, 10_000)
        }],
        "tie broken by ascending portal id; distance 20_000 == one budget hop"
    );
}

#[test]
fn nearest_bot_steps_within_move_budget() {
    let cfg = nearest_cfg();
    let map = fixture_map();
    let mut bot = make_bot(BotKind::NearestPortal, Faction::Enl, 1, &cfg, &map);
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(35_000, 10_000), Owner::Neutral, 0)
        .agent(0, Coord::new(10_000, 10_000), 20)
        .build();
    let submissions = bot.submit_round(&obs);
    let Command::Move { destination } = &submissions[0].commands[0] else {
        panic!("expected a move");
    };
    assert_eq!(*destination, Coord::new(30_000, 10_000));
    assert!(move_cost(Coord::new(10_000, 10_000), *destination) <= cfg.move_budget_cm as u64);
}

#[test]
fn nearest_bot_captures_attacks_and_hacks_in_range() {
    let cfg = nearest_cfg();
    let map = fixture_map();
    // Neutral target in range -> Capture.
    let mut bot = make_bot(BotKind::NearestPortal, Faction::Enl, 1, &cfg, &map);
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(10_000, 10_000), Owner::Neutral, 0)
        .agent(0, Coord::new(10_000, 10_000), 20)
        .build();
    assert_eq!(
        bot.submit_round(&obs)[0].commands,
        vec![Command::Capture {
            portal: PortalId(0)
        }]
    );

    // Enemy target in range -> Attack.
    let mut bot = make_bot(BotKind::NearestPortal, Faction::Enl, 1, &cfg, &map);
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(10_000, 10_000), Owner::Res, 100)
        .agent(0, Coord::new(10_000, 10_000), 20)
        .build();
    assert_eq!(bot.submit_round(&obs)[0].commands, vec![Command::Attack]);

    // Own portal without a key (the hack leg) -> Hack, then the ledger
    // suppresses the cooldown and epoch-limit drops.
    let mut bot = make_bot(BotKind::NearestPortal, Faction::Enl, 1, &cfg, &map);
    let obs_at = |round: u32| {
        ObsBuilder::new(round)
            .portal(0, Coord::new(10_000, 10_000), Owner::Enl, 100)
            .agent(0, Coord::new(10_000, 10_000), 20)
            .build()
    };
    // Rounds 1, 4, 7, 10, 13: five hacks (cooldown 3, limit 5 per epoch).
    for round in [1u32, 4, 7, 10, 13] {
        let submissions = bot.submit_round(&obs_at(round));
        assert_eq!(
            submissions[0].commands,
            vec![Command::Hack {
                portal: PortalId(0)
            }],
            "round {round}"
        );
    }
    // Round 16: epoch limit reached for the pair -> no hack (idle round).
    let submissions = bot.submit_round(&obs_at(16));
    assert!(submissions.is_empty(), "epoch-limited pair idles");

    // Round 102 (new epoch, cooldown long over): hack again.
    let submissions = bot.submit_round(&obs_at(102));
    assert_eq!(
        submissions[0].commands,
        vec![Command::Hack {
            portal: PortalId(0)
        }]
    );
}

#[test]
fn nearest_bot_120_round_match_has_no_hack_drops() {
    let cfg = nearest_cfg();
    let map = fixture_map();
    let res_spawns = corner_spawns(&cfg);
    let (reports, _) = run_bot_match(
        &cfg,
        &map,
        BotKind::NearestPortal,
        Box::new(IdleStrategy),
        res_spawns,
        7,
        99,
        120,
    );
    assert_eq!(reports.len(), 120);
    let mut hacks = 0;
    for report in &reports {
        for stream in &report.execution_order {
            for outcome in &stream.outcomes {
                assert!(
                    !matches!(
                        outcome,
                        CommandOutcome::Drop(
                            arena_core::resolve::DropReason::HackCooldownActive { .. }
                                | arena_core::resolve::DropReason::HackEpochLimitReached { .. }
                        )
                    ),
                    "own-ledger must suppress hack drops"
                );
                if matches!(outcome, CommandOutcome::Ok(CommandEffect::Hacked { .. })) {
                    hacks += 1;
                }
            }
        }
    }
    assert!(hacks > 0, "the hack leg really ran");
}

fn corner_spawns(cfg: &GameConfig) -> arena_core::spawn::InitialSpawns {
    let res_roster = roster(cfg, Faction::Res);
    let entries: Vec<(AgentId, Coord)> = res_roster
        .iter()
        .map(|&id| (id, Coord::new(0, 0)))
        .collect();
    validate_bot_spawns(cfg, Faction::Res, &fixture_map(), &entries)
}

// ---------------------------------------------------------------------
// Checklist 7: GreedyFieldBot
// ---------------------------------------------------------------------

/// The round in which the faction's first field closed, from the reports.
fn first_field_round(reports: &[RoundReport]) -> Option<u32> {
    reports
        .iter()
        .position(|r| {
            r.execution_order.iter().any(|s| {
                s.outcomes.iter().any(|o| {
                    matches!(
                        o,
                        CommandOutcome::Ok(CommandEffect::Linked { fields, .. })
                            if !fields.is_empty()
                    )
                })
            })
        })
        .map(|index| index as u32 + 1)
}

fn fields_closed(reports: &[RoundReport]) -> usize {
    reports
        .iter()
        .flat_map(|r| r.execution_order.iter())
        .flat_map(|s| s.outcomes.iter())
        .filter(|o| {
            matches!(
                o,
                CommandOutcome::Ok(CommandEffect::Linked { fields, .. }) if !fields.is_empty()
            )
        })
        .count()
}

#[test]
fn greedy_bot_builds_then_replans_for_a_second_field() {
    let cfg = fixture_cfg();
    let map = fixture_map();
    let (reports, state) = run_bot_match(
        &cfg,
        &map,
        BotKind::GreedyField,
        Box::new(IdleStrategy),
        corner_spawns(&cfg),
        11,
        5,
        150,
    );
    // Both edges then the field: the first field closes by round 40 on the
    // one-hop fixture map.
    assert!(
        first_field_round(&reports).is_some_and(|r| r <= 40),
        "first field by round 40"
    );
    // Plan complete -> replan -> a second field closes later.
    assert!(
        fields_closed(&reports) >= 2,
        "the bot replans for another triangle after completing one"
    );
    assert!(state.link_graph().fields().len() >= 1);
    assert!(state.score(Faction::Enl) > 0);
}

#[test]
fn greedy_bot_replans_away_from_a_lost_vertex() {
    let cfg = fixture_cfg();
    let map = fixture_map();
    // P0 is a vertex of the fixture map's seed triangle: the scripted
    // opponent sits on it, waits for the ENL field, then neutralizes and
    // takes it — the plan must replan around the loss.
    let (reports, state) = run_bot_match(
        &cfg,
        &map,
        BotKind::GreedyField,
        Box::new(NeutralizeAndCapture::new(PortalId(0), Faction::Enl)),
        on_portal_spawns(&cfg, &map, PortalId(0)),
        13,
        6,
        110,
    );
    // The disruption really happened: a built field died in a cascade.
    let destroyed = state
        .events()
        .iter()
        .filter(|e| matches!(e.kind, EventKind::FieldDestroyed { .. }))
        .count();
    assert!(destroyed >= 1, "the enemy neutralized a field vertex");
    // And the builder kept fielding through the loss.
    assert!(fields_closed(&reports) >= 2, "re-plan closed another field");
    let enl_fields: Vec<_> = state
        .link_graph()
        .fields()
        .iter()
        .filter(|(_, f)| f.faction == Faction::Enl)
        .collect();
    assert!(!enl_fields.is_empty(), "ENL still holds a live field");
    // No stream was ever structurally rejected (D27 caps hold by
    // construction across every bot decision).
    for report in &reports {
        for status in report.slots.values() {
            assert!(
                !matches!(status, arena_core::resolve::SlotStatus::Rejected(_)),
                "no structurally invalid stream"
            );
        }
    }
}

fn on_portal_spawns(
    cfg: &GameConfig,
    map: &arena_core::map::MapDefinition,
    portal: PortalId,
) -> arena_core::spawn::InitialSpawns {
    let res_roster = roster(cfg, Faction::Res);
    let position = map.portal(portal).expect("portal").position;
    let entries: Vec<(AgentId, Coord)> = res_roster.iter().map(|&id| (id, position)).collect();
    validate_bot_spawns(cfg, Faction::Res, map, &entries)
}

#[test]
fn greedy_bot_falls_back_to_expansion_without_illegal_links() {
    let cfg = fixture_cfg();
    // A map whose portals are ALL collinear: every triple has area2 == 0 —
    // no viable triangle anywhere (spawn search and every round re-search
    // agree), so the round falls back to expansion.
    let collinear = |positions: [(i32, i32); 4]| {
        arena_core::map::MapDefinition::new(
            positions
                .iter()
                .enumerate()
                .map(|(i, &(x, y))| arena_core::types::Portal {
                    id: PortalId(i as u32),
                    position: Coord::new(x, y),
                    owner: Owner::Neutral,
                    hp: 0,
                })
                .collect(),
        )
    };
    let map = collinear([
        (1_000, 10_000),
        (11_000, 10_000),
        (21_000, 10_000),
        (31_000, 10_000),
    ]);
    let mut bot = make_bot(BotKind::GreedyField, Faction::Enl, 3, &cfg, &map);
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(1_000, 10_000), Owner::Neutral, 0)
        .portal(1, Coord::new(11_000, 10_000), Owner::Neutral, 0)
        .portal(2, Coord::new(21_000, 10_000), Owner::Neutral, 0)
        .portal(3, Coord::new(31_000, 10_000), Owner::Neutral, 0)
        .agent(0, Coord::new(5_000, 5_000), 20)
        .agent(1, Coord::new(15_000, 5_000), 20)
        .agent(2, Coord::new(25_000, 5_000), 20)
        .build();
    for _ in 0..10 {
        let submissions = bot.submit_round(&obs);
        for submission in &submissions {
            assert!(
                !submission
                    .commands
                    .iter()
                    .any(|c| matches!(c, Command::Link { .. })),
                "fallback never links"
            );
            assert!(
                !submission.commands.is_empty(),
                "fallback never idles while unfinished portals remain"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Checklist 9: DestroyAnchorBot targeting
// ---------------------------------------------------------------------

fn anchor_cfg() -> GameConfig {
    fixture_cfg()
}

#[test]
fn destroy_anchor_targets_largest_enemy_anchor_then_ties_ascending() {
    let cfg = anchor_cfg();
    let map = fixture_map();
    let mut bot = make_bot(BotKind::DestroyAnchor, Faction::Enl, 1, &cfg, &map);
    // Two enemy fields: area2 100 on (P0,P1,P2), area2 200 on (P3,P4,P5).
    // Anchor values: P0/P1/P2 = 100; P3/P4/P5 = 200 -> argmax tie -> P3.
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(2_000, 2_000), Owner::Res, 100)
        .portal(1, Coord::new(6_000, 2_000), Owner::Res, 100)
        .portal(2, Coord::new(2_000, 6_000), Owner::Res, 100)
        .portal(3, Coord::new(30_000, 30_000), Owner::Res, 100)
        .portal(4, Coord::new(36_000, 30_000), Owner::Res, 100)
        .portal(5, Coord::new(30_000, 36_000), Owner::Res, 100)
        .field([0, 1, 2], Faction::Res, 100)
        .field([3, 4, 5], Faction::Res, 200)
        .agent(0, Coord::new(10_000, 10_000), 20)
        .build();
    let submissions = bot.submit_round(&obs);
    assert_eq!(submissions.len(), 1);
    // One budget step toward P3: dist = ceil_sqrt(2 * 20_000^2) = 28_285,
    // per-axis step = 20_000 * 20_000 / 28_285 = 14_141.
    assert_eq!(
        submissions[0].commands,
        vec![Command::Move {
            destination: Coord::new(24_141, 24_141)
        }]
    );

    // Equal-area fields: argmax tie over all six -> the lowest vertex id.
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(2_000, 2_000), Owner::Res, 100)
        .portal(1, Coord::new(6_000, 2_000), Owner::Res, 100)
        .portal(2, Coord::new(2_000, 6_000), Owner::Res, 100)
        .portal(3, Coord::new(30_000, 30_000), Owner::Res, 100)
        .portal(4, Coord::new(36_000, 30_000), Owner::Res, 100)
        .portal(5, Coord::new(30_000, 36_000), Owner::Res, 100)
        .field([0, 1, 2], Faction::Res, 150)
        .field([3, 4, 5], Faction::Res, 150)
        .agent(0, Coord::new(10_000, 10_000), 20)
        .build();
    let submissions = bot.submit_round(&obs);
    // P0 is only 11_314 cm away: the whole portal is within one hop.
    assert_eq!(
        submissions[0].commands,
        vec![Command::Move {
            destination: Coord::new(2_000, 2_000)
        }]
    );
}

#[test]
fn destroy_anchor_all_zero_falls_back_to_nearest_enemy_portal() {
    let cfg = anchor_cfg();
    let map = fixture_map();
    let mut bot = make_bot(BotKind::DestroyAnchor, Faction::Enl, 1, &cfg, &map);
    // No fields at all: nearest enemy portal (P2 at dist 3_000, in closed
    // operation range) -> Attack.
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(30_000, 30_000), Owner::Res, 100)
        .portal(1, Coord::new(2_000, 30_000), Owner::Neutral, 0)
        .portal(2, Coord::new(13_000, 10_000), Owner::Res, 100)
        .agent(0, Coord::new(10_000, 10_000), 20)
        .build();
    assert_eq!(
        bot.submit_round(&obs)[0].commands,
        vec![Command::Attack],
        "nearest enemy portal in range gets attacked"
    );

    // No fields and no enemy portals: nearest neutral -> Capture.
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(30_000, 30_000), Owner::Enl, 100)
        .portal(2, Coord::new(13_000, 10_000), Owner::Neutral, 0)
        .agent(0, Coord::new(10_000, 10_000), 20)
        .build();
    assert_eq!(
        bot.submit_round(&obs)[0].commands,
        vec![Command::Capture {
            portal: PortalId(2)
        }]
    );
}

#[test]
fn destroy_anchor_converges_and_recharges_in_passing() {
    let cfg = anchor_cfg();
    let map = fixture_map();
    let mut bot = make_bot(BotKind::DestroyAnchor, Faction::Enl, 1, &cfg, &map);
    // One enemy field on (P3,P4,P5): both agents converge on P3; agent 0
    // (lowest id) stands in range of a damaged own portal P9 -> it
    // recharges instead of attacking.
    let obs = ObsBuilder::new(1)
        .portal(3, Coord::new(30_000, 30_000), Owner::Res, 100)
        .portal(4, Coord::new(36_000, 30_000), Owner::Res, 100)
        .portal(5, Coord::new(30_000, 36_000), Owner::Res, 100)
        .portal(9, Coord::new(10_000, 10_000), Owner::Enl, 40)
        .field([3, 4, 5], Faction::Res, 200)
        .agent(0, Coord::new(9_000, 9_000), 20) // in range of damaged P9
        .agent(1, Coord::new(20_000, 20_000), 20)
        .build();
    let submissions = bot.submit_round(&obs);
    let by_agent: std::collections::BTreeMap<AgentId, &AgentSubmission> =
        submissions.iter().map(|s| (s.agent_id, s)).collect();
    assert_eq!(
        by_agent[&AgentId(0)].commands,
        vec![Command::Recharge {
            portal: PortalId(9)
        }],
        "lowest-id agent in range of a damaged own portal defends in passing"
    );
    assert_eq!(
        by_agent[&AgentId(1)].commands,
        vec![Command::Move {
            destination: Coord::new(30_000, 30_000)
        }],
        "everyone else converges on the anchor (14_143 cm hop, within budget)"
    );
}

// ---------------------------------------------------------------------
// Checklist 10: HybridBot roles
// ---------------------------------------------------------------------

#[test]
fn hybrid_builders_field_and_reinforcements_attack() {
    let cfg = fixture_cfg();
    let map = fixture_map();
    let (reports, state) = run_bot_match(
        &cfg,
        &map,
        BotKind::Hybrid,
        Box::new(IdleStrategy),
        corner_spawns(&cfg),
        17,
        8,
        160,
    );
    // The initial trio builds a field.
    assert!(
        first_field_round(&reports).is_some_and(|r| r <= 45),
        "trio completes the first field"
    );
    assert!(state.score(Faction::Enl) > 0);

    // The round-100 reinforcement (AgentId 6, id >= 2n with n = 3) is an
    // attacker: it never LINKs, and it does engage portals.
    let reinforcement = AgentId(6);
    let mut engaged = 0;
    let mut linked = 0;
    for report in &reports {
        for stream in &report.execution_order {
            if stream.agent_id != reinforcement {
                continue;
            }
            for outcome in &stream.outcomes {
                if matches!(outcome, CommandOutcome::Ok(CommandEffect::Linked { .. })) {
                    linked += 1;
                }
                if matches!(
                    outcome,
                    CommandOutcome::Ok(
                        CommandEffect::Captured { .. }
                            | CommandEffect::Moved { .. }
                            | CommandEffect::Attacked { .. }
                    )
                ) {
                    engaged += 1;
                }
            }
        }
    }
    assert_eq!(linked, 0, "reinforcements never build");
    assert!(engaged > 0, "reinforcement agents take the attacker role");
}

#[test]
fn hybrid_recharge_branch_fires_below_half_hp() {
    let cfg = fixture_cfg();
    // A map consistent with the observation world below (three ENL-owned
    // portals forming one own field over P0/P1/P2).
    let map = arena_core::map::MapDefinition::new(vec![
        arena_core::types::Portal {
            id: PortalId(0),
            position: Coord::new(10_000, 10_000),
            owner: Owner::Enl,
            hp: 45,
        },
        arena_core::types::Portal {
            id: PortalId(1),
            position: Coord::new(30_000, 10_000),
            owner: Owner::Enl,
            hp: 100,
        },
        arena_core::types::Portal {
            id: PortalId(2),
            position: Coord::new(10_000, 30_000),
            owner: Owner::Enl,
            hp: 100,
        },
    ]);
    let mut bot = make_bot(BotKind::Hybrid, Faction::Enl, 1, &cfg, &map);
    // No agent stands in range of the wounded portal, so only the ANCHOR
    // defense dispatch (not DestroyAnchor's in-passing rule) can send
    // anyone at P0.
    let base = |hp_after: u32| {
        ObsBuilder::new(10)
            .portal(0, Coord::new(10_000, 10_000), Owner::Enl, hp_after)
            .portal(1, Coord::new(30_000, 10_000), Owner::Enl, 100)
            .portal(2, Coord::new(10_000, 30_000), Owner::Enl, 100)
            .field([0, 1, 2], Faction::Enl, 5_000_000)
            .agent(0, Coord::new(0, 0), 20) // builders far away
            .agent(1, Coord::new(0, 1_000), 20)
            .agent(2, Coord::new(1_000, 0), 20)
            .agent(6, Coord::new(30_000, 30_000), 20) // nearest attacker
            .agent(7, Coord::new(35_000, 35_000), 20)
            .event(Event {
                seq: 0,
                round: 9,
                kind: EventKind::PortalAttacked {
                    faction: Faction::Res,
                    agent_id: AgentId(3),
                    portal: PortalId(0),
                    damage: 20,
                    hp_after,
                },
            })
    };
    // Wounded anchor (45 hp < 50% of 100): the nearest attacker diverts —
    // out of operation range and keyless, so it steps toward P0.
    let obs = base(45).build();
    let submissions = bot.submit_round(&obs);
    let agent6 = submissions
        .iter()
        .find(|s| s.agent_id == AgentId(6))
        .expect("the nearest attacker diverts to the wounded anchor");
    assert!(
        matches!(&agent6.commands[..], [Command::Move { destination }]
            if *destination != Coord::new(30_000, 30_000)),
        "agent 6 moves toward the anchor instead of idling: {:?}",
        agent6.commands
    );

    // Above 50% (60 hp): no dispatch — the attackers have no enemy target
    // on this observation, so agent 6 idles entirely.
    let mut bot = make_bot(BotKind::Hybrid, Faction::Enl, 1, &cfg, &map);
    let obs = base(60).build();
    let submissions = bot.submit_round(&obs);
    assert!(
        !submissions.iter().any(|s| s.agent_id == AgentId(6)),
        "no defense dispatch at 60 hp"
    );
}

// ---------------------------------------------------------------------
// Extra pin: GameConfig Default sanity inside the fixture scenes
// ---------------------------------------------------------------------

#[test]
fn fixture_scenes_use_valid_configs() {
    fixture_cfg().validate().expect("fixture config");
    GameConfig::default().validate().expect("default config");
    let _ = GameState::initial_roster(&fixture_cfg());
}

//! Compound combat/build plans must preserve auxiliary work, not just heal HP.
mod support;

use arena_bots::r2::{
    R2Policy,
    builder_defender::{BuilderDefender, BuilderDefenderParams},
};
use arena_core::{
    config::GameConfig,
    event::{Event, EventKind},
    observe::Observation,
    types::{AgentId, Command, Coord, Faction, Owner, PortalId},
};

fn remote_builder() -> Observation {
    support::ObsBuilder::new(101)
        .portal(0, Coord::new(4_000, 4_000), Owner::Enl, 20)
        .portal(1, Coord::new(16_000, 4_000), Owner::Enl, 100)
        .portal(2, Coord::new(4_000, 16_000), Owner::Enl, 100)
        .link(0, 1, Faction::Enl)
        .link(0, 2, Faction::Enl)
        .link(1, 2, Faction::Enl)
        .field([0, 1, 2], Faction::Enl, 144_000_000)
        .portal(3, Coord::new(50_000, 50_000), Owner::Enl, 100)
        .portal(4, Coord::new(54_000, 50_000), Owner::Enl, 100)
        .portal(5, Coord::new(50_000, 54_000), Owner::Enl, 100)
        .link(3, 4, Faction::Enl)
        .link(3, 5, Faction::Enl)
        .agent(0, Coord::new(54_000, 50_000), 20)
        .agent_keys(0, 0, 1)
        .agent_keys(0, 5, 1)
        .event(Event {
            seq: 0,
            round: 100,
            kind: EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(3),
                portal: PortalId(0),
                damage: 20,
                hp_after: 20,
            },
        })
        .build()
}

fn config() -> GameConfig {
    GameConfig {
        max_link_distance_cm: 20_000,
        ..GameConfig::default()
    }
}

#[test]
fn defender_remotely_recharges_while_closing_a_field() {
    let cfg = config();
    let mut policy =
        BuilderDefender::new(Faction::Enl, 0x2501, &cfg, BuilderDefenderParams::default());
    let commands = policy.submit_round(&remote_builder());
    let stream = &commands[0].commands;
    assert!(
        stream.iter().any(|c| matches!(c, Command::Link { .. })),
        "{commands:?}"
    );
    assert!(
        stream.contains(&Command::Recharge {
            portal: PortalId(0)
        }),
        "{commands:?}"
    );
}

#[test]
fn defender_keeps_its_build_route_while_remotely_recharging() {
    let cfg = config();
    let mut obs = remote_builder();
    obs.own_agents[0].position = Coord::new(100_000, 100_000);
    let mut policy =
        BuilderDefender::new(Faction::Enl, 0x2502, &cfg, BuilderDefenderParams::default());
    let commands = policy.submit_round(&obs);
    let stream = &commands[0].commands;
    assert!(
        stream.contains(&Command::Recharge {
            portal: PortalId(0)
        }),
        "{commands:?}"
    );
    assert!(stream.iter().any(|c| matches!(c, Command::Move { destination } if destination.x > 50_000 && destination.y > 50_000)), "retain construction route, not a march to the recharge target: {commands:?}");
}

#[test]
fn defender_hacks_locally_without_sacrificing_recharge() {
    let cfg = config();
    let mut obs = remote_builder();
    obs.own_agents[0].position = obs.portals[0].position;
    obs.own_agents[0].supply = 1;
    obs.portals.truncate(3);
    obs.links.retain(|l| l.id.max.0 < 3);
    obs.own_agents[0].keys.retain(|p, _| p.0 < 3);
    let mut policy =
        BuilderDefender::new(Faction::Enl, 0x2503, &cfg, BuilderDefenderParams::default());
    let commands = policy.submit_round(&obs);
    let stream = &commands[0].commands;
    assert!(
        stream.contains(&Command::Recharge {
            portal: PortalId(0)
        }),
        "{commands:?}"
    );
    assert!(
        stream.iter().any(|c| matches!(c, Command::Hack { .. })),
        "{commands:?}"
    );
}

#[test]
fn legacy_mode_does_not_silently_enable_compound_work() {
    let mut params = BuilderDefenderParams::default();
    params.core.combat_auxiliaries = false;
    let mut policy = BuilderDefender::new(Faction::Enl, 0x2501, &config(), params);
    let commands = policy.submit_round(&remote_builder());
    assert_eq!(
        commands[0].commands,
        vec![Command::Link {
            source_portal: PortalId(4),
            target_portal: PortalId(5),
        }]
    );
}

#[test]
fn rescue_does_not_reserve_hp_that_only_a_future_enemy_hit_could_create() {
    // R470 of development m6/leg1 exposed this at 95 HP: the model
    // inserted a predicted hit between two heals, but the attacker left.
    // Recent pressure is evidence, not a reservation of missing HP.
    let obs = support::ObsBuilder::new(101)
        .portal(0, Coord::new(4_000, 4_000), Owner::Enl, 95)
        .portal(1, Coord::new(16_000, 4_000), Owner::Enl, 100)
        .portal(2, Coord::new(4_000, 16_000), Owner::Enl, 100)
        .link(0, 1, Faction::Enl)
        .link(0, 2, Faction::Enl)
        .link(1, 2, Faction::Enl)
        .field([0, 1, 2], Faction::Enl, 144_000_000)
        .agent(0, Coord::new(4_000, 4_000), 20)
        .agent(1, Coord::new(4_000, 4_000), 20)
        .event(Event {
            seq: 0,
            round: 99,
            kind: EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(3),
                portal: PortalId(0),
                damage: 20,
                hp_after: 80,
            },
        })
        .build();
    let mut policy = BuilderDefender::new(
        Faction::Enl,
        0x2510,
        &config(),
        BuilderDefenderParams::default(),
    );
    let commands = policy.submit_round(&obs);
    let heals = commands
        .iter()
        .flat_map(|s| &s.commands)
        .filter(|c| {
            matches!(
                c,
                Command::Recharge {
                    portal: PortalId(0)
                }
            )
        })
        .count();
    assert!(heals <= 1, "only 5 HP is missing: {commands:?}");
}

#[test]
fn attacker_keeps_a_compatible_link_and_hack_without_leaving_attack_range() {
    use arena_bots::r2::{
        build::{BuilderCore, CoreParams},
        rng::R2Stream,
        tasks::{Task, TaskRequest},
    };
    let obs = support::ObsBuilder::new(101)
        .portal(0, Coord::new(4_000, 4_000), Owner::Res, 20)
        .portal(1, Coord::new(16_000, 4_000), Owner::Res, 100)
        .portal(2, Coord::new(4_000, 16_000), Owner::Res, 100)
        .link(0, 1, Faction::Res)
        .link(0, 2, Faction::Res)
        .link(1, 2, Faction::Res)
        .field([0, 1, 2], Faction::Res, 144_000_000)
        .portal(3, Coord::new(8_000, 0), Owner::Enl, 100)
        .portal(4, Coord::new(12_000, 0), Owner::Enl, 100)
        .agent(0, Coord::new(6_000, 0), 20)
        .agent_keys(0, 4, 1)
        .build();
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2508,
        R2Stream::BuilderDisruptor,
        &config(),
        CoreParams::default(),
    );
    let requests = [(
        AgentId(0),
        vec![
            TaskRequest::one_shot(
                Task::Attack {
                    portal: PortalId(0),
                },
                i64::MAX,
            ),
            TaskRequest::one_shot(
                Task::Link {
                    source: PortalId(3),
                    target: PortalId(4),
                },
                1,
            ),
        ],
    )]
    .into_iter()
    .collect();
    let commands = core
        .allocate_and_execute(&obs, &requests)
        .remove(&AgentId(0))
        .unwrap()
        .build();
    assert!(commands.contains(&Command::Attack));
    assert!(commands.iter().any(|c| matches!(c, Command::Link { .. })));
    assert!(commands.iter().any(|c| matches!(c, Command::Hack { .. })));
    assert!(!commands.iter().any(|c| matches!(c, Command::Move { .. })));
    let harvested = commands
        .iter()
        .find_map(|c| match c {
            Command::Hack { portal } => Some(*portal),
            _ => None,
        })
        .unwrap();
    // An allocation preview does not reserve the Hack cooldown; actual
    // emission does, so a repeated round cannot emit it a second time.
    let commands = core
        .allocate_and_execute(&obs, &requests)
        .remove(&AgentId(0))
        .unwrap()
        .build();
    assert!(!commands.contains(&Command::Hack { portal: harvested }));
}

#[test]
fn a_link_must_not_spend_the_last_remote_key_before_recharge() {
    use arena_bots::r2::{
        build::{BuilderCore, CoreParams},
        rng::R2Stream,
        tasks::{Task, TaskRequest},
    };
    let mut cfg = config();
    cfg.max_link_distance_cm = 100_000;
    let mut obs = remote_builder();
    obs.portals.truncate(4);
    obs.links.retain(|l| l.id.max.0 < 3);
    obs.portals[3].position = Coord::new(30_000, 0);
    obs.own_agents[0].position = obs.portals[3].position;
    obs.own_agents[0].keys.retain(|p, _| p.0 == 0);
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2504,
        R2Stream::BuilderDefender,
        &cfg,
        CoreParams::default(),
    );
    let requests = [(
        AgentId(0),
        vec![
            TaskRequest::one_shot(
                Task::Recharge {
                    portal: PortalId(0),
                },
                i64::MAX,
            ),
            TaskRequest::one_shot(
                Task::Link {
                    source: PortalId(3),
                    target: PortalId(0),
                },
                1,
            ),
        ],
    )]
    .into_iter()
    .collect();
    let stream = core
        .allocate_and_execute(&obs, &requests)
        .remove(&AgentId(0))
        .unwrap()
        .build();
    let recharge = stream
        .iter()
        .position(|c| matches!(c, Command::Recharge { .. }))
        .unwrap();
    let link = stream
        .iter()
        .position(|c| matches!(c, Command::Link { .. }))
        .unwrap();
    assert!(recharge < link, "{stream:?}");
}

#[test]
fn compound_work_obeys_the_command_limit_and_single_major_slot() {
    let mut cfg = config();
    cfg.max_commands_per_agent_round = 1;
    let mut policy =
        BuilderDefender::new(Faction::Enl, 0x2505, &cfg, BuilderDefenderParams::default());
    let commands = policy.submit_round(&remote_builder());
    assert_eq!(commands[0].commands.len(), 1);
    let mut obs = remote_builder();
    obs.portals[4].owner = Owner::Neutral;
    obs.portals[4].hp = 0;
    obs.links.retain(|l| !l.id.contains(PortalId(4)));
    let mut policy = BuilderDefender::new(
        Faction::Enl,
        0x2506,
        &config(),
        BuilderDefenderParams::default(),
    );
    let commands = policy.submit_round(&obs);
    assert!(
        commands
            .iter()
            .all(|s| s.commands.iter().filter(|c| c.is_major()).count() <= 1)
    );
}

#[test]
fn compound_allocation_releases_a_strike_when_a_required_member_captures() {
    use arena_bots::r2::{
        build::{BuilderCore, CoreParams},
        rng::R2Stream,
        tasks::{Task, TaskRequest},
    };
    let cfg = GameConfig::default();
    let obs = support::ObsBuilder::new(cfg.total_rounds - 4)
        .portal(0, Coord::new(4_000, 4_000), Owner::Res, 100)
        .portal(1, Coord::new(4_000, 8_000), Owner::Neutral, 0)
        .portal(2, Coord::new(65_000, 8_000), Owner::Neutral, 0)
        .agent(0, Coord::new(4_000, 4_000), 20)
        .agent(1, Coord::new(65_000, 4_000), 20)
        .build();
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2509,
        R2Stream::BuilderDisruptor,
        &cfg,
        CoreParams::default(),
    );
    let attack = Task::Attack {
        portal: PortalId(0),
    };
    let requests = [
        (
            AgentId(0),
            vec![
                TaskRequest::one_shot(
                    Task::Capture {
                        portal: PortalId(1),
                    },
                    1000,
                ),
                TaskRequest::coordinated(attack, 900, 2),
            ],
        ),
        (
            AgentId(1),
            vec![
                TaskRequest::coordinated(attack, 800, 2),
                TaskRequest::one_shot(
                    Task::Capture {
                        portal: PortalId(2),
                    },
                    10,
                ),
            ],
        ),
    ]
    .into_iter()
    .collect();
    core.allocate_and_execute(&obs, &requests);
    let trace = core.decision_trace().unwrap();
    assert!(
        trace
            .agents
            .iter()
            .flat_map(|a| [a.grant, a.companion])
            .flatten()
            .all(|t| t != attack),
        "{trace:?}"
    );
    assert_eq!(
        trace.agents[1].grant,
        Some(Task::Capture {
            portal: PortalId(2)
        })
    );
}

struct World {
    state: arena_core::state::GameState,
}

impl World {
    fn step(
        &mut self,
        own: Vec<arena_core::harness::AgentSubmission>,
        enemy_attacks: bool,
    ) -> arena_core::resolve::RoundReport {
        use arena_core::resolve::{CommandOutcome, Resolver, SubmittedCommandStream};
        let mut streams: Vec<_> = own
            .into_iter()
            .enumerate()
            .map(|(seq, sub)| SubmittedCommandStream {
                round: self.state.round(),
                faction: Faction::Enl,
                agent_id: sub.agent_id,
                received_at_ms: 0,
                receive_seq: seq as u32,
                commands: sub.commands,
            })
            .collect();
        if enemy_attacks {
            streams.push(SubmittedCommandStream {
                round: self.state.round(),
                faction: Faction::Res,
                agent_id: AgentId(3),
                received_at_ms: 0,
                receive_seq: 0,
                commands: vec![Command::Attack],
            });
        }
        let report = Resolver::with_seed(0x2500).run_round(&mut self.state, &streams);
        assert!(
            report
                .execution_order
                .iter()
                .flat_map(|s| &s.outcomes)
                .all(|o| matches!(o, CommandOutcome::Ok(_))),
            "{report:?}"
        );
        self.state.advance_round();
        report
    }

    fn built() -> Self {
        use arena_core::{
            harness::AgentSubmission, map::MapDefinition, spawn::validate_initial_spawn_set,
            state::GameState, types::Portal,
        };
        let cfg = GameConfig {
            portal_count: 6,
            portal_cluster_count: 6,
            portal_min_distance_cm: 4_000,
            hack_key_yield_max: 1,
            ..config()
        };
        let positions = [
            Coord::new(4_000, 4_000),
            Coord::new(16_000, 4_000),
            Coord::new(4_000, 16_000),
            Coord::new(50_000, 50_000),
            Coord::new(54_000, 50_000),
            Coord::new(50_000, 54_000),
        ];
        let map = MapDefinition::new(
            positions
                .iter()
                .enumerate()
                .map(|(i, &position)| Portal {
                    id: PortalId(i as u32),
                    position,
                    owner: Owner::Neutral,
                    hp: 0,
                })
                .collect(),
        );
        let (enl, res) = GameState::initial_roster(&cfg);
        let left = validate_initial_spawn_set(
            &enl,
            &enl.iter()
                .enumerate()
                .map(|(i, &a)| (a, positions[i]))
                .collect::<Vec<_>>(),
            &cfg,
        )
        .unwrap();
        let right = validate_initial_spawn_set(
            &res,
            &res.iter().map(|&a| (a, positions[0])).collect::<Vec<_>>(),
            &cfg,
        )
        .unwrap();
        let mut world = Self {
            state: GameState::new(cfg, &map, &left, &right).unwrap(),
        };
        world.step(
            (0..3)
                .map(|i| {
                    AgentSubmission::new(
                        AgentId(i),
                        vec![
                            Command::Capture {
                                portal: PortalId(i),
                            },
                            Command::Hack {
                                portal: PortalId(i),
                            },
                        ],
                    )
                })
                .collect(),
            false,
        );
        world.step(
            (0..3)
                .map(|i| {
                    let next = (i + 1) % 3;
                    AgentSubmission::new(
                        AgentId(i),
                        vec![
                            Command::Move {
                                destination: positions[next as usize],
                            },
                            Command::Hack {
                                portal: PortalId(next),
                            },
                            Command::Link {
                                source_portal: PortalId(next),
                                target_portal: PortalId(i),
                            },
                        ],
                    )
                })
                .collect(),
            false,
        );
        while (0..3)
            .any(|i| world.state.agent(AgentId(i)).unwrap().position != positions[i as usize + 3])
        {
            let moves = (0..3)
                .filter_map(|i| {
                    let from = world.state.agent(AgentId(i)).unwrap().position;
                    let to = positions[i as usize + 3];
                    (from != to).then(|| {
                        AgentSubmission::new(
                            AgentId(i),
                            vec![Command::Move {
                                destination: arena_bots::common::step_toward(
                                    from,
                                    to,
                                    cfg.move_budget_cm,
                                ),
                            }],
                        )
                    })
                })
                .collect();
            world.step(moves, false);
        }
        world.step(
            (0..3)
                .map(|i| {
                    AgentSubmission::new(
                        AgentId(i),
                        vec![
                            Command::Capture {
                                portal: PortalId(i + 3),
                            },
                            Command::Hack {
                                portal: PortalId(i + 3),
                            },
                        ],
                    )
                })
                .collect(),
            false,
        );
        for _ in 0..2 {
            world.step(vec![], false);
        }
        world.step(
            (1..3)
                .map(|i| {
                    let mut commands = vec![Command::Move {
                        destination: positions[3],
                    }];
                    if i == 2 {
                        commands.push(Command::Hack {
                            portal: PortalId(5),
                        });
                    }
                    commands.push(Command::Link {
                        source_portal: PortalId(3),
                        target_portal: PortalId(i + 3),
                    });
                    AgentSubmission::new(AgentId(i), commands)
                })
                .collect(),
            false,
        );
        // Keep the first defender in an ENL-first round with a 20 HP anchor.
        if world.state.round() % 2 == 0 {
            world.step(vec![], false);
        }
        for _ in 0..4 {
            world.step(vec![], true);
        }
        assert_eq!(world.state.portal(PortalId(0)).unwrap().hp, 20);
        assert_eq!(world.state.link_graph().fields().len(), 1);
        world
    }
}

#[test]
fn real_settlement_preserves_one_field_and_builds_another_in_the_same_stream() {
    use arena_core::resolve::{CommandEffect, CommandOutcome};
    for enabled in [false, true] {
        let mut world = World::built();
        let mut params = BuilderDefenderParams::default();
        params.core.combat_auxiliaries = enabled;
        let mut policy = BuilderDefender::new(Faction::Enl, 0x2507, world.state.config(), params);
        let commands = policy.submit_round(&Observation::for_faction(&world.state, Faction::Enl));
        let report = world.step(commands, true);
        assert_eq!(
            world.state.link_graph().fields().len(),
            if enabled { 2 } else { 1 }
        );
        if enabled {
            assert!(
                report.execution_order.iter().any(|s| {
                    s.outcomes
                        .iter()
                        .any(|o| matches!(o, CommandOutcome::Ok(CommandEffect::Recharged { .. })))
                        && s.outcomes
                            .iter()
                            .any(|o| matches!(o, CommandOutcome::Ok(CommandEffect::Linked { .. })))
                }),
                "{report:?}"
            );
            assert_eq!(report.score2_delta[&Faction::Enl], 160_000_000);
            assert_eq!(world.state.portal(PortalId(0)).unwrap().hp, 15);
        } else {
            assert_eq!(report.score2_delta[&Faction::Enl], 16_000_000);
        }
    }
}

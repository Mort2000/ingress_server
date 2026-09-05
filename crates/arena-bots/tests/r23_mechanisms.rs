//! Behavior regressions for the complete r2.3 policy portfolio.
mod support;

use arena_bots::r2::logistics_builder::{LogisticsBuilder, LogisticsBuilderParams};
use arena_bots::r2::{
    R2Policy,
    build::{BuilderCore, CoreParams},
    builder_defender::{BuilderDefender, BuilderDefenderParams},
    greedy_coordinated::GreedyCoordinated,
    rng::R2Stream,
    tasks::{Task, TaskRequest},
};
use arena_core::{
    config::GameConfig,
    harness::AgentSubmission,
    map::MapDefinition,
    observe::Observation,
    resolve::{CommandOutcome, Resolver, RoundReport, SubmittedCommandStream},
    spawn::validate_initial_spawn_set,
    state::GameState,
    types::{AgentId, Command, Coord, Faction, Owner, Portal, PortalId},
};

struct World {
    state: GameState,
    resolver: Resolver,
}

impl World {
    fn new(positions: &[Coord], agents: u32) -> Self {
        let cfg = GameConfig {
            portal_count: positions.len() as u32,
            portal_cluster_count: positions.len() as u32,
            map_width_cm: 100_000,
            map_height_cm: 100_000,
            portal_min_distance_cm: 1,
            initial_agents_per_faction: agents,
            hack_key_yield_max: 1,
            ..GameConfig::default()
        };
        let map = MapDefinition::new(
            positions
                .iter()
                .enumerate()
                .map(|(id, &position)| Portal {
                    id: PortalId(id as u32),
                    position,
                    owner: Owner::Neutral,
                    hp: 0,
                })
                .collect(),
        );
        let (enl, res) = GameState::initial_roster(&cfg);
        let spawn = |roster: &[AgentId], offset: usize| {
            validate_initial_spawn_set(
                roster,
                &roster
                    .iter()
                    .enumerate()
                    .map(|(i, &id)| (id, positions[(i + offset) % positions.len()]))
                    .collect::<Vec<_>>(),
                &cfg,
            )
            .unwrap()
        };
        let state = GameState::new(cfg, &map, &spawn(&enl, 0), &spawn(&res, 1)).unwrap();
        Self {
            state,
            resolver: Resolver::with_seed(0x2300),
        }
    }

    fn step(&mut self, enl: Vec<AgentSubmission>, res: Vec<AgentSubmission>) -> RoundReport {
        self.resolve(enl, res, true)
    }

    fn resolve(
        &mut self,
        enl: Vec<AgentSubmission>,
        res: Vec<AgentSubmission>,
        require_legal: bool,
    ) -> RoundReport {
        let streams = [(Faction::Enl, enl), (Faction::Res, res)]
            .into_iter()
            .flat_map(|(faction, subs)| {
                subs.into_iter()
                    .enumerate()
                    .map(move |(seq, sub)| (faction, seq, sub))
            })
            .map(|(faction, seq, sub)| SubmittedCommandStream {
                round: self.state.round(),
                faction,
                agent_id: sub.agent_id,
                received_at_ms: sub.bucket * self.state.config().priority_bucket_ms as u64,
                receive_seq: seq as u32,
                commands: sub.commands,
            })
            .collect::<Vec<_>>();
        let report = self.resolver.run_round(&mut self.state, &streams);
        assert!(
            !require_legal
                || report
                    .execution_order
                    .iter()
                    .flat_map(|sub| &sub.outcomes)
                    .all(|outcome| matches!(outcome, CommandOutcome::Ok(_))),
            "{report:?}"
        );
        self.state.advance_round();
        report
    }

    fn observation(&self) -> Observation {
        Observation::for_faction(&self.state, Faction::Enl)
    }

    fn walk(&mut self, agent: AgentId, to: Coord) {
        while self.state.agent(agent).unwrap().position != to {
            let destination = arena_bots::common::step_toward(
                self.state.agent(agent).unwrap().position,
                to,
                self.state.config().move_budget_cm,
            );
            self.step(
                vec![AgentSubmission::new(
                    agent,
                    vec![Command::Move { destination }],
                )],
                vec![],
            );
        }
    }

    fn built_field() -> Self {
        let positions = [
            Coord::new(4_000, 4_000),
            Coord::new(16_000, 4_000),
            Coord::new(4_000, 16_000),
        ];
        let mut world = Self::new(&positions, 3);
        world.step(
            (0..3)
                .map(|id| {
                    AgentSubmission::new(
                        AgentId(id),
                        vec![
                            Command::Capture {
                                portal: PortalId(id),
                            },
                            Command::Hack {
                                portal: PortalId(id),
                            },
                        ],
                    )
                })
                .collect(),
            vec![],
        );
        world.step(
            (0..3)
                .map(|id| {
                    let source = (id + 1) % 3;
                    AgentSubmission::new(
                        AgentId(id),
                        vec![
                            Command::Move {
                                destination: positions[source as usize],
                            },
                            Command::Hack {
                                portal: PortalId(source),
                            },
                            Command::Link {
                                source_portal: PortalId(source),
                                target_portal: PortalId(id),
                            },
                        ],
                    )
                })
                .collect(),
            vec![],
        );
        world
    }
}

#[test]
fn fallback_does_not_wait_for_another_enemy_key_when_a_capture_is_available() {
    let mut world = World::new(
        &[
            Coord::new(4_000, 4_000),
            Coord::new(8_000, 4_000),
            Coord::new(4_000, 12_000),
        ],
        1,
    );
    world.step(
        vec![AgentSubmission::new(
            AgentId(0),
            vec![
                Command::Capture {
                    portal: PortalId(0),
                },
                Command::Hack {
                    portal: PortalId(0),
                },
            ],
        )],
        vec![AgentSubmission::new(
            AgentId(1),
            vec![Command::Capture {
                portal: PortalId(1),
            }],
        )],
    );
    let mut params = CoreParams::default();
    params.value.max_field_candidates = 0;
    let mut policy = GreedyCoordinated::new(Faction::Enl, 0x2301, world.state.config(), params);
    let commands = policy.submit_round(&world.observation());
    world.step(commands, vec![]);
    let commands = policy.submit_round(&world.observation());
    assert!(
        commands
            .iter()
            .flat_map(|sub| &sub.commands)
            .any(|command| matches!(command, Command::Move { .. } | Command::Capture { .. })),
        "a redundant KeyFarm must not hide the reachable neutral portal: {commands:?}"
    );
    world.step(commands, vec![]);
    for _ in 0..3 {
        let commands = policy.submit_round(&world.observation());
        world.step(commands, vec![]);
    }
    assert_eq!(world.state.portal(PortalId(2)).unwrap().owner, Owner::Enl);
}

#[test]
fn replanning_reuses_owned_edges_and_finishes_the_field_this_round() {
    let positions = [
        Coord::new(4_000, 4_000),
        Coord::new(16_000, 4_000),
        Coord::new(4_000, 16_000),
    ];
    let mut world = World::new(&positions, 3);
    world.step(
        (0..3)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![
                        Command::Capture {
                            portal: PortalId(id),
                        },
                        Command::Hack {
                            portal: PortalId(id),
                        },
                    ],
                )
            })
            .collect(),
        vec![],
    );
    world.step(
        (0..2)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![
                        Command::Move {
                            destination: positions[2],
                        },
                        Command::Link {
                            source_portal: PortalId(2),
                            target_portal: PortalId(id),
                        },
                    ],
                )
            })
            .collect(),
        vec![],
    );
    world.step(
        vec![AgentSubmission::new(
            AgentId(0),
            vec![
                Command::Move {
                    destination: positions[1],
                },
                Command::Hack {
                    portal: PortalId(1),
                },
            ],
        )],
        vec![],
    );
    let mut policy = GreedyCoordinated::new(
        Faction::Enl,
        0x2302,
        world.state.config(),
        CoreParams::default(),
    );
    let commands = policy.submit_round(&world.observation());
    assert!(
        policy.decision_trace().unwrap().active.is_some(),
        "two owned links are progress toward a new field, not disqualifying obstacles"
    );
    world.step(commands, vec![]);
    assert_eq!(world.state.link_graph().fields().len(), 1);
}

#[test]
fn an_unavailable_hack_cannot_hide_an_executable_lower_ranked_task() {
    let mut world = World::new(
        &[
            Coord::new(4_000, 4_000),
            Coord::new(8_000, 4_000),
            Coord::new(4_000, 8_000),
        ],
        1,
    );
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2303,
        R2Stream::GreedyCoordinated,
        world.state.config(),
        CoreParams::default(),
    );
    let requests = [(
        AgentId(0),
        vec![TaskRequest::one_shot(
            Task::KeyFarm {
                portal: PortalId(1),
            },
            100,
        )],
    )]
    .into_iter()
    .collect();
    let commands = core.allocate_and_execute(&world.observation(), &requests);
    world.step(
        arena_bots::r2::greedy_coordinated::submissions(&world.observation(), commands),
        vec![],
    );
    let requests = [(
        AgentId(0),
        vec![
            TaskRequest::one_shot(
                Task::KeyFarm {
                    portal: PortalId(1),
                },
                100,
            ),
            TaskRequest::one_shot(
                Task::Capture {
                    portal: PortalId(2),
                },
                10,
            ),
        ],
    )]
    .into_iter()
    .collect();
    let commands = core.allocate_and_execute(&world.observation(), &requests);
    let commands = arena_bots::r2::greedy_coordinated::submissions(&world.observation(), commands);
    assert!(
        commands.iter().flat_map(|s| &s.commands).any(|c| matches!(
            c,
            Command::Capture {
                portal: PortalId(2)
            }
        )),
        "cooldown must be considered before allocation: {commands:?}"
    );
    world.step(commands, vec![]);
    assert_eq!(world.state.portal(PortalId(2)).unwrap().owner, Owner::Enl);
}

#[test]
fn a_surplus_builder_can_capture_outside_the_current_triangle() {
    let mut world = World::new(
        &[
            Coord::new(4_000, 4_000),
            Coord::new(16_000, 4_000),
            Coord::new(4_000, 16_000),
            Coord::new(16_000, 16_000),
        ],
        4,
    );
    let mut policy = GreedyCoordinated::new(
        Faction::Enl,
        0x2304,
        world.state.config(),
        CoreParams {
            second_plan_discount_permille: 0,
            ..Default::default()
        },
    );
    let commands = policy.submit_round(&world.observation());
    assert_eq!(
        commands.len(),
        4,
        "one triangle must not hide a fourth immediately capturable portal: {commands:?}"
    );
    world.step(commands, vec![]);
    assert!(world.state.portals().iter().all(|p| p.owner == Owner::Enl));
}

#[test]
fn an_urgent_recharge_is_submitted_before_routine_work_and_survives_the_burst() {
    let mut world = World::built_field();
    world.step(
        (0..3)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![Command::Move {
                        destination: Coord::new(4_000, 16_000),
                    }],
                )
            })
            .collect(),
        vec![
            AgentSubmission::new(
                AgentId(3),
                vec![Command::Move {
                    destination: Coord::new(4_000, 4_000),
                }],
            ),
            AgentSubmission::new(
                AgentId(4),
                vec![Command::Move {
                    destination: Coord::new(4_000, 4_000),
                }],
            ),
            AgentSubmission::new(AgentId(5), vec![Command::Attack]),
        ],
    );
    world.step(
        vec![],
        vec![AgentSubmission::new(AgentId(5), vec![Command::Attack])],
    );
    assert_eq!(world.state.portal(PortalId(0)).unwrap().hp, 60);
    let mut policy = BuilderDefender::new(
        Faction::Enl,
        0x2305,
        world.state.config(),
        BuilderDefenderParams::default(),
    );
    let commands = policy.submit_round(&world.observation());
    assert!(
        commands[0].commands.contains(&Command::Recharge {
            portal: PortalId(0)
        }),
        "urgent rescue must not be queued behind routine work by AgentId: {commands:?}"
    );
    world.step(
        commands,
        (3..6)
            .map(|id| AgentSubmission::new(AgentId(id), vec![Command::Attack]))
            .collect(),
    );
    assert_eq!(world.state.portal(PortalId(0)).unwrap().hp, 15);
    assert_eq!(world.state.link_graph().fields().len(), 1);
}

fn delivery_world() -> World {
    let positions = [
        Coord::new(4_000, 4_000),
        Coord::new(60_000, 4_000),
        Coord::new(4_000, 60_000),
    ];
    let mut world = World::new(&positions, 3);
    world.step(
        (0..3)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![
                        Command::Capture {
                            portal: PortalId(id),
                        },
                        Command::Hack {
                            portal: PortalId(id),
                        },
                    ],
                )
            })
            .collect(),
        vec![],
    );
    for id in 0..2 {
        world.walk(AgentId(id), positions[2]);
        world.step(
            vec![AgentSubmission::new(
                AgentId(id),
                vec![Command::Link {
                    source_portal: PortalId(2),
                    target_portal: PortalId(id),
                }],
            )],
            vec![],
        );
    }
    world.walk(AgentId(2), positions[1]);
    world.step(
        vec![AgentSubmission::new(
            AgentId(2),
            vec![Command::Hack {
                portal: PortalId(1),
            }],
        )],
        vec![],
    );
    world.walk(AgentId(2), Coord::new(32_000, 4_000));
    world.walk(AgentId(0), Coord::new(28_000, 4_000));
    world.walk(AgentId(1), Coord::new(90_000, 90_000));
    world
}

#[test]
fn ordered_delivery_advances_a_default_rule_field_and_uses_the_key_in_the_same_round() {
    let mut relay = delivery_world();
    let mut direct = delivery_world();
    let mut enabled = LogisticsBuilder::new(
        Faction::Enl,
        0x2306,
        relay.state.config(),
        LogisticsBuilderParams::default(),
    );
    let mut disabled = LogisticsBuilder::new(
        Faction::Enl,
        0x2306,
        direct.state.config(),
        LogisticsBuilderParams {
            max_transfers_per_round: 0,
            ..Default::default()
        },
    );
    let commands = enabled.submit_round(&relay.observation());
    assert!(
        commands
            .first()
            .is_some_and(|sub| sub.agent_id == AgentId(2)
                && sub
                    .commands
                    .iter()
                    .any(|c| matches!(c, Command::Transfer { supply: 0, .. }))),
        "the distant holder must deliver before the receiver moves and links: {commands:?}"
    );
    relay.step(commands, vec![]);
    let commands = disabled.submit_round(&direct.observation());
    direct.step(commands, vec![]);
    assert_eq!(relay.state.link_graph().fields().len(), 1);
    assert_eq!(direct.state.link_graph().fields().len(), 0);
    assert_eq!(
        relay
            .state
            .agent(AgentId(0))
            .unwrap()
            .keys
            .get(&PortalId(1))
            .copied()
            .unwrap_or(0),
        0,
        "the received Key must be consumed immediately"
    );
    let commands = disabled.submit_round(&direct.observation());
    direct.step(commands, vec![]);
    assert_eq!(direct.state.link_graph().fields().len(), 1);
}

#[test]
fn one_recharge_is_not_credited_with_the_entire_fields_future_income() {
    let cfg = GameConfig {
        max_link_distance_cm: 20_000,
        ..GameConfig::default()
    };
    let obs = support::ObsBuilder::new(10)
        .portal(0, Coord::new(4_000, 4_000), Owner::Enl, 80)
        .portal(1, Coord::new(16_000, 4_000), Owner::Enl, 100)
        .portal(2, Coord::new(4_000, 16_000), Owner::Enl, 100)
        .portal(3, Coord::new(60_000, 60_000), Owner::Enl, 100)
        .portal(4, Coord::new(66_000, 60_000), Owner::Enl, 100)
        .portal(5, Coord::new(60_000, 66_000), Owner::Enl, 100)
        .link(0, 1, Faction::Enl)
        .link(0, 2, Faction::Enl)
        .link(1, 2, Faction::Enl)
        .link(3, 5, Faction::Enl)
        .link(4, 5, Faction::Enl)
        .field([0, 1, 2], Faction::Enl, 144_000_000)
        .event(arena_core::event::Event {
            seq: 0,
            round: 9,
            kind: arena_core::event::EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(3),
                portal: PortalId(0),
                damage: 20,
                hp_after: 80,
            },
        })
        .agent(0, Coord::new(60_000, 60_000), 20)
        .agent_keys(0, 0, 1)
        .agent_keys(0, 4, 1)
        .build();
    let mut policy =
        BuilderDefender::new(Faction::Enl, 0x2307, &cfg, BuilderDefenderParams::default());
    let commands = policy.submit_round(&obs);
    assert!(
        commands.iter().flat_map(|s| &s.commands).any(|c| matches!(
            c,
            Command::Link {
                source_portal: PortalId(3),
                target_portal: PortalId(4)
            }
        )),
        "a 15 HP intervention must compete at its marginal benefit, not 100 rounds of anchor income: {commands:?}"
    );
}

#[test]
fn surplus_agents_expand_even_when_no_triangle_exists() {
    let mut world = World::new(
        &[
            Coord::new(4_000, 4_000),
            Coord::new(8_000, 4_000),
            Coord::new(12_000, 4_000),
            Coord::new(16_000, 4_000),
        ],
        4,
    );
    world.step(
        (0..4)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![Command::Move {
                        destination: Coord::new(4_000, 4_000),
                    }],
                )
            })
            .collect(),
        vec![],
    );
    let mut policy = GreedyCoordinated::new(
        Faction::Enl,
        0x2308,
        world.state.config(),
        CoreParams::default(),
    );
    let commands = policy.submit_round(&world.observation());
    assert!(policy.decision_trace().unwrap().active.is_none());
    assert_eq!(
        commands.len(),
        4,
        "fallback truncation must not waste a reachable capture"
    );
    world.step(commands, vec![]);
    let commands = policy.submit_round(&world.observation());
    world.step(commands, vec![]);
    assert!(world.state.portals().iter().all(|p| p.owner == Owner::Enl));
}

#[test]
fn last_round_delivery_can_complete_a_field_that_direct_travel_cannot() {
    let mut world = delivery_world();
    while world.state.round() < world.state.config().total_rounds {
        world.step(vec![], vec![]);
    }
    let mut policy = LogisticsBuilder::new(
        Faction::Enl,
        0x2309,
        world.state.config(),
        LogisticsBuilderParams::default(),
    );
    let commands = policy.submit_round(&world.observation());
    assert!(
        commands
            .iter()
            .flat_map(|s| &s.commands)
            .any(|c| matches!(c, Command::Transfer { .. })),
        "a missing direct completion is a worse baseline: {commands:?}; comparisons: {:?}; trace: {:?}",
        policy.delivery_comparisons(),
        policy.decision_trace()
    );
    world.step(commands, vec![]);
    assert_eq!(world.state.link_graph().fields().len(), 1);
}

#[test]
fn a_far_attacker_is_not_sent_after_the_near_attacker_can_finish() {
    let mut world = World::built_field();
    for _ in 0..7 {
        let commands = (3..5)
            .filter_map(|id| {
                let position = world.state.agent(AgentId(id)).unwrap().position;
                let to = Coord::new(90_000, 90_000);
                (position != to).then(|| {
                    AgentSubmission::new(
                        AgentId(id),
                        vec![Command::Move {
                            destination: arena_bots::common::step_toward(
                                position,
                                to,
                                world.state.config().move_budget_cm,
                            ),
                        }],
                    )
                })
            })
            .collect();
        world.step(vec![], commands);
    }
    for _ in 0..3 {
        world.step(
            vec![],
            vec![AgentSubmission::new(AgentId(5), vec![Command::Attack])],
        );
    }
    let mut policy = arena_bots::r2::builder_disruptor::BuilderDisruptor::new(
        Faction::Res,
        0x2310,
        world.state.config(),
        Default::default(),
    );
    let commands = policy.submit_round(&Observation::for_faction(&world.state, Faction::Res));
    let attackers: Vec<_> = policy
        .decision_trace()
        .unwrap()
        .agents
        .iter()
        .filter(|a| matches!(a.grant, Some(Task::Attack { .. })))
        .map(|a| a.agent)
        .collect();
    assert_eq!(
        attackers,
        vec![AgentId(5)],
        "the nearby attacker finishes in two rounds; a five-move march adds no damage"
    );
    world.step(vec![], commands);
    let commands = policy.submit_round(&Observation::for_faction(&world.state, Faction::Res));
    world.step(vec![], commands);
    assert!(world.state.link_graph().fields().is_empty());
}

#[test]
fn a_blocking_link_can_use_this_rounds_move_budget() {
    let cfg = GameConfig::default();
    let obs = support::ObsBuilder::new(1)
        .portal(0, Coord::new(4_000, 20_000), Owner::Res, 100)
        .portal(1, Coord::new(60_000, 20_000), Owner::Res, 100)
        .portal(2, Coord::new(4_000, 80_000), Owner::Res, 100)
        .portal(3, Coord::new(20_000, 10_000), Owner::Enl, 100)
        .portal(4, Coord::new(20_000, 30_000), Owner::Enl, 100)
        .agent(0, Coord::new(20_000, 0), 20)
        .agent_keys(0, 4, 1)
        .build();
    let mut policy = arena_bots::r2::blocker_builder::BlockerBuilder::new(
        Faction::Enl,
        0x2311,
        &cfg,
        Default::default(),
    );
    let commands = policy.submit_round(&obs);
    assert_eq!(
        commands[0].commands,
        vec![
            Command::Move {
                destination: Coord::new(20_000, 10_000)
            },
            Command::Link {
                source_portal: PortalId(3),
                target_portal: PortalId(4)
            }
        ]
    );
}

#[test]
fn adaptive_endgame_saves_a_field_instead_of_starting_unfinishable_work() {
    let mut world = World::built_field();
    while world.state.round() < world.state.config().total_rounds - 5 {
        world.step(vec![], vec![]);
    }
    for _ in 0..4 {
        world.step(
            vec![],
            vec![AgentSubmission::new(AgentId(5), vec![Command::Attack])],
        );
    }
    let mut policy = arena_bots::r2::adaptive_hybrid_r2::AdaptiveHybridR2::new(
        Faction::Enl,
        0x2312,
        world.state.config(),
        Default::default(),
    );
    let commands = policy.submit_round(&world.observation());
    assert!(
        commands.iter().flat_map(|s| &s.commands).any(|c| matches!(
            c,
            Command::Recharge {
                portal: PortalId(0)
            }
        )),
        "endgame field income is still worth saving: {commands:?}"
    );
    world.step(
        commands,
        vec![AgentSubmission::new(AgentId(5), vec![Command::Attack])],
    );
    assert_eq!(world.state.portal(PortalId(0)).unwrap().hp, 15);
    assert_eq!(world.state.link_graph().fields().len(), 1);
}

#[test]
fn recapture_clears_adaptive_pressure_and_unsettled_attacks_are_not_history() {
    use arena_bots::r2::opponent::{OpponentModel, OpponentParams};
    use arena_core::event::{Event, EventKind};
    let attack = |seq, round| Event {
        seq,
        round,
        kind: EventKind::PortalAttacked {
            faction: Faction::Res,
            agent_id: AgentId(1),
            portal: PortalId(0),
            damage: 20,
            hp_after: 80,
        },
    };
    let mut model = OpponentModel::new(OpponentParams::default(), Faction::Res);
    let obs = support::ObsBuilder::new(10).event(attack(0, 9)).build();
    model.update(&obs);
    assert_eq!(model.pressure(&obs, PortalId(0)), 1);
    let obs = support::ObsBuilder::new(11)
        .event(attack(0, 9))
        .event(Event {
            seq: 1,
            round: 10,
            kind: EventKind::PortalCaptured {
                faction: Faction::Enl,
                agent_id: AgentId(0),
                portal: PortalId(0),
            },
        })
        .event(attack(2, 11))
        .build();
    model.update(&obs);
    assert_eq!(
        model.pressure(&obs, PortalId(0)),
        0,
        "old lifecycle and this unsettled round are not current pressure"
    );
}

#[test]
fn endgame_candidate_budget_keeps_an_executable_partial_field() {
    let cfg = GameConfig {
        max_link_distance_cm: 20_000,
        ..Default::default()
    };
    let obs = support::ObsBuilder::new(cfg.total_rounds)
        .portal(0, Coord::new(4_000, 4_000), Owner::Neutral, 0)
        .portal(1, Coord::new(16_000, 4_000), Owner::Neutral, 0)
        .portal(2, Coord::new(4_000, 16_000), Owner::Neutral, 0)
        .portal(3, Coord::new(60_000, 60_000), Owner::Enl, 100)
        .portal(4, Coord::new(66_000, 60_000), Owner::Enl, 100)
        .portal(5, Coord::new(60_000, 66_000), Owner::Enl, 100)
        .link(3, 5, Faction::Enl)
        .link(4, 5, Faction::Enl)
        .agent(0, Coord::new(60_000, 60_000), 20)
        .agent_keys(0, 4, 1)
        .build();
    let mut params = CoreParams::default();
    params.value.max_field_candidates = 1;
    let mut policy = GreedyCoordinated::new(Faction::Enl, 0x2314, &cfg, params);
    let commands = policy.submit_round(&obs);
    assert!(
        commands.iter().flat_map(|s| &s.commands).any(|c| matches!(
            c,
            Command::Link {
                source_portal: PortalId(3),
                target_portal: PortalId(4)
            }
        )),
        "an immediately payable closing edge must survive the candidate bound: {commands:?}"
    );
}

#[test]
fn defense_does_not_promise_rescue_after_the_enemy_first_fatal_strike() {
    let mut world = World::built_field();
    while world.state.round() < world.state.config().total_rounds - 4 {
        world.step(vec![], vec![]);
    }
    for _ in 0..4 {
        world.step(
            vec![],
            vec![AgentSubmission::new(AgentId(5), vec![Command::Attack])],
        );
    }
    let mut policy = BuilderDefender::new(
        Faction::Enl,
        0x2315,
        world.state.config(),
        Default::default(),
    );
    let commands = policy.submit_round(&world.observation());
    assert!(
        commands
            .iter()
            .flat_map(|s| &s.commands)
            .all(|c| !matches!(c, Command::Recharge { .. })),
        "the observed first strike is fatal before ENL can recharge on an even round"
    );
    world.step(
        commands,
        vec![AgentSubmission::new(AgentId(5), vec![Command::Attack])],
    );
    assert!(world.state.link_graph().fields().is_empty());
}

#[test]
fn adaptive_endgame_forms_a_strike_that_only_a_team_can_finish() {
    let mut world = World::built_field();
    world.step(
        vec![],
        (3..5)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![Command::Move {
                        destination: Coord::new(4_000, 4_000),
                    }],
                )
            })
            .collect(),
    );
    for _ in 0..3 {
        world.step(
            vec![],
            vec![AgentSubmission::new(AgentId(5), vec![Command::Attack])],
        );
    }
    while world.state.round() < world.state.config().total_rounds {
        world.step(vec![], vec![]);
    }
    let mut policy = arena_bots::r2::adaptive_hybrid_r2::AdaptiveHybridR2::new(
        Faction::Res,
        0x2316,
        world.state.config(),
        Default::default(),
    );
    let commands = policy.submit_round(&Observation::for_faction(&world.state, Faction::Res));
    assert_eq!(
        commands
            .iter()
            .flat_map(|s| &s.commands)
            .filter(|c| matches!(c, Command::Attack))
            .count(),
        2,
        "two attackers can jointly finish the 40 HP anchor this round: {commands:?}"
    );
    world.step(vec![], commands);
    assert!(world.state.link_graph().fields().is_empty());
}

#[test]
fn allocation_releases_a_strike_when_its_required_teammate_builds_instead() {
    let cfg = GameConfig::default();
    let obs = support::ObsBuilder::new(cfg.total_rounds - 4)
        .portal(0, Coord::new(4_000, 4_000), Owner::Res, 100)
        .portal(1, Coord::new(60_000, 4_000), Owner::Res, 100)
        .portal(2, Coord::new(4_000, 8_000), Owner::Enl, 100)
        .portal(3, Coord::new(4_000, 16_000), Owner::Enl, 100)
        .portal(4, Coord::new(65_000, 8_000), Owner::Neutral, 0)
        .agent(0, Coord::new(4_000, 4_000), 20)
        .agent_keys(0, 3, 1)
        .agent(1, Coord::new(65_000, 4_000), 20)
        .build();
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2317,
        R2Stream::BuilderDisruptor,
        &cfg,
        Default::default(),
    );
    let requests = [
        (
            AgentId(0),
            vec![
                TaskRequest::one_shot(
                    Task::Link {
                        source: PortalId(2),
                        target: PortalId(3),
                    },
                    1000,
                ),
                TaskRequest::coordinated(
                    Task::Attack {
                        portal: PortalId(0),
                    },
                    900,
                    2,
                ),
            ],
        ),
        (
            AgentId(1),
            vec![
                TaskRequest::coordinated(
                    Task::Attack {
                        portal: PortalId(0),
                    },
                    800,
                    2,
                ),
                TaskRequest::one_shot(
                    Task::Capture {
                        portal: PortalId(4),
                    },
                    10,
                ),
            ],
        ),
    ]
    .into_iter()
    .collect();
    core.allocate_and_execute(&obs, &requests);
    assert_eq!(
        core.decision_trace().unwrap().agents[1].grant,
        Some(Task::Capture {
            portal: PortalId(4)
        }),
        "a solo late attacker cannot execute the two-agent promise"
    );
}

#[test]
fn recharge_must_buy_a_settled_income_round_under_the_observed_burst() {
    let mut world = World::built_field();
    world.step(vec![], vec![]);
    world.step(
        vec![],
        vec![AgentSubmission::new(
            AgentId(3),
            vec![Command::Move {
                destination: Coord::new(4_000, 4_000),
            }],
        )],
    );
    for _ in 0..2 {
        world.step(
            vec![],
            [3, 5]
                .into_iter()
                .map(|id| AgentSubmission::new(AgentId(id), vec![Command::Attack]))
                .collect(),
        );
    }
    let mut policy = BuilderDefender::new(
        Faction::Enl,
        0x2318,
        world.state.config(),
        BuilderDefenderParams {
            defense_task_cap: 1,
            ..Default::default()
        },
    );
    let commands = policy.submit_round(&world.observation());
    assert!(
        commands
            .iter()
            .flat_map(|s| &s.commands)
            .all(|c| !matches!(c, Command::Recharge { .. })),
        "20 + 15 HP cannot survive the observed 40-damage round"
    );
}

#[test]
fn a_distant_unusable_key_does_not_suppress_a_local_two_round_completion() {
    let positions = [
        Coord::new(4_000, 4_000),
        Coord::new(16_000, 4_000),
        Coord::new(4_000, 16_000),
    ];
    let mut world = World::new(&positions, 3);
    world.step(
        (0..3)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![
                        Command::Capture {
                            portal: PortalId(id),
                        },
                        Command::Hack {
                            portal: PortalId(id),
                        },
                    ],
                )
            })
            .collect(),
        vec![],
    );
    world.step(
        (0..2)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![
                        Command::Move {
                            destination: positions[2],
                        },
                        Command::Link {
                            source_portal: PortalId(2),
                            target_portal: PortalId(id),
                        },
                    ],
                )
            })
            .collect(),
        vec![],
    );
    world.walk(AgentId(2), positions[0]);
    world.step(
        vec![AgentSubmission::new(
            AgentId(2),
            vec![Command::Hack {
                portal: PortalId(0),
            }],
        )],
        vec![],
    );
    world.walk(AgentId(2), Coord::new(90_000, 90_000));
    world.walk(AgentId(1), Coord::new(90_000, 90_000));
    world.walk(AgentId(0), positions[1]);
    while world.state.round() < world.state.config().total_rounds - 1 {
        world.step(vec![], vec![]);
    }
    let mut policy = GreedyCoordinated::new(
        Faction::Enl,
        0x2319,
        world.state.config(),
        Default::default(),
    );
    let commands = policy.submit_round(&world.observation());
    assert!(
        commands.iter().any(|s| s.agent_id == AgentId(0)
            && s.commands.contains(&Command::Hack {
                portal: PortalId(1)
            })),
        "local key acquisition can beat an unusable remote inventory: {commands:?}"
    );
    world.step(commands, vec![]);
    let commands = policy.submit_round(&world.observation());
    world.step(commands, vec![]);
    assert_eq!(world.state.link_graph().fields().len(), 1);
}

#[test]
fn the_production_blocker_denies_an_otherwise_legal_enemy_link() {
    fn prepare() -> World {
        let positions = [
            Coord::new(4_000, 20_000),
            Coord::new(60_000, 20_000),
            Coord::new(4_000, 80_000),
            Coord::new(20_000, 10_000),
            Coord::new(20_000, 30_000),
        ];
        let mut world = World::new(&positions, 3);
        world.step(
            vec![],
            vec![
                AgentSubmission::new(
                    AgentId(3),
                    vec![
                        Command::Capture {
                            portal: PortalId(1),
                        },
                        Command::Hack {
                            portal: PortalId(1),
                        },
                    ],
                ),
                AgentSubmission::new(
                    AgentId(4),
                    vec![Command::Capture {
                        portal: PortalId(2),
                    }],
                ),
            ],
        );
        world.walk(AgentId(0), positions[3]);
        world.walk(AgentId(1), positions[4]);
        world.step(
            vec![
                AgentSubmission::new(
                    AgentId(0),
                    vec![Command::Capture {
                        portal: PortalId(3),
                    }],
                ),
                AgentSubmission::new(
                    AgentId(1),
                    vec![
                        Command::Capture {
                            portal: PortalId(4),
                        },
                        Command::Hack {
                            portal: PortalId(4),
                        },
                    ],
                ),
            ],
            vec![],
        );
        while world.state.agent(AgentId(3)).unwrap().position != positions[0] {
            let destination = arena_bots::common::step_toward(
                world.state.agent(AgentId(3)).unwrap().position,
                positions[0],
                world.state.config().move_budget_cm,
            );
            world.step(
                vec![],
                vec![AgentSubmission::new(
                    AgentId(3),
                    vec![Command::Move { destination }],
                )],
            );
        }
        world.step(
            vec![],
            vec![AgentSubmission::new(
                AgentId(3),
                vec![Command::Capture {
                    portal: PortalId(0),
                }],
            )],
        );
        world
    }
    let mut blocked = prepare();
    let mut open = prepare();
    let mut policy = arena_bots::r2::blocker_builder::BlockerBuilder::new(
        Faction::Enl,
        0x2320,
        blocked.state.config(),
        Default::default(),
    );
    let commands = policy.submit_round(&blocked.observation());
    assert!(commands.iter().any(
        |s| s.commands.iter().any(|c| matches!(c, Command::Move { .. }))
            && s.commands.contains(&Command::Link {
                source_portal: PortalId(3),
                target_portal: PortalId(4)
            })
    ));
    blocked.step(commands, vec![]);
    open.step(vec![], vec![]);
    let enemy_link = || {
        vec![AgentSubmission::new(
            AgentId(3),
            vec![Command::Link {
                source_portal: PortalId(0),
                target_portal: PortalId(1),
            }],
        )]
    };
    open.step(vec![], enemy_link());
    let report = blocked.resolve(vec![], enemy_link(), false);
    assert!(
        report
            .execution_order
            .iter()
            .flat_map(|s| &s.outcomes)
            .any(|outcome| matches!(
                outcome,
                CommandOutcome::Drop(arena_core::resolve::DropReason::LinkIntersectsLink { .. })
            )),
        "the crossing link is denied by the actual resolver"
    );
    assert!(open.state.link_graph().links().len() > 0);
    assert!(
        blocked
            .state
            .link_graph()
            .links()
            .values()
            .all(|link| link.faction == Faction::Enl)
    );
}

#[test]
fn allocation_releases_a_strike_whose_remaining_team_loses_its_net_value() {
    let cfg = GameConfig::default();
    let mut builder = support::ObsBuilder::new(cfg.total_rounds - 4)
        .portal(0, Coord::new(4_000, 4_000), Owner::Res, 100)
        .portal(1, Coord::new(60_000, 4_000), Owner::Res, 100)
        .portal(2, Coord::new(4_000, 60_000), Owner::Res, 100)
        .field([0, 1, 2], Faction::Res, 100)
        .portal(3, Coord::new(6_000, 6_000), Owner::Neutral, 0)
        .portal(4, Coord::new(6_010, 6_000), Owner::Neutral, 0)
        .portal(5, Coord::new(6_000, 6_008), Owner::Neutral, 0);
    for id in 0..5 {
        builder = builder.agent(id, Coord::new(4_000, 4_000), 20);
    }
    let obs = builder.build();
    let mut params = CoreParams::default();
    params.value.capture_gain_share_permille = 0;
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2321,
        R2Stream::BuilderDisruptor,
        &cfg,
        params,
    );
    core.refresh(&obs);
    assert_eq!(core.economy(&obs).opportunity_per_round, 80);
    let mut requests: std::collections::BTreeMap<_, _> = (0..5)
        .map(|id| {
            (
                AgentId(id),
                vec![TaskRequest::coordinated(
                    Task::Attack {
                        portal: PortalId(0),
                    },
                    20,
                    5,
                )],
            )
        })
        .collect();
    requests
        .get_mut(&AgentId(0))
        .unwrap()
        .push(TaskRequest::one_shot(
            Task::Capture {
                portal: PortalId(3),
            },
            1000,
        ));
    core.allocate_and_execute(&obs, &requests);
    assert!(
        core.decision_trace()
            .unwrap()
            .agents
            .iter()
            .all(|a| !matches!(a.grant, Some(Task::Attack { .. }))),
        "four remaining attackers can finish, but their delayed denial no longer pays its opportunity cost"
    );
}

#[test]
fn simultaneously_granted_links_reserve_geometry_and_fall_back_to_a_compatible_edge() {
    let positions = [
        Coord::new(4_000, 4_000),
        Coord::new(16_000, 16_000),
        Coord::new(4_000, 16_000),
        Coord::new(16_000, 4_000),
    ];
    let mut world = World::new(&positions, 4);
    world.step(
        (0..4)
            .map(|id| {
                AgentSubmission::new(
                    AgentId(id),
                    vec![
                        Command::Capture {
                            portal: PortalId(id),
                        },
                        Command::Hack {
                            portal: PortalId(id),
                        },
                    ],
                )
            })
            .collect(),
        vec![],
    );
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2322,
        R2Stream::GreedyCoordinated,
        world.state.config(),
        Default::default(),
    );
    let requests = [
        (
            AgentId(0),
            vec![TaskRequest::one_shot(
                Task::Link {
                    source: PortalId(1),
                    target: PortalId(0),
                },
                100,
            )],
        ),
        (
            AgentId(2),
            vec![
                TaskRequest::one_shot(
                    Task::Link {
                        source: PortalId(3),
                        target: PortalId(2),
                    },
                    90,
                ),
                TaskRequest::one_shot(
                    Task::Link {
                        source: PortalId(0),
                        target: PortalId(2),
                    },
                    80,
                ),
            ],
        ),
    ]
    .into_iter()
    .collect();
    let commands = core.allocate_and_execute(&world.observation(), &requests);
    assert_eq!(
        core.decision_trace().unwrap().agents[2].grant,
        Some(Task::Link {
            source: PortalId(0),
            target: PortalId(2)
        }),
        "the other diagonal cannot coexist with the first reserved link"
    );
    world.step(
        arena_bots::r2::greedy_coordinated::submissions(&world.observation(), commands),
        vec![],
    );
    assert_eq!(world.state.link_graph().links().len(), 2);
}

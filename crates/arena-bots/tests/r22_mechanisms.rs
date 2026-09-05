//! r2.2 regressions exercise production decisions and real rule settlement.
mod support;

use arena_bots::r2::{
    R2Policy,
    build::{BuilderCore, CoreParams},
    builder_defender::{BuilderDefender, BuilderDefenderParams, defense_overlay},
    builder_disruptor::{BuilderDisruptor, BuilderDisruptorParams},
    greedy_coordinated::GreedyCoordinated,
    logistics_builder::{LogisticsBuilder, LogisticsBuilderParams},
    rng::R2Stream,
    tasks::{Task, TaskBoard, TaskRequest, allocate},
};
use arena_core::{
    config::GameConfig,
    harness::AgentSubmission,
    map::MapDefinition,
    observe::Observation,
    resolve::{CommandEffect, CommandOutcome, Resolver, RoundReport, SubmittedCommandStream},
    spawn::validate_initial_spawn_set,
    state::GameState,
    types::{AgentId, Command, Coord, Faction, Owner, Portal, PortalId},
};

struct Fixture {
    state: GameState,
    resolver: Resolver,
    map: MapDefinition,
}

fn siege_observation(near_agent: bool) -> Observation {
    let mut builder = support::ObsBuilder::new(10)
        .portal(0, Coord::new(4_000, 4_000), Owner::Enl, 80)
        .portal(1, Coord::new(16_000, 4_000), Owner::Enl, 100)
        .portal(2, Coord::new(4_000, 16_000), Owner::Enl, 100)
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
        .agent(1, Coord::new(40_000, 40_000), 20);
    if near_agent {
        builder = builder.agent(0, Coord::new(4_000, 4_000), 20);
    }
    builder.build()
}

#[test]
fn defense_deadlines_measure_only_settled_rounds() {
    let cfg = GameConfig::default();
    let mut obs = siege_observation(false);
    obs.own_agents[0].position = Coord::new(65_000, 65_000);
    let core = BuilderCore::new(
        Faction::Enl,
        0x2202,
        R2Stream::BuilderDefender,
        &cfg,
        CoreParams::default(),
    );
    let requests = defense_overlay(&core, &obs, 1);
    let grants = allocate(&mut TaskBoard::new(obs.round), &[AgentId(1)], &requests);
    assert!(
        grants.is_empty(),
        "one attack in the one settled round implies four rounds to collapse; five Moves cannot beat it even with same-round Recharge: {grants:?}"
    );
}

#[test]
fn defense_cannot_assign_a_fatal_march_when_the_nearest_agent_takes_other_work() {
    let cfg = GameConfig::default();
    let mut obs = siege_observation(true);
    obs.portals[0].hp = 40;
    let core = BuilderCore::new(
        Faction::Enl,
        0x2202,
        R2Stream::BuilderDefender,
        &cfg,
        CoreParams::default(),
    );
    let mut requests = defense_overlay(&core, &obs, 1);
    assert!(
        requests[&AgentId(0)]
            .iter()
            .any(|r| matches!(r.task, Task::Recharge { .. }))
    );
    requests
        .entry(AgentId(0))
        .or_default()
        .push(TaskRequest::one_shot(
            Task::KeyFarm {
                portal: PortalId(1),
            },
            i64::MAX,
        ));
    let grants = allocate(
        &mut TaskBoard::new(obs.round),
        &[AgentId(0), AgentId(1)],
        &requests,
    );
    assert!(
        !grants.contains_key(&AgentId(1)),
        "the other agent's deadline cannot be borrowed: {grants:?}"
    );
}

impl Fixture {
    fn triangle() -> Self {
        Self::triangle_with_budget(20_000)
    }

    fn triangle_with_budget(move_budget_cm: u32) -> Self {
        Self::triangle_with_right_vertex(move_budget_cm, 16_000)
    }

    fn triangle_with_right_vertex(move_budget_cm: u32, right_x: i32) -> Self {
        let cfg = GameConfig {
            portal_count: 3,
            portal_cluster_count: 3,
            map_width_cm: 40_000,
            map_height_cm: 40_000,
            portal_min_distance_cm: 4_000,
            hack_key_yield_max: 1,
            move_budget_cm,
            ..GameConfig::default()
        };
        let positions = [
            Coord::new(4_000, 4_000),
            Coord::new(right_x, 4_000),
            Coord::new(4_000, 16_000),
        ];
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
        let spawns = |roster: &[AgentId]| {
            validate_initial_spawn_set(
                roster,
                &roster.iter().copied().zip(positions).collect::<Vec<_>>(),
                &cfg,
            )
            .unwrap()
        };
        let state = GameState::new(cfg, &map, &spawns(&enl), &spawns(&res)).unwrap();
        Self {
            state,
            resolver: Resolver::with_seed(0x2200),
            map,
        }
    }

    fn walk(&mut self, agent: u32, destination: Coord) {
        while self.state.agent(AgentId(agent)).unwrap().position != destination {
            let position = self.state.agent(AgentId(agent)).unwrap().position;
            let step = arena_bots::common::step_toward(
                position,
                destination,
                self.state.config().move_budget_cm,
            );
            self.step(
                vec![AgentSubmission::new(
                    AgentId(agent),
                    vec![Command::Move { destination: step }],
                )],
                vec![],
            );
        }
    }

    fn open_triangle(move_budget_cm: u32) -> Self {
        Self::open_triangle_with_right_vertex(move_budget_cm, 16_000)
    }

    fn open_triangle_with_right_vertex(move_budget_cm: u32, right_x: i32) -> Self {
        let mut fixture = Self::triangle_with_right_vertex(move_budget_cm, right_x);
        fixture.step(
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
            fixture.walk(id, Coord::new(4_000, 16_000));
            fixture.step(
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
        fixture.walk(0, Coord::new(right_x, 4_000));
        fixture.step(
            vec![AgentSubmission::new(
                AgentId(0),
                vec![Command::Hack {
                    portal: PortalId(1),
                }],
            )],
            vec![],
        );
        fixture.walk(0, Coord::new(12_000, 4_000));
        fixture.walk(1, Coord::new(8_000, 4_000));
        assert!(fixture.state.link_graph().fields().is_empty());
        fixture
    }

    fn step(&mut self, enl: Vec<AgentSubmission>, res: Vec<AgentSubmission>) -> RoundReport {
        let streams: Vec<_> = [(Faction::Enl, enl), (Faction::Res, res)]
            .into_iter()
            .flat_map(|(faction, submissions)| {
                submissions
                    .into_iter()
                    .enumerate()
                    .map(move |(seq, sub)| (faction, seq as u32, sub))
            })
            .map(|(faction, receive_seq, sub)| SubmittedCommandStream {
                round: self.state.round(),
                faction,
                agent_id: sub.agent_id,
                received_at_ms: sub.bucket * self.state.config().priority_bucket_ms as u64,
                receive_seq,
                commands: sub.commands,
            })
            .collect();
        let report = self.resolver.run_round(&mut self.state, &streams);
        assert!(
            report
                .execution_order
                .iter()
                .flat_map(|s| &s.outcomes)
                .all(|o| matches!(o, CommandOutcome::Ok(_))),
            "fixture dropped: {report:?}"
        );
        self.state.advance_round();
        report
    }

    fn built_anchor() -> Self {
        let mut fixture = Self::triangle();
        fixture.step(
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
        let positions: Vec<_> = fixture.state.portals().iter().map(|p| p.position).collect();
        fixture.step(
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
        assert_eq!(fixture.state.link_graph().fields().len(), 1);
        fixture
    }
}

#[test]
fn remote_key_holder_recharges_without_walking_and_preserves_the_key() {
    let mut fixture = Fixture::built_anchor();
    // Non-key holders must need more than one Move: Move + Recharge now
    // legitimately ties a remote Recharge when the walk fits this round.
    let far = Coord::new(40_000, 40_000);
    for id in 0..3 {
        fixture.walk(id, far);
    }
    fixture.step(
        vec![],
        vec![AgentSubmission::new(AgentId(3), vec![Command::Attack])],
    );
    let obs = Observation::for_faction(&fixture.state, Faction::Enl);
    let holder = &obs.own_agents[2];
    let keys = holder.keys[&PortalId(0)];
    let supply = holder.supply;
    let mut defender = BuilderDefender::new(
        Faction::Enl,
        0x2201,
        fixture.state.config(),
        BuilderDefenderParams::default(),
    );
    let commands = defender.submit_round(&obs);
    assert!(
        commands.iter().any(|s| s.agent_id == holder.id
            && s.commands
                == vec![Command::Recharge {
                    portal: PortalId(0)
                }]),
        "the remote key holder must recharge now: {commands:?}"
    );
    let report = fixture.step(commands, vec![]);
    assert!(
        report
            .execution_order
            .iter()
            .flat_map(|s| &s.outcomes)
            .any(|o| matches!(
                o,
                CommandOutcome::Ok(CommandEffect::Recharged {
                    portal: PortalId(0),
                    hp_before: 80,
                    hp_after: 95
                })
            ))
    );
    assert_eq!(
        fixture.state.agent(holder.id).unwrap().keys[&PortalId(0)],
        keys
    );
    assert_eq!(fixture.state.agent(holder.id).unwrap().supply, supply - 1);
}

#[test]
fn a_full_anchor_releases_defense_capacity_instead_of_emitting_a_stationary_move() {
    let mut fixture = Fixture::built_anchor();
    fixture.step(
        vec![],
        vec![AgentSubmission::new(AgentId(3), vec![Command::Attack])],
    );
    for _ in 0..2 {
        fixture.step(
            vec![AgentSubmission::new(
                AgentId(2),
                vec![Command::Recharge {
                    portal: PortalId(0),
                }],
            )],
            vec![],
        );
    }
    let obs = Observation::for_faction(&fixture.state, Faction::Enl);
    assert_eq!(obs.portals[0].hp, 100);
    let mut defender = BuilderDefender::new(
        Faction::Enl,
        0x2201,
        fixture.state.config(),
        BuilderDefenderParams::default(),
    );
    let commands = defender.submit_round(&obs);
    for sub in &commands {
        let me = obs
            .own_agents
            .iter()
            .find(|a| a.id == sub.agent_id)
            .unwrap();
        assert!(
            sub.commands.iter().all(
                |c| !matches!(c, Command::Move { destination } if *destination == me.position)
            ),
            "full anchor must not consume a task with a stationary move: {commands:?}"
        );
    }
    let report = fixture.step(commands, vec![]);
    assert!(
        report
            .execution_order
            .iter()
            .flat_map(|s| &s.outcomes)
            .all(|o| !matches!(o, CommandOutcome::Ok(CommandEffect::Recharged { .. })))
    );
}

#[test]
fn defense_deadline_includes_integer_movement_at_the_operation_boundary() {
    let mut fixture = Fixture::built_anchor();
    fixture.walk(1, Coord::new(21_677, 21_677));
    for _ in 0..2 {
        fixture.step(
            vec![],
            vec![AgentSubmission::new(AgentId(3), vec![Command::Attack])],
        );
    }
    let mut obs = Observation::for_faction(&fixture.state, Faction::Enl);
    obs.own_agents.retain(|agent| agent.id == AgentId(1));
    assert_eq!(obs.portals[0].hp, 60);
    assert!(!obs.own_agents[0].keys.contains_key(&PortalId(0)));
    let mut params = BuilderDefenderParams::default();
    params.core.value.threat_window_rounds = 1;
    let mut policy = BuilderDefender::new(Faction::Enl, 0x2206, fixture.state.config(), params);
    let commands = policy.submit_round(&obs);
    assert!(
        policy
            .decision_trace()
            .unwrap()
            .agents
            .iter()
            .any(|agent| matches!(
                agent.grant,
                Some(Task::Recharge {
                    portal: PortalId(0)
                })
            )),
        "two integer Moves with Recharge on arrival beat collapse in three rounds: {commands:?}"
    );
    assert_eq!(
        commands[0].commands,
        vec![Command::Move {
            destination: Coord::new(7_536, 7_536)
        }]
    );
    let attack = || vec![AgentSubmission::new(AgentId(3), vec![Command::Attack])];
    fixture.step(commands, attack());
    assert!(!arena_bots::common::in_range(
        fixture.state.agent(AgentId(1)).unwrap().position,
        obs.portals[0].position,
        fixture.state.config().portal_operation_range_cm
    ));
    let mut next = Observation::for_faction(&fixture.state, Faction::Enl);
    next.own_agents.retain(|agent| agent.id == AgentId(1));
    let commands = policy.submit_round(&next);
    assert_eq!(
        commands[0]
            .commands
            .iter()
            .filter(|c| !matches!(c, Command::Hack { .. }))
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            Command::Move {
                destination: Coord::new(4_000, 4_000)
            },
            Command::Recharge {
                portal: PortalId(0)
            },
        ]
    );
    fixture.step(commands, attack());
    assert_eq!(fixture.state.portal(PortalId(0)).unwrap().hp, 35);
    assert_eq!(fixture.state.link_graph().fields().len(), 1);
}

#[test]
fn logistics_executes_the_reverse_direct_link_used_by_its_delivery_comparison() {
    let mut fixture = Fixture::open_triangle(1_000);
    fixture.walk(2, Coord::new(4_000, 4_000));
    fixture.step(
        vec![AgentSubmission::new(
            AgentId(2),
            vec![Command::Hack {
                portal: PortalId(0),
            }],
        )],
        vec![],
    );
    fixture.walk(2, Coord::new(16_000, 4_000));
    let mut policy = LogisticsBuilder::new(
        Faction::Enl,
        0x2203,
        fixture.state.config(),
        LogisticsBuilderParams::default(),
    );
    policy.choose_spawns(
        fixture.state.config(),
        &fixture.map,
        &[AgentId(0), AgentId(1), AgentId(2)],
    );
    let commands = policy.submit_round(&Observation::for_faction(&fixture.state, Faction::Enl));
    assert!(
        commands.iter().any(|sub| sub.agent_id == AgentId(2)
            && sub.commands.contains(&Command::Link {
                source_portal: PortalId(1),
                target_portal: PortalId(0)
            })),
        "the reverse direct option must actually execute: {commands:?}"
    );
    assert_eq!(
        commands
            .iter()
            .flat_map(|sub| &sub.commands)
            .filter(|c| matches!(c, Command::Link { .. }))
            .count(),
        1,
        "the two directions reserve one undirected edge"
    );
    let report = fixture.step(commands, vec![]);
    assert!(report.execution_order.iter().flat_map(|sub| &sub.outcomes).any(|outcome|
        matches!(outcome, CommandOutcome::Ok(CommandEffect::Linked { fields, .. }) if fields.len() == 1)));
}

#[test]
fn an_accepted_delivery_keeps_the_receiver_on_the_predicted_source_path() {
    let mut fixture = Fixture::open_triangle_with_right_vertex(1_000, 20_000);
    fixture.walk(1, Coord::new(4_000, 4_000));
    fixture.step(
        vec![AgentSubmission::new(
            AgentId(1),
            vec![Command::Hack {
                portal: PortalId(0),
            }],
        )],
        vec![],
    );
    fixture.walk(1, Coord::new(10_000, 4_000));
    let mut policy = LogisticsBuilder::new(
        Faction::Enl,
        0x2203,
        fixture.state.config(),
        LogisticsBuilderParams::default(),
    );
    policy.choose_spawns(
        fixture.state.config(),
        &fixture.map,
        &[AgentId(0), AgentId(1), AgentId(2)],
    );
    for turn in 0..2 {
        let commands = policy.submit_round(&Observation::for_faction(&fixture.state, Faction::Enl));
        if turn == 0 {
            let comparison = policy
                .delivery_comparisons()
                .iter()
                .find(|c| {
                    c.decision == arena_bots::r2::logistics_builder::DeliveryDecision::Accepted
                })
                .unwrap();
            assert_eq!(comparison.direct_delay, Some(2));
            assert_eq!(comparison.delivery_delay, Some(0));
            assert_eq!(
                comparison.decision,
                arena_bots::r2::logistics_builder::DeliveryDecision::Accepted
            );
            assert!(
                commands.iter().any(|sub| sub.agent_id == AgentId(1)
                    && sub.commands.contains(&Command::Move {
                        destination: Coord::new(9_000, 4_000)
                    })),
                "the receiver must approach the delivery source, not the opposite endpoint: {commands:?}"
            );
        }
        fixture.step(commands, vec![]);
        assert_eq!(fixture.state.link_graph().fields().len(), 1);
    }
}

#[test]
fn logistics_moves_then_closes_the_field_instead_of_delaying_it_with_a_transfer() {
    let mut fixture = Fixture::open_triangle(20_000);
    let mut logistics = LogisticsBuilder::new(
        Faction::Enl,
        0x2203,
        fixture.state.config(),
        LogisticsBuilderParams::default(),
    );
    // Seed the production plan from the original neutral map; the setup
    // then completes two of its edges through scripted legal actions.
    logistics.choose_spawns(
        fixture.state.config(),
        &fixture.map,
        &[AgentId(0), AgentId(1), AgentId(2)],
    );
    let obs = Observation::for_faction(&fixture.state, Faction::Enl);
    let commands = logistics.submit_round(&obs);
    assert!(
        commands
            .iter()
            .flat_map(|s| &s.commands)
            .all(|c| !matches!(c, Command::Transfer { .. })),
        "Move + Link completes earlier than handover: {commands:?}"
    );
    let report = fixture.step(commands, vec![]);
    assert!(report.execution_order.iter().flat_map(|s| &s.outcomes)
        .any(|o| matches!(o, CommandOutcome::Ok(CommandEffect::Linked { fields, .. }) if fields.len() == 1)),
        "the production policy must close the field in this round: {report:?}");
}

#[test]
fn logistics_rejects_a_handover_that_only_ties_direct_field_completion() {
    let mut fixture = Fixture::open_triangle(3_000);
    let mut logistics = LogisticsBuilder::new(
        Faction::Enl,
        0x2203,
        fixture.state.config(),
        LogisticsBuilderParams::default(),
    );
    logistics.choose_spawns(
        fixture.state.config(),
        &fixture.map,
        &[AgentId(0), AgentId(1), AgentId(2)],
    );
    for _ in 0..2 {
        let commands =
            logistics.submit_round(&Observation::for_faction(&fixture.state, Faction::Enl));
        assert!(
            commands
                .iter()
                .flat_map(|s| &s.commands)
                .all(|c| !matches!(c, Command::Transfer { .. })),
            "direct Move + Link and ordered delivery both finish this round: {commands:?}"
        );
        fixture.step(commands, vec![]);
        assert_eq!(fixture.state.link_graph().fields().len(), 1);
    }
}

#[test]
fn a_beneficial_key_delivery_advances_the_complete_field_and_spends_no_unneeded_supply() {
    // A slow-movement rules fixture gives the relay a genuine advantage:
    // direct needs three Moves, while an ordered delivery closes this round.
    let mut relay = Fixture::open_triangle(1_000);
    let mut direct = Fixture::open_triangle(1_000);
    let mut with_delivery = LogisticsBuilder::new(
        Faction::Enl,
        0x2203,
        relay.state.config(),
        LogisticsBuilderParams::default(),
    );
    let mut without_delivery = LogisticsBuilder::new(
        Faction::Enl,
        0x2203,
        direct.state.config(),
        LogisticsBuilderParams {
            max_transfers_per_round: 0,
            ..LogisticsBuilderParams::default()
        },
    );
    for policy in [&mut with_delivery, &mut without_delivery] {
        policy.choose_spawns(
            relay.state.config(),
            &relay.map,
            &[AgentId(0), AgentId(1), AgentId(2)],
        );
    }
    for turn in 0..3 {
        let commands =
            with_delivery.submit_round(&Observation::for_faction(&relay.state, Faction::Enl));
        if turn == 0 {
            assert!(commands.iter().flat_map(|s| &s.commands).any(|c|
                matches!(c, Command::Transfer { supply: 0, keys, .. } if keys.get(&PortalId(1)) == Some(&1))),
                "deliver the useful key without dumping Supply: {commands:?}");
            let comparison = with_delivery
                .delivery_comparisons()
                .iter()
                .find(|c| {
                    c.decision == arena_bots::r2::logistics_builder::DeliveryDecision::Accepted
                })
                .unwrap();
            assert_eq!(comparison.direct_delay, Some(2));
            assert_eq!(comparison.delivery_delay, Some(0));
        }
        relay.step(commands, vec![]);
        let commands =
            without_delivery.submit_round(&Observation::for_faction(&direct.state, Faction::Enl));
        assert!(
            commands
                .iter()
                .flat_map(|s| &s.commands)
                .all(|c| !matches!(c, Command::Transfer { .. }))
        );
        direct.step(commands, vec![]);
        assert_eq!(relay.state.link_graph().fields().len(), 1);
        assert_eq!(
            direct.state.link_graph().fields().len(),
            usize::from(turn >= 2)
        );
    }
    assert_eq!(
        relay.state.score(Faction::Enl) - direct.state.score(Faction::Enl),
        288_000_000
    );
}

#[test]
fn zero_attack_share_disables_the_overlay_without_changing_policy_identity() {
    let fixture = Fixture::built_anchor();
    let obs = Observation::for_faction(&fixture.state, Faction::Res);
    for (share, enabled) in [(1_000, true), (0, false)] {
        let mut policy = BuilderDisruptor::new(
            Faction::Res,
            0x2204,
            fixture.state.config(),
            BuilderDisruptorParams {
                attack_share_permille: share,
                ..BuilderDisruptorParams::default()
            },
        );
        let commands = policy.submit_round(&obs);
        assert_eq!(
            commands
                .iter()
                .flat_map(|s| &s.commands)
                .any(|c| matches!(c, Command::Attack)),
            enabled,
            "share={share}: {commands:?}"
        );
    }
}

#[test]
fn second_plans_complete_and_reinforcements_keep_contributing_after_replanning() {
    let cfg = GameConfig {
        total_rounds: 30,
        reinforcement_interval_rounds: 5,
        max_link_distance_cm: 20_000,
        ..support::fixture_cfg()
    };
    let map = support::fixture_map();
    let mut policy = GreedyCoordinated::new(
        Faction::Enl,
        0x2205,
        &cfg,
        CoreParams {
            near_equal_permille: 0,
            spawn_jitter_cm: 0,
            replan_retry_rounds: 1,
            ..CoreParams::default()
        },
    );
    let (enl_roster, _) = GameState::initial_roster(&cfg);
    let spawns = policy.choose_spawns(&cfg, &map, &enl_roster);
    let enl = validate_initial_spawn_set(&enl_roster, &spawns, &cfg).unwrap();
    let res = support::portal_spawns(&cfg, &map, Faction::Res);
    let state = GameState::new(cfg, &map, &enl, &res).unwrap();
    let mut fixture = Fixture {
        state,
        resolver: Resolver::with_seed(0x2206),
        map,
    };
    let mut secondary_plans = std::collections::BTreeSet::new();
    let mut secondary_completed = false;
    let mut reinforcement_action_rounds = std::collections::BTreeSet::new();
    for _ in 0..cfg.total_rounds {
        let obs = Observation::for_faction(&fixture.state, Faction::Enl);
        let commands = policy.submit_round(&obs);
        let trace = policy.decision_trace().unwrap();
        if let Some(second) = trace.second {
            assert!(
                !obs.fields
                    .iter()
                    .any(|f| [f.id.a, f.id.b, f.id.c] == second.vertices),
                "completed second plans must be retired before allocation"
            );
            secondary_plans.insert(second.vertices);
        }
        let report = fixture.step(commands, vec![]);
        for stream in &report.execution_order {
            for outcome in &stream.outcomes {
                if let CommandOutcome::Ok(CommandEffect::Linked { fields, .. }) = outcome {
                    secondary_completed |= fields
                        .iter()
                        .any(|f| secondary_plans.contains(&[f.a, f.b, f.c]));
                }
                if stream.faction == Faction::Enl
                    && stream.agent_id.0 >= 6
                    && matches!(
                        outcome,
                        CommandOutcome::Ok(
                            CommandEffect::Captured { .. }
                                | CommandEffect::Linked { .. }
                                | CommandEffect::Hacked { .. }
                        )
                    )
                {
                    reinforcement_action_rounds.insert(obs.round);
                }
            }
        }
    }
    assert!(
        secondary_completed,
        "an observed second plan must actually close a field"
    );
    assert!(
        reinforcement_action_rounds.len() >= 2,
        "reinforcements must make sustained non-Move contributions"
    );
}

#[test]
fn an_invalid_second_plan_is_retired_before_more_tasks_are_assigned() {
    let cfg = support::fixture_cfg();
    let map = support::fixture_map();
    let mut policy = GreedyCoordinated::new(
        Faction::Enl,
        0x2207,
        &cfg,
        CoreParams {
            near_equal_permille: 0,
            spawn_jitter_cm: 0,
            replan_retry_rounds: 1,
            ..CoreParams::default()
        },
    );
    let mut obs = support::ObsBuilder::new(1);
    for portal in map.portals() {
        obs = obs.portal(portal.id.0, portal.position, Owner::Neutral, 0);
    }
    let mut obs = obs
        .agent(0, Coord::new(4_000, 4_000), 20)
        .agent(1, Coord::new(16_000, 4_000), 20)
        .agent(2, Coord::new(4_000, 16_000), 20)
        .build();
    policy.submit_round(&obs);
    let second = policy
        .decision_trace()
        .unwrap()
        .second
        .expect("fixture has two plans");
    let primary = policy.decision_trace().unwrap().active.unwrap();
    let invalid_vertex = *second
        .vertices
        .iter()
        .find(|v| !primary.vertices.contains(v))
        .unwrap();
    obs.round += 1;
    obs.portals[invalid_vertex.0 as usize].owner = Owner::Res;
    obs.portals[invalid_vertex.0 as usize].hp = 100;
    policy.submit_round(&obs);
    let trace = policy.decision_trace().unwrap();
    assert_ne!(trace.second, Some(second));
    assert!(trace.agents.iter().all(|a| a.grant
        != Some(Task::Capture {
            portal: invalid_vertex
        })));
}

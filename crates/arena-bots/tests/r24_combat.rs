//! Combat decisions must compete on achievable income, including while behind.
mod support;

use arena_bots::r2::{
    R2Policy,
    adaptive_hybrid_r2::{AdaptiveHybridParams, AdaptiveHybridR2},
    tasks::Task,
};
use arena_core::{
    config::GameConfig,
    types::{Command, Coord, Faction, Owner},
};

#[test]
fn adaptive_considers_a_profitable_midgame_strike_while_behind_and_building() {
    let cfg = GameConfig::default();
    let mut obs = support::ObsBuilder::new(200)
        .portal(0, Coord::new(4_000, 4_000), Owner::Res, 20)
        .portal(1, Coord::new(16_000, 4_000), Owner::Res, 100)
        .portal(2, Coord::new(4_000, 16_000), Owner::Res, 100)
        .link(0, 1, Faction::Res)
        .link(0, 2, Faction::Res)
        .link(1, 2, Faction::Res)
        .field([0, 1, 2], Faction::Res, 144_000_000)
        .portal(3, Coord::new(50_000, 50_000), Owner::Neutral, 0)
        .portal(4, Coord::new(54_000, 50_000), Owner::Neutral, 0)
        .portal(5, Coord::new(50_000, 54_000), Owner::Neutral, 0)
        .agent(0, Coord::new(4_000, 4_000), 20)
        .agent(1, Coord::new(50_000, 50_000), 20)
        .build();
    obs.scores.insert(Faction::Enl, 1);
    obs.scores.insert(Faction::Res, 100);
    let mut policy =
        AdaptiveHybridR2::new(Faction::Enl, 0x2401, &cfg, AdaptiveHybridParams::default());
    let commands = policy.submit_round(&obs);
    let trace = policy.decision_trace().unwrap();
    assert!(
        trace.active.is_some(),
        "the alternative construction plan must remain available"
    );
    assert!(
        trace
            .agents
            .iter()
            .any(|agent| matches!(agent.grant, Some(Task::Attack { .. }))),
        "a losing score must not prevent evaluating a profitable strike: {trace:?}"
    );
    assert!(
        commands
            .iter()
            .flat_map(|s| &s.commands)
            .any(|command| matches!(command, Command::Attack))
    );
}

struct CombatWorld {
    state: arena_core::state::GameState,
}

#[test]
fn the_last_round_still_allows_one_move_and_a_profitable_recharge() {
    use arena_bots::r2::builder_defender::{BuilderDefender, BuilderDefenderParams};
    use arena_core::{
        event::{Event, EventKind},
        types::{AgentId, PortalId},
    };
    let cfg = GameConfig {
        total_rounds: 499,
        ..GameConfig::default()
    };
    let obs = support::ObsBuilder::new(499)
        .portal(0, Coord::new(4_000, 4_000), Owner::Enl, 20)
        .portal(1, Coord::new(16_000, 4_000), Owner::Enl, 100)
        .portal(2, Coord::new(4_000, 16_000), Owner::Enl, 100)
        .link(0, 1, Faction::Enl)
        .link(0, 2, Faction::Enl)
        .link(1, 2, Faction::Enl)
        .field([0, 1, 2], Faction::Enl, 144_000_000)
        .agent(0, Coord::new(10_000, 4_000), 20)
        .event(Event {
            seq: 0,
            round: 498,
            kind: EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(3),
                portal: PortalId(0),
                damage: 20,
                hp_after: 20,
            },
        })
        .build();
    let mut policy =
        BuilderDefender::new(Faction::Enl, 0x2406, &cfg, BuilderDefenderParams::default());
    let commands = policy.submit_round(&obs);
    assert!(
        policy
            .decision_trace()
            .unwrap()
            .agents
            .iter()
            .any(|a| matches!(a.grant, Some(Task::Recharge { .. }))),
        "{:?}",
        policy.decision_trace()
    );
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
}

impl CombatWorld {
    fn built() -> Self {
        use arena_core::{
            harness::AgentSubmission,
            map::MapDefinition,
            spawn::validate_initial_spawn_set,
            state::GameState,
            types::{AgentId, Portal, PortalId},
        };
        let cfg = GameConfig {
            portal_count: 3,
            portal_cluster_count: 3,
            hack_key_yield_max: 1,
            ..GameConfig::default()
        };
        let positions = [
            Coord::new(4_000, 4_000),
            Coord::new(16_000, 4_000),
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
        let left = validate_initial_spawn_set(
            &enl,
            &enl.iter()
                .enumerate()
                .map(|(i, &id)| (id, positions[i]))
                .collect::<Vec<_>>(),
            &cfg,
        )
        .unwrap();
        let right = validate_initial_spawn_set(
            &res,
            &res.iter().map(|&id| (id, positions[0])).collect::<Vec<_>>(),
            &cfg,
        )
        .unwrap();
        let mut world = Self {
            state: GameState::new(cfg, &map, &left, &right).unwrap(),
        };
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
                    AgentSubmission::new(
                        AgentId(id),
                        vec![
                            Command::Move {
                                destination: positions[(id as usize + 1) % 3],
                            },
                            Command::Link {
                                source_portal: PortalId((id + 1) % 3),
                                target_portal: PortalId(id),
                            },
                        ],
                    )
                })
                .collect(),
            vec![],
        );
        assert_eq!(world.state.link_graph().fields().len(), 1);
        world
    }

    fn step(
        &mut self,
        enl: Vec<arena_core::harness::AgentSubmission>,
        res: Vec<arena_core::harness::AgentSubmission>,
    ) {
        use arena_core::resolve::{CommandOutcome, Resolver, SubmittedCommandStream};
        let streams: Vec<_> = [(Faction::Enl, enl), (Faction::Res, res)]
            .into_iter()
            .flat_map(|(faction, submissions)| {
                submissions
                    .into_iter()
                    .enumerate()
                    .map(move |(seq, sub)| (faction, seq, sub))
            })
            .map(|(faction, seq, sub)| SubmittedCommandStream {
                round: self.state.round(),
                faction,
                agent_id: sub.agent_id,
                received_at_ms: 0,
                receive_seq: seq as u32,
                commands: sub.commands,
            })
            .collect();
        let report = Resolver::with_seed(0x2400).run_round(&mut self.state, &streams);
        assert!(
            report
                .execution_order
                .iter()
                .flat_map(|s| &s.outcomes)
                .all(|outcome| matches!(outcome, CommandOutcome::Ok(_))),
            "{report:?}"
        );
        self.state.advance_round();
    }

    fn attacks(count: u32) -> Vec<arena_core::harness::AgentSubmission> {
        (3..3 + count)
            .map(|id| {
                arena_core::harness::AgentSubmission::new(
                    arena_core::types::AgentId(id),
                    vec![Command::Attack],
                )
            })
            .collect()
    }
}

#[test]
fn defender_moves_and_recharges_before_this_rounds_fatal_attack() {
    use arena_bots::r2::builder_defender::{BuilderDefender, BuilderDefenderParams};
    use arena_core::{observe::Observation, types::PortalId};
    let mut world = CombatWorld::built();
    for _ in 0..4 {
        world.step(vec![], CombatWorld::attacks(1));
    }
    let mut obs = Observation::for_faction(&world.state, Faction::Enl);
    // Isolate the prospective defender in this decision fixture. Agent 0
    // is one move away; the resulting stream is then settled in the world.
    obs.own_agents.retain(|a| a.id.0 == 0);
    obs.own_agents[0].keys.clear();
    let mut policy = BuilderDefender::new(
        Faction::Enl,
        0x2402,
        world.state.config(),
        BuilderDefenderParams::default(),
    );
    let submissions = policy.submit_round(&obs);
    assert!(submissions.iter().any(|s| matches!(s.commands.iter().filter(|c| !matches!(c, Command::Hack { .. })).cloned().collect::<Vec<_>>().as_slice(), [Command::Move { .. }, Command::Recharge { portal }] if *portal == PortalId(0))),
        "the defender can arrive and rescue the field this round: {submissions:?}");
    world.step(submissions, CombatWorld::attacks(1));
    assert_eq!(world.state.portal(PortalId(0)).unwrap().hp, 15);
    assert_eq!(world.state.link_graph().fields().len(), 1);
}

#[test]
fn two_defenders_can_jointly_save_a_field_that_one_recharge_cannot_save() {
    use arena_bots::r2::builder_defender::{BuilderDefender, BuilderDefenderParams};
    use arena_core::observe::Observation;
    let mut world = CombatWorld::built();
    for _ in 0..2 {
        world.step(vec![], CombatWorld::attacks(2));
    }
    let obs = Observation::for_faction(&world.state, Faction::Enl);
    let mut policy = BuilderDefender::new(
        Faction::Enl,
        0x2403,
        world.state.config(),
        BuilderDefenderParams::default(),
    );
    let submissions = policy.submit_round(&obs);
    let recharges = submissions
        .iter()
        .flat_map(|s| &s.commands)
        .filter(|c| matches!(c, Command::Recharge { .. }))
        .count();
    assert_eq!(
        recharges, 2,
        "two heals cross the survival threshold; one fails and a third adds no scoring round: {submissions:?}"
    );
    world.step(submissions, CombatWorld::attacks(2));
    assert_eq!(world.state.link_graph().fields().len(), 1);
}

#[test]
fn a_rescue_recruits_another_ready_defender_when_a_builder_wins_one_member() {
    use arena_bots::r2::{
        build::{BuilderCore, CoreParams},
        builder_defender::defense_overlay,
        rng::R2Stream,
        tasks::TaskRequest,
    };
    use arena_core::{
        observe::Observation,
        types::{AgentId, PortalId},
    };
    let mut world = CombatWorld::built();
    for _ in 0..2 {
        world.step(vec![], CombatWorld::attacks(2));
    }
    let obs = Observation::for_faction(&world.state, Faction::Enl);
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2404,
        R2Stream::BuilderDefender,
        world.state.config(),
        CoreParams {
            combat_auxiliaries: false,
            ..CoreParams::default()
        },
    );
    core.refresh(&obs);
    let mut requests = defense_overlay(&core, &obs, 2);
    requests
        .entry(AgentId(0))
        .or_default()
        .push(TaskRequest::one_shot(
            Task::KeyFarm {
                portal: PortalId(1),
            },
            i64::MAX,
        ));
    let trace = core.preview_allocation(&obs, &requests);
    assert_eq!(
        trace
            .agents
            .iter()
            .filter(|a| matches!(a.grant, Some(Task::Recharge { .. })))
            .count(),
        2
    );
    requests
        .entry(AgentId(1))
        .or_default()
        .push(TaskRequest::one_shot(
            Task::KeyFarm {
                portal: PortalId(2),
            },
            i64::MAX,
        ));
    let trace = core.preview_allocation(&obs, &requests);
    assert!(
        trace
            .agents
            .iter()
            .all(|a| !matches!(a.grant, Some(Task::Recharge { .. }))),
        "the remaining single defender cannot buy an income round: {trace:?}"
    );
}

#[test]
fn a_coordinated_rescue_cannot_revive_an_enemy_first_lethal_anchor() {
    use arena_bots::r2::builder_defender::{BuilderDefender, BuilderDefenderParams};
    use arena_core::observe::Observation;
    let mut world = CombatWorld::built();
    world.step(vec![], vec![]);
    for _ in 0..2 {
        world.step(vec![], CombatWorld::attacks(2));
    }
    let obs = Observation::for_faction(&world.state, Faction::Enl);
    assert_eq!(obs.round % 2, 0);
    let mut policy = BuilderDefender::new(
        Faction::Enl,
        0x2405,
        world.state.config(),
        BuilderDefenderParams::default(),
    );
    let submissions = policy.submit_round(&obs);
    assert!(
        submissions
            .iter()
            .flat_map(|s| &s.commands)
            .all(|c| !matches!(c, Command::Recharge { .. }))
    );
}

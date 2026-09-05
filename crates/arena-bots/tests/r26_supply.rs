//! Regression windows for independent replenishment and mobile remote defense.
mod support;

use arena_bots::r2::{
    build::{BuilderCore, CoreParams},
    rng::R2Stream,
    tasks::{Task, TaskRequest},
};
use arena_core::{
    config::GameConfig,
    observe::Observation,
    types::{AgentId, Command, Coord, Faction, Owner, PortalId},
};
use std::collections::BTreeMap;

fn observation(supply: u32) -> Observation {
    support::ObsBuilder::new(101)
        .portal(0, Coord::new(4_000, 4_000), Owner::Enl, 20)
        .portal(1, Coord::new(60_000, 40_000), Owner::Enl, 100)
        .portal(2, Coord::new(80_000, 40_000), Owner::Enl, 100)
        .portal(3, Coord::new(16_000, 4_000), Owner::Enl, 100)
        .portal(4, Coord::new(4_000, 16_000), Owner::Enl, 100)
        .link(0, 3, Faction::Enl)
        .link(0, 4, Faction::Enl)
        .link(3, 4, Faction::Enl)
        .field([0, 3, 4], Faction::Enl, 144_000_000)
        .agent(0, Coord::new(60_000, 40_000), supply)
        .agent_keys(0, 0, 1)
        .agent_keys(0, 2, 1)
        .event(arena_core::event::Event {
            seq: 0,
            round: 100,
            kind: arena_core::event::EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(3),
                portal: PortalId(0),
                damage: 20,
                hp_after: 20,
            },
        })
        .build()
}

fn core(cfg: &GameConfig, reserve: u32) -> BuilderCore {
    BuilderCore::new(
        Faction::Enl,
        0x2600,
        R2Stream::BuilderDefender,
        cfg,
        CoreParams {
            supply_reserve_recharges: reserve,
            ..CoreParams::default()
        },
    )
}

fn submit(core: &mut BuilderCore, obs: &Observation, task: Option<Task>) -> Vec<Command> {
    let requests = task
        .map(|task| (AgentId(0), vec![TaskRequest::one_shot(task, i64::MAX)]))
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    core.allocate_and_execute(obs, &requests)
        .remove(&AgentId(0))
        .map_or_else(Vec::new, |stream| stream.build())
}

#[test]
fn empty_inventory_can_hack_without_a_combat_or_build_grant() {
    let obs = observation(0);
    let mut core = core(&GameConfig::default(), 5);
    assert_eq!(
        submit(&mut core, &obs, None),
        vec![Command::Hack {
            portal: PortalId(1)
        }]
    );
    assert!(core.decision_trace().unwrap().agents[0].grant.is_none());
}

#[test]
fn link_only_stream_refills_after_using_its_existing_key() {
    let mut core = core(&GameConfig::default(), 5);
    assert_eq!(
        submit(
            &mut core,
            &observation(0),
            Some(Task::Link {
                source: PortalId(1),
                target: PortalId(2),
            })
        ),
        vec![
            Command::Link {
                source_portal: PortalId(1),
                target_portal: PortalId(2)
            },
            Command::Hack {
                portal: PortalId(1)
            }
        ]
    );
}

#[test]
fn remote_charge_uses_free_movement_to_replenish_before_stockout() {
    let mut obs = observation(5);
    obs.own_agents[0].position = Coord::new(45_000, 40_000);
    let mut core = core(&GameConfig::default(), 5);
    assert_eq!(
        submit(
            &mut core,
            &obs,
            Some(Task::Recharge {
                portal: PortalId(0)
            })
        ),
        vec![
            Command::Recharge {
                portal: PortalId(0)
            },
            Command::Move {
                destination: obs.portals[1].position
            },
            Command::Hack {
                portal: PortalId(1)
            }
        ]
    );
}

#[test]
fn low_supply_does_not_override_a_construction_route() {
    let mut obs = observation(0);
    obs.own_agents[0].position = Coord::new(45_000, 40_000);
    let task = Some(Task::Link {
        source: PortalId(2),
        target: PortalId(1),
    });
    obs.own_agents[0].keys.insert(PortalId(1), 1);
    let cfg = GameConfig::default();
    let old = submit(&mut core(&cfg, 0), &obs, task);
    let new = submit(&mut core(&cfg, 5), &obs, task);
    let moves = |commands: Vec<Command>| {
        commands
            .into_iter()
            .filter(|c| matches!(c, Command::Move { .. }))
            .collect::<Vec<_>>()
    };
    assert_eq!(moves(new), moves(old));
}

#[test]
fn local_defender_without_key_does_not_leave_its_anchor_to_harvest() {
    let mut obs = observation(5);
    obs.own_agents[0].position = obs.portals[0].position;
    obs.own_agents[0].keys.clear();
    let mut core = core(&GameConfig::default(), 5);
    // Exhaust local harvest first, then verify no diversion to a distant portal.
    core.ledger_mut()
        .record(obs.round, &GameConfig::default(), AgentId(0), PortalId(0));
    assert_eq!(
        submit(
            &mut core,
            &obs,
            Some(Task::Recharge {
                portal: PortalId(0)
            })
        ),
        vec![Command::Recharge {
            portal: PortalId(0)
        }]
    );
}

#[test]
fn replenishment_respects_cooldown_epoch_and_preview_purity() {
    let cfg = GameConfig::default();
    let mut obs = observation(0);
    obs.portals.truncate(2);
    let mut core = core(&cfg, 5);
    for n in 0..cfg.hack_limit_per_epoch {
        obs.round = 101 + n * cfg.hack_cooldown_rounds;
        core.preview_allocation(&obs, &BTreeMap::new());
        assert!(submit(&mut core, &obs, None).contains(&Command::Hack {
            portal: PortalId(1)
        }));
        // Cooldown can select a different remote supply point, but never this one.
        assert!(!submit(&mut core, &obs, None).contains(&Command::Hack {
            portal: PortalId(1)
        }));
    }
    obs.round += cfg.hack_cooldown_rounds;
    assert!(!submit(&mut core, &obs, None).contains(&Command::Hack {
        portal: PortalId(1)
    }));
    obs.round = 201;
    assert!(submit(&mut core, &obs, None).contains(&Command::Hack {
        portal: PortalId(1)
    }));
}

#[test]
fn disabled_replenishment_keeps_idle_and_remote_charge_streams_unchanged() {
    let mut obs = observation(0);
    let mut core = core(&GameConfig::default(), 0);
    assert!(submit(&mut core, &obs, None).is_empty());
    obs.own_agents[0].supply = 5;
    obs.own_agents[0].position = Coord::new(45_000, 40_000);
    assert_eq!(
        submit(
            &mut core,
            &obs,
            Some(Task::Recharge {
                portal: PortalId(0)
            })
        ),
        vec![Command::Recharge {
            portal: PortalId(0)
        }]
    );
}

#[test]
fn command_limit_keeps_charge_and_defers_replenishment() {
    let cfg = GameConfig {
        max_commands_per_agent_round: 1,
        ..GameConfig::default()
    };
    let mut obs = observation(5);
    obs.own_agents[0].position = Coord::new(45_000, 40_000);
    assert_eq!(
        submit(
            &mut core(&cfg, 5),
            &obs,
            Some(Task::Recharge {
                portal: PortalId(0)
            })
        ),
        vec![Command::Recharge {
            portal: PortalId(0)
        }]
    );
}

#[test]
fn old_parameter_records_disable_supply_support_and_new_records_round_trip() {
    let params = CoreParams {
        supply_reserve_recharges: 5,
        ..CoreParams::default()
    };
    assert_eq!(CoreParams::from_tag(&params.tag()), Some(params));
    let old = params.tag().replace(";supplyreserve=5", "");
    assert_eq!(
        CoreParams::from_tag(&old).unwrap().supply_reserve_recharges,
        0
    );
    assert!(
        CoreParams::from_tag(&params.tag().replace("supplyreserve=5", "supplyreserve=-1"))
            .is_none()
    );
    assert_eq!(
        arena_bots::r2::logistics_builder::LogisticsBuilderParams::default()
            .core
            .supply_reserve_recharges,
        0
    );
}

#[test]
fn independent_refill_does_not_require_combat_composition() {
    let cfg = GameConfig::default();
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x2600,
        R2Stream::BuilderDefender,
        &cfg,
        CoreParams {
            combat_auxiliaries: false,
            supply_reserve_recharges: 5,
            ..CoreParams::default()
        },
    );
    assert_eq!(
        submit(&mut core, &observation(0), None),
        vec![Command::Hack {
            portal: PortalId(1)
        }]
    );
}

#[test]
fn reserve_is_in_recharge_costs_and_does_not_trigger_at_healthy_inventory() {
    let cfg = GameConfig {
        recharge_cost_supply: 2,
        ..GameConfig::default()
    };
    assert!(submit(&mut core(&cfg, 5), &observation(11), None).is_empty());
    assert_eq!(
        submit(&mut core(&cfg, 5), &observation(10), None),
        vec![Command::Hack {
            portal: PortalId(1)
        }]
    );
}

#[test]
fn refill_does_not_start_a_multi_round_tour_without_a_hack_this_round() {
    let mut obs = observation(0);
    obs.own_agents[0].position = Coord::new(100_000, 100_000);
    let cfg = GameConfig::default();
    assert!(submit(&mut core(&cfg, 5), &obs, None).is_empty());
    obs.own_agents[0].supply = 5;
    assert_eq!(
        submit(
            &mut core(&cfg, 5),
            &obs,
            Some(Task::Recharge {
                portal: PortalId(0)
            })
        ),
        vec![Command::Recharge {
            portal: PortalId(0)
        }]
    );
}

#[test]
fn free_move_without_a_hack_slot_does_not_start_a_supply_detour() {
    let cfg = GameConfig {
        max_commands_per_agent_round: 2,
        ..GameConfig::default()
    };
    let mut obs = observation(5);
    obs.own_agents[0].position = Coord::new(45_000, 40_000);
    assert_eq!(
        submit(
            &mut core(&cfg, 5),
            &obs,
            Some(Task::Recharge {
                portal: PortalId(0)
            })
        ),
        vec![Command::Recharge {
            portal: PortalId(0)
        }]
    );
}

#[test]
fn attacker_does_not_leave_attack_range_when_local_hack_is_cooling_down() {
    let cfg = GameConfig::default();
    let mut obs = observation(0);
    for index in [0, 3, 4] {
        obs.portals[index].owner = Owner::Res;
    }
    for link in &mut obs.links {
        link.faction = Faction::Res;
    }
    obs.fields[0].faction = Faction::Res;
    obs.own_agents[0].position = obs.portals[0].position;
    let mut core = core(&cfg, 5);
    core.ledger_mut()
        .record(obs.round, &cfg, AgentId(0), PortalId(0));
    assert_eq!(
        submit(
            &mut core,
            &obs,
            Some(Task::Attack {
                portal: PortalId(0)
            })
        ),
        vec![Command::Attack]
    );
}

#[test]
fn transfer_rendezvous_is_not_augmented_even_when_supply_support_is_enabled() {
    let cfg = GameConfig::default();
    let mut obs = observation(0);
    let mut receiver = obs.own_agents[0].clone();
    receiver.id = AgentId(1);
    receiver.position = Coord::new(75_000, 40_000);
    obs.own_agents.push(receiver);
    let task = Some(Task::Transfer {
        receiver: AgentId(1),
        key: PortalId(2),
    });
    let old = submit(&mut core(&cfg, 0), &obs, task);
    assert!(!old.is_empty());
    assert_eq!(submit(&mut core(&cfg, 5), &obs, task), old);
}

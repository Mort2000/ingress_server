//! Broadcast events for the Public Event History (R-EVENT-01).
//!
//! `MOVE`, `HACK`, `RECHARGE`, and `TRANSFER` never emit events. `seq` is a
//! global, 0-based, strictly increasing counter owned by
//! [`crate::state::GameState`]; the log is append-only per round and never
//! reordered by execution results.

use crate::types::{AgentId, Coord, Faction, FieldId, LinkId, PortalId};

/// One entry of the Public Event History.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Event {
    /// Global 0-based sequence number, strictly increasing across rounds.
    pub seq: u64,
    /// Round in which the event was generated (1-based).
    pub round: u32,
    pub kind: EventKind,
}

/// Broadcast event payloads. Every event carries the acting faction, agent,
/// and the affected portal (R-EVENT-01 minimum fields).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventKind {
    /// A portal changed owner via CAPTURE (R-CAPTURE-01).
    PortalCaptured {
        faction: Faction,
        agent_id: AgentId,
        portal: PortalId,
    },
    /// A portal took fixed damage from ATTACK (R-ATTACK-01); `hp_after` is
    /// the post-damage HP (0 when the attack neutralized the portal).
    PortalAttacked {
        faction: Faction,
        agent_id: AgentId,
        portal: PortalId,
        damage: u32,
        hp_after: u32,
    },
    /// A portal's HP reached 0 and it was neutralized (R-ATTACK-02).
    PortalNeutralized {
        faction: Faction,
        agent_id: AgentId,
        portal: PortalId,
    },
    /// A link was created by LINK (R-LINK-01); `faction`/`agent_id` name the
    /// acting agent, `link` the canonical undirected edge.
    LinkCreated {
        faction: Faction,
        agent_id: AgentId,
        link: LinkId,
    },
    /// A link was destroyed by the neutralization cascade of one of its
    /// endpoints (R-ATTACK-02/D40); `faction`/`agent_id` name the ATTACK
    /// actor that caused the cascade, not the link's owner.
    LinkDestroyed {
        faction: Faction,
        agent_id: AgentId,
        link: LinkId,
    },
    /// A field was derived by a closing link (R-FIELD-01); the actor is the
    /// linking agent.
    FieldCreated {
        faction: Faction,
        agent_id: AgentId,
        field: FieldId,
    },
    /// A field died with its links in a neutralization cascade (R-FIELD-01
    /// cache maintenance); the actor is the ATTACK that caused it.
    FieldDestroyed {
        faction: Faction,
        agent_id: AgentId,
        field: FieldId,
    },
    /// Reinforcement agents were appended after a boundary round's
    /// settlement (R-SPAWN-02): `index` is the reinforcement interval index
    /// (`round / REINFORCEMENT_INTERVAL`), `position` the shared spawn
    /// coordinate drawn from the reinforcement RNG domain, and
    /// `first_agent_id` the start of the ENL block — the RES block follows
    /// after `count_per_faction` consecutive ids.
    Reinforcement {
        index: u32,
        position: Coord,
        count_per_faction: u32,
        first_agent_id: AgentId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameConfig;
    use crate::map::MapDefinition;
    use crate::resolve::{Resolver, SubmittedCommandStream};
    use crate::spawn::validate_initial_spawn_set;
    use crate::state::GameState;
    use crate::types::Command::Link;
    use crate::types::{Command, Owner, Portal};

    /// Static shape contract: every EventKind stays `Copy` (R-EVENT-01
    /// payloads are plain broadcast data; the log clones cheaply).
    #[test]
    fn event_kind_stays_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<EventKind>();
        assert_copy::<Event>();
    }

    #[test]
    fn event_carries_seq_round_and_payload() {
        let event = Event {
            seq: 7,
            round: 3,
            kind: EventKind::PortalCaptured {
                faction: Faction::Res,
                agent_id: AgentId(4),
                portal: PortalId(17),
            },
        };
        assert_eq!(event.seq, 7);
        assert_eq!(event.round, 3);
        assert_eq!(
            event.kind,
            EventKind::PortalCaptured {
                faction: Faction::Res,
                agent_id: AgentId(4),
                portal: PortalId(17)
            }
        );
    }

    #[test]
    fn attacked_event_exposes_damage_and_hp_after() {
        let kind = EventKind::PortalAttacked {
            faction: Faction::Enl,
            agent_id: AgentId(0),
            portal: PortalId(2),
            damage: 20,
            hp_after: 80,
        };
        match kind {
            EventKind::PortalAttacked {
                faction,
                agent_id,
                portal,
                damage,
                hp_after,
            } => {
                assert_eq!(faction, Faction::Enl);
                assert_eq!(agent_id, AgentId(0));
                assert_eq!(portal, PortalId(2));
                assert_eq!(damage, 20);
                assert_eq!(hp_after, 80);
            }
            _ => panic!("unexpected event kind"),
        }
    }

    /// Checklist 41 (M4): the new payloads carry the actor and the affected
    /// identifiers — validated against RESOLVER-PRODUCED events, not
    /// standalone constructions. Drives a full link/field lifecycle plus a
    /// reinforcement boundary in-process.
    #[test]
    fn new_payloads_carry_actor_and_identifiers() {
        // --- Scenario 1: create a field, then neutralize one of its portals.
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 30_000;
        cfg.map_height_cm = 30_000;
        cfg.portal_min_distance_cm = 4_000;
        cfg.portal_count = 3;
        cfg.portal_cluster_count = 3;
        cfg.initial_agents_per_faction = 1;
        cfg.attack_damage = 100; // one attack neutralizes a full-HP portal
        let map = MapDefinition::new(vec![
            Portal {
                id: PortalId(0),
                position: Coord::new(10_000, 10_000),
                owner: Owner::Enl,
                hp: 100,
            },
            Portal {
                id: PortalId(1),
                position: Coord::new(14_000, 10_000),
                owner: Owner::Enl,
                hp: 100,
            },
            Portal {
                id: PortalId(2),
                position: Coord::new(10_000, 14_000),
                owner: Owner::Enl,
                hp: 100,
            },
        ]);
        let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
        let enl_entries = vec![(AgentId(0), Coord::new(10_000, 10_000))];
        let res_entries = vec![(AgentId(1), Coord::new(10_000, 6_000))];
        let enl_spawns =
            validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg).expect("spawns");
        let res_spawns =
            validate_initial_spawn_set(&res_roster, &res_entries, &cfg).expect("spawns");
        let mut state = GameState::new(cfg, &map, &enl_spawns, &res_spawns).expect("state");
        // Keys granted directly: this test pins event payloads, not HACK.
        state.grant_keys(AgentId(0), PortalId(1), 1);
        state.grant_keys(AgentId(0), PortalId(2), 2);

        let link_cmd = |a: u32, b: u32| Link {
            source_portal: PortalId(a),
            target_portal: PortalId(b),
        };
        let _ = Resolver::new().run_round(
            &mut state,
            &[SubmittedCommandStream {
                round: 1,
                faction: Faction::Enl,
                agent_id: AgentId(0),
                received_at_ms: 0,
                receive_seq: 0,
                commands: vec![link_cmd(0, 1), link_cmd(0, 2)],
            }],
        );
        state.advance_round();
        let _ = Resolver::new().run_round(
            &mut state,
            &[SubmittedCommandStream {
                round: 2,
                faction: Faction::Enl,
                agent_id: AgentId(0),
                received_at_ms: 0,
                receive_seq: 0,
                commands: vec![link_cmd(1, 2)],
            }],
        );
        state.advance_round();
        let _ = Resolver::new().run_round(
            &mut state,
            &[SubmittedCommandStream {
                round: 3,
                faction: Faction::Res,
                agent_id: AgentId(1),
                received_at_ms: 0,
                receive_seq: 0,
                commands: vec![Command::Attack],
            }],
        );

        let kinds: Vec<EventKind> = state.events().iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EventKind::LinkCreated {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    link: LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
                },
                EventKind::LinkCreated {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    link: LinkId::new(PortalId(0), PortalId(2)).expect("canonical"),
                },
                EventKind::LinkCreated {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    link: LinkId::new(PortalId(1), PortalId(2)).expect("canonical"),
                },
                EventKind::FieldCreated {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    field: FieldId::new(PortalId(0), PortalId(1), PortalId(2)).expect("canonical"),
                },
                EventKind::PortalAttacked {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    portal: PortalId(0),
                    damage: 100,
                    hp_after: 0,
                },
                EventKind::PortalNeutralized {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    portal: PortalId(0),
                },
                // Cascade order: LINK_DESTROYED ascending, then fields.
                EventKind::LinkDestroyed {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    link: LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
                },
                EventKind::LinkDestroyed {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    link: LinkId::new(PortalId(0), PortalId(2)).expect("canonical"),
                },
                EventKind::FieldDestroyed {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    field: FieldId::new(PortalId(0), PortalId(1), PortalId(2)).expect("canonical"),
                },
            ]
        );
        // Every event of the round is round-tagged and seq-stamped in order.
        for (index, event) in state.events().iter().enumerate() {
            assert_eq!(event.seq, index as u64);
        }
        assert_eq!(
            state.events()[3].round,
            2,
            "the field event belongs to the closing-link round"
        );
        assert_eq!(state.events()[6].round, 3, "cascade in round 3");

        // --- Scenario 2: a reinforcement boundary emits REINFORCEMENT with
        // the interval index, shared position, count, and first id.
        let mut cfg2 = GameConfig::default();
        cfg2.map_width_cm = 30_000;
        cfg2.map_height_cm = 30_000;
        cfg2.portal_min_distance_cm = 4_000;
        cfg2.portal_count = 2;
        cfg2.portal_cluster_count = 2;
        cfg2.initial_agents_per_faction = 1;
        cfg2.total_rounds = 3;
        cfg2.reinforcement_interval_rounds = 1;
        let map2 = MapDefinition::new(vec![
            Portal {
                id: PortalId(0),
                position: Coord::new(10_000, 10_000),
                owner: Owner::Neutral,
                hp: 0,
            },
            Portal {
                id: PortalId(1),
                position: Coord::new(25_000, 25_000),
                owner: Owner::Neutral,
                hp: 0,
            },
        ]);
        let (enl_roster, res_roster) = GameState::initial_roster(&cfg2);
        let enl_entries = vec![(AgentId(0), Coord::new(0, 0))];
        let res_entries = vec![(AgentId(1), Coord::new(0, 0))];
        let enl_spawns =
            validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg2).expect("spawns");
        let res_spawns =
            validate_initial_spawn_set(&res_roster, &res_entries, &cfg2).expect("spawns");
        let mut state = GameState::new(cfg2, &map2, &enl_spawns, &res_spawns).expect("state");
        let _ = Resolver::new().run_round(&mut state, &[]);

        let reinforcement = state
            .events()
            .iter()
            .find(|e| matches!(e.kind, EventKind::Reinforcement { .. }))
            .expect("boundary round emitted a reinforcement event");
        assert_eq!(reinforcement.round, 1);
        match reinforcement.kind {
            EventKind::Reinforcement {
                index,
                position,
                count_per_faction,
                first_agent_id,
            } => {
                assert_eq!(index, 1, "round 1 / interval 1");
                // Independent oracle derivation of the shared position.
                let oracle = crate::rng::RandomOracle::new(0);
                assert_eq!(position, oracle.reinforcement_position(1, 30_001, 30_001));
                assert_eq!(count_per_faction, 1);
                assert_eq!(first_agent_id, AgentId(2));
                assert_eq!(state.agent(AgentId(2)).map(|a| a.position), Some(position));
                assert_eq!(state.agent(AgentId(3)).map(|a| a.position), Some(position));
            }
            other => panic!("unexpected kind {other:?}"),
        }
    }
}

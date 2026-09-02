//! Faction observations (R-VIS-01/02). An `Observation` is a fully cloned
//! immutable snapshot of what one faction may legally read at round start:
//! the public state (round, portal positions/owners/HP, every link and
//! field, full event history, scores) plus that faction's private agent
//! state. Enemy agents appear nowhere — not in construction and not in
//! content. Scores are real accumulated `score2` values since M4 (D29
//! placeholder retired).

use std::collections::BTreeMap;

use crate::event::Event;
use crate::state::GameState;
use crate::types::{AgentId, Area2, Coord, Faction, FieldId, LinkId, Owner, PortalId, Score2};

/// Public portal view: position, ownership, and HP are readable by both
/// factions (R-VIS-01).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PortalPublic {
    pub id: PortalId,
    pub position: Coord,
    pub owner: Owner,
    pub hp: u32,
}

/// Public link view: the canonical undirected id and the owning faction
/// (R-VIS-01) — endpoints resolve through `portals`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkPublic {
    pub id: LinkId,
    pub faction: Faction,
}

/// Public field view: the canonical triangle id, the owning faction, and the
/// cached twice-area (R-VIS-01, R-FIELD-02 — `area2` stays the exact
/// integer, never a float).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FieldPublic {
    pub id: FieldId,
    pub faction: Faction,
    pub area2: Area2,
}

/// Private own-agent view: position, supply, and key inventory (R-VIS-02).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AgentPrivate {
    pub id: AgentId,
    pub position: Coord,
    pub supply: u32,
    pub keys: BTreeMap<PortalId, u32>,
}

/// A consistent round-start snapshot for one faction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Observation {
    pub round: u32,
    pub portals: Vec<PortalPublic>,
    pub links: Vec<LinkPublic>,
    pub fields: Vec<FieldPublic>,
    pub events: Vec<Event>,
    pub scores: BTreeMap<Faction, Score2>,
    pub own_agents: Vec<AgentPrivate>,
}

impl Observation {
    /// Clones everything `faction` may read from `state`. No enemy agent
    /// data is reachable from the result by construction. Links and fields
    /// iterate the `BTreeMap` caches, so both vectors are ascending and the
    /// public part is identical for both factions.
    pub fn for_faction(state: &GameState, faction: Faction) -> Observation {
        Observation {
            round: state.round(),
            portals: state
                .portals()
                .iter()
                .map(|p| PortalPublic {
                    id: p.id,
                    position: p.position,
                    owner: p.owner,
                    hp: p.hp,
                })
                .collect(),
            links: state
                .link_graph()
                .links()
                .iter()
                .map(|(&id, link)| LinkPublic {
                    id,
                    faction: link.faction,
                })
                .collect(),
            fields: state
                .link_graph()
                .fields()
                .iter()
                .map(|(&id, field)| FieldPublic {
                    id,
                    faction: field.faction,
                    area2: field.area2,
                })
                .collect(),
            events: state.events().to_vec(),
            scores: state.scores().clone(),
            own_agents: state
                .agents()
                .values()
                .filter(|agent| agent.faction == faction)
                .map(|agent| AgentPrivate {
                    id: agent.id,
                    position: agent.position,
                    supply: agent.supply,
                    keys: agent.keys.clone(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameConfig;
    use crate::map::MapDefinition;
    use crate::spawn::validate_initial_spawn_set;
    use crate::types::{Agent, Faction, Portal};

    fn observed_state() -> GameState {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 30_000;
        cfg.map_height_cm = 30_000;
        cfg.portal_min_distance_cm = 4_000;
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;
        cfg.initial_agents_per_faction = 1;
        let map = MapDefinition::new(vec![
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
        let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
        let enl_entries: Vec<(AgentId, Coord)> = enl_roster
            .iter()
            .copied()
            .zip([Coord::new(10_000, 5_000)])
            .collect();
        let res_entries: Vec<(AgentId, Coord)> = res_roster
            .iter()
            .copied()
            .zip([Coord::new(6_000, 10_000)])
            .collect();
        let enl = validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg).expect("spawns");
        let res = validate_initial_spawn_set(&res_roster, &res_entries, &cfg).expect("spawns");
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state");
        // Give the world some visible and private state.
        state.apply_capture(AgentId(0), PortalId(0));
        state.apply_move(AgentId(1), Coord::new(7_000, 10_000), 1_000);
        state
    }

    // Checklist 40: both factions see an identical public part.
    #[test]
    fn public_part_is_identical_for_both_factions() {
        let state = observed_state();
        let enl = Observation::for_faction(&state, Faction::Enl);
        let res = Observation::for_faction(&state, Faction::Res);
        assert_eq!(enl.round, res.round);
        assert_eq!(enl.portals, res.portals);
        assert_eq!(enl.events, res.events);
        assert_eq!(enl.scores, res.scores);
        // And the public content matches the authoritative state.
        assert_eq!(enl.round, 1);
        assert_eq!(enl.portals.len(), 2);
        assert_eq!(enl.portals[0].owner, Owner::Enl);
        assert_eq!(enl.portals[0].hp, 100);
        assert_eq!(enl.portals[0].position, Coord::new(10_000, 10_000));
        assert_eq!(enl.events.len(), 1);
        assert_eq!(enl.scores.get(&Faction::Enl), Some(&0));
        assert_eq!(enl.scores.get(&Faction::Res), Some(&0));
    }

    // Checklist 41: own agents present, enemy agents absent.
    #[test]
    fn own_agents_private_state_only_no_enemy_data() {
        let state = observed_state();
        let enl = Observation::for_faction(&state, Faction::Enl);
        let res = Observation::for_faction(&state, Faction::Res);

        assert_eq!(enl.own_agents.len(), 1);
        assert_eq!(enl.own_agents[0].id, AgentId(0));
        assert_eq!(enl.own_agents[0].position, Coord::new(10_000, 5_000));
        assert_eq!(enl.own_agents[0].supply, 20);
        assert!(enl.own_agents[0].keys.is_empty());

        // The RES observation sees the RES agent's private state (its move
        // is reflected) and contains no ENL agent by construction.
        assert_eq!(res.own_agents.len(), 1);
        assert_eq!(res.own_agents[0].id, AgentId(1));
        assert_eq!(res.own_agents[0].position, Coord::new(7_000, 10_000));
        let observed_ids: Vec<AgentId> = res.own_agents.iter().map(|a| a.id).collect();
        assert!(!observed_ids.contains(&AgentId(0)));
        let enl_ids: Vec<AgentId> = enl.own_agents.iter().map(|a| a.id).collect();
        assert!(!enl_ids.contains(&AgentId(1)));

        // Cross-check against the authoritative enemy agent: none of its
        // private values leak into the opposing view's own_agents.
        let enemy: &Agent = state.agent(AgentId(0)).expect("agent exists");
        assert!(
            res.own_agents
                .iter()
                .all(|a| a.position != enemy.position || a.id == enemy.id)
        );
    }

    // Checklist 42: the snapshot is immutable.
    #[test]
    fn observation_is_an_immutable_snapshot() {
        let state = observed_state();
        let before = Observation::for_faction(&state, Faction::Enl);
        let mut state = state;
        state.apply_attack_damage(PortalId(0), 30);
        state.apply_move(AgentId(0), Coord::new(11_000, 5_000), 1_000);
        state.push_event(crate::event::EventKind::PortalAttacked {
            faction: Faction::Res,
            agent_id: AgentId(1),
            portal: PortalId(0),
            damage: 30,
            hp_after: 70,
        });
        state.advance_round();
        let after = Observation::for_faction(&state, Faction::Enl);
        assert_eq!(before.portals[0].hp, 100);
        assert_eq!(before.events.len(), 1);
        assert_eq!(before.round, 1);
        assert_eq!(before.own_agents[0].position, Coord::new(10_000, 5_000));
        // The fresh observation reflects the mutations; the old one did not.
        assert_eq!(after.portals[0].hp, 70);
        assert_eq!(after.events.len(), 2);
        assert_eq!(after.round, 2);
        assert_eq!(after.own_agents[0].position, Coord::new(11_000, 5_000));
    }

    // ------------------------------------------------------------------
    // M4 (checklist 40): links, fields, and real scores in the public part.
    // ------------------------------------------------------------------

    /// A linked-up world: an ENL triangle field over portals 0/1/2 plus one
    /// lone RES edge (3,4), and one settled round of real scores.
    fn fielded_state() -> GameState {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 40_000;
        cfg.map_height_cm = 40_000;
        cfg.portal_min_distance_cm = 4_000;
        cfg.portal_count = 5;
        cfg.portal_cluster_count = 5;
        cfg.initial_agents_per_faction = 1;
        let map = MapDefinition::new(vec![
            Portal {
                id: PortalId(0),
                position: Coord::new(4_000, 4_000),
                owner: Owner::Enl,
                hp: 100,
            },
            Portal {
                id: PortalId(1),
                position: Coord::new(20_000, 4_000),
                owner: Owner::Enl,
                hp: 100,
            },
            Portal {
                id: PortalId(2),
                position: Coord::new(4_000, 20_000),
                owner: Owner::Enl,
                hp: 100,
            },
            Portal {
                id: PortalId(3),
                position: Coord::new(30_000, 4_000),
                owner: Owner::Res,
                hp: 100,
            },
            Portal {
                id: PortalId(4),
                position: Coord::new(30_000, 20_000),
                owner: Owner::Res,
                hp: 100,
            },
        ]);
        let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
        let enl_entries: Vec<(AgentId, Coord)> = vec![(AgentId(0), Coord::new(0, 0))];
        let res_entries: Vec<(AgentId, Coord)> = vec![(AgentId(1), Coord::new(0, 0))];
        let enl = validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg).expect("spawns");
        let res = validate_initial_spawn_set(&res_roster, &res_entries, &cfg).expect("spawns");
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state");
        for &(a, b) in &[(0u32, 1u32), (0, 2), (1, 2)] {
            state.grant_keys(AgentId(0), PortalId(b), 1);
            state.apply_link(AgentId(0), PortalId(a), PortalId(b));
        }
        state.grant_keys(AgentId(1), PortalId(4), 1);
        state.apply_link(AgentId(1), PortalId(3), PortalId(4));
        let _ = state.settle_scores(); // real scores: Enl 256_000_000, Res 0
        state
    }

    // Checklist 40: ascending LinkPublic/FieldPublic identical for both
    // factions, carrying faction and exact area2; enemy agents still absent.
    #[test]
    fn observation_exposes_links_fields_and_real_scores() {
        let state = fielded_state();
        let enl = Observation::for_faction(&state, Faction::Enl);
        let res = Observation::for_faction(&state, Faction::Res);

        // The public part — links, fields, scores — is identical.
        assert_eq!(enl.links, res.links);
        assert_eq!(enl.fields, res.fields);
        assert_eq!(enl.scores, res.scores);

        // Ascending order with the owning faction and exact integer area2.
        let link_ids: Vec<_> = enl.links.iter().map(|l| l.id).collect();
        assert_eq!(
            link_ids,
            vec![
                crate::types::LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
                crate::types::LinkId::new(PortalId(0), PortalId(2)).expect("canonical"),
                crate::types::LinkId::new(PortalId(1), PortalId(2)).expect("canonical"),
                crate::types::LinkId::new(PortalId(3), PortalId(4)).expect("canonical"),
            ]
        );
        assert_eq!(enl.links[0].faction, Faction::Enl);
        assert_eq!(enl.links[3].faction, Faction::Res);
        assert_eq!(enl.fields.len(), 1);
        assert_eq!(
            enl.fields[0].id,
            FieldId::new(PortalId(0), PortalId(1), PortalId(2)).expect("canonical")
        );
        assert_eq!(enl.fields[0].faction, Faction::Enl);
        assert_eq!(enl.fields[0].area2, 256_000_000u64);

        // Scores are the real settled values (D29 placeholder retired).
        assert_eq!(enl.scores.get(&Faction::Enl), Some(&256_000_000u128));
        assert_eq!(enl.scores.get(&Faction::Res), Some(&0));

        // Enemy agents remain invisible: each view sees exactly its own one.
        assert_eq!(enl.own_agents.len(), 1);
        assert_eq!(res.own_agents.len(), 1);
        assert_eq!(enl.own_agents[0].id, AgentId(0));
        assert_eq!(res.own_agents[0].id, AgentId(1));
    }
}

//! Faction observations (R-VIS-01/02). An `Observation` is a fully cloned
//! immutable snapshot of what one faction may legally read at round start:
//! the public state (round, portal positions/owners/HP, full event history,
//! scores) plus that faction's private agent state. Enemy agents appear
//! nowhere — not in construction and not in content. Links and fields join
//! the public part in M4; scores stay zero until then (D29).

use std::collections::BTreeMap;

use crate::event::Event;
use crate::state::GameState;
use crate::types::{AgentId, Coord, Faction, Owner, PortalId, Score2};

/// Public portal view: position, ownership, and HP are readable by both
/// factions (R-VIS-01).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PortalPublic {
    pub id: PortalId,
    pub position: Coord,
    pub owner: Owner,
    pub hp: u32,
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
    pub events: Vec<Event>,
    pub scores: BTreeMap<Faction, Score2>,
    pub own_agents: Vec<AgentPrivate>,
}

impl Observation {
    /// Clones everything `faction` may read from `state`. No enemy agent
    /// data is reachable from the result by construction.
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
}

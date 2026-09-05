//! The diagnostic `idle` policy (plan section 10.1,): a
//! first-class RECORDED opponent for the readiness idle gate — every R2
//! policy must score stably against it. Family `idle` is diagnostic: it
//! never enters competitive-balance aggregation. Spawns stand on map
//! portals (any legal coordinates); every round is an explicit PASS
//! stream (every agent idles, auto-PASS).

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord};

use crate::r2::R2Policy;

/// The `idle` policy (version `r2.0`, diagnostic family `idle`, no
/// parameters).
pub struct IdlePolicy;

impl R2Policy for IdlePolicy {
    fn choose_spawns(
        &mut self,
        _cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        let count = map.len().max(1) as u32;
        roster
            .iter()
            .enumerate()
            .map(|(index, &agent)| {
                let portal = map
                    .portal(arena_core::types::PortalId(index as u32 % count))
                    .expect("dense portal ids");
                (agent, portal.position)
            })
            .collect()
    }

    fn submit_round(&mut self, _obs: &Observation) -> Vec<AgentSubmission> {
        Vec::new()
    }
}

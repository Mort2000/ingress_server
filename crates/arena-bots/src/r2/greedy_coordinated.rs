//! `greedy-coordinated-r2` (plan section 6.1, Field Builder): the shared
//! coordinated builder core made a policy — staggered task assignment,
//! target reservation, route- and key-aware planning, no unintentional
//! self-competition. This is the credible pure-build baseline the plan's
//! ablations measure everything else against; the other five policies
//! are role overlays over exactly this core.

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction};

use crate::common::{self, CommandList};
use crate::r2::R2Policy;
use crate::r2::build::{BuilderCore, CoreParams};

/// The `greedy-coordinated-r2` policy (version `r2.0`, family
/// `field-builder`). Parameters are exactly the shared [`CoreParams`].
pub struct GreedyCoordinated {
    core: BuilderCore,
}

impl GreedyCoordinated {
    pub fn new(faction: Faction, bot_seed: u64, cfg: &GameConfig, params: CoreParams) -> Self {
        GreedyCoordinated {
            core: BuilderCore::new(
                faction,
                bot_seed,
                crate::r2::rng::R2Stream::GreedyCoordinated,
                cfg,
                params,
            ),
        }
    }
}

impl R2Policy for GreedyCoordinated {
    fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        self.core.choose_spawns(cfg, map, roster)
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let commands = self.core.core_round(obs);
        submissions(obs, commands)
    }
}

/// Emits one faction's submissions from per-agent command lists, in
/// observation agent order (the faction's virtual arrival order), skipping
/// empty lists (an empty stream is an explicit PASS). Shared by every
/// R2 policy file; consumes the command map.
pub fn submissions(
    obs: &Observation,
    commands: std::collections::BTreeMap<AgentId, CommandList>,
) -> Vec<AgentSubmission> {
    let mut commands = commands;
    obs.own_agents
        .iter()
        .filter_map(|me| {
            commands
                .remove(&me.id)
                .map(|list| common::submission(me.id, list.build()))
        })
        .collect()
}

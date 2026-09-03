//! `builder-disruptor-r2` (plan section 6.1, Builder + Disruptor): the
//! coordinated builder core plus the attack overlay — attackers allocated
//! by the TARGET's value, the agents' distance, the portal's HP and the
//! required attack count ([`crate::r2::value::attack_value`]). The
//! required count is the overlay's coordinated-attack CAPACITY (the one
//! sanctioned multi-assignee exception, plan 6.3): exactly
//! `ceil(hp / ATTACK_DAMAGE)` attackers march, never "everyone".

use std::collections::BTreeMap;

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction, PortalId};

use crate::common::{self, CommandList};
use crate::r2::R2Policy;
use crate::r2::build::{BuilderCore, CoreParams};
use crate::r2::builder_defender::merge_requests;
use crate::r2::greedy_coordinated::submissions;
use crate::r2::tasks::{Task, TaskBoard, TaskRequest, allocate};
use crate::r2::value::{attack_value, attacks_needed, travel_rounds_between};

/// The recorded `builder-disruptor-r2` parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BuilderDisruptorParams {
    /// Ceiling (permille of the faction's live agents) on how many agents
    /// one coordinated strike may claim, ON TOP of the attacks-needed
    /// bound (whichever is smaller wins). The r2.0 default 1000 leaves
    /// the attacks-needed count operative.
    pub attack_share_permille: u32,
    /// The shared builder-core + value-model parameters.
    pub core: CoreParams,
}

impl Default for BuilderDisruptorParams {
    fn default() -> Self {
        BuilderDisruptorParams {
            attack_share_permille: 1000,
            core: CoreParams::default(),
        }
    }
}

impl BuilderDisruptorParams {
    /// Canonical serialization: `share=..|core=<CoreParams::tag>`.
    pub fn tag(&self) -> String {
        format!(
            "share={}|core={}",
            self.attack_share_permille,
            self.core.tag()
        )
    }

    /// Parses [`BuilderDisruptorParams::tag`] back — strictly canonical.
    pub fn from_tag(tag: &str) -> Option<BuilderDisruptorParams> {
        let (own, core) = tag.split_once('|')?;
        let core = core.strip_prefix("core=")?;
        let (key, raw) = own.split_once('=')?;
        if key != "share" {
            return None;
        }
        Some(BuilderDisruptorParams {
            attack_share_permille: u32::try_from(raw.parse::<u64>().ok()?).ok()?,
            core: CoreParams::from_tag(core)?,
        })
    }
}

/// The attack overlay's task requests (shared with `adaptive-hybrid-r2`):
/// the single best enemy-owned portal by positive [`attack_value`] becomes
/// ONE coordinated [`Task::Attack`] whose declared capacity is
/// `min(ceil(hp / ATTACK_DAMAGE), attack_share)` — the attackers-needed
/// count, bounded by the recorded share of the faction. Every agent
/// requests it at `attack_value` minus its own travel at the opportunity
/// rate, so the nearest willing agents are granted first.
pub fn attack_overlay(
    core: &BuilderCore,
    obs: &Observation,
    params: &BuilderDisruptorParams,
) -> BTreeMap<AgentId, Vec<TaskRequest>> {
    let mut requests = BTreeMap::new();
    let value_params = &core.params().value;
    let economy = core.economy(obs);
    let enemy_owner = common::owners(core.faction()).1;
    let mut targets: Vec<(i64, PortalId)> = obs
        .portals
        .iter()
        .filter(|p| p.owner == enemy_owner)
        .map(|p| {
            (
                attack_value(
                    obs,
                    core.faction(),
                    core.cfg(),
                    value_params,
                    &economy,
                    p.id,
                ),
                p.id,
            )
        })
        .filter(|(value, _)| *value > 0)
        .collect();
    targets.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let Some((value, portal)) = targets.first().copied() else {
        return requests;
    };
    let hp = common::portal(obs, portal).hp;
    let needed = attacks_needed(hp, core.cfg()).max(1);
    let share_cap =
        ((obs.own_agents.len() as u64 * params.attack_share_permille as u64) / 1000).max(1) as u32;
    let capacity = needed.min(share_cap).max(1);
    for me in obs.own_agents.iter() {
        let target = common::portal(obs, portal);
        let rounds = travel_rounds_between(me.position, target.position, core.cfg().move_budget_cm);
        let priority = value.saturating_sub(
            (rounds as i128 * economy.opportunity_per_round as i128).min(i64::MAX as i128) as i64,
        );
        requests.insert(
            me.id,
            vec![TaskRequest::coordinated(
                Task::Attack { portal },
                priority,
                capacity,
            )],
        );
    }
    requests
}

/// The `builder-disruptor-r2` policy (version `r2.0`, family
/// `disruptor`).
pub struct BuilderDisruptor {
    core: BuilderCore,
    params: BuilderDisruptorParams,
}

impl BuilderDisruptor {
    pub fn new(
        faction: Faction,
        bot_seed: u64,
        cfg: &GameConfig,
        params: BuilderDisruptorParams,
    ) -> Self {
        BuilderDisruptor {
            core: BuilderCore::new(
                faction,
                bot_seed,
                crate::r2::rng::R2Stream::BuilderDisruptor,
                cfg,
                params.core,
            ),
            params,
        }
    }

    /// One round (shared with the adaptive hybrid's composition).
    pub fn round_commands(&mut self, obs: &Observation) -> BTreeMap<AgentId, CommandList> {
        self.core.refresh(obs);
        let agents: Vec<AgentId> = obs.own_agents.iter().map(|me| me.id).collect();
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        merge_requests(&mut requests, attack_overlay(&self.core, obs, &self.params));
        let mut board = TaskBoard::new(obs.round);
        let grants = allocate(&mut board, &agents, &requests);
        self.core.execute(obs, &grants)
    }
}

impl R2Policy for BuilderDisruptor {
    fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        self.core.choose_spawns(cfg, map, roster)
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        let commands = self.round_commands(obs);
        submissions(obs, commands)
    }
}

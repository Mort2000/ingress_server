//! `builder-disruptor-r2`: building plus coordinated attacks. Candidate
//! teams are prefixes of actual arrival order, capped by damage required
//! and the recorded faction share. Whole-team completion is valued against
//! forgone construction; the allocator rechecks the granted team's deadline.

use std::collections::BTreeMap;

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction};

use crate::common::{self, CommandList};
use crate::r2::R2Policy;
use crate::r2::build::{BuilderCore, CoreParams};
use crate::r2::builder_defender::merge_requests;
use crate::r2::greedy_coordinated::submissions;
use crate::r2::tasks::{Task, TaskRequest};
use crate::r2::value::{AttackArrival, attack_income_per_round, attacks_needed, strike_value};

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

/// Propose the best positive-value strike among arrival-ordered team
/// prefixes. Complete prefixes cross discrete damage thresholds even when
/// one additional attacker alone cannot advance completion. The team's
/// value is shared across requests and competes with productive building.
pub fn attack_overlay(
    core: &BuilderCore,
    obs: &Observation,
    params: &BuilderDisruptorParams,
) -> BTreeMap<AgentId, Vec<TaskRequest>> {
    let mut requests = BTreeMap::new();
    if params.attack_share_permille == 0 {
        return requests;
    }
    let cfg = core.cfg();
    let economy = core.economy(obs);
    let enemy_owner = common::owners(core.faction()).1;
    let share_cap = ((obs.own_agents.len() as u64 * params.attack_share_permille as u64) / 1000)
        .max(1) as usize;
    let mut best_value = 0i128;
    for target in obs.portals.iter().filter(|p| p.owner == enemy_owner) {
        let needed = attacks_needed(target.hp, cfg).max(1);
        let income =
            attack_income_per_round(obs, core.faction(), cfg, &core.params().value, target.id);
        if income == 0 {
            continue;
        }
        let mut arrivals: Vec<_> = obs
            .own_agents
            .iter()
            .filter_map(|agent| {
                AttackArrival::for_agent(agent, target.position, cfg, economy.horizon_rounds)
            })
            .collect();
        arrivals.sort_by_key(|agent| {
            (
                agent.first_attack_delay(),
                agent.agent,
                agent.movement_rounds,
            )
        });
        let mut selected = Vec::new();
        for arrival in arrivals {
            if selected.len() >= share_cap.min(needed as usize) {
                break;
            }
            selected.push(arrival);
            let Some(value) = strike_value(&economy, income, needed, &selected) else {
                continue;
            };
            if value > best_value {
                best_value = value;
                let capacity = selected.len() as u32;
                // The whole strike's benefit is shared by its members;
                // it is not credited in full to every attacker separately.
                let priority = (value / capacity as i128).min(i64::MAX as i128) as i64;
                requests = selected
                    .iter()
                    .map(|arrival| {
                        (
                            arrival.agent,
                            vec![TaskRequest::coordinated(
                                Task::Attack { portal: target.id },
                                priority,
                                capacity,
                            )],
                        )
                    })
                    .collect();
            }
        }
    }
    requests
}

/// The `builder-disruptor-r2` policy (current R2 version, family
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
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        merge_requests(&mut requests, attack_overlay(&self.core, obs, &self.params));
        self.core.allocate_and_execute(obs, &requests)
    }
}

impl R2Policy for BuilderDisruptor {
    fn decision_trace(&self) -> Option<&crate::r2::build::DecisionTrace> {
        self.core.decision_trace()
    }

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

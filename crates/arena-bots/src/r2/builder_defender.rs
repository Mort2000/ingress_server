//! `builder-defender-r2` (plan section 6.1, Builder + Defender): the
//! coordinated builder core plus the defense overlay — Recharge decisions
//! driven by the EXTRA SETTLED INCOME one Recharge can preserve
//! ([`crate::r2::value::defense_value`]), never by identity or routine.
//! A healthy, unthreatened anchor scores exactly 0, so the overlay
//! touches nothing while building is the whole game — and when an anchor
//! carrying live field income dips under the recorded HP share or comes
//! under recent enemy attack, its recharge competes in the SAME allocator
//! at its defense value, keeping the scoring path alive.

use std::collections::BTreeMap;

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction, PortalId};

use crate::common::CommandList;
use crate::r2::R2Policy;
use crate::r2::build::{BuilderCore, CoreParams};
use crate::r2::greedy_coordinated::submissions;
use crate::r2::tasks::{Task, TaskRequest};
use crate::r2::value::DefenseModel;

/// The recorded `builder-defender-r2` parameters: the shared core plus
/// the defense overlay's own thresholds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BuilderDefenderParams {
    /// At most this many concurrent Recharge tasks per round (the defense
    /// headroom — one anchor at a time by default).
    pub defense_task_cap: u32,
    /// The shared builder-core + value-model parameters.
    pub core: CoreParams,
}

impl Default for BuilderDefenderParams {
    fn default() -> Self {
        BuilderDefenderParams {
            defense_task_cap: 1,
            core: CoreParams::default(),
        }
    }
}

impl BuilderDefenderParams {
    /// Canonical serialization: `cap=..|core=<CoreParams::tag>`.
    pub fn tag(&self) -> String {
        format!("cap={}|core={}", self.defense_task_cap, self.core.tag())
    }

    /// Parses [`BuilderDefenderParams::tag`] back — strictly canonical.
    pub fn from_tag(tag: &str) -> Option<BuilderDefenderParams> {
        let (own, core) = tag.split_once('|')?;
        let core = core.strip_prefix("core=")?;
        let (key, raw) = own.split_once('=')?;
        if key != "cap" {
            return None;
        }
        Some(BuilderDefenderParams {
            defense_task_cap: u32::try_from(raw.parse::<u64>().ok()?).ok()?,
            core: CoreParams::from_tag(core)?,
        })
    }
}

/// The defense overlay's task requests (shared with
/// `adaptive-hybrid-r2`): the top `cap` own portals by positive
/// [`defense_value`] — value-selected anchors only — each requested by
/// every agent at `defense_value` minus the agent's own travel at the
/// economy's opportunity rate. `Ok` requests are empty when nothing is
/// defense-urgent.
pub fn defense_overlay(
    core: &BuilderCore,
    obs: &Observation,
    cap: u32,
) -> BTreeMap<AgentId, Vec<TaskRequest>> {
    let mut requests = BTreeMap::new();
    let params = core.params();
    let economy = core.economy(obs);
    let model = DefenseModel::new(obs, core.faction(), core.cfg(), &params.value, &economy);
    // Value-ranked urgent anchors: (defense_value, portal), value
    // descending then ascending portal id.
    let mut urgent: Vec<(i64, PortalId)> = obs
        .portals
        .iter()
        .filter(|p| p.owner == crate::common::owners(core.faction()).0)
        .map(|p| {
            (
                obs.own_agents
                    .iter()
                    .map(|agent| model.value(agent, p.id))
                    .max()
                    .unwrap_or(0),
                p.id,
            )
        })
        .filter(|(value, _)| *value > 0)
        .collect();
    urgent.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    urgent.truncate(cap.max(0) as usize);
    if urgent.is_empty() {
        return requests;
    }
    for me in obs.own_agents.iter() {
        let mut mine = Vec::new();
        for (_, portal) in &urgent {
            let priority = model.value(me, *portal);
            if priority > 0 {
                mine.push(TaskRequest::one_shot(
                    Task::Recharge { portal: *portal },
                    priority,
                ));
            }
        }
        requests.insert(me.id, mine);
    }
    requests
}

/// Merges overlay requests into the per-agent request map (the overlay
/// appends; the allocator arbitrates by priority).
pub fn merge_requests(
    into: &mut BTreeMap<AgentId, Vec<TaskRequest>>,
    overlay: BTreeMap<AgentId, Vec<TaskRequest>>,
) {
    for (agent, mut mine) in overlay {
        into.entry(agent).or_default().append(&mut mine);
    }
}

/// The `builder-defender-r2` policy (current R2 version, family `defender`).
pub struct BuilderDefender {
    core: BuilderCore,
    params: BuilderDefenderParams,
}

impl BuilderDefender {
    pub fn new(
        faction: Faction,
        bot_seed: u64,
        cfg: &GameConfig,
        params: BuilderDefenderParams,
    ) -> Self {
        BuilderDefender {
            core: BuilderCore::new(
                faction,
                bot_seed,
                crate::r2::rng::R2Stream::BuilderDefender,
                cfg,
                params.core,
            ),
            params,
        }
    }

    /// One round: refresh, build (or fallback) requests, defense overlay,
    /// allocate, execute. Exposed for the adaptive hybrid's composition.
    pub fn round_commands(&mut self, obs: &Observation) -> BTreeMap<AgentId, CommandList> {
        self.core.refresh(obs);
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        merge_requests(
            &mut requests,
            defense_overlay(&self.core, obs, self.params.defense_task_cap),
        );
        self.core.allocate_and_execute(obs, &requests)
    }
}

impl R2Policy for BuilderDefender {
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

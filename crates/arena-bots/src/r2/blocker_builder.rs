//! `blocker-builder-r2` (plan section 6.1, Blocker + Builder): the
//! coordinated builder core plus the geometric counter-overlay. The
//! blocker evaluates the deny value of every own link it could legally
//! emit this round ([`crate::r2::value::block_values`]: the enemy
//! candidate-field income the link's closed segment kills, minus the own
//! candidate income the same segment precludes) and, when the best link
//! clears the recorded threshold, it competes in the SAME allocator as
//! the build tasks at its block value — so denial only ever happens when
//! it is worth more than the forgone building, and the policy keeps its
//! own scoring path.

use std::collections::BTreeMap;

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction};

use crate::common::CommandList;
use crate::r2::R2Policy;
use crate::r2::build::{BuilderCore, CoreParams};
use crate::r2::builder_defender::merge_requests;
use crate::r2::greedy_coordinated::submissions;
use crate::r2::tasks::{Task, TaskBoard, TaskRequest, allocate};
use crate::r2::value::block_values;

/// The recorded `blocker-builder-r2` parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BlockerBuilderParams {
    /// A deny link must clear this block value (score2-equivalents) to
    /// preempt ordinary building; 1 = any net-positive denial.
    pub block_min_value: i64,
    /// The shared builder-core + value-model parameters.
    pub core: CoreParams,
}

impl Default for BlockerBuilderParams {
    fn default() -> Self {
        BlockerBuilderParams {
            block_min_value: 1,
            core: CoreParams::default(),
        }
    }
}

impl BlockerBuilderParams {
    /// Canonical serialization: `min=..|core=<CoreParams::tag>`.
    pub fn tag(&self) -> String {
        format!("min={}|core={}", self.block_min_value, self.core.tag())
    }

    /// Parses [`BlockerBuilderParams::tag`] back — strictly canonical.
    pub fn from_tag(tag: &str) -> Option<BlockerBuilderParams> {
        let (own, core) = tag.split_once('|')?;
        let core = core.strip_prefix("core=")?;
        let (key, raw) = own.split_once('=')?;
        if key != "min" {
            return None;
        }
        Some(BlockerBuilderParams {
            block_min_value: raw.parse().ok()?,
            core: CoreParams::from_tag(core)?,
        })
    }
}

/// The blocking overlay's task requests: every legal own link this round
/// whose block value clears the recorded threshold, as one-shot
/// [`Task::Link`] requests at their block value — each requested only by
/// the agent already in range of its source (deterministic ranking:
/// value descending, then ascending `(source, target, agent)`). The
/// allocator arbitrates denial against the build tasks by value.
pub fn blocking_requests(
    core: &BuilderCore,
    obs: &Observation,
    params: &BlockerBuilderParams,
) -> BTreeMap<AgentId, Vec<TaskRequest>> {
    let mut requests = BTreeMap::new();
    let candidates = core.blockable_pairs(obs);
    if candidates.is_empty() {
        return requests;
    }
    let pairs: Vec<(arena_core::types::PortalId, arena_core::types::PortalId)> = candidates
        .iter()
        .map(|(source, target, _)| (*source, *target))
        .collect();
    let values = block_values(
        obs,
        core.faction(),
        core.cfg(),
        &core.params().value,
        &pairs,
    );
    let mut ranked: Vec<(usize, i64)> = values
        .into_iter()
        .enumerate()
        .filter(|(_, value)| *value >= params.block_min_value)
        .collect();
    ranked.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            candidates[a.0]
                .0
                .cmp(&candidates[b.0].0)
                .then_with(|| candidates[a.0].1.cmp(&candidates[b.0].1))
                .then_with(|| candidates[a.0].2.cmp(&candidates[b.0].2))
        })
    });
    for (index, value) in ranked {
        let (source, target, agent) = candidates[index];
        requests
            .entry(agent)
            .or_default()
            .push(TaskRequest::one_shot(Task::Link { source, target }, value));
    }
    requests
}

/// The `blocker-builder-r2` policy (version `r2.0`, family `blocker`).
pub struct BlockerBuilder {
    core: BuilderCore,
    params: BlockerBuilderParams,
}

impl BlockerBuilder {
    pub fn new(
        faction: Faction,
        bot_seed: u64,
        cfg: &GameConfig,
        params: BlockerBuilderParams,
    ) -> Self {
        BlockerBuilder {
            core: BuilderCore::new(
                faction,
                bot_seed,
                crate::r2::rng::R2Stream::BlockerBuilder,
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
        merge_requests(
            &mut requests,
            blocking_requests(&self.core, obs, &self.params),
        );
        let mut board = TaskBoard::new(obs.round);
        let grants = allocate(&mut board, &agents, &requests);
        self.core.execute(obs, &grants)
    }
}

impl R2Policy for BlockerBuilder {
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

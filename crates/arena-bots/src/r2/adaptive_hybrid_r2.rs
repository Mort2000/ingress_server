//! Adaptive composition over the shared allocator. Default combat candidates
//! compete from the opening, independently of the score lead. Optional
//! phase/lead gates remain recorded parameters for controlled contrasts;
//! the endgame keeps actions that can preserve or deny remaining income.
//! Role diagnostics describe the actual grants from the same request path.

use std::collections::BTreeMap;

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction};

use crate::common::CommandList;
use crate::r2::R2Policy;
use crate::r2::build::{BuilderCore, CoreParams};
use crate::r2::builder_defender::{defense_overlay, merge_requests};
use crate::r2::builder_disruptor::{BuilderDisruptorParams, attack_overlay};
use crate::r2::greedy_coordinated::submissions;
use crate::r2::opponent::{OpponentModel, OpponentParams};

/// The recorded `adaptive-hybrid-r2` parameters: the shared core, the
/// opponent model's windows, and the phase/lead thresholds that switch
/// the role mix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdaptiveHybridParams {
    /// Rounds before this share (permille of `total_rounds`) are the
    /// opening: build preference with urgent defense. Zero by default so
    /// a profitable response is not hidden behind a fixed opening period.
    pub opening_permille: u32,
    /// Rounds from this share (permille) are the endgame: feasible build, defense and attack.
    pub endgame_permille: u32,
    /// The faction's score share (permille of the pair total) at or above
    /// which the mid-game adds the disruptor overlay. Zero by default:
    /// attack value, arrival and opportunity cost decide even while behind.
    pub attack_lead_share_permille: u32,
    /// The disruptor overlay's attacker-share ceiling
    /// (permille of live agents; r2.0 default leaves attacks-needed
    /// operative).
    pub attack_share_permille: u32,
    /// The defender overlay's total simultaneous-defender budget.
    pub defense_task_cap: u32,
    /// The opponent model's windows.
    pub opponent: OpponentParams,
    /// The shared builder-core + value-model parameters.
    pub core: CoreParams,
}

impl Default for AdaptiveHybridParams {
    fn default() -> Self {
        AdaptiveHybridParams {
            opening_permille: 0,
            endgame_permille: 850,
            attack_lead_share_permille: 0,
            attack_share_permille: 1000,
            defense_task_cap: 2,
            opponent: OpponentParams::default(),
            core: CoreParams {
                supply_reserve_recharges: 5,
                ..CoreParams::default()
            },
        }
    }
}

impl AdaptiveHybridParams {
    /// Canonical serialization:
    /// `opening=..;end=..;lead=..;share=..;cap=..|opp=<OpponentParams::tag>|core=<CoreParams::tag>`.
    pub fn tag(&self) -> String {
        format!(
            "opening={};end={};lead={};share={};cap={}|opp={}|core={}",
            self.opening_permille,
            self.endgame_permille,
            self.attack_lead_share_permille,
            self.attack_share_permille,
            self.defense_task_cap,
            self.opponent.tag(),
            self.core.tag()
        )
    }

    /// Parses [`AdaptiveHybridParams::tag`] back — strictly canonical.
    pub fn from_tag(tag: &str) -> Option<AdaptiveHybridParams> {
        // The core body itself contains a `|` (its inlined `value=` block),
        // so the sections split on their explicit `|core=` marker.
        let (own, rest) = tag.split_once('|')?;
        let rest = rest.strip_prefix("opp=")?;
        let (opp, core) = rest.split_once("|core=")?;
        let fields: Vec<&str> = own.split(';').collect();
        if fields.len() != 5 {
            return None;
        }
        let mut values = [0u64; 5];
        for (index, field) in fields.iter().enumerate() {
            let (key, raw) = field.split_once('=')?;
            let expected = ["opening", "end", "lead", "share", "cap"][index];
            if key != expected {
                return None;
            }
            values[index] = raw.parse().ok()?;
        }
        Some(AdaptiveHybridParams {
            opening_permille: u32::try_from(values[0]).ok()?,
            endgame_permille: u32::try_from(values[1]).ok()?,
            attack_lead_share_permille: u32::try_from(values[2]).ok()?,
            attack_share_permille: u32::try_from(values[3]).ok()?,
            defense_task_cap: u32::try_from(values[4]).ok()?,
            opponent: OpponentParams::from_tag(opp)?,
            core: CoreParams::from_tag(core)?,
        })
    }
}

/// The mid-game role mix of one round (diagnostic label; the decision
/// itself is the overlay set below).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoleMix {
    Opening,
    MidBuild,
    MidDefend,
    MidDisrupt,
    MidMixed,
    OpeningDefend,
    Endgame,
    EndDefend,
    EndDisrupt,
    EndMixed,
}

impl RoleMix {
    pub fn tag(self) -> &'static str {
        match self {
            RoleMix::Opening => "opening",
            RoleMix::MidBuild => "mid-build",
            RoleMix::MidDefend => "mid-defend",
            RoleMix::MidDisrupt => "mid-disrupt",
            RoleMix::MidMixed => "mid-mixed",
            RoleMix::OpeningDefend => "opening-defend",
            RoleMix::Endgame => "endgame",
            RoleMix::EndDefend => "end-defend",
            RoleMix::EndDisrupt => "end-disrupt",
            RoleMix::EndMixed => "end-mixed",
        }
    }
}

/// The `adaptive-hybrid-r2` policy (current R2 version, family
/// `adaptive-hybrid`).
pub struct AdaptiveHybridR2 {
    core: BuilderCore,
    params: AdaptiveHybridParams,
    opponent: OpponentModel,
    last_mix: RoleMix,
}

impl AdaptiveHybridR2 {
    pub fn new(
        faction: Faction,
        bot_seed: u64,
        cfg: &GameConfig,
        params: AdaptiveHybridParams,
    ) -> Self {
        AdaptiveHybridR2 {
            core: BuilderCore::new(
                faction,
                bot_seed,
                crate::r2::rng::R2Stream::AdaptiveHybrid,
                cfg,
                params.core,
            ),
            opponent: OpponentModel::new(params.opponent, faction.opposite()),
            params,
            last_mix: RoleMix::Opening,
        }
    }

    fn phase(&self, obs: &Observation) -> RoleMix {
        let permille = obs.round as u64 * 1000 / self.core.cfg().total_rounds.max(1) as u64;
        if permille < self.params.opening_permille as u64 {
            RoleMix::Opening
        } else if permille >= self.params.endgame_permille as u64 {
            RoleMix::Endgame
        } else {
            RoleMix::MidBuild
        }
    }

    fn round_requests(
        &mut self,
        obs: &Observation,
    ) -> BTreeMap<AgentId, Vec<crate::r2::tasks::TaskRequest>> {
        self.core.refresh(obs);
        self.opponent.update(obs);
        let phase = self.phase(obs);
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        if self.opponent.any_threat(obs) || phase == RoleMix::Endgame {
            merge_requests(
                &mut requests,
                defense_overlay(&self.core, obs, self.params.defense_task_cap),
            );
        }
        let own_score = *obs.scores.get(&self.core.faction()).unwrap_or(&0);
        let enemy_score = *obs
            .scores
            .get(&self.core.faction().opposite())
            .unwrap_or(&0);
        let share = if own_score + enemy_score == 0 {
            500
        } else {
            own_score * 1000 / (own_score + enemy_score)
        };
        if phase != RoleMix::Opening
            && (phase == RoleMix::Endgame
                || share >= self.params.attack_lead_share_permille as u128
                || self.core.active().is_none())
        {
            let disruptor = BuilderDisruptorParams {
                attack_share_permille: self.params.attack_share_permille,
                core: self.params.core,
            };
            merge_requests(&mut requests, attack_overlay(&self.core, obs, &disruptor));
        }
        requests
    }

    fn granted_mix(
        &self,
        obs: &Observation,
        decision: &crate::r2::build::DecisionTrace,
    ) -> RoleMix {
        use crate::r2::tasks::Task;
        let defend = decision.agents.iter().any(|a| {
            [a.grant, a.companion]
                .into_iter()
                .flatten()
                .any(|t| matches!(t, Task::Recharge { .. }))
        });
        let attack = decision.agents.iter().any(|a| {
            [a.grant, a.companion]
                .into_iter()
                .flatten()
                .any(|t| matches!(t, Task::Attack { .. }))
        });
        match (self.phase(obs), defend, attack) {
            (RoleMix::Opening, true, _) => RoleMix::OpeningDefend,
            (RoleMix::Opening, false, _) => RoleMix::Opening,
            (RoleMix::Endgame, true, true) => RoleMix::EndMixed,
            (RoleMix::Endgame, true, false) => RoleMix::EndDefend,
            (RoleMix::Endgame, false, true) => RoleMix::EndDisrupt,
            (RoleMix::Endgame, false, false) => RoleMix::Endgame,
            (_, true, true) => RoleMix::MidMixed,
            (_, true, false) => RoleMix::MidDefend,
            (_, false, true) => RoleMix::MidDisrupt,
            _ => RoleMix::MidBuild,
        }
    }

    /// Preview the same requests and grants used by submit_round. Labels
    /// describe feasible granted roles, not merely enabled overlays.
    pub fn role_mix(&mut self, obs: &Observation) -> RoleMix {
        let requests = self.round_requests(obs);
        self.granted_mix(obs, &self.core.preview_allocation(obs, &requests))
    }

    /// The role mix chosen for the most recent round (diagnostic echo).
    pub fn last_role_mix(&self) -> RoleMix {
        self.last_mix
    }
}

impl R2Policy for AdaptiveHybridR2 {
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
        let requests = self.round_requests(obs);
        let commands: BTreeMap<AgentId, CommandList> =
            self.core.allocate_and_execute(obs, &requests);
        self.last_mix = self.granted_mix(obs, self.core.decision_trace().expect("just allocated"));
        submissions(obs, commands)
    }
}

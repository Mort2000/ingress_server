//! `adaptive-hybrid-r2` (plan section 6.1, Adaptive Hybrid): the
//! coordinated builder core whose ROLE MIX changes dynamically by round
//! phase, score differential and public-history threat — never by
//! `AgentId`. Roles are overlays over the SAME allocator: opening and
//! endgame play pure build; the mid-game adds the defender overlay while
//! the opponent model sees live attack pressure, and the disruptor
//! overlay while the faction holds a recorded score lead — so
//! reinforcement agents and role switches flow through one allocation
//! path (plan 6.3's "roles are grants, not identities").
//!
//! Score and phase inputs are public: `obs.scores` and the round against
//! `cfg.total_rounds`; threat comes from the
//! [`crate::r2::opponent::OpponentModel`] over `obs.events` only.

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
use crate::r2::tasks::{TaskBoard, allocate};

/// The recorded `adaptive-hybrid-r2` parameters: the shared core, the
/// opponent model's windows, and the phase/lead thresholds that switch
/// the role mix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdaptiveHybridParams {
    /// Rounds before this share (permille of `total_rounds`) are the
    /// opening: pure build.
    pub opening_permille: u32,
    /// Rounds from this share (permille) are the endgame: pure build.
    pub endgame_permille: u32,
    /// The faction's score share (permille of the pair total) at or above
    /// which the mid-game adds the disruptor overlay.
    pub attack_lead_share_permille: u32,
    /// The disruptor overlay's attacker-share ceiling while leading
    /// (permille of live agents).
    pub attack_share_permille: u32,
    /// The defender overlay's concurrent-recharge cap.
    pub defense_task_cap: u32,
    /// The opponent model's windows.
    pub opponent: OpponentParams,
    /// The shared builder-core + value-model parameters.
    pub core: CoreParams,
}

impl Default for AdaptiveHybridParams {
    fn default() -> Self {
        AdaptiveHybridParams {
            opening_permille: 300,
            endgame_permille: 850,
            attack_lead_share_permille: 550,
            attack_share_permille: 600,
            defense_task_cap: 1,
            opponent: OpponentParams::default(),
            core: CoreParams::default(),
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
    Endgame,
}

impl RoleMix {
    pub fn tag(self) -> &'static str {
        match self {
            RoleMix::Opening => "opening",
            RoleMix::MidBuild => "mid-build",
            RoleMix::MidDefend => "mid-defend",
            RoleMix::MidDisrupt => "mid-disrupt",
            RoleMix::Endgame => "endgame",
        }
    }
}

/// The `adaptive-hybrid-r2` policy (version `r2.0`, family
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

    /// The role mix this round: a pure function of the round, the public
    /// scores and the opponent model's pressure state (plan 6.1 — map,
    /// round, score, threats; never AgentId).
    pub fn role_mix(&self, obs: &Observation) -> RoleMix {
        let total = self.core.cfg().total_rounds;
        let permille = if total == 0 {
            1000
        } else {
            obs.round * 1000 / total
        };
        if permille < self.params.opening_permille {
            return RoleMix::Opening;
        }
        if permille >= self.params.endgame_permille {
            return RoleMix::Endgame;
        }
        if self.opponent.any_threat(obs) {
            return RoleMix::MidDefend;
        }
        let own_score = *obs.scores.get(&self.core.faction()).unwrap_or(&0) as u128;
        let enemy_score = *obs
            .scores
            .get(&self.core.faction().opposite())
            .unwrap_or(&0) as u128;
        let total_score = own_score + enemy_score;
        let share = if total_score == 0 {
            500
        } else {
            (own_score * 1000 / total_score) as u32
        };
        if share >= self.params.attack_lead_share_permille {
            return RoleMix::MidDisrupt;
        }
        RoleMix::MidBuild
    }

    /// The role mix chosen for the most recent round (diagnostic echo).
    pub fn last_role_mix(&self) -> RoleMix {
        self.last_mix
    }
}

impl R2Policy for AdaptiveHybridR2 {
    fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        self.core.choose_spawns(cfg, map, roster)
    }

    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        self.opponent.update(obs);
        self.core.refresh(obs);
        self.last_mix = self.role_mix(obs);
        let agents: Vec<AgentId> = obs.own_agents.iter().map(|me| me.id).collect();
        let mut requests = if self.core.active().is_some() {
            self.core.build_requests(obs)
        } else {
            self.core.fallback_requests(obs)
        };
        match self.last_mix {
            RoleMix::MidDefend => {
                merge_requests(
                    &mut requests,
                    defense_overlay(&self.core, obs, self.params.defense_task_cap),
                );
            }
            RoleMix::MidDisrupt => {
                let disruptor = BuilderDisruptorParams {
                    attack_share_permille: self.params.attack_share_permille,
                    core: self.params.core,
                };
                merge_requests(&mut requests, attack_overlay(&self.core, obs, &disruptor));
            }
            _ => {}
        }
        let mut board = TaskBoard::new(obs.round);
        let grants = allocate(&mut board, &agents, &requests);
        let commands: BTreeMap<AgentId, CommandList> = self.core.execute(obs, &grants);
        submissions(obs, commands)
    }
}

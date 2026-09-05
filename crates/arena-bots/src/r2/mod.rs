//! R2 shared capability layer (`docs/balance/ai-balance-plan.md` sections 5.1,
//! 6.2 and 6.3): the deep, individually testable modules the six R2
//! policies compose, instead of per-policy re-implementation.
//!
//! - [`rng`]: the R2 policy RNG namespace — the frozen D53 key shape with
//!   FRESH stream kind-tag bytes 6..=12, disjoint from the frozen m6
//!   kind tags 1..=5;
//! - [`value`]: the interpretable INTEGER value model of plan 6.2 (field /
//!   attack / recharge-defense / transfer / block value and field
//!   candidate generation), pure over the legal `Observation` and public
//!   event history only;
//! - [`tasks`]: the agent-task allocator with per-round target reservation
//!   and explicitly coordinated strikes/rescues;
//! - [`sample`]: seeded tie-break and near-equal candidate sampling,
//!   deterministic given `(observation, seed, faction, version)`;
//! - [`opponent`]: the limited opponent model from the public event
//!   history;
//! - [`build`]: the coordinated builder core every build-side policy
//!   composes.
//!
//! The six plan-6.1 policies themselves live in
//! [`greedy_coordinated`], [`builder_defender`], [`builder_disruptor`],
//! [`logistics_builder`], [`blocker_builder`], [`adaptive_hybrid_r2`],
//! plus the DIAGNOSTIC [`idle`] opponent of the plan-10.1 idle gate.
//! Policy identity does NOT live here: ids, families, versions and the
//! serialized parameter records are [`crate::policy`] (/0016);
//! every B2 policy carries the fixed version tag [`R2_VERSION`].

/// Fixed version tag of every B2 R2 policy (plan section 14 row B2; the
/// policies are registered in `crate::policy` with exactly this version).
///
/// r2.6 (2026-09-05 Supply maintenance): defense policies replenish low
/// inventory independently of combat grants and use free remote-charge
/// movement to approach harvest points. Existing compound work is preserved.
/// Frozen M6 policies, rules and earlier experiment records are unchanged.
pub const R2_VERSION: &str = "r2.6";

pub mod adaptive_hybrid_r2;
pub mod blocker_builder;
pub mod build;
pub mod builder_defender;
pub mod builder_disruptor;
mod compound;
pub mod greedy_coordinated;
pub mod idle;
pub mod logistics_builder;
pub mod opponent;
pub mod rng;
pub mod sample;
pub mod tasks;
pub mod value;

use arena_core::config::GameConfig;
use arena_core::harness::{AgentSubmission, RoundStrategy};
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord};

use crate::bot::Bot;

/// A B2 R2 policy: the minimal strategy interface WITHOUT the frozen
/// `Bot` trait's `kind() -> BotKind` (the five-variant `BotKind` registry
/// is frozen read-only,/0016). A policy's identity — id, family,
/// version, parameters — lives entirely in its constructing
/// [`crate::policy::PolicySpec`]; the executable object is just
/// `choose_spawns` + `submit_round`.
///
/// Determinism contract (plan 6.3): identical `(spec, bot_seed, faction,
/// cfg, map)` plus an identical observation sequence produce identical
/// submissions; all runtime randomness flows through the policy's own
/// [`rng::R2Rng`] stream.
pub trait R2Policy: Send {
    /// Optional diagnostics from the previous decision; reading it never
    /// advances strategy memory or consumes the policy's random stream.
    fn decision_trace(&self) -> Option<&build::DecisionTrace> {
        None
    }

    fn delivery_comparisons(&self) -> &[logistics_builder::DeliveryComparison] {
        &[]
    }

    /// Full-map initial spawn strategy (same contract as `Bot`').
    fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)>;

    /// One round of submissions for the faction (same contract as `Bot`').
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission>;
}

/// Adapts an [`R2Policy`] to the arena-core [`RoundStrategy`] (the R2
/// counterpart of the frozen `BotStrategy`).
pub struct R2Strategy(pub Box<dyn R2Policy>);

impl RoundStrategy for R2Strategy {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        self.0.submit_round(obs)
    }
}

/// One constructed policy of a balance experiment, era-agnostic:
/// either a frozen m6.0 baseline (delegated 1:1 to the untouched
/// `make_bot` — the B1 lockstep pin) or an [`R2Policy`]. The balance
/// executor addresses both through this one type and never branches on
/// policy era.
pub enum PolicyPlayer {
    /// One of the five frozen m6.0 baselines, byte-identical to
    /// `make_bot` of the mirrored kind.
    Frozen(Box<dyn Bot>),
    /// A B2 R2 policy (the six plan-6.1 policies or the diagnostic idle).
    R2(Box<dyn R2Policy>),
}

impl PolicyPlayer {
    pub fn decision_trace(&self) -> Option<&build::DecisionTrace> {
        match self {
            Self::R2(policy) => policy.decision_trace(),
            Self::Frozen(_) => None,
        }
    }

    pub fn delivery_comparisons(&self) -> &[logistics_builder::DeliveryComparison] {
        match self {
            Self::R2(policy) => policy.delivery_comparisons(),
            Self::Frozen(_) => &[],
        }
    }

    /// Full-map initial spawn strategy.
    pub fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        match self {
            PolicyPlayer::Frozen(bot) => bot.choose_spawns(cfg, map, roster),
            PolicyPlayer::R2(policy) => policy.choose_spawns(cfg, map, roster),
        }
    }

    /// One round of submissions.
    pub fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        match self {
            PolicyPlayer::Frozen(bot) => bot.submit_round(obs),
            PolicyPlayer::R2(policy) => policy.submit_round(obs),
        }
    }

    /// Consumes the player into a harness [`RoundStrategy`] adapter.
    pub fn into_strategy(self) -> Box<dyn RoundStrategy> {
        match self {
            PolicyPlayer::Frozen(bot) => Box::new(crate::bot::BotStrategy(bot)),
            PolicyPlayer::R2(policy) => Box::new(R2Strategy(policy)),
        }
    }
}

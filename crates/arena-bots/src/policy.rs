//! Independent policy identity (B1, `docs/balance/ai-balance-plan.md` section 5.1,
//! ADR-0015): balance experiments address strategies as POLICIES with their
//! own `policy_id`, `family`, `version` and structured parameters — not as
//! the frozen P2 `BotKind` registry. This module is a PARALLEL type: for
//! the five frozen m6.0 baselines it delegates construction to the
//! untouched `make_bot` and adds no behavior.
//!
//! The five [`PolicyId`]s that mirror the m6.0 baselines reuse the
//! `BotKind::tag` strings (artifact vocabulary continuity with
//! `runs/m6-r1/`); their stream kind-tags are permanently bytes 1..=5.
//! B2 (ADR-0016) adds the six competitive R2 policies of plan section 6.1
//! plus the DIAGNOSTIC `idle` opponent — fresh tags, version `r2.0`,
//! fresh stream kind-tag bytes 6..=12 — with their parameters recorded in
//! full-value canonical form ([`PolicyParameters::tag`]). B3 (ADR-0017)
//! appends the three DIAGNOSTIC ablation arms (version `b3.0` for the
//! Hybrid arms, `r2.0` + stream kind-tag 13 for the canonical arm). The
//! m6 mapping stays total and injective over [`PolicyId::MIRRORED`]; R2
//! and ablation policies have no `BotKind` ([`PolicyId::bot_kind`] is
//! `Option`).

use arena_core::config::GameConfig;
use arena_core::map::MapDefinition;
use arena_core::types::Faction;

use crate::ablation::{
    B3_VERSION, GreedyCoordinatedCanonical, HybridAblation, HybridAblationParams,
    canonical_core_params,
};
use crate::bot::{BOT_VERSION, BotKind, make_bot};
use crate::r2::adaptive_hybrid_r2::{AdaptiveHybridParams, AdaptiveHybridR2};
use crate::r2::blocker_builder::{BlockerBuilder, BlockerBuilderParams};
use crate::r2::build::CoreParams;
use crate::r2::builder_defender::{BuilderDefender, BuilderDefenderParams};
use crate::r2::builder_disruptor::{BuilderDisruptor, BuilderDisruptorParams};
use crate::r2::greedy_coordinated::GreedyCoordinated;
use crate::r2::idle::IdlePolicy;
use crate::r2::logistics_builder::{LogisticsBuilder, LogisticsBuilderParams};

/// A strategy's identity in a balance experiment (ADR-0015/0016/0017). The
/// five `MIRRORED` ids map one-to-one onto the frozen m6.0 baselines
/// (total and injective, pinned by the tests); the seven R2 ids are the
/// six plan-6.1 policies plus the diagnostic idle opponent; the three
/// ablation ids are the B3 measurement arms (ADR-0017).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PolicyId {
    Random,
    NearestPortal,
    GreedyField,
    DestroyAnchor,
    Hybrid,
    GreedyCoordinatedR2,
    BuilderDefenderR2,
    BuilderDisruptorR2,
    LogisticsBuilderR2,
    BlockerBuilderR2,
    AdaptiveHybridR2,
    Idle,
    HybridBuilderOnly,
    HybridBuilderAttack,
    GreedyCoordinatedCanonical,
}

impl PolicyId {
    /// The frozen m6.0 mirror registry, declaration order (the B1
    /// registry; `frozen_m6_specs()` and the lockstep pin iterate THIS
    /// array, never [`PolicyId::ALL`]).
    pub const MIRRORED: [PolicyId; 5] = [
        PolicyId::Random,
        PolicyId::NearestPortal,
        PolicyId::GreedyField,
        PolicyId::DestroyAnchor,
        PolicyId::Hybrid,
    ];

    /// The six competitive R2 policies of plan section 6.1, declaration
    /// order (the bot-RNG stream kind-tag order 6..=11, ADR-0016).
    pub const R2_COMPETITIVE: [PolicyId; 6] = [
        PolicyId::GreedyCoordinatedR2,
        PolicyId::BuilderDefenderR2,
        PolicyId::BuilderDisruptorR2,
        PolicyId::LogisticsBuilderR2,
        PolicyId::BlockerBuilderR2,
        PolicyId::AdaptiveHybridR2,
    ];

    /// The B3 ablation-arm registry (ADR-0017), declaration order.
    pub const ABLATION: [PolicyId; 3] = [
        PolicyId::HybridBuilderOnly,
        PolicyId::HybridBuilderAttack,
        PolicyId::GreedyCoordinatedCanonical,
    ];

    /// The full registry (B2 + B3), declaration order.
    pub const ALL: [PolicyId; 15] = [
        PolicyId::Random,
        PolicyId::NearestPortal,
        PolicyId::GreedyField,
        PolicyId::DestroyAnchor,
        PolicyId::Hybrid,
        PolicyId::GreedyCoordinatedR2,
        PolicyId::BuilderDefenderR2,
        PolicyId::BuilderDisruptorR2,
        PolicyId::LogisticsBuilderR2,
        PolicyId::BlockerBuilderR2,
        PolicyId::AdaptiveHybridR2,
        PolicyId::Idle,
        PolicyId::HybridBuilderOnly,
        PolicyId::HybridBuilderAttack,
        PolicyId::GreedyCoordinatedCanonical,
    ];

    /// Stable lowercase tag. The mirrored ids reuse the frozen
    /// `BotKind::tag` strings; the R2 ids carry the plan section 6.1 tags;
    /// the ablation ids carry the ADR-0017 arm tags.
    pub fn tag(self) -> &'static str {
        match self {
            PolicyId::Random => "random",
            PolicyId::NearestPortal => "nearest-portal",
            PolicyId::GreedyField => "greedy-field",
            PolicyId::DestroyAnchor => "destroy-anchor",
            PolicyId::Hybrid => "hybrid",
            PolicyId::GreedyCoordinatedR2 => "greedy-coordinated-r2",
            PolicyId::BuilderDefenderR2 => "builder-defender-r2",
            PolicyId::BuilderDisruptorR2 => "builder-disruptor-r2",
            PolicyId::LogisticsBuilderR2 => "logistics-builder-r2",
            PolicyId::BlockerBuilderR2 => "blocker-builder-r2",
            PolicyId::AdaptiveHybridR2 => "adaptive-hybrid-r2",
            PolicyId::Idle => "idle",
            PolicyId::HybridBuilderOnly => "hybrid-builder-only",
            PolicyId::HybridBuilderAttack => "hybrid-builder-attack",
            PolicyId::GreedyCoordinatedCanonical => "greedy-coordinated-canonical",
        }
    }

    /// Parses [`PolicyId::tag`] back (record round-trips).
    pub fn from_tag(tag: &str) -> Option<PolicyId> {
        match tag {
            "random" => Some(PolicyId::Random),
            "nearest-portal" => Some(PolicyId::NearestPortal),
            "greedy-field" => Some(PolicyId::GreedyField),
            "destroy-anchor" => Some(PolicyId::DestroyAnchor),
            "hybrid" => Some(PolicyId::Hybrid),
            "greedy-coordinated-r2" => Some(PolicyId::GreedyCoordinatedR2),
            "builder-defender-r2" => Some(PolicyId::BuilderDefenderR2),
            "builder-disruptor-r2" => Some(PolicyId::BuilderDisruptorR2),
            "logistics-builder-r2" => Some(PolicyId::LogisticsBuilderR2),
            "blocker-builder-r2" => Some(PolicyId::BlockerBuilderR2),
            "adaptive-hybrid-r2" => Some(PolicyId::AdaptiveHybridR2),
            "idle" => Some(PolicyId::Idle),
            "hybrid-builder-only" => Some(PolicyId::HybridBuilderOnly),
            "hybrid-builder-attack" => Some(PolicyId::HybridBuilderAttack),
            "greedy-coordinated-canonical" => Some(PolicyId::GreedyCoordinatedCanonical),
            _ => None,
        }
    }

    /// The frozen m6.0 bot this policy executes (`None` for R2 policies,
    /// which are not `Bot` implementations — ADR-0016). Total and
    /// injective over [`PolicyId::MIRRORED`] (pinned by the tests).
    pub fn bot_kind(self) -> Option<BotKind> {
        match self {
            PolicyId::Random => Some(BotKind::Random),
            PolicyId::NearestPortal => Some(BotKind::NearestPortal),
            PolicyId::GreedyField => Some(BotKind::GreedyField),
            PolicyId::DestroyAnchor => Some(BotKind::DestroyAnchor),
            PolicyId::Hybrid => Some(BotKind::Hybrid),
            _ => None,
        }
    }

    /// Inverse of [`PolicyId::bot_kind`] over the mirrored registry.
    pub fn from_kind(kind: BotKind) -> PolicyId {
        match kind {
            BotKind::Random => PolicyId::Random,
            BotKind::NearestPortal => PolicyId::NearestPortal,
            BotKind::GreedyField => PolicyId::GreedyField,
            BotKind::DestroyAnchor => PolicyId::DestroyAnchor,
            BotKind::Hybrid => PolicyId::Hybrid,
        }
    }

    /// The plan section 6.1 strategy family of this policy. Random,
    /// NearestPortal, DestroyAnchor and Idle are DIAGNOSTIC policies:
    /// they stay in the pool but never enter the competitive-balance
    /// summaries.
    pub fn family(self) -> PolicyFamily {
        match self {
            PolicyId::Random => PolicyFamily::Random,
            PolicyId::NearestPortal => PolicyFamily::Nearest,
            PolicyId::GreedyField => PolicyFamily::FieldBuilder,
            PolicyId::DestroyAnchor => PolicyFamily::AnchorDestroyer,
            PolicyId::Hybrid => PolicyFamily::AdaptiveHybrid,
            PolicyId::GreedyCoordinatedR2 => PolicyFamily::FieldBuilder,
            PolicyId::BuilderDefenderR2 => PolicyFamily::Defender,
            PolicyId::BuilderDisruptorR2 => PolicyFamily::Disruptor,
            PolicyId::LogisticsBuilderR2 => PolicyFamily::KeyRunner,
            PolicyId::BlockerBuilderR2 => PolicyFamily::Blocker,
            PolicyId::AdaptiveHybridR2 => PolicyFamily::AdaptiveHybrid,
            PolicyId::Idle => PolicyFamily::Idle,
            PolicyId::HybridBuilderOnly => PolicyFamily::Ablation,
            PolicyId::HybridBuilderAttack => PolicyFamily::Ablation,
            PolicyId::GreedyCoordinatedCanonical => PolicyFamily::Ablation,
        }
    }

    /// The frozen m6.0 policy spec of a mirrored id (B1's only
    /// parameters).
    pub fn frozen_m6_spec(self) -> PolicySpec {
        PolicySpec {
            policy_id: self,
            family: self.family(),
            version: BOT_VERSION,
            parameters: PolicyParameters::FrozenM6,
        }
    }

    /// The default r2.0 policy spec of an R2 id (recorded parameters at
    /// their initial values; every value round-trips through
    /// [`PolicyParameters::tag`]).
    pub fn r2_spec(self) -> PolicySpec {
        let parameters = match self {
            PolicyId::GreedyCoordinatedR2 => {
                PolicyParameters::GreedyCoordinated(CoreParams::default())
            }
            PolicyId::BuilderDefenderR2 => {
                PolicyParameters::BuilderDefender(BuilderDefenderParams::default())
            }
            PolicyId::BuilderDisruptorR2 => {
                PolicyParameters::BuilderDisruptor(BuilderDisruptorParams::default())
            }
            PolicyId::LogisticsBuilderR2 => {
                PolicyParameters::LogisticsBuilder(LogisticsBuilderParams::default())
            }
            PolicyId::BlockerBuilderR2 => {
                PolicyParameters::BlockerBuilder(BlockerBuilderParams::default())
            }
            PolicyId::AdaptiveHybridR2 => {
                PolicyParameters::AdaptiveHybridR2(AdaptiveHybridParams::default())
            }
            PolicyId::Idle => PolicyParameters::Idle,
            mirrored => return mirrored.frozen_m6_spec(),
        };
        PolicySpec {
            policy_id: self,
            family: self.family(),
            version: crate::r2::R2_VERSION,
            parameters,
        }
    }

    /// The default B3 ablation-arm spec of an ABLATION id (ADR-0017):
    /// version `b3.0` for the two Hybrid arms, `r2.0` for the canonical
    /// arm (it IS the r2 core); family always `ablation` (diagnostic).
    pub fn ablation_spec(self) -> PolicySpec {
        let parameters = match self {
            PolicyId::HybridBuilderOnly => {
                PolicyParameters::HybridAblation(HybridAblationParams::builder_only())
            }
            PolicyId::HybridBuilderAttack => {
                PolicyParameters::HybridAblation(HybridAblationParams::builder_attack())
            }
            PolicyId::GreedyCoordinatedCanonical => {
                PolicyParameters::GreedyCoordinated(canonical_core_params())
            }
            other => {
                unreachable!("{} is not an ablation id (use ABLATION)", other.tag())
            }
        };
        PolicySpec {
            policy_id: self,
            family: self.family(),
            version: match self {
                PolicyId::GreedyCoordinatedCanonical => crate::r2::R2_VERSION,
                _ => B3_VERSION,
            },
            parameters,
        }
    }
}

/// The plan section 6.1 strategy families.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PolicyFamily {
    /// Diagnostic: no scoring path.
    Random,
    /// Diagnostic: no scoring path.
    Nearest,
    /// Diagnostic: no scoring path.
    AnchorDestroyer,
    /// Competitive: pure field construction.
    FieldBuilder,
    /// Competitive: dynamic role allocation.
    AdaptiveHybrid,
    /// Competitive: builder + anchor defense (B2).
    Defender,
    /// Competitive: builder + attack (B2).
    Disruptor,
    /// Competitive: key/supply transport + builder (B2).
    KeyRunner,
    /// Competitive: geometric denial + builder (B2).
    Blocker,
    /// Diagnostic: the plan-10.1 idle opponent (B2).
    Idle,
    /// Diagnostic: the B3 default-config ablation arms (ADR-0017) —
    /// measurement policies never enter competitive summaries.
    Ablation,
}

impl PolicyFamily {
    /// The full family registry, declaration order.
    pub const ALL: [PolicyFamily; 11] = [
        PolicyFamily::Random,
        PolicyFamily::Nearest,
        PolicyFamily::AnchorDestroyer,
        PolicyFamily::FieldBuilder,
        PolicyFamily::AdaptiveHybrid,
        PolicyFamily::Defender,
        PolicyFamily::Disruptor,
        PolicyFamily::KeyRunner,
        PolicyFamily::Blocker,
        PolicyFamily::Idle,
        PolicyFamily::Ablation,
    ];

    /// Stable lowercase tag for artifacts and record round-trips.
    pub fn tag(self) -> &'static str {
        match self {
            PolicyFamily::Random => "random",
            PolicyFamily::Nearest => "nearest",
            PolicyFamily::AnchorDestroyer => "anchor-destroyer",
            PolicyFamily::FieldBuilder => "field-builder",
            PolicyFamily::AdaptiveHybrid => "adaptive-hybrid",
            PolicyFamily::Defender => "defender",
            PolicyFamily::Disruptor => "disruptor",
            PolicyFamily::KeyRunner => "key-runner",
            PolicyFamily::Blocker => "blocker",
            PolicyFamily::Idle => "idle",
            PolicyFamily::Ablation => "ablation",
        }
    }

    /// Parses [`PolicyFamily::tag`] back.
    pub fn from_tag(tag: &str) -> Option<PolicyFamily> {
        match tag {
            "random" => Some(PolicyFamily::Random),
            "nearest" => Some(PolicyFamily::Nearest),
            "anchor-destroyer" => Some(PolicyFamily::AnchorDestroyer),
            "field-builder" => Some(PolicyFamily::FieldBuilder),
            "adaptive-hybrid" => Some(PolicyFamily::AdaptiveHybrid),
            "defender" => Some(PolicyFamily::Defender),
            "disruptor" => Some(PolicyFamily::Disruptor),
            "key-runner" => Some(PolicyFamily::KeyRunner),
            "blocker" => Some(PolicyFamily::Blocker),
            "idle" => Some(PolicyFamily::Idle),
            "ablation" => Some(PolicyFamily::Ablation),
            _ => None,
        }
    }

    /// Diagnostic families stay out of the competitive-balance summaries
    /// (plan section 6.1).
    pub fn is_diagnostic(self) -> bool {
        matches!(
            self,
            PolicyFamily::Random
                | PolicyFamily::Nearest
                | PolicyFamily::AnchorDestroyer
                | PolicyFamily::Idle
                | PolicyFamily::Ablation
        )
    }
}

/// A policy's structured parameters (plan sections 5.1/6.3, ADR-0016).
/// Every threshold of every policy lives in its variant — nowhere else —
/// and [`PolicyParameters::tag`] serializes the COMPLETE value set
/// canonically, so `policies.tsv` carries every strategy parameter and
/// records round-trip with identity sensitivity (an edited parameter
/// string parses back to a different parameter set, never silently to
/// the defaults).
///
/// Version semantics are additive (ADR-0015): new behavior arrives as a
/// NEW variant, never as an edit of `FrozenM6` or of an existing R2
/// parameter set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PolicyParameters {
    /// The frozen m6.0 baseline: no tunable parameters, byte-identical to
    /// `make_bot` of the mirrored kind.
    FrozenM6,
    /// `greedy-coordinated-r2`: the shared builder core alone.
    GreedyCoordinated(CoreParams),
    /// `builder-defender-r2`: core + defense overlay.
    BuilderDefender(BuilderDefenderParams),
    /// `builder-disruptor-r2`: core + attack overlay.
    BuilderDisruptor(BuilderDisruptorParams),
    /// `logistics-builder-r2`: core + key/supply transport overlay.
    LogisticsBuilder(LogisticsBuilderParams),
    /// `blocker-builder-r2`: core + geometric denial overlay.
    BlockerBuilder(BlockerBuilderParams),
    /// `adaptive-hybrid-r2`: core + phase/score/threat role switching.
    AdaptiveHybridR2(AdaptiveHybridParams),
    /// The B3 Hybrid ablation arms (`hybrid-builder-only` /
    /// `hybrid-builder-attack`, ADR-0017): the frozen round body minus the
    /// toggled-off role branches.
    HybridAblation(HybridAblationParams),
    /// The diagnostic idle opponent: no parameters.
    Idle,
}

impl PolicyParameters {
    /// Canonical full-value serialization. `<policy-tag>|<params-tag>`
    /// for the R2 variants (`<params-tag>` itself inlines the shared
    /// `CoreParams` and `ValueParams`); `frozen-m6` and `idle` are bare.
    pub fn tag(&self) -> String {
        match self {
            PolicyParameters::FrozenM6 => "frozen-m6".to_string(),
            PolicyParameters::Idle => "idle".to_string(),
            PolicyParameters::GreedyCoordinated(p) => {
                format!("greedy-coordinated-r2|{}", p.tag())
            }
            PolicyParameters::BuilderDefender(p) => format!("builder-defender-r2|{}", p.tag()),
            PolicyParameters::BuilderDisruptor(p) => {
                format!("builder-disruptor-r2|{}", p.tag())
            }
            PolicyParameters::LogisticsBuilder(p) => {
                format!("logistics-builder-r2|{}", p.tag())
            }
            PolicyParameters::BlockerBuilder(p) => format!("blocker-builder-r2|{}", p.tag()),
            PolicyParameters::AdaptiveHybridR2(p) => format!("adaptive-hybrid-r2|{}", p.tag()),
            PolicyParameters::HybridAblation(p) => format!("hybrid-ablation|{}", p.tag()),
        }
    }

    /// Parses [`PolicyParameters::tag`] back — strictly canonical; a
    /// mismatched variant prefix or a malformed parameter body is `None`.
    pub fn from_tag(tag: &str) -> Option<PolicyParameters> {
        if tag == "frozen-m6" {
            return Some(PolicyParameters::FrozenM6);
        }
        if tag == "idle" {
            return Some(PolicyParameters::Idle);
        }
        let (prefix, body) = tag.split_once('|')?;
        match prefix {
            "greedy-coordinated-r2" => {
                CoreParams::from_tag(body).map(PolicyParameters::GreedyCoordinated)
            }
            "builder-defender-r2" => {
                BuilderDefenderParams::from_tag(body).map(PolicyParameters::BuilderDefender)
            }
            "builder-disruptor-r2" => {
                BuilderDisruptorParams::from_tag(body).map(PolicyParameters::BuilderDisruptor)
            }
            "logistics-builder-r2" => {
                LogisticsBuilderParams::from_tag(body).map(PolicyParameters::LogisticsBuilder)
            }
            "blocker-builder-r2" => {
                BlockerBuilderParams::from_tag(body).map(PolicyParameters::BlockerBuilder)
            }
            "adaptive-hybrid-r2" => {
                AdaptiveHybridParams::from_tag(body).map(PolicyParameters::AdaptiveHybridR2)
            }
            "hybrid-ablation" => {
                HybridAblationParams::from_tag(body).map(PolicyParameters::HybridAblation)
            }
            _ => None,
        }
    }
}

/// Everything a balance experiment needs to know about one strategy (plan
/// section 5.3): independent identity, family, fixed version and
/// structured parameters. The single global `BOT_VERSION` keeps
/// representing ONLY the m6.0 baselines (ADR-0015); every R2 policy
/// carries `crate::r2::R2_VERSION` (ADR-0016).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PolicySpec {
    pub policy_id: PolicyId,
    pub family: PolicyFamily,
    /// Fixed version string of this policy's algorithm; two specs with
    /// the same `policy_id` and different versions are different strategies.
    pub version: &'static str,
    pub parameters: PolicyParameters,
}

impl PolicySpec {
    /// Structural consistency of the spec: the parameters variant belongs
    /// to exactly one policy id, the version matches the variant's era,
    /// and the family is the policy's own.
    pub fn validate(&self) -> Result<(), String> {
        let era_ok = match self.parameters {
            PolicyParameters::FrozenM6 => {
                self.version == BOT_VERSION && self.policy_id.bot_kind().is_some()
            }
            PolicyParameters::Idle => {
                self.version == crate::r2::R2_VERSION && self.policy_id == PolicyId::Idle
            }
            PolicyParameters::HybridAblation(_) => {
                self.version == B3_VERSION && self.policy_id.bot_kind().is_none()
            }
            PolicyParameters::GreedyCoordinated(_)
            | PolicyParameters::BuilderDefender(_)
            | PolicyParameters::BuilderDisruptor(_)
            | PolicyParameters::LogisticsBuilder(_)
            | PolicyParameters::BlockerBuilder(_)
            | PolicyParameters::AdaptiveHybridR2(_) => {
                self.version == crate::r2::R2_VERSION && self.policy_id.bot_kind().is_none()
            }
        };
        if !era_ok {
            return Err(format!(
                "policy {}: parameters {:?} do not belong to version {}",
                self.policy_id.tag(),
                self.parameters.tag(),
                self.version
            ));
        }
        if self.family != self.policy_id.family() {
            return Err(format!(
                "policy {} belongs to family {}, spec says {}",
                self.policy_id.tag(),
                self.policy_id.family().tag(),
                self.family.tag()
            ));
        }
        // Parameter-set identity: each R2 parameters variant exists for
        // exactly its own policy id (the ablation canonical arm shares
        // the r2 core's parameter variant — its whole point — and the
        // Hybrid ablation variant exists for exactly the two Hybrid
        // arms).
        let matches_id = match (self.policy_id, &self.parameters) {
            (PolicyId::GreedyCoordinatedR2, PolicyParameters::GreedyCoordinated(_))
            | (PolicyId::GreedyCoordinatedCanonical, PolicyParameters::GreedyCoordinated(_))
            | (PolicyId::BuilderDefenderR2, PolicyParameters::BuilderDefender(_))
            | (PolicyId::BuilderDisruptorR2, PolicyParameters::BuilderDisruptor(_))
            | (PolicyId::LogisticsBuilderR2, PolicyParameters::LogisticsBuilder(_))
            | (PolicyId::BlockerBuilderR2, PolicyParameters::BlockerBuilder(_))
            | (PolicyId::AdaptiveHybridR2, PolicyParameters::AdaptiveHybridR2(_))
            | (PolicyId::Idle, PolicyParameters::Idle) => true,
            // The two Hybrid arms share ONE parameters variant: the roles
            // value inside it is the arm — a builder-only id carrying the
            // builder-attack roles would be a mislabeled policy, refused.
            (
                PolicyId::HybridBuilderOnly,
                PolicyParameters::HybridAblation(HybridAblationParams {
                    roles: crate::ablation::HybridAblationRoles::BuilderOnly,
                    ..
                }),
            )
            | (
                PolicyId::HybridBuilderAttack,
                PolicyParameters::HybridAblation(HybridAblationParams {
                    roles: crate::ablation::HybridAblationRoles::BuilderAttack,
                    ..
                }),
            ) => true,
            (id, PolicyParameters::FrozenM6) => id.bot_kind().is_some(),
            _ => false,
        };
        if !matches_id {
            return Err(format!(
                "policy {} cannot carry parameters {:?}",
                self.policy_id.tag(),
                self.parameters.tag()
            ));
        }
        Ok(())
    }
}

/// The five frozen m6.0 policy specs, registry order.
pub fn frozen_m6_specs() -> Vec<PolicySpec> {
    PolicyId::MIRRORED
        .iter()
        .map(|id| id.frozen_m6_spec())
        .collect()
}

/// The seven B2 R2 policy specs (six competitive + the diagnostic idle),
/// declaration order (plan section 6.1 / ADR-0016).
pub fn r2_specs() -> Vec<PolicySpec> {
    PolicyId::R2_COMPETITIVE
        .iter()
        .map(|id| id.r2_spec())
        .chain([PolicyId::Idle.r2_spec()])
        .collect()
}

/// The three B3 ablation-arm specs (ADR-0017), declaration order.
pub fn ablation_specs() -> Vec<PolicySpec> {
    PolicyId::ABLATION
        .iter()
        .map(|id| id.ablation_spec())
        .collect()
}

/// Constructs one faction's player for a policy spec: a frozen m6.0
/// baseline delegates 1:1 to the untouched `make_bot` (the executable
/// behavior pin is the lockstep test below); an R2 policy is built from
/// its own parameters. The executor addresses both through
/// [`crate::r2::PolicyPlayer`] and never branches on policy era.
///
/// # Panics
///
/// A structurally inconsistent spec (see [`PolicySpec::validate`]) or an
/// id without a construction arm is a programmer error and panics,
/// mirroring the `make_bot`/spawn contracts.
pub fn make_policy_player(
    spec: &PolicySpec,
    faction: Faction,
    bot_seed: u64,
    cfg: &GameConfig,
    map: &MapDefinition,
) -> crate::r2::PolicyPlayer {
    spec.validate().expect("policy spec");
    match (spec.policy_id, &spec.parameters) {
        (id, PolicyParameters::FrozenM6) => crate::r2::PolicyPlayer::Frozen(make_bot(
            id.bot_kind().expect("validated: mirrored id"),
            faction,
            bot_seed,
            cfg,
            map,
        )),
        // The canonical ablation arm shares the r2 core's parameter
        // variant, so its (more specific) arm must precede the general
        // r2 construction arm.
        (PolicyId::GreedyCoordinatedCanonical, PolicyParameters::GreedyCoordinated(params)) => {
            crate::r2::PolicyPlayer::R2(Box::new(GreedyCoordinatedCanonical::new(
                faction, bot_seed, cfg, *params,
            )))
        }
        (_, PolicyParameters::GreedyCoordinated(params)) => crate::r2::PolicyPlayer::R2(Box::new(
            GreedyCoordinated::new(faction, bot_seed, cfg, *params),
        )),
        (_, PolicyParameters::BuilderDefender(params)) => crate::r2::PolicyPlayer::R2(Box::new(
            BuilderDefender::new(faction, bot_seed, cfg, *params),
        )),
        (_, PolicyParameters::BuilderDisruptor(params)) => crate::r2::PolicyPlayer::R2(Box::new(
            BuilderDisruptor::new(faction, bot_seed, cfg, *params),
        )),
        (_, PolicyParameters::LogisticsBuilder(params)) => crate::r2::PolicyPlayer::R2(Box::new(
            LogisticsBuilder::new(faction, bot_seed, cfg, *params),
        )),
        (_, PolicyParameters::BlockerBuilder(params)) => crate::r2::PolicyPlayer::R2(Box::new(
            BlockerBuilder::new(faction, bot_seed, cfg, *params),
        )),
        (_, PolicyParameters::AdaptiveHybridR2(params)) => crate::r2::PolicyPlayer::R2(Box::new(
            AdaptiveHybridR2::new(faction, bot_seed, cfg, *params),
        )),
        (_, PolicyParameters::HybridAblation(params)) => crate::r2::PolicyPlayer::R2(Box::new(
            HybridAblation::new(faction, bot_seed, cfg, map, *params),
        )),
        (_, PolicyParameters::Idle) => crate::r2::PolicyPlayer::R2(Box::new(IdlePolicy)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::{Bot, BotStrategy};
    use arena_core::harness::MatchHarness;
    use arena_core::resolve::RoundReport;
    use arena_core::spawn::validate_initial_spawn_set;
    use arena_core::state::GameState;
    use arena_core::types::Score2;

    fn all_specs() -> Vec<PolicySpec> {
        frozen_m6_specs()
    }

    // (a) Every policy id has a spec with a non-empty id/family tag; the
    // m6.0 mirror carries version exactly "m6.0", every R2 id exactly
    // "r2.0" with its own parameters variant.
    #[test]
    fn specs_carry_identity_family_and_exact_versions() {
        assert_eq!(all_specs().len(), 5);
        for spec in all_specs() {
            assert!(!spec.policy_id.tag().is_empty());
            assert!(!spec.family.tag().is_empty());
            assert_eq!(spec.version, "m6.0");
            assert_eq!(spec.version, BOT_VERSION);
            assert_eq!(spec.parameters, PolicyParameters::FrozenM6);
            assert_eq!(spec.validate(), Ok(()));
        }
        let r2 = r2_specs();
        assert_eq!(r2.len(), 7);
        for spec in &r2 {
            assert_eq!(spec.version, crate::r2::R2_VERSION);
            assert_eq!(spec.version, "r2.0");
            assert_eq!(spec.validate(), Ok(()));
            assert!(
                spec.policy_id == PolicyId::Idle || !spec.family.is_diagnostic(),
                "the six R2 policies are competitive"
            );
        }
        assert_eq!(PolicyId::Idle.r2_spec().family, PolicyFamily::Idle);
        assert!(PolicyFamily::Idle.is_diagnostic());
        // B3 (ADR-0017): the three ablation arms register with their own
        // versions and validate.
        let ablation = ablation_specs();
        assert_eq!(ablation.len(), 3);
        for spec in &ablation {
            assert_eq!(spec.validate(), Ok(()));
            assert!(spec.family.is_diagnostic());
            assert!(
                spec.policy_id == PolicyId::GreedyCoordinatedCanonical
                    || spec.version == B3_VERSION,
                "hybrid arms are b3.0; the canonical arm is r2.0"
            );
        }
        assert_eq!(
            PolicyId::GreedyCoordinatedCanonical.ablation_spec().version,
            crate::r2::R2_VERSION
        );
    }

    // (b) tag/from_tag round-trips for every PolicyId, PolicyFamily and
    // PolicyParameters; registry tags are distinct; parameter records
    // round-trip with identity sensitivity.
    #[test]
    fn tags_round_trip_for_every_id_family_and_parameters() {
        assert_eq!(PolicyId::ALL.len(), 15, "5 m6 mirrors + 7 R2 + 3 ablation");
        assert_eq!(
            PolicyId::MIRRORED.len(),
            5,
            "the m6 mirror registry is untouched"
        );
        assert_eq!(
            PolicyId::R2_COMPETITIVE.len(),
            6,
            "the B2 registry is untouched"
        );
        assert_eq!(PolicyId::ABLATION.len(), 3);
        assert_eq!(PolicyFamily::ALL.len(), 11);
        for id in PolicyId::ALL {
            assert_eq!(PolicyId::from_tag(id.tag()), Some(id));
        }
        assert_eq!(PolicyId::from_tag("unknown"), None);
        for family in PolicyFamily::ALL {
            assert_eq!(PolicyFamily::from_tag(family.tag()), Some(family));
        }
        assert_eq!(
            PolicyFamily::from_tag("field-builder"),
            Some(PolicyFamily::FieldBuilder)
        );
        assert_eq!(
            PolicyFamily::from_tag("ablation"),
            Some(PolicyFamily::Ablation)
        );
        assert_eq!(
            PolicyParameters::from_tag(&PolicyParameters::FrozenM6.tag()),
            Some(PolicyParameters::FrozenM6)
        );
        assert_eq!(
            PolicyParameters::from_tag(&PolicyParameters::Idle.tag()),
            Some(PolicyParameters::Idle)
        );
        assert_eq!(PolicyParameters::from_tag("frozen-m6-v2"), None);
        let mut tags: Vec<&str> = PolicyId::ALL.iter().map(|id| id.tag()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), PolicyId::ALL.len(), "policy tags distinct");
        // Diagnostic families are exactly the three m6 probes, Idle and
        // the B3 ablation family (ADR-0017).
        let diagnostic: Vec<PolicyFamily> = PolicyFamily::ALL
            .iter()
            .copied()
            .filter(|f| f.is_diagnostic())
            .collect();
        assert_eq!(
            diagnostic,
            vec![
                PolicyFamily::Random,
                PolicyFamily::Nearest,
                PolicyFamily::AnchorDestroyer,
                PolicyFamily::Idle,
                PolicyFamily::Ablation
            ]
        );
        // Six competitive families are represented by the R2 registry.
        let mut competitive: Vec<PolicyFamily> = PolicyId::R2_COMPETITIVE
            .iter()
            .map(|id| id.family())
            .collect();
        competitive.sort();
        competitive.dedup();
        assert_eq!(competitive.len(), 6, "six distinct competitive families");
    }

    // R2 parameter records carry the full value set and parse back
    // exactly; an edited record parses to different parameters or not at
    // all (identity sensitivity, ADR-0016).
    #[test]
    fn r2_parameter_records_round_trip_with_identity_sensitivity() {
        for spec in r2_specs() {
            let tag = spec.parameters.tag();
            assert_eq!(
                PolicyParameters::from_tag(&tag),
                Some(spec.parameters),
                "{} round-trips",
                spec.policy_id.tag()
            );
        }
        // Edited values change the parsed parameter set...
        let tuned = PolicyParameters::BuilderDefender(BuilderDefenderParams {
            defense_task_cap: 2,
            ..BuilderDefenderParams::default()
        });
        assert_ne!(
            PolicyParameters::from_tag(&tuned.tag()),
            Some(PolicyId::BuilderDefenderR2.r2_spec().parameters)
        );
        assert_eq!(PolicyParameters::from_tag(&tuned.tag()), Some(tuned));
        // ...and malformed records never parse.
        for malformed in [
            "",
            "greedy-coordinated-r2",
            "greedy-coordinated-r2|nearequal=100",
            "builder-defender-r2|wrong=1|core=x",
            "greedy-coordinated-r2|nearequal=100;retry=25;fbpri=1|value=horizon=100",
            "idle|",
        ] {
            assert_eq!(
                PolicyParameters::from_tag(malformed),
                None,
                "malformed record {malformed:?} must not parse"
            );
        }
    }

    // (c) PolicyId -> BotKind is total and injective over the MIRRORED
    // registry (and None everywhere else), and the mirrored kinds own the
    // frozen bot-RNG kind-tag bytes 1..=5 while the R2 ids own 6..=12
    // (ADR-0015/0016 namespace rule).
    #[test]
    fn policy_id_maps_total_and_injectively_onto_bot_kinds() {
        let mut kinds = Vec::new();
        for id in PolicyId::MIRRORED {
            let kind = id.bot_kind().expect("mirrored ids carry a kind");
            assert_eq!(PolicyId::from_kind(kind), id, "inverse round-trips");
            kinds.push(kind);
        }
        let mut distinct = kinds.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(kinds.len(), distinct.len(), "mapping is injective");
        assert_eq!(kinds.len(), BotKind::ALL.len(), "mapping is total");
        for id in PolicyId::ALL {
            if PolicyId::MIRRORED.contains(&id) {
                continue;
            }
            assert_eq!(id.bot_kind(), None, "R2 policy {} has no BotKind", id.tag());
        }
        let mut tags: Vec<u8> = BotKind::ALL
            .iter()
            .map(|k| crate::rng::kind_tag(*k))
            .collect();
        tags.sort_unstable();
        assert_eq!(tags, vec![1, 2, 3, 4, 5], "m6 kinds own kind tags 1-5");
        // The R2 ids map 1:1 onto the R2 stream registry (kind tags
        // 6..=12 in declaration order).
        for (id, stream) in PolicyId::R2_COMPETITIVE
            .iter()
            .zip(crate::r2::rng::R2Stream::ALL.iter().take(6))
        {
            assert_eq!(id.tag(), stream.tag());
        }
        assert_eq!(PolicyId::Idle.tag(), crate::r2::rng::R2Stream::Idle.tag());
        // The canonical ablation arm owns stream kind tag 13 (ADR-0017);
        // the two Hybrid arms draw no randomness and own no stream.
        assert_eq!(
            PolicyId::GreedyCoordinatedCanonical.tag(),
            crate::r2::rng::R2Stream::GreedyCoordinatedCanonical.tag()
        );
        assert_eq!(
            crate::r2::rng::R2Stream::GreedyCoordinatedCanonical.kind_tag(),
            13
        );
    }

    // A spec with the frozen-m6 parameters but a non-m6 version (or a
    // wrong family, or a foreign parameters variant) is refused —
    // versions are additive, never edited.
    #[test]
    fn inconsistent_specs_are_refused() {
        let mut spec = PolicyId::GreedyField.frozen_m6_spec();
        spec.version = "m6.1";
        assert!(spec.validate().is_err());
        let mut spec = PolicyId::Hybrid.frozen_m6_spec();
        spec.family = PolicyFamily::FieldBuilder;
        assert!(spec.validate().is_err());
        // An R2 id carrying frozen-m6 parameters, and vice versa.
        let mut spec = PolicyId::GreedyCoordinatedR2.r2_spec();
        spec.parameters = PolicyParameters::FrozenM6;
        assert!(spec.validate().is_err());
        let mut spec = PolicyId::Hybrid.frozen_m6_spec();
        spec.parameters = PolicyId::GreedyCoordinatedR2.r2_spec().parameters;
        assert!(spec.validate().is_err());
        // Wrong family on an R2 spec.
        let mut spec = PolicyId::BlockerBuilderR2.r2_spec();
        spec.family = PolicyFamily::FieldBuilder;
        assert!(spec.validate().is_err());
        // Idle is version r2.0 only.
        let mut spec = PolicyId::Idle.r2_spec();
        spec.version = "r2.1";
        assert!(spec.validate().is_err());
    }

    // (d) FROZEN-BEHAVIOR PIN: for a fixed (bot_seed, faction, cfg, map)
    // and the observation sequence of a real match, make_policy_player of
    // a MIRRORED spec behaves EXACTLY like make_bot of the mirrored kind
    // — identical spawn sets and identical per-round submissions all the
    // way through, for all five kinds. This is the executable guarantee
    // that the policy layer changes no m6.0 behavior.
    fn drive_match(
        enl: Box<dyn Bot>,
        res: Box<dyn Bot>,
        game_seed: u64,
        cfg: &GameConfig,
        map: &arena_core::map::MapDefinition,
    ) -> (Vec<RoundReport>, (Score2, Score2)) {
        let mut enl_bot = enl;
        let mut res_bot = res;
        let (enl_roster, res_roster) = GameState::initial_roster(cfg);
        let enl_spawns = validate_initial_spawn_set(
            &enl_roster,
            &enl_bot.choose_spawns(cfg, map, &enl_roster),
            cfg,
        )
        .expect("bot spawn set");
        let res_spawns = validate_initial_spawn_set(
            &res_roster,
            &res_bot.choose_spawns(cfg, map, &res_roster),
            cfg,
        )
        .expect("bot spawn set");
        let mut harness = MatchHarness::new(
            game_seed,
            *cfg,
            map,
            &enl_spawns,
            &res_spawns,
            Box::new(BotStrategy(enl_bot)),
            Box::new(BotStrategy(res_bot)),
        )
        .expect("match builds from validated inputs");
        let mut reports = Vec::new();
        for _ in 0..cfg.total_rounds {
            reports.push(harness.run_round());
        }
        let state = harness.state();
        (
            reports,
            (
                state.score(arena_core::types::Faction::Enl),
                state.score(arena_core::types::Faction::Res),
            ),
        )
    }

    fn policy_as_bot(
        spec: &PolicySpec,
        faction: Faction,
        bot_seed: u64,
        cfg: &GameConfig,
        map: &arena_core::map::MapDefinition,
    ) -> Box<dyn Bot> {
        // The frozen arm of PolicyPlayer delegates 1:1; wrap it in the
        // Bot-facing adapter so the identical harness path can drive it.
        struct PlayerBot(crate::r2::PolicyPlayer);
        impl Bot for PlayerBot {
            fn kind(&self) -> BotKind {
                unreachable!("the frozen arm always carries a kind");
            }
            fn choose_spawns(
                &mut self,
                cfg: &GameConfig,
                map: &MapDefinition,
                roster: &[arena_core::types::AgentId],
            ) -> Vec<(arena_core::types::AgentId, arena_core::types::Coord)> {
                self.0.choose_spawns(cfg, map, roster)
            }
            fn submit_round(
                &mut self,
                obs: &arena_core::observe::Observation,
            ) -> Vec<arena_core::harness::AgentSubmission> {
                self.0.submit_round(obs)
            }
        }
        Box::new(PlayerBot(make_policy_player(
            spec, faction, bot_seed, cfg, map,
        )))
    }

    #[test]
    fn make_policy_player_is_byte_identical_to_make_bot_for_all_five_kinds() {
        let mut cfg = GameConfig::default();
        cfg.total_rounds = 120; // pin depth; the decision function is round-independent
        let map = arena_core::map::generate_map(0xB1_F10E_5EED, &cfg).expect("map generates");
        let bot_seed = 0x0707_0707_0000_00B1;
        let game_seed = 0x6A1E_5EED_0000_0001;
        for id in PolicyId::MIRRORED {
            let spec = id.frozen_m6_spec();
            let (enl_roster, _res_roster) = GameState::initial_roster(&cfg);
            // Identical spawn sets from identical construction inputs.
            let policy_spawns = policy_as_bot(&spec, Faction::Enl, bot_seed, &cfg, &map)
                .choose_spawns(&cfg, &map, &enl_roster);
            let kind_spawns = make_bot(id.bot_kind().unwrap(), Faction::Enl, bot_seed, &cfg, &map)
                .choose_spawns(&cfg, &map, &enl_roster);
            assert_eq!(policy_spawns, kind_spawns, "spawns differ for {}", id.tag());
            // Lockstep matches: policy player as ENL vs kind bot as ENL,
            // same deterministic opponent and seeds — any behavioral
            // divergence surfaces as a differing RoundReport submission
            // at the first divergent round.
            let opponent = if id == PolicyId::Hybrid {
                BotKind::GreedyField
            } else {
                BotKind::Hybrid
            };
            let (policy_reports, policy_scores) = drive_match(
                policy_as_bot(&spec, Faction::Enl, bot_seed, &cfg, &map),
                make_bot(opponent, Faction::Res, bot_seed, &cfg, &map),
                game_seed,
                &cfg,
                &map,
            );
            let (kind_reports, kind_scores) = drive_match(
                make_bot(id.bot_kind().unwrap(), Faction::Enl, bot_seed, &cfg, &map),
                make_bot(opponent, Faction::Res, bot_seed, &cfg, &map),
                game_seed,
                &cfg,
                &map,
            );
            assert_eq!(policy_reports.len(), kind_reports.len());
            for (round, (policy, kind)) in
                policy_reports.iter().zip(kind_reports.iter()).enumerate()
            {
                assert_eq!(
                    policy.submissions,
                    kind.submissions,
                    "policy {} diverged from the frozen kind at round {}",
                    id.tag(),
                    round + 1
                );
                assert_eq!(policy.execution_order, kind.execution_order);
                assert_eq!(policy.score2_delta, kind.score2_delta);
            }
            assert_eq!(
                policy_scores,
                kind_scores,
                "final scores differ for {}",
                id.tag()
            );
        }
    }

    // The spec registries are queryable by id (executor convenience).
    #[test]
    fn frozen_m6_specs_cover_the_mirrored_registry() {
        let specs = frozen_m6_specs();
        for id in PolicyId::MIRRORED {
            assert!(
                specs.iter().any(|s| s.policy_id == id),
                "registry covers {}",
                id.tag()
            );
        }
        for spec in r2_specs() {
            assert!(
                specs.iter().all(|s| s.policy_id != spec.policy_id),
                "R2 ids never leak into the m6 registry"
            );
        }
    }
}

//! The B3 default-config ablation arms (ADR-0017,
//! `docs/balance/ai-balance-plan.md` section 7.2): measurement policies that
//! decompose the frozen m6.0 `HybridBot`'s roles and the r2 core's spawn
//! determinism into separately versioned strategies. Every arm is
//! composed STRICTLY from already-visible pieces of the frozen bots —
//! [`crate::greedy_field::FieldPlanner::agent_commands`],
//! [`crate::destroy_anchor::attack_targets`]/[`attack_commands`],
//! [`crate::common::HackLedger`]/[`crate::common::CommandList`] — with
//! ZERO edits to the eight frozen bot files:
//!
//! - [`HybridAblation`] with [`HybridAblationRoles::BuilderOnly`]: every
//!   agent (reinforcements included) walks the builder path with its own
//!   observation-index station offset — no attack, no defense;
//! - [`HybridAblation`] with [`HybridAblationRoles::BuilderAttack`]: the
//!   frozen trio/rest split with the supply-floor hack preamble and the
//!   attack body, but NO defense dispatch branch — byte-identical to the
//!   frozen `HybridBot` in any match where the defense dispatch cannot
//!   fire (pinned by test);
//! - [`GreedyCoordinatedCanonical`]: the shared r2 builder core with
//!   `near_equal_permille = 0` (only exact ties of the best value stay
//!   eligible) and `spawn_jitter_cm = 0` (spawns exactly on the chosen
//!   triangle's vertices) — the canonical deterministic spawn of plan
//!   7.2's fifth contrast, drawn from its own stream
//!   ([`crate::r2::rng::R2Stream::GreedyCoordinatedCanonical`], kind-tag
//!   13).
//!
//! The Hybrid arms draw no randomness at all (like the frozen Hybrid's
//! unused `_rng`); they accept `bot_seed` only for the era-agnostic
//! construction contract and discard it.

use arena_core::config::GameConfig;
use arena_core::harness::AgentSubmission;
use arena_core::map::MapDefinition;
use arena_core::observe::{AgentPrivate, Observation};
use arena_core::types::{AgentId, Command, Coord, Faction, Owner, PortalId};

use crate::common::{self, CommandList, HackLedger, in_range, nearest_matching, owners};
use crate::destroy_anchor::{attack_commands, attack_targets};
use crate::greedy_field::FieldPlanner;
use crate::r2::R2Policy;
use crate::r2::build::{BuilderCore, CoreParams};
use crate::r2::rng::R2Stream;

/// Fixed version tag of the two Hybrid ablation arms (ADR-0017; the
/// canonical arm is the r2 core and keeps `r2.0`).
pub const B3_VERSION: &str = "b3.0";

/// Which roles of the frozen Hybrid the arm keeps (ADR-0017).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HybridAblationRoles {
    /// Every agent builds (its own observation-index station offset over
    /// the shared tour); no attack, no defense.
    BuilderOnly,
    /// The frozen trio builds, every later agent (reinforcements
    /// included) attacks; NO defense dispatch branch.
    BuilderAttack,
}

impl HybridAblationRoles {
    /// Stable lowercase tag (parameter records).
    pub fn tag(self) -> &'static str {
        match self {
            HybridAblationRoles::BuilderOnly => "builder-only",
            HybridAblationRoles::BuilderAttack => "builder-attack",
        }
    }

    /// Parses [`HybridAblationRoles::tag`] back.
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "builder-only" => Some(HybridAblationRoles::BuilderOnly),
            "builder-attack" => Some(HybridAblationRoles::BuilderAttack),
            _ => None,
        }
    }
}

/// The recorded parameters of the Hybrid ablation arms (plan 6.3,
/// ADR-0017): every threshold is data, serialized in full by
/// [`HybridAblationParams::tag`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HybridAblationParams {
    /// Which roles the arm keeps.
    pub roles: HybridAblationRoles,
    /// How many of the faction's lowest-id agents form the builder trio
    /// (the frozen `hybrid::BUILDER_COUNT`; inert data under
    /// [`HybridAblationRoles::BuilderOnly`], where every agent builds).
    pub builder_count: usize,
    /// Agents below this supply hack an in-range portal before anything
    /// else (the frozen `hybrid::HYBRID_HACK_SUPPLY`).
    pub hack_supply_floor: u32,
}

impl HybridAblationParams {
    /// The `builder-only` arm's frozen parameters.
    pub fn builder_only() -> Self {
        HybridAblationParams {
            roles: HybridAblationRoles::BuilderOnly,
            builder_count: crate::hybrid::BUILDER_COUNT,
            hack_supply_floor: crate::hybrid::HYBRID_HACK_SUPPLY,
        }
    }

    /// The `builder-attack` arm's frozen parameters.
    pub fn builder_attack() -> Self {
        HybridAblationParams {
            roles: HybridAblationRoles::BuilderAttack,
            builder_count: crate::hybrid::BUILDER_COUNT,
            hack_supply_floor: crate::hybrid::HYBRID_HACK_SUPPLY,
        }
    }

    /// Canonical serialization:
    /// `roles=..;builders=..;hacksupply=..`.
    pub fn tag(&self) -> String {
        format!(
            "roles={};builders={};hacksupply={}",
            self.roles.tag(),
            self.builder_count,
            self.hack_supply_floor
        )
    }

    /// Parses [`HybridAblationParams::tag`] back — strictly canonical
    /// (exact keys in order, decimal values, known roles only).
    pub fn from_tag(tag: &str) -> Option<Self> {
        let parts: Vec<&str> = tag.split(';').collect();
        if parts.len() != 3 {
            return None;
        }
        let roles = parts[0].strip_prefix("roles=")?;
        let builders = parts[1].strip_prefix("builders=")?;
        let hacksupply = parts[2].strip_prefix("hacksupply=")?;
        Some(HybridAblationParams {
            roles: HybridAblationRoles::from_tag(roles)?,
            builder_count: builders.parse().ok()?,
            hack_supply_floor: hacksupply.parse().ok()?,
        })
    }
}

/// The frozen-Hybrid role ablation (ADR-0017): the frozen round body
/// minus the toggled-off role branches, composed from the frozen bots'
/// own pieces. The spawn set is the frozen Hybrid's seed-triangle spawn
/// (ladder steps differ by roles only).
pub struct HybridAblation {
    faction: Faction,
    own: Owner,
    cfg: GameConfig,
    params: HybridAblationParams,
    planner: FieldPlanner,
    ledger: HackLedger,
}

impl HybridAblation {
    /// Constructs the arm from the match inputs. The frozen bots build
    /// their `FieldPlanner` from the map at construction time, so this
    /// arm's constructor carries the map (the `make_policy_player`
    /// contract already passes it). `bot_seed` is accepted for the
    /// era-agnostic construction contract and DISCARDED: this arm draws no
    /// randomness (the frozen Hybrid's RNG key is likewise unused).
    pub fn new(
        faction: Faction,
        bot_seed: u64,
        cfg: &GameConfig,
        map: &MapDefinition,
        params: HybridAblationParams,
    ) -> Self {
        let _ = bot_seed;
        let (own, _) = owners(faction);
        HybridAblation {
            faction,
            own,
            cfg: *cfg,
            params,
            planner: FieldPlanner::new(faction, cfg, map),
            ledger: HackLedger::new(),
        }
    }
}

impl R2Policy for HybridAblation {
    /// The frozen Hybrid's spawn, verbatim: roster agent `i` on the
    /// seed-triangle vertex `i % 3`; without a viable triangle, portal-id
    /// spawns.
    fn choose_spawns(
        &mut self,
        _cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)> {
        if let Some(triangle) = self.planner.seed_triangle() {
            return roster
                .iter()
                .enumerate()
                .map(|(i, &agent)| {
                    let portal = map
                        .portal(triangle[i % 3])
                        .expect("search only returns map portals");
                    (agent, portal.position)
                })
                .collect();
        }
        let count = map.len() as u32;
        roster
            .iter()
            .enumerate()
            .map(|(i, &agent)| {
                let portal = map
                    .portal(PortalId((i as u32) % count))
                    .expect("dense portal ids");
                (agent, portal.position)
            })
            .collect()
    }

    /// The frozen Hybrid's round body minus the toggled-off branches:
    /// every agent below the supply floor hacks an in-range portal first;
    /// builders run the field plan with their own observation-index
    /// offset; under [`HybridAblationRoles::BuilderAttack`] every other
    /// agent runs the frozen attack body. The defense dispatch branch is
    /// absent in BOTH roles.
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        self.planner.refresh(obs);
        let round = obs.round;
        let enemy = self.faction.opposite();
        let builders: Vec<AgentId> = match self.params.roles {
            HybridAblationRoles::BuilderOnly => obs.own_agents.iter().map(|a| a.id).collect(),
            HybridAblationRoles::BuilderAttack => obs
                .own_agents
                .iter()
                .take(self.params.builder_count.min(obs.own_agents.len()))
                .map(|a| a.id)
                .collect(),
        };
        let attackers: Vec<&AgentPrivate> = obs
            .own_agents
            .iter()
            .filter(|a| !builders.contains(&a.id))
            .collect();
        let targets = attack_targets(obs, enemy, &attackers);

        let mut submissions = Vec::with_capacity(obs.own_agents.len());
        for (index, me) in obs.own_agents.iter().enumerate() {
            let mut commands = CommandList::new();

            // Any agent below the supply floor hacks an in-range portal
            // first (auxiliary; may ride along with the major below) —
            // the frozen preamble, verbatim.
            if me.supply < self.params.hack_supply_floor {
                if let Some(portal) = nearest_matching(obs, me.position, |p| {
                    in_range(me.position, p.position, self.cfg.portal_operation_range_cm)
                }) {
                    if self.ledger.eligible(round, &self.cfg, me.id, portal) {
                        self.ledger.record(round, &self.cfg, me.id, portal);
                        commands.aux(Command::Hack { portal });
                    }
                }
            }

            if builders.contains(&me.id) {
                // Builder: the field plan with a per-agent station offset
                // (the trio staggers over the shared tour).
                let planned = self
                    .planner
                    .agent_commands(obs, me, index, &mut self.ledger);
                commands.merge(planned);
            } else {
                // Attacker: the frozen DestroyAnchor body (the arm's own
                // supply floor applies inside it, exactly as in Hybrid).
                let attack = attack_commands(
                    obs,
                    me,
                    self.own,
                    enemy,
                    &self.cfg,
                    &mut self.ledger,
                    self.params.hack_supply_floor,
                    targets.get(&me.id).copied(),
                );
                commands.merge(attack);
            }

            if commands.is_empty() {
                continue;
            }
            submissions.push(common::submission(me.id, commands.build()));
        }
        submissions
    }
}

/// The frozen canonical `CoreParams` of `greedy-coordinated-canonical`
/// (ADR-0017): the r2 core with `near_equal_permille = 0` (only exact
/// ties of the best value stay eligible) and `spawn_jitter_cm = 0`
/// (spawns exactly on the chosen triangle's vertices).
pub fn canonical_core_params() -> CoreParams {
    CoreParams {
        near_equal_permille: 0,
        spawn_jitter_cm: 0,
        ..CoreParams::default()
    }
}

/// The `greedy-coordinated-canonical` arm (version `r2.0`, family
/// `ablation`, stream kind-tag 13): the shared
/// [`crate::r2::build::BuilderCore`] — the same decision function as
/// `greedy-coordinated-r2` — configured for canonical deterministic
/// spawns. This arm exists to MEASURE the mirror first-round collision
/// contribution of deterministic spawning (plan 7.2 row 5); it is a
/// diagnostic policy and never enters competitive summaries.
pub struct GreedyCoordinatedCanonical {
    core: BuilderCore,
}

impl GreedyCoordinatedCanonical {
    pub fn new(faction: Faction, bot_seed: u64, cfg: &GameConfig, params: CoreParams) -> Self {
        GreedyCoordinatedCanonical {
            core: BuilderCore::new(
                faction,
                bot_seed,
                R2Stream::GreedyCoordinatedCanonical,
                cfg,
                params,
            ),
        }
    }
}

impl R2Policy for GreedyCoordinatedCanonical {
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
        crate::r2::greedy_coordinated::submissions(obs, commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hybrid_ablation_params_round_trip_strictly() {
        for params in [
            HybridAblationParams::builder_only(),
            HybridAblationParams::builder_attack(),
        ] {
            assert_eq!(
                HybridAblationParams::from_tag(&params.tag()),
                Some(params),
                "{} round-trips",
                params.tag()
            );
        }
        assert_eq!(
            HybridAblationParams::builder_only().tag(),
            "roles=builder-only;builders=3;hacksupply=3"
        );
        assert_eq!(
            HybridAblationParams::builder_attack().tag(),
            "roles=builder-attack;builders=3;hacksupply=3"
        );
        // The frozen constants are the defaults.
        assert_eq!(crate::hybrid::BUILDER_COUNT, 3);
        assert_eq!(crate::hybrid::HYBRID_HACK_SUPPLY, 3);
        for malformed in [
            "",
            "roles=builder-only",
            "roles=builder-only;builders=3",
            "roles=builder-only;builders=3;hacksupply=3;x=1",
            "hacksupply=3;builders=3;roles=builder-only",
            "roles=builder-only;builders=-1;hacksupply=3",
            "roles=BuilderOnly;builders=3;hacksupply=3",
        ] {
            assert_eq!(
                HybridAblationParams::from_tag(malformed),
                None,
                "{malformed:?} must not parse"
            );
        }
    }

    #[test]
    fn canonical_params_are_the_r2_core_with_zero_band_and_zero_jitter() {
        let params = canonical_core_params();
        assert_eq!(params.near_equal_permille, 0);
        assert_eq!(params.spawn_jitter_cm, 0);
        let default = CoreParams::default();
        assert_eq!(params.replan_retry_rounds, default.replan_retry_rounds);
        assert_eq!(params.fallback_priority, default.fallback_priority);
        assert_eq!(params.value, default.value);
        // The parameter record is the standard CoreParams serialization
        // with the two zeros (records stay comparable across the arm and
        // its r2 parent).
        assert!(params.tag().starts_with("nearequal=0;retry="));
        assert!(params.tag().contains(";jit=0|value="));
    }
}

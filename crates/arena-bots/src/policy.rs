//! Independent policy identity (B1, `docs/ai-balance-plan.md` section 5.1,
//! ADR-0015): balance experiments address strategies as POLICIES with their
//! own `policy_id`, `family`, `version` and structured parameters — not as
//! the frozen P2 `BotKind` registry. This module is a PARALLEL type: it
//! delegates construction to the untouched `make_bot` and adds no behavior.
//!
//! In B1 the five [`PolicyId`]s mirror the five frozen m6.0 baselines one
//! to one (version exactly `BOT_VERSION`, parameters
//! [`PolicyParameters::FrozenM6`]); their tags reuse the `BotKind::tag`
//! strings so the artifact vocabulary stays continuous with
//! `runs/m6-r1/`. B2 adds new policies as new `PolicyId` variants with
//! FRESH tags and fresh bot-RNG kind tags (bytes >= 6; tags 1-5 are
//! permanently the m6 kinds' — ADR-0015, pinned by the tests below).

use arena_core::config::GameConfig;
use arena_core::map::MapDefinition;
use arena_core::types::Faction;

use crate::bot::{BOT_VERSION, Bot, BotKind, make_bot};

/// A strategy's identity in a balance experiment (ADR-0015). B1 mirrors the
/// five frozen m6.0 baselines; the mapping to `BotKind` is total and
/// injective (pinned by the tests).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PolicyId {
    Random,
    NearestPortal,
    GreedyField,
    DestroyAnchor,
    Hybrid,
}

impl PolicyId {
    /// The full B1 registry, declaration order.
    pub const ALL: [PolicyId; 5] = [
        PolicyId::Random,
        PolicyId::NearestPortal,
        PolicyId::GreedyField,
        PolicyId::DestroyAnchor,
        PolicyId::Hybrid,
    ];

    /// Stable lowercase tag — reuses the frozen `BotKind::tag` strings so
    /// balance artifacts share the m6 vocabulary.
    pub fn tag(self) -> &'static str {
        self.bot_kind().tag()
    }

    /// Parses [`PolicyId::tag`] back (record round-trips).
    pub fn from_tag(tag: &str) -> Option<PolicyId> {
        BotKind::from_tag(tag).map(PolicyId::from_kind)
    }

    /// The frozen m6.0 bot this policy executes in B1.
    pub fn bot_kind(self) -> BotKind {
        match self {
            PolicyId::Random => BotKind::Random,
            PolicyId::NearestPortal => BotKind::NearestPortal,
            PolicyId::GreedyField => BotKind::GreedyField,
            PolicyId::DestroyAnchor => BotKind::DestroyAnchor,
            PolicyId::Hybrid => BotKind::Hybrid,
        }
    }

    /// Inverse of [`PolicyId::bot_kind`] (total by construction).
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
    /// NearestPortal and DestroyAnchor are DIAGNOSTIC policies: they stay
    /// in the pool but never enter the competitive-balance summaries.
    pub fn family(self) -> PolicyFamily {
        match self {
            PolicyId::Random => PolicyFamily::Random,
            PolicyId::NearestPortal => PolicyFamily::Nearest,
            PolicyId::GreedyField => PolicyFamily::FieldBuilder,
            PolicyId::DestroyAnchor => PolicyFamily::AnchorDestroyer,
            PolicyId::Hybrid => PolicyFamily::AdaptiveHybrid,
        }
    }

    /// The frozen m6.0 policy spec of this policy (B1's only parameters).
    pub fn frozen_m6_spec(self) -> PolicySpec {
        PolicySpec {
            policy_id: self,
            family: self.family(),
            version: BOT_VERSION,
            parameters: PolicyParameters::FrozenM6,
        }
    }
}

/// The plan section 6.1 strategy families. B1 needs only the five below;
/// Defender, Disruptor, KeyRunner and Blocker join in B2 (additive).
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
}

impl PolicyFamily {
    /// The full B1 family registry, declaration order.
    pub const ALL: [PolicyFamily; 5] = [
        PolicyFamily::Random,
        PolicyFamily::Nearest,
        PolicyFamily::AnchorDestroyer,
        PolicyFamily::FieldBuilder,
        PolicyFamily::AdaptiveHybrid,
    ];

    /// Stable lowercase tag for artifacts and record round-trips.
    pub fn tag(self) -> &'static str {
        match self {
            PolicyFamily::Random => "random",
            PolicyFamily::Nearest => "nearest",
            PolicyFamily::AnchorDestroyer => "anchor-destroyer",
            PolicyFamily::FieldBuilder => "field-builder",
            PolicyFamily::AdaptiveHybrid => "adaptive-hybrid",
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
            _ => None,
        }
    }

    /// Diagnostic families stay out of the competitive-balance summaries
    /// (plan section 6.1).
    pub fn is_diagnostic(self) -> bool {
        matches!(
            self,
            PolicyFamily::Random | PolicyFamily::Nearest | PolicyFamily::AnchorDestroyer
        )
    }
}

/// A policy's structured parameters (plan sections 5.1/6.3). B1 has exactly
/// one variant — the unmodified frozen m6.0 behavior; structured R2
/// parameter sets arrive in B2 and MUST arrive as NEW variants, never as a
/// change to `FrozenM6` (ADR-0015: version semantics are additive).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PolicyParameters {
    /// The frozen m6.0 baseline: no tunable parameters, byte-identical to
    /// `make_bot` of the mirrored kind.
    FrozenM6,
}

impl PolicyParameters {
    /// Stable lowercase tag for artifacts and record round-trips.
    pub fn tag(self) -> &'static str {
        match self {
            PolicyParameters::FrozenM6 => "frozen-m6",
        }
    }

    /// Parses [`PolicyParameters::tag`] back.
    pub fn from_tag(tag: &str) -> Option<PolicyParameters> {
        match tag {
            "frozen-m6" => Some(PolicyParameters::FrozenM6),
            _ => None,
        }
    }
}

/// Everything a balance experiment needs to know about one strategy (plan
/// section 5.3): independent identity, family, fixed version and
/// structured parameters. The single global `BOT_VERSION` keeps
/// representing ONLY the m6.0 baselines (ADR-0015).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PolicySpec {
    pub policy_id: PolicyId,
    pub family: PolicyFamily,
    /// Fixed version string of this policy's algorithm; two specs with the
    /// same `policy_id` and different versions are different strategies.
    pub version: &'static str,
    pub parameters: PolicyParameters,
}

impl PolicySpec {
    /// Structural consistency of the spec: the frozen-m6 parameters exist
    /// only at version `BOT_VERSION` and only with the mirrored kind's own
    /// family (a spec is built through [`PolicyId::frozen_m6_spec`] or, in
    /// B2, through a new policy's own constructor).
    pub fn validate(&self) -> Result<(), String> {
        match self.parameters {
            PolicyParameters::FrozenM6 => {
                if self.version != BOT_VERSION {
                    return Err(format!(
                        "frozen-m6 parameters require version {BOT_VERSION}, got {}",
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
            }
        }
        Ok(())
    }
}

/// The five frozen m6.0 policy specs, registry order.
pub fn frozen_m6_specs() -> Vec<PolicySpec> {
    PolicyId::ALL.iter().map(|id| id.frozen_m6_spec()).collect()
}

/// Constructs one faction's bot for a policy spec. Delegates 1:1 to the
/// untouched frozen `make_bot` in B1 — the executable m6.0 behavior pin is
/// the lockstep test below.
///
/// # Panics
///
/// A structurally inconsistent spec (see [`PolicySpec::validate`]) is a
/// programmer error and panics, mirroring the `make_bot`/spawn contracts.
pub fn make_policy_bot(
    spec: &PolicySpec,
    faction: Faction,
    bot_seed: u64,
    cfg: &GameConfig,
    map: &MapDefinition,
) -> Box<dyn Bot> {
    spec.validate().expect("policy spec");
    match spec.parameters {
        PolicyParameters::FrozenM6 => {
            make_bot(spec.policy_id.bot_kind(), faction, bot_seed, cfg, map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::BotStrategy;
    use arena_core::harness::MatchHarness;
    use arena_core::resolve::RoundReport;
    use arena_core::spawn::validate_initial_spawn_set;
    use arena_core::state::GameState;
    use arena_core::types::Score2;

    fn all_specs() -> Vec<PolicySpec> {
        frozen_m6_specs()
    }

    // (a) Every policy id has a spec with a non-empty id/family tag and
    // version exactly "m6.0".
    #[test]
    fn specs_carry_identity_family_and_exact_m6_version() {
        assert_eq!(all_specs().len(), 5);
        for spec in all_specs() {
            assert!(!spec.policy_id.tag().is_empty());
            assert!(!spec.family.tag().is_empty());
            assert_eq!(spec.version, "m6.0");
            assert_eq!(spec.version, BOT_VERSION);
            assert_eq!(spec.parameters, PolicyParameters::FrozenM6);
            assert_eq!(spec.validate(), Ok(()));
        }
    }

    // (b) tag/from_tag round-trips for every PolicyId, PolicyFamily and
    // PolicyParameters; registry tags are distinct.
    #[test]
    fn tags_round_trip_for_every_id_family_and_parameters() {
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
            PolicyFamily::from_tag("defender"),
            None,
            "B2 family not yet defined"
        );
        assert_eq!(
            PolicyParameters::from_tag(PolicyParameters::FrozenM6.tag()),
            Some(PolicyParameters::FrozenM6)
        );
        assert_eq!(PolicyParameters::from_tag("frozen-m6-v2"), None);
        let mut tags: Vec<&str> = PolicyId::ALL.iter().map(|id| id.tag()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), PolicyId::ALL.len(), "policy tags distinct");
        // Diagnostic families are exactly the three probes (plan 6.1).
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
                PolicyFamily::AnchorDestroyer
            ]
        );
    }

    // (c) PolicyId -> BotKind is total and injective (and its inverse
    // round-trips), and the mirrored kinds own the frozen bot-RNG kind-tag
    // bytes 1..=5 (ADR-0015 namespace rule).
    #[test]
    fn policy_id_maps_total_and_injectively_onto_bot_kinds() {
        let mut kinds = Vec::new();
        for id in PolicyId::ALL {
            let kind = id.bot_kind();
            assert_eq!(PolicyId::from_kind(kind), id, "inverse round-trips");
            kinds.push(kind);
        }
        let mut distinct = kinds.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(kinds.len(), distinct.len(), "mapping is injective");
        assert_eq!(kinds.len(), BotKind::ALL.len(), "mapping is total");
        let mut tags: Vec<u8> = BotKind::ALL
            .iter()
            .map(|k| crate::rng::kind_tag(*k))
            .collect();
        tags.sort_unstable();
        assert_eq!(tags, vec![1, 2, 3, 4, 5], "m6 kinds own kind tags 1-5");
    }

    // A spec with the frozen-m6 parameters but a non-m6 version (or a
    // wrong family) is refused — versions are additive, never edited.
    #[test]
    fn inconsistent_specs_are_refused() {
        let mut spec = PolicyId::GreedyField.frozen_m6_spec();
        spec.version = "m6.1";
        assert!(spec.validate().is_err());
        let mut spec = PolicyId::Hybrid.frozen_m6_spec();
        spec.family = PolicyFamily::FieldBuilder;
        assert!(spec.validate().is_err());
    }

    // (d) FROZEN-BEHAVIOR PIN: for a fixed (bot_seed, faction, cfg, map)
    // and the observation sequence of a real match, make_policy_bot behaves
    // EXACTLY like make_bot of the mirrored kind — identical spawn sets and
    // identical per-round submissions all the way through, for all five
    // kinds. This is the executable guarantee that B1 changes no m6.0
    // behavior.
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

    #[test]
    fn make_policy_bot_is_byte_identical_to_make_bot_for_all_five_kinds() {
        let mut cfg = GameConfig::default();
        cfg.total_rounds = 120; // pin depth; the decision function is round-independent
        let map = arena_core::map::generate_map(0xB1_F10E_5EED, &cfg).expect("map generates");
        let bot_seed = 0x0707_0707_0000_00B1;
        let game_seed = 0x6A1E_5EED_0000_0001;
        for id in PolicyId::ALL {
            let spec = id.frozen_m6_spec();
            let (enl_roster, _res_roster) = GameState::initial_roster(&cfg);
            // Identical spawn sets from identical construction inputs.
            let policy_spawns = make_policy_bot(&spec, Faction::Enl, bot_seed, &cfg, &map)
                .choose_spawns(&cfg, &map, &enl_roster);
            let kind_spawns = make_bot(id.bot_kind(), Faction::Enl, bot_seed, &cfg, &map)
                .choose_spawns(&cfg, &map, &enl_roster);
            assert_eq!(policy_spawns, kind_spawns, "spawns differ for {}", id.tag());
            // Lockstep matches: policy bot as ENL vs kind bot as ENL, same
            // deterministic opponent and seeds — any behavioral divergence
            // surfaces as a differing RoundReport submission at the first
            // divergent round.
            let opponent = if id == PolicyId::Hybrid {
                BotKind::GreedyField
            } else {
                BotKind::Hybrid
            };
            let (policy_reports, policy_scores) = drive_match(
                make_policy_bot(&spec, Faction::Enl, bot_seed, &cfg, &map),
                make_bot(opponent, Faction::Res, bot_seed, &cfg, &map),
                game_seed,
                &cfg,
                &map,
            );
            let (kind_reports, kind_scores) = drive_match(
                make_bot(id.bot_kind(), Faction::Enl, bot_seed, &cfg, &map),
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

    // The spec registry is queryable by id (executor convenience).
    #[test]
    fn frozen_m6_specs_cover_the_registry() {
        let specs = frozen_m6_specs();
        for id in PolicyId::ALL {
            assert!(
                specs.iter().any(|s| s.policy_id == id),
                "registry covers {}",
                id.tag()
            );
        }
    }
}

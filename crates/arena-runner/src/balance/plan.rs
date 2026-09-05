//! Experiment plans, config variants and the frozen R2 seed sets (B1,
//! `docs/balance/ai-balance-plan.md` sections 5.3 and 8.1, ADR-0014): what a
//! balance experiment runs — its stage, seed set, complete config
//! variants, policy specs, matchups and acceptance gates — plus the frozen
//! R2 seed derivations. Nothing here executes matches; the frozen m6
//! `BatchConfig` keeps serving the frozen M6 batches (plan section 5.2).

use arena_bots::policy::{PolicyId, PolicySpec};
use arena_core::config::GameConfig;

use crate::policy::BucketPolicy;

// ---------------------------------------------------------------------
// Frozen R2 seed sets (plan section 8.1, ADR-0014)
// ---------------------------------------------------------------------

/// The frozen master seed of the Development set (300 maps; `'r' '2' 'd'`
/// prefix, map count in the low word).
pub const R2_DEVELOPMENT_MASTER_SEED: u64 = 0x0072_3264_0000_0300;

/// The frozen master seed of the Validation set (100 maps).
pub const R2_VALIDATION_MASTER_SEED: u64 = 0x0072_3276_0000_0100;

/// The frozen master seed of the Holdout set (200 maps; maps are NEVER
/// generated or run before the B5 Confirm stage — ADR-0014).
pub const R2_HOLDOUT_MASTER_SEED: u64 = 0x0072_3268_0000_0200;

/// R2 seed domain labels (ADR-0014): disjoint from every m5/m6/m9/conflict
/// label; the tests also prove value-level disjointness.
pub const R2_MAP_SEED_LABEL: &[u8] = b"ingress-arena-r2-map";
pub const R2_GAME_SEED_LABEL: &[u8] = b"ingress-arena-r2-game";
pub const R2_BOT_SEED_LABEL: &[u8] = b"ingress-arena-r2-bot";
pub const R2_SCHEDULE_SEED_LABEL: &[u8] = b"ingress-arena-r2-sched";

/// One of the three frozen R2 map sets (plan section 8.1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SeedSet {
    /// 300 maps for implementation debugging, policy weight search and
    /// pilots.
    Development,
    /// 100 maps for choosing policy parameters and candidate configs.
    Validation,
    /// 200 maps, only for final confirmation after the plan is frozen.
    /// Viewing holdout results downgrades the set (ADR-0014 discipline).
    Holdout,
}

impl SeedSet {
    /// The three sets, declaration order.
    pub const ALL: [SeedSet; 3] = [SeedSet::Development, SeedSet::Validation, SeedSet::Holdout];

    /// Stable lowercase tag (appears in every seed-derivation payload and
    /// artifact row).
    pub fn tag(self) -> &'static str {
        match self {
            SeedSet::Development => "development",
            SeedSet::Validation => "validation",
            SeedSet::Holdout => "holdout",
        }
    }

    /// Parses [`SeedSet::tag`] back.
    pub fn from_tag(tag: &str) -> Option<SeedSet> {
        match tag {
            "development" => Some(SeedSet::Development),
            "validation" => Some(SeedSet::Validation),
            "holdout" => Some(SeedSet::Holdout),
            _ => None,
        }
    }

    /// The frozen master seed (ADR-0014).
    pub const fn master_seed(self) -> u64 {
        match self {
            SeedSet::Development => R2_DEVELOPMENT_MASTER_SEED,
            SeedSet::Validation => R2_VALIDATION_MASTER_SEED,
            SeedSet::Holdout => R2_HOLDOUT_MASTER_SEED,
        }
    }

    /// The set's frozen map count (ADR-0014).
    pub const fn map_count(self) -> u32 {
        match self {
            SeedSet::Development => 300,
            SeedSet::Validation => 100,
            SeedSet::Holdout => 200,
        }
    }
}

/// `seed_set_tag || master_seed_le64 || suffix` — the common payload head
/// of every R2 derivation (the set tag and master seed always travel
/// together, ADR-0014).
fn seed_set_payload(set: SeedSet, suffix: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(set.tag().len() + 4 + suffix.len());
    payload.extend_from_slice(set.tag().as_bytes());
    payload.extend_from_slice(&set.master_seed().to_le_bytes());
    payload.extend_from_slice(suffix);
    payload
}

/// `seed_set_payload(set, pair_tag || k_le32)` — one comparable unit's
/// payload (identical for the game and bot domains' label separation).
fn unit_payload(set: SeedSet, pair_tag: &str, k: u32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(set.tag().len() + 4 + pair_tag.len() + 4);
    payload.extend_from_slice(set.tag().as_bytes());
    payload.extend_from_slice(&set.master_seed().to_le_bytes());
    payload.extend_from_slice(pair_tag.as_bytes());
    payload.extend_from_slice(&k.to_le_bytes());
    payload
}

/// Map seed `k` of `set`: shared by every leg and every config variant of
/// the experiments on that map (plan section 8.2 common map).
pub fn map_seed(set: SeedSet, k: u32) -> u64 {
    crate::derive_seed(R2_MAP_SEED_LABEL, &seed_set_payload(set, &k.to_le_bytes()))
}

/// Game seed of pair `pair_tag` on map `k`: shared by BOTH legs and ALL
/// config variants of that unit (plan section 8.2, principle 4 common
/// random numbers — never derived from a variant id or leg).
pub fn game_seed(set: SeedSet, pair_tag: &str, k: u32) -> u64 {
    crate::derive_seed(R2_GAME_SEED_LABEL, &unit_payload(set, pair_tag, k))
}

/// Bot seed of pair `pair_tag` on map `k` (one seed for both policies;
/// their RNG streams stay disjoint through the faction/kind tags, D53).
pub fn bot_seed(set: SeedSet, pair_tag: &str, k: u32) -> u64 {
    crate::derive_seed(R2_BOT_SEED_LABEL, &unit_payload(set, pair_tag, k))
}

/// The schedule seed of one experiment. B1's faction swap is by
/// construction (leg 0 / leg 1), so this seed identifies the experiment's
/// scheduling context in records and manifests for continuity with the m6
/// descriptor echo — it is frozen with the experiment either way.
pub fn schedule_seed(experiment_id: &str, set: SeedSet) -> u64 {
    let mut payload = Vec::with_capacity(experiment_id.len() + 4 + set.tag().len());
    payload.extend_from_slice(experiment_id.as_bytes());
    payload.extend_from_slice(&set.master_seed().to_le_bytes());
    crate::derive_seed(R2_SCHEDULE_SEED_LABEL, &payload)
}

/// The exact map-seed derivation formula (manifest echo, ADR-0014).
pub fn map_seed_derivation() -> String {
    "LE-u64(blake3(\"ingress-arena-r2-map\" || seed_set_tag || master_seed_le64 || k_le32))"
        .to_string()
}

/// The exact game-seed derivation formula (manifest echo).
pub fn game_seed_derivation() -> String {
    "LE-u64(blake3(\"ingress-arena-r2-game\" || seed_set_tag || master_seed_le64 || pair_tag || k_le32))"
        .to_string()
}

/// The exact bot-seed derivation formula (manifest echo).
pub fn bot_seed_derivation() -> String {
    "LE-u64(blake3(\"ingress-arena-r2-bot\" || seed_set_tag || master_seed_le64 || pair_tag || k_le32))"
        .to_string()
}

/// The exact schedule-seed derivation formula (manifest echo).
pub fn schedule_seed_derivation() -> String {
    "LE-u64(blake3(\"ingress-arena-r2-sched\" || experiment_id || master_seed_le64))".to_string()
}

// ---------------------------------------------------------------------
// Config variants
// ---------------------------------------------------------------------

/// The 31 canonical `GameConfig` field names in codec word order
/// (`arena_core::codec::push_config`) — the vocabulary of every
/// changed-fields diff.
pub const CONFIG_FIELD_NAMES: [&str; arena_core::codec::CONFIG_U32_COUNT] = [
    "total_rounds",
    "initial_spawn_window_ms",
    "decision_window_ms",
    "priority_bucket_ms",
    "epoch_length",
    "map_width_cm",
    "map_height_cm",
    "portal_count",
    "portal_min_distance_cm",
    "clustered_portal_ratio_permille",
    "portal_cluster_count",
    "initial_agents_per_faction",
    "reinforcement_interval_rounds",
    "reinforcement_agents_per_faction",
    "move_budget_cm",
    "portal_operation_range_cm",
    "transfer_range_cm",
    "initial_supply",
    "max_supply",
    "portal_max_hp",
    "attack_damage",
    "recharge_amount",
    "recharge_cost_supply",
    "hack_supply_yield",
    "hack_key_yield_min",
    "hack_key_yield_max",
    "hack_cooldown_rounds",
    "hack_limit_per_epoch",
    "max_link_distance_cm",
    "max_commands_per_agent_round",
    "max_frame_payload_bytes",
];

/// The canonical 31-word encoding of a config (the codec's exact bytes).
fn config_words(cfg: &GameConfig) -> [u32; arena_core::codec::CONFIG_U32_COUNT] {
    let mut bytes = Vec::with_capacity(arena_core::codec::CONFIG_BYTES);
    arena_core::codec::push_config(cfg, &mut bytes);
    let mut words = [0u32; arena_core::codec::CONFIG_U32_COUNT];
    for (index, word) in words.iter_mut().enumerate() {
        *word = u32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().expect("4 bytes"));
    }
    words
}

/// The six config fields `arena_core::map::generate_map` reads — the
/// fields that DEFINE the generated MapDefinition (ADR-0018). A variant
/// declaring (proven by `validate_contrast` to actually change) any of
/// them is a MAP-REGIME variant: it regenerates its own map from the
/// same map_seed instead of byte-sharing the base map.
pub const MAP_DEFINING_FIELDS: [&str; 6] = [
    "map_width_cm",
    "map_height_cm",
    "portal_count",
    "portal_min_distance_cm",
    "clustered_portal_ratio_permille",
    "portal_cluster_count",
];

/// One complete configuration plus the fields it DECLARES to change
/// against the plan's base variant (plan sections 5.3 and 7.3: a variant
/// is the complete `GameConfig`; the declared-only difference is verified,
/// never assumed).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigVariant {
    pub variant_id: String,
    pub config: GameConfig,
    /// Field names this variant declares as differing from the BASE
    /// variant; the base variant itself declares none.
    pub declared_changed_fields: Vec<&'static str>,
}

impl ConfigVariant {
    pub fn new(
        variant_id: &str,
        config: GameConfig,
        declared_changed_fields: Vec<&'static str>,
    ) -> Self {
        ConfigVariant {
            variant_id: variant_id.to_string(),
            config,
            declared_changed_fields,
        }
    }

    /// The fields whose canonical words actually differ from `base`, in
    /// canonical word order (comparing the 31-word encodings IS comparing
    /// the values, two's-complement dimensions included).
    pub fn changed_fields_against(&self, base: &ConfigVariant) -> Vec<&'static str> {
        let ours = config_words(&self.config);
        let theirs = config_words(&base.config);
        CONFIG_FIELD_NAMES
            .iter()
            .enumerate()
            .filter(|(index, _)| ours[*index] != theirs[*index])
            .map(|(_, name)| *name)
            .collect()
    }

    /// blake3 identity of the complete config's canonical encoding — the
    /// same value the m6 path records (`tournament::config::config_identity`).
    pub fn config_hash(&self) -> u64 {
        crate::tournament::config::config_identity(&self.config)
    }

    /// Whether this variant changes (declares) at least one
    /// map-defining field (ADR-0018): such variants generate their OWN
    /// MapDefinition from the same map_seed; all others must byte-share
    /// the base map (ADR-0014, executor-verified).
    pub fn is_map_regime(&self) -> bool {
        self.declared_changed_fields
            .iter()
            .any(|field| MAP_DEFINING_FIELDS.contains(field))
    }
}

/// Plan-build-time enforcement of the declared-only difference (plan
/// section 7.3): every differing field is declared and every declaration
/// is real. Order-insensitive; reports the offending fields.
pub fn validate_contrast(variant: &ConfigVariant, base: &ConfigVariant) -> Result<(), String> {
    use std::collections::BTreeSet;
    let actual: BTreeSet<&str> = variant.changed_fields_against(base).into_iter().collect();
    let declared: BTreeSet<&str> = variant.declared_changed_fields.iter().copied().collect();
    if actual == declared {
        return Ok(());
    }
    let missing: Vec<&str> = actual.difference(&declared).copied().collect();
    let invented: Vec<&str> = declared.difference(&actual).copied().collect();
    Err(format!(
        "variant {} vs base {}: undeclared changes {:?}, false declarations {:?}",
        variant.variant_id, base.variant_id, missing, invented
    ))
}

// ---------------------------------------------------------------------
// Stage, matchups, gates
// ---------------------------------------------------------------------

/// The experiment phase (plan sections 7/8.3). The stage decides which
/// plan-build-time rules apply (single-variable sensitivity, config-identical
/// ablation, readiness validation, holdout confirmation).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Stage {
    /// Tool-shakeout batch proving the layer end to end (B1).
    Smoke,
    /// Readiness validation of the measurement tooling itself (B2, plan
    /// section 10.1): default-config only, Development maps only.
    Readiness,
    /// Default-config policy ablation (plan 7.2): no rule parameter changes.
    Ablation,
    /// Single-variable sensitivity pilot/full (plan 7.3): one declared
    /// field per non-base variant.
    Sensitivity,
    /// Pre-registered factor interactions (plan 7.4).
    Interaction,
    /// Retuned-policy evaluation (plan 7.5).
    Retune,
    /// Final confirmation on the Holdout set (plan 8.3) — the only stage
    /// that may run holdout maps.
    Confirm,
}

impl Stage {
    /// Stable lowercase tag for manifests and round-trips.
    pub fn tag(self) -> &'static str {
        match self {
            Stage::Smoke => "smoke",
            Stage::Readiness => "readiness",
            Stage::Ablation => "ablation",
            Stage::Sensitivity => "sensitivity",
            Stage::Interaction => "interaction",
            Stage::Retune => "retune",
            Stage::Confirm => "confirm",
        }
    }

    /// Parses [`Stage::tag`] back.
    pub fn from_tag(tag: &str) -> Option<Stage> {
        match tag {
            "smoke" => Some(Stage::Smoke),
            "readiness" => Some(Stage::Readiness),
            "ablation" => Some(Stage::Ablation),
            "sensitivity" => Some(Stage::Sensitivity),
            "interaction" => Some(Stage::Interaction),
            "retune" => Some(Stage::Retune),
            "confirm" => Some(Stage::Confirm),
            _ => None,
        }
    }
}

/// One policy pairing of a balance experiment (the m6 `Matchup` shape,
/// over `PolicyId`s).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PolicyMatchup {
    /// Stable tag; part of the unit seed-derivation payload.
    pub tag: String,
    pub a: PolicyId,
    pub b: PolicyId,
}

impl PolicyMatchup {
    pub fn new(tag: &str, a: PolicyId, b: PolicyId) -> Self {
        PolicyMatchup {
            tag: tag.to_string(),
            a,
            b,
        }
    }
}

/// A named acceptance gate (plan section 10). B1 plans carry NONE — gate
/// evaluation is B3/B4; the typed slot exists so plans can pre-register
/// gates before the analysis layer exists.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Gate {
    /// Declared but not yet evaluated by any analysis.
    Pending { gate_id: String, statement: String },
}

// ---------------------------------------------------------------------
// The plan
// ---------------------------------------------------------------------

/// One complete balance experiment (plan section 5.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExperimentPlan {
    pub experiment_id: String,
    pub stage: Stage,
    pub seed_set: SeedSet,
    /// Maps actually used, `0..map_count` of the seed set.
    pub map_count: u32,
    /// The `variant_id` every other variant is contrasted against.
    pub base_variant: String,
    pub variants: Vec<ConfigVariant>,
    pub policies: Vec<PolicySpec>,
    pub matchups: Vec<PolicyMatchup>,
    pub bucket_policy: BucketPolicy,
    pub gates: Vec<Gate>,
}

impl ExperimentPlan {
    /// The base variant (plan validation guarantees it exists).
    pub fn base(&self) -> &ConfigVariant {
        self.variants
            .iter()
            .find(|v| v.variant_id == self.base_variant)
            .expect("validate() guarantees the base variant exists")
    }

    /// Plan-build-time validation: structural sanity, the declared-only
    /// config contrasts (with the per-stage single-variable rule), policy
    /// registration, seed-set capacity and the holdout discipline.
    pub fn validate(&self) -> Result<(), String> {
        if self.experiment_id.is_empty() || self.experiment_id.contains('/') {
            return Err(format!(
                "experiment_id {:?} must be a non-empty path component",
                self.experiment_id
            ));
        }
        if self.variants.is_empty() {
            return Err("no config variants".to_string());
        }
        let mut ids: Vec<&str> = self
            .variants
            .iter()
            .map(|v| v.variant_id.as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != self.variants.len() {
            return Err("duplicate variant ids".to_string());
        }
        let base = self
            .variants
            .iter()
            .find(|v| v.variant_id == self.base_variant)
            .ok_or_else(|| {
                format!(
                    "base variant {:?} not among the variants",
                    self.base_variant
                )
            })?;
        if !base.declared_changed_fields.is_empty() {
            return Err(format!(
                "base variant {} declares changed fields {:?}",
                base.variant_id, base.declared_changed_fields
            ));
        }
        if self.map_count == 0 || self.map_count > self.seed_set.map_count() {
            return Err(format!(
                "map_count {} outside 1..={} of seed set {}",
                self.map_count,
                self.seed_set.map_count(),
                self.seed_set.tag()
            ));
        }
        // Holdout discipline (plan 8.1 / ADR-0014): holdout maps exist only
        // at the Confirm stage.
        if self.seed_set == SeedSet::Holdout && self.stage != Stage::Confirm {
            return Err(format!(
                "seed set holdout may only run at stage confirm (plan is {})",
                self.stage.tag()
            ));
        }
        // Declared-only contrasts, plus the stage rules on their size.
        for variant in &self.variants {
            if variant.variant_id == self.base_variant {
                continue;
            }
            validate_contrast(variant, base).map_err(|e| e.to_string())?;
            let changed = variant.declared_changed_fields.len();
            match self.stage {
                Stage::Sensitivity => {
                    if changed != 1 {
                        return Err(format!(
                            "stage sensitivity is single-variable: variant {} declares {}",
                            variant.variant_id, changed
                        ));
                    }
                }
                Stage::Interaction => {
                    // Plan 7.4 small factorial experiments: base, A, B, A+B
                    // — at most two declared fields per variant (ADR-0018).
                    if changed > 2 {
                        return Err(format!(
                            "stage interaction declares at most two fields: variant {} declares {:?}",
                            variant.variant_id, variant.declared_changed_fields
                        ));
                    }
                }
                Stage::Ablation | Stage::Readiness => {
                    return Err(format!(
                        "stage {} may not change rule parameters (variant {} declares {:?})",
                        self.stage.tag(),
                        variant.variant_id,
                        variant.declared_changed_fields
                    ));
                }
                _ => {}
            }
            // Belt-and-braces: a plan never carries a config arena-core
            // itself would refuse (generate_map validates first anyway).
            if let Err(error) = variant.config.validate() {
                return Err(format!(
                    "variant {}: config does not validate: {error:?}",
                    variant.variant_id
                ));
            }
        }
        // The base variant's config validates too.
        if let Err(error) = base.config.validate() {
            return Err(format!(
                "variant {}: config does not validate: {error:?}",
                base.variant_id
            ));
        }
        if matches!(self.stage, Stage::Ablation | Stage::Readiness) {
            // Belt-and-braces: the ablation and readiness stages are
            // default-config only, so no variant may actually differ from
            // the base either.
            for variant in &self.variants {
                let diff = variant.changed_fields_against(base);
                if !diff.is_empty() {
                    return Err(format!(
                        "stage {}: variant {} changes {:?}",
                        self.stage.tag(),
                        variant.variant_id,
                        diff
                    ));
                }
            }
        }
        if self.policies.len() < 2 {
            return Err("an experiment needs at least two policies".to_string());
        }
        for spec in &self.policies {
            spec.validate()
                .map_err(|e| format!("policy {}: {}", spec.policy_id.tag(), e))?;
        }
        let mut policy_ids: Vec<arena_bots::policy::PolicyId> =
            self.policies.iter().map(|s| s.policy_id).collect();
        policy_ids.sort_unstable();
        policy_ids.dedup();
        if policy_ids.len() != self.policies.len() {
            return Err("duplicate policy ids in the registry".to_string());
        }
        if self.matchups.is_empty() {
            return Err("no matchups".to_string());
        }
        for matchup in &self.matchups {
            let known = |id: PolicyId| self.policies.iter().any(|s| s.policy_id == id);
            if !known(matchup.a) || !known(matchup.b) {
                return Err(format!(
                    "matchup {} references an unregistered policy",
                    matchup.tag
                ));
            }
        }
        Ok(())
    }

    /// The plan's schedule seed (record echo / manifest).
    pub fn schedule_seed(&self) -> u64 {
        schedule_seed(&self.experiment_id, self.seed_set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_bots::policy::PolicyId;
    use arena_core::config::GameConfig;

    fn base_and_move_budget_variant() -> Vec<ConfigVariant> {
        vec![
            ConfigVariant::new("default", GameConfig::default(), Vec::new()),
            {
                let mut cfg = GameConfig::default();
                cfg.move_budget_cm = 24_000;
                ConfigVariant::new("move-budget-240m", cfg, vec!["move_budget_cm"])
            },
        ]
    }

    fn valid_plan() -> ExperimentPlan {
        ExperimentPlan {
            experiment_id: "unit-plan".to_string(),
            stage: Stage::Sensitivity,
            seed_set: SeedSet::Development,
            map_count: 4,
            base_variant: "default".to_string(),
            variants: base_and_move_budget_variant(),
            policies: vec![
                PolicyId::GreedyField.frozen_m6_spec(),
                PolicyId::Hybrid.frozen_m6_spec(),
            ],
            matchups: vec![PolicyMatchup::new(
                "greedy-hybrid",
                PolicyId::GreedyField,
                PolicyId::Hybrid,
            )],
            bucket_policy: crate::policy::BucketPolicy::Flat(0),
            gates: Vec::new(),
        }
    }

    // ---------------------------------------------------------------
    // ConfigVariant changed-fields diff
    // ---------------------------------------------------------------

    #[test]
    fn changed_fields_diff_reports_exactly_the_differing_words() {
        let base = ConfigVariant::new("default", GameConfig::default(), Vec::new());
        let same = ConfigVariant::new("same", GameConfig::default(), Vec::new());
        assert_eq!(
            same.changed_fields_against(&base),
            Vec::<&'static str>::new()
        );

        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = 24_000;
        let one = ConfigVariant::new("move", cfg, vec!["move_budget_cm"]);
        assert_eq!(one.changed_fields_against(&base), vec!["move_budget_cm"]);

        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = 24_000;
        cfg.attack_damage = 25;
        let two = ConfigVariant::new("two", cfg, vec!["move_budget_cm", "attack_damage"]);
        let mut diff = two.changed_fields_against(&base);
        diff.sort_unstable();
        assert_eq!(diff, vec!["attack_damage", "move_budget_cm"]);
    }

    #[test]
    fn validate_contrast_enforces_the_declared_only_difference() {
        let base = ConfigVariant::new("default", GameConfig::default(), Vec::new());
        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = 24_000;
        cfg.attack_damage = 25;
        // Declares one field but changes two: refused.
        let under = ConfigVariant::new("under", cfg.clone(), vec!["move_budget_cm"]);
        assert!(validate_contrast(&under, &base).is_err());
        // Declares both: accepted.
        let exact = ConfigVariant::new("exact", cfg, vec!["move_budget_cm", "attack_damage"]);
        assert_eq!(validate_contrast(&exact, &base), Ok(()));
        // Declares a field that did not change: refused (false declaration).
        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = 24_000;
        let over = ConfigVariant::new("over", cfg, vec!["move_budget_cm", "attack_damage"]);
        assert!(validate_contrast(&over, &base).is_err());
    }

    #[test]
    fn config_hash_equals_the_shared_canonical_config_identity() {
        let variant = ConfigVariant::new("default", GameConfig::default(), Vec::new());
        assert_eq!(
            variant.config_hash(),
            crate::tournament::config::config_identity(&GameConfig::default())
        );
        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = 24_000;
        let variant = ConfigVariant::new("move", cfg, Vec::new());
        assert_eq!(
            variant.config_hash(),
            crate::tournament::config::config_identity(&variant.config)
        );
    }

    #[test]
    fn field_names_follow_the_canonical_word_order() {
        assert_eq!(
            CONFIG_FIELD_NAMES.len(),
            arena_core::codec::CONFIG_U32_COUNT
        );
        assert_eq!(CONFIG_FIELD_NAMES[0], "total_rounds");
        assert_eq!(CONFIG_FIELD_NAMES[7], "portal_count");
        assert_eq!(CONFIG_FIELD_NAMES[14], "move_budget_cm");
        assert_eq!(CONFIG_FIELD_NAMES[30], "max_frame_payload_bytes");
        // The name->index map is dense and reversible.
        let mut sorted = CONFIG_FIELD_NAMES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), CONFIG_FIELD_NAMES.len());
    }

    // ---------------------------------------------------------------
    // Frozen R2 seed derivations (ADR-0014)
    // ---------------------------------------------------------------

    #[test]
    fn seed_derivations_are_deterministic() {
        for set in SeedSet::ALL {
            assert_eq!(map_seed(set, 7), map_seed(set, 7));
            assert_eq!(
                game_seed(set, "greedy-hybrid", 7),
                game_seed(set, "greedy-hybrid", 7)
            );
            assert_eq!(
                bot_seed(set, "greedy-hybrid", 7),
                bot_seed(set, "greedy-hybrid", 7)
            );
        }
        // k, pair tag and seed set all move the derived seeds.
        assert_ne!(
            map_seed(SeedSet::Development, 0),
            map_seed(SeedSet::Development, 1)
        );
        assert_ne!(
            game_seed(SeedSet::Development, "a-b", 0),
            game_seed(SeedSet::Development, "b-a", 0)
        );
        assert_ne!(
            bot_seed(SeedSet::Development, "a-b", 0),
            bot_seed(SeedSet::Validation, "a-b", 0)
        );
    }

    // Compares SEED VALUES ONLY — holdout maps are never generated here
    // (plan 8.1 holdout discipline).
    #[test]
    fn seed_set_domains_are_pairwise_disjoint_over_300_maps() {
        for k in 0..300u32 {
            let dev = map_seed(SeedSet::Development, k);
            let val = map_seed(SeedSet::Validation, k);
            let hold = map_seed(SeedSet::Holdout, k);
            assert_ne!(dev, val, "dev/val collide at k={k}");
            assert_ne!(dev, hold, "dev/holdout collide at k={k}");
            assert_ne!(val, hold, "val/holdout collide at k={k}");
        }
    }

    #[test]
    fn r2_domains_are_disjoint_from_m6_domains_and_each_other() {
        // Same payload under an m6 label vs the r2 label: different values
        // (label separation), for every r2 domain.
        let sets = SeedSet::ALL;
        for set in sets {
            let k = 42u32;
            let tag = "greedy-hybrid";
            let map_payload = seed_set_payload(set, &k.to_le_bytes());
            assert_ne!(
                map_seed(set, k),
                crate::derive_seed(crate::batch::MAP_SEED_LABEL, &map_payload)
            );
            let unit_payload = unit_payload(set, tag, k);
            assert_ne!(
                game_seed(set, tag, k),
                crate::derive_seed(crate::batch::GAME_SEED_LABEL, &unit_payload)
            );
            assert_ne!(
                bot_seed(set, tag, k),
                crate::derive_seed(crate::batch::BOT_SEED_LABEL, &unit_payload)
            );
        }
        // The four r2 domains never collide on a shared payload shape.
        let set = SeedSet::Development;
        let k = 5u32;
        let payload = unit_payload(set, "greedy-hybrid", k);
        let map = crate::derive_seed(R2_MAP_SEED_LABEL, &payload);
        let game = crate::derive_seed(R2_GAME_SEED_LABEL, &payload);
        let bot = crate::derive_seed(R2_BOT_SEED_LABEL, &payload);
        let sched = crate::derive_seed(R2_SCHEDULE_SEED_LABEL, &payload);
        let mut all = vec![map, game, bot, sched];
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 4, "r2 domains are distinct streams");
    }

    #[test]
    fn manifest_strings_echo_the_exact_formulas() {
        assert_eq!(
            map_seed_derivation(),
            "LE-u64(blake3(\"ingress-arena-r2-map\" || seed_set_tag || master_seed_le64 || k_le32))"
        );
        assert_eq!(
            game_seed_derivation(),
            "LE-u64(blake3(\"ingress-arena-r2-game\" || seed_set_tag || master_seed_le64 || pair_tag || k_le32))"
        );
        assert_eq!(
            bot_seed_derivation(),
            "LE-u64(blake3(\"ingress-arena-r2-bot\" || seed_set_tag || master_seed_le64 || pair_tag || k_le32))"
        );
        assert_eq!(
            schedule_seed_derivation(),
            "LE-u64(blake3(\"ingress-arena-r2-sched\" || experiment_id || master_seed_le64))"
        );
    }

    #[test]
    fn frozen_master_seeds_and_set_sizes_are_pinned() {
        assert_eq!(SeedSet::Development.master_seed(), 0x0072_3264_0000_0300);
        assert_eq!(SeedSet::Validation.master_seed(), 0x0072_3276_0000_0100);
        assert_eq!(SeedSet::Holdout.master_seed(), 0x0072_3268_0000_0200);
        assert_eq!(SeedSet::Development.map_count(), 300);
        assert_eq!(SeedSet::Validation.map_count(), 100);
        assert_eq!(SeedSet::Holdout.map_count(), 200);
        for set in SeedSet::ALL {
            assert!(set.master_seed() != 0);
            assert_eq!(SeedSet::from_tag(set.tag()), Some(set));
        }
        assert_eq!(Stage::from_tag(Stage::Smoke.tag()), Some(Stage::Smoke));
    }

    // ---------------------------------------------------------------
    // ExperimentPlan validation
    // ---------------------------------------------------------------

    #[test]
    fn valid_plans_validate() {
        assert_eq!(valid_plan().validate(), Ok(()));
    }

    #[test]
    fn plan_validation_refuses_structural_defects() {
        let mut plan = valid_plan();
        plan.base_variant = "missing".to_string();
        assert!(plan.validate().is_err());

        let mut plan = valid_plan();
        plan.variants[0].declared_changed_fields = vec!["move_budget_cm"];
        assert!(plan.validate().is_err(), "base variant declares a diff");

        let mut plan = valid_plan();
        plan.variants[1].declared_changed_fields = Vec::new();
        assert!(plan.validate().is_err(), "undeclared config change");

        let mut plan = valid_plan();
        plan.variants.push(plan.variants[0].clone());
        assert!(plan.validate().is_err(), "duplicate variant ids");

        let mut plan = valid_plan();
        plan.map_count = 301;
        assert!(
            plan.validate().is_err(),
            "more maps than the seed set holds"
        );

        let mut plan = valid_plan();
        plan.map_count = 0;
        assert!(plan.validate().is_err(), "empty map set");

        let mut plan = valid_plan();
        plan.policies.pop();
        assert!(plan.validate().is_err(), "fewer than two policies");

        let mut plan = valid_plan();
        plan.matchups[0] = PolicyMatchup::new("bad", PolicyId::Random, PolicyId::Hybrid);
        assert!(
            plan.validate().is_err(),
            "matchup references unregistered policy"
        );

        let mut plan = valid_plan();
        plan.experiment_id = "a/b".to_string();
        assert!(
            plan.validate().is_err(),
            "experiment id is a path component"
        );
    }

    #[test]
    fn single_variable_rule_is_enforced_per_stage() {
        // Sensitivity: exactly one declared field per non-base variant.
        assert_eq!(valid_plan().validate(), Ok(()));
        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = 24_000;
        cfg.attack_damage = 25;
        let mut plan = valid_plan();
        plan.variants[1] =
            ConfigVariant::new("two-fields", cfg, vec!["move_budget_cm", "attack_damage"]);
        assert!(plan.validate().is_err(), "sensitivity is single-variable");
        // Interaction stage accepts the two-field contrast.
        plan.stage = Stage::Interaction;
        assert_eq!(plan.validate(), Ok(()));
    }

    // B4 map-regime classification (ADR-0018): exactly the six
    // generate_map-read fields make a variant regenerate its own
    // MapDefinition from the same map_seed; rule-only fields keep the
    // ADR-0014 byte-shared base map.
    #[test]
    fn map_regime_is_exactly_the_map_defining_declarations() {
        // The frozen default values of the six map-defining fields (the
        // frozen-default guard: B4 candidates change exactly one field
        // against THESE values).
        let default = GameConfig::default();
        assert_eq!(default.map_width_cm, 600_000);
        assert_eq!(default.map_height_cm, 600_000);
        assert_eq!(default.portal_count, 120);
        assert_eq!(default.portal_min_distance_cm, 8_000);
        assert_eq!(default.clustered_portal_ratio_permille, 700);
        assert_eq!(default.portal_cluster_count, 6);
        // Every map-defining name is a canonical config field name.
        let canonical: std::collections::BTreeSet<&str> =
            CONFIG_FIELD_NAMES.iter().copied().collect();
        for name in MAP_DEFINING_FIELDS {
            assert!(canonical.contains(name), "{name} is not a config field");
        }
        assert_eq!(MAP_DEFINING_FIELDS.len(), 6);

        let base = ConfigVariant::new("default", GameConfig::default(), Vec::new());
        let is_regime = |field: &'static str, regime: bool| {
            let mut cfg = GameConfig::default();
            match field {
                "map_width_cm" => cfg.map_width_cm = 550_000,
                "map_height_cm" => cfg.map_height_cm = 550_000,
                "portal_count" => cfg.portal_count = 80,
                "portal_min_distance_cm" => cfg.portal_min_distance_cm = 6_000,
                "clustered_portal_ratio_permille" => cfg.clustered_portal_ratio_permille = 600,
                "portal_cluster_count" => cfg.portal_cluster_count = 5,
                "move_budget_cm" => cfg.move_budget_cm = 24_000,
                "max_link_distance_cm" => cfg.max_link_distance_cm = 250_000,
                "attack_damage" => cfg.attack_damage = 25,
                "recharge_amount" => cfg.recharge_amount = 20,
                "recharge_cost_supply" => cfg.recharge_cost_supply = 2,
                "initial_supply" => cfg.initial_supply = 40,
                _ => panic!("unknown field {field}"),
            }
            let variant = ConfigVariant::new("probe", cfg, vec![field]);
            assert_eq!(validate_contrast(&variant, &base), Ok(()));
            assert_eq!(
                variant.is_map_regime(),
                regime,
                "{field} classification (ADR-0018)"
            );
        };
        for field in MAP_DEFINING_FIELDS {
            is_regime(field, true);
        }
        for field in [
            "move_budget_cm",
            "max_link_distance_cm",
            "attack_damage",
            "recharge_amount",
            "recharge_cost_supply",
            "initial_supply",
        ] {
            is_regime(field, false);
        }
        // A mixed declaration is map-regime (>=1 map-defining field).
        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = 24_000;
        cfg.portal_count = 80;
        let mixed = ConfigVariant::new("mixed", cfg, vec!["move_budget_cm", "portal_count"]);
        assert!(mixed.is_map_regime());
        // The base variant (no declarations) is not.
        assert!(!base.is_map_regime());
    }

    // Interaction plans are SMALL factorial experiments (plan 7.4): a
    // non-base variant may declare at most two fields (base, A, B, A+B).
    #[test]
    fn interaction_variants_declare_at_most_two_fields() {
        let mut cfg = GameConfig::default();
        cfg.move_budget_cm = 24_000;
        cfg.attack_damage = 25;
        cfg.initial_supply = 40;
        let mut plan = valid_plan();
        plan.stage = Stage::Interaction;
        plan.variants[1] = ConfigVariant::new(
            "three-fields",
            cfg,
            vec!["move_budget_cm", "attack_damage", "initial_supply"],
        );
        assert!(
            plan.validate().is_err(),
            "interaction variants declare at most two fields"
        );
        // Two fields still validate (the A+B arm).
        plan.variants[1].config.initial_supply = GameConfig::default().initial_supply;
        plan.variants[1].declared_changed_fields = vec!["move_budget_cm", "attack_damage"];
        assert_eq!(plan.validate(), Ok(()));
    }

    // Belt-and-braces: every variant's complete config must itself
    // validate (arena-core GameConfig::validate) — a plan never carries a
    // config arena-core would refuse.
    #[test]
    fn every_variant_config_must_validate() {
        let mut cfg = GameConfig::default();
        cfg.attack_damage = 0; // arena-core refuses (HpBounds)
        let mut plan = valid_plan();
        plan.variants[1] = ConfigVariant::new("zero-damage", cfg, vec!["attack_damage"]);
        let error = plan.validate().expect_err("invalid variant config refused");
        assert!(
            error.contains("zero-damage"),
            "error names the variant: {error}"
        );
        assert!(
            error.contains("config"),
            "error says the config is invalid: {error}"
        );
    }

    #[test]
    fn ablation_may_not_change_any_rule_parameter() {
        let mut plan = valid_plan();
        plan.stage = Stage::Ablation;
        assert!(
            plan.validate().is_err(),
            "ablation stage refuses config variants"
        );
        let mut plan = valid_plan();
        plan.stage = Stage::Ablation;
        plan.variants.truncate(1);
        assert_eq!(plan.validate(), Ok(()), "config-identical ablation is fine");
    }

    #[test]
    fn holdout_requires_the_confirm_stage() {
        let mut plan = valid_plan();
        plan.seed_set = SeedSet::Holdout;
        plan.map_count = 4;
        assert!(plan.validate().is_err(), "holdout only at Confirm");
        plan.stage = Stage::Confirm;
        assert_eq!(plan.validate(), Ok(()));
    }
}

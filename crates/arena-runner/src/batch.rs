//! Batch data model (D57/D58): matchups, seed derivations, and the
//! per-match descriptors the executor consumes. Every seed comes from a
//! domain-separated blake3 derivation of the batch `master_seed`; the map
//! pool is shared across matchups so map variance cancels in cross-matchup
//! comparisons.

use arena_bots::bot::{BOT_VERSION, BotKind};
use arena_core::config::GameConfig;

use crate::derive_seed;
use crate::policy::BucketPolicy;
use crate::schedule;

/// One matchup row of a batch: bot A vs bot B over `count` matches (the
/// schedule keeps each bot's ENL/RES exposure exactly equal, so `count`
/// must be even).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Matchup {
    /// Stable tag; part of the seed derivation payload (frozen with the
    /// experiment).
    pub tag: String,
    pub a: BotKind,
    pub b: BotKind,
    pub count: u32,
}

impl Matchup {
    pub fn new(tag: &str, a: BotKind, b: BotKind, count: u32) -> Self {
        Matchup {
            tag: tag.to_string(),
            a,
            b,
            count,
        }
    }
}

/// One complete batch definition.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BatchConfig {
    pub batch_id: String,
    pub matchups: Vec<Matchup>,
    /// Distinct generated maps; each matchup cycles through the pool.
    pub map_count: u32,
    pub master_seed: u64,
    pub bucket_policy: BucketPolicy,
    pub config: GameConfig,
}

/// Seed domain labels (D58): distinct streams per derived quantity.
pub const MAP_SEED_LABEL: &[u8] = b"ingress-arena-m6-map";
pub const GAME_SEED_LABEL: &[u8] = b"ingress-arena-m6-game";
pub const BOT_SEED_LABEL: &[u8] = b"ingress-arena-m6-bot";
pub const SCHEDULE_SEED_LABEL: &[u8] = b"ingress-arena-m6-sched";

impl BatchConfig {
    pub fn total_matches(&self) -> u64 {
        self.matchups.iter().map(|m| m.count as u64).sum()
    }

    /// The one schedule seed of the batch (D55).
    pub fn schedule_seed(&self) -> u64 {
        derive_seed(SCHEDULE_SEED_LABEL, &self.master_seed.to_le_bytes())
    }

    /// Map-pool seed `k` (D58); the pool is shared across matchups.
    pub fn map_seed(&self, k: u32) -> u64 {
        derive_seed(MAP_SEED_LABEL, &k.to_le_bytes())
    }

    /// Game seed of match `j` inside matchup `tag` (D58).
    pub fn game_seed(&self, tag: &str, j: u32) -> u64 {
        let mut payload = Vec::with_capacity(tag.len() + 4);
        payload.extend_from_slice(tag.as_bytes());
        payload.extend_from_slice(&j.to_le_bytes());
        derive_seed(GAME_SEED_LABEL, &payload)
    }

    /// Bot seed of match `j` inside matchup `tag` (D58): one seed for both
    /// bots — their streams stay disjoint through the faction/kind tags of
    /// the bot RNG key (D53).
    pub fn bot_seed(&self, tag: &str, j: u32) -> u64 {
        let mut payload = Vec::with_capacity(tag.len() + 4);
        payload.extend_from_slice(tag.as_bytes());
        payload.extend_from_slice(&j.to_le_bytes());
        derive_seed(BOT_SEED_LABEL, &payload)
    }

    /// blake3 identity of the config's canonical 31-word encoding
    /// (`arena_core::codec`, the D44 config identity) as LE-u64 — now the
    /// shared `config_identity` helper (extracted in M9; the value is
    /// unchanged and pinned by the tests).
    pub fn config_hash(&self) -> u64 {
        crate::tournament::config::config_identity(&self.config)
    }

    /// Expands the whole batch into per-match descriptors, in
    /// (matchup order, match index) order. Applies the schedule's faction
    /// assignment (D55) and the seed derivations (D58); map pool index `k`
    /// cycles per matchup (`j % map_count`).
    pub fn descriptors(&self) -> Vec<MatchDescriptor> {
        let schedule_seed = self.schedule_seed();
        let config_hash = self.config_hash();
        let mut descriptors = Vec::with_capacity(self.total_matches() as usize);
        let mut index = 0u64;
        for (matchup_index, matchup) in self.matchups.iter().enumerate() {
            let factions = schedule::a_is_enl_decisions(schedule_seed, &matchup.tag, matchup.count);
            for (j, bot_a_is_enl) in factions.into_iter().enumerate() {
                let map_index = j as u32 % self.map_count;
                descriptors.push(MatchDescriptor {
                    index,
                    matchup_index: matchup_index as u32,
                    matchup_tag: matchup.tag.clone(),
                    j: j as u32,
                    bot_a: matchup.a.tag().to_string(),
                    bot_b: matchup.b.tag().to_string(),
                    bot_a_is_enl,
                    game_seed: self.game_seed(&matchup.tag, j as u32),
                    map_seed: self.map_seed(map_index),
                    map_index,
                    bot_seed: self.bot_seed(&matchup.tag, j as u32),
                    schedule_seed,
                    bucket_policy: self.bucket_policy,
                    config_hash,
                    bot_a_version: BOT_VERSION,
                    bot_b_version: BOT_VERSION,
                });
                index += 1;
            }
        }
        descriptors
    }

    /// Generates the whole map pool up front (each map once). A
    /// `MapError::Infeasible` aborts the batch — that is a config defect,
    /// not a match outcome (no map skip rule, D58).
    pub fn map_pool(
        &self,
    ) -> Result<Vec<arena_core::map::MapDefinition>, arena_core::map::MapError> {
        (0..self.map_count)
            .map(|k| arena_core::map::generate_map(self.map_seed(k), &self.config))
            .collect()
    }
}

/// Everything one match needs, derived and frozen up front (D57): the
/// descriptor is echoed verbatim into the MatchRecord and the artifacts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MatchDescriptor {
    /// Global 0-based batch index.
    pub index: u64,
    pub matchup_index: u32,
    pub matchup_tag: String,
    /// Match index within the matchup.
    pub j: u32,
    /// Policy tag of side A (the frozen m6 batches carry `BotKind` tags
    /// byte-identically; balance cells carry any registered policy tag —
    ///).
    pub bot_a: String,
    /// Policy tag of side B.
    pub bot_b: String,
    /// The schedule's faction assignment for this match (D55).
    pub bot_a_is_enl: bool,
    pub game_seed: u64,
    pub map_seed: u64,
    pub map_index: u32,
    pub bot_seed: u64,
    pub schedule_seed: u64,
    pub bucket_policy: BucketPolicy,
    pub config_hash: u64,
    pub bot_a_version: &'static str,
    pub bot_b_version: &'static str,
}

impl MatchDescriptor {
    /// `(enl_tag, res_tag)` for this match.
    pub fn faction_policy_tags(&self) -> (&str, &str) {
        if self.bot_a_is_enl {
            (&self.bot_a, &self.bot_b)
        } else {
            (&self.bot_b, &self.bot_a)
        }
    }

    /// `(enl_kind, res_kind)` for this match — the m6 path, whose tags are
    /// always `BotKind` tags. Panics on a non-m6 tag (the m6 batch
    /// constructor only produces `BotKind` tags; balance cells never call
    /// this).
    pub fn faction_kinds(&self) -> (BotKind, BotKind) {
        let (enl, res) = self.faction_policy_tags();
        (
            BotKind::from_tag(enl).expect("m6 batch carries BotKind tags"),
            BotKind::from_tag(res).expect("m6 batch carries BotKind tags"),
        )
    }
}

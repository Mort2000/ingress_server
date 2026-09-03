//! Tournament scheduling (P5-SCHEDULING, D80): expands a frozen
//! `TournamentConfig` into the item list — ONE match per item, faction
//! assignment through the UNCHANGED M6/D55 algorithm (exact M/2 per
//! side), maps cycling the Benchmark Seed Set, and all seeds derived up
//! front. `build_items` is a pure function of the config (map generation
//! is in-memory only) — the determinism test target. There is NO auto
//! faction-swap rematch anywhere: each (matchup, j) appears exactly once.

use std::fmt;

use arena_core::map::MapDefinition;

use crate::tournament::config::TournamentConfig;
use crate::tournament::entrant::EntrantId;

/// Seed domain labels (D80): all four are NEW for M9 so the Benchmark
/// Seed Set (formal scoring samples) stays disjoint from the M6 tuning
/// pool (`ingress-arena-m6-map` & co).
pub const MAP_SEED_LABEL: &[u8] = b"ingress-arena-m9-benchmap";
pub const GAME_SEED_LABEL: &[u8] = b"ingress-arena-m9-game";
pub const CLIENT_SEED_LABEL: &[u8] = b"ingress-arena-m9-client";
pub const SCHEDULE_SEED_LABEL: &[u8] = b"ingress-arena-m9-sched";
/// blake3 domain over the D46 map codec bytes (per-map identity).
pub const MAPDEF_HASH_DOMAIN: &[u8] = b"ingress-arena-m9-mapdef";

/// Tournament-level failures that abort the run before any match (never
/// match outcomes).
#[derive(Debug, PartialEq, Eq)]
pub enum TournamentError {
    /// A generated Benchmark Seed Set map was infeasible — a config
    /// defect (no map skip rule, the M6/D58 convention).
    MapInfeasible { map_index: u32 },
    /// `TournamentConfig::validate` failed.
    Invalid(String),
}

impl fmt::Display for TournamentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TournamentError::MapInfeasible { map_index } => write!(
                f,
                "generated Benchmark Seed Set map {map_index} is infeasible for this config (config defect)"
            ),
            TournamentError::Invalid(reason) => {
                write!(f, "tournament config invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for TournamentError {}

/// Everything one match needs, derived and frozen up front. Echoed into
/// `index.tsv` and each `meta.json`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TournamentItem {
    /// Global 0-based item index (the Elo replay order).
    pub index: u64,
    pub tag: String,
    /// Match index within the matchup.
    pub j: u32,
    pub a: EntrantId,
    pub b: EntrantId,
    /// The D55 faction assignment for this match.
    pub a_is_enl: bool,
    pub map_index: u32,
    pub map_seed: u64,
    /// blake3 hex of the D46 map codec bytes (the actual generated map).
    pub map_hash: String,
    pub game_seed: u64,
    pub client_seed: u64,
    pub schedule_seed: u64,
    pub config_hash: u64,
}

impl TournamentItem {
    /// Which entrant plays ENL.
    pub fn enl_entrant(&self) -> EntrantId {
        if self.a_is_enl { self.a } else { self.b }
    }

    pub fn res_entrant(&self) -> EntrantId {
        if self.a_is_enl { self.b } else { self.a }
    }
}

/// Builds the whole item list in (matchup order, j) order. Pure: depends
/// only on the config (map generation included). Validates first.
pub fn build_items(cfg: &TournamentConfig) -> Result<Vec<TournamentItem>, TournamentError> {
    cfg.validate().map_err(TournamentError::Invalid)?;
    let maps = map_pool(cfg)?;
    let schedule_seed = cfg.schedule_seed();
    let config_hash = cfg.config_identity();
    let mut items = Vec::with_capacity(cfg.total_matches() as usize);
    let mut index = 0u64;
    for matchup in &cfg.matchups {
        // D55 reused VERBATIM: exact count/2 ENL slots for entrant A.
        let factions =
            crate::schedule::a_is_enl_decisions(schedule_seed, &matchup.tag, matchup.count);
        for (j, a_is_enl) in factions.into_iter().enumerate() {
            let j = j as u32;
            let map_index = j % cfg.map_count;
            items.push(TournamentItem {
                index,
                tag: matchup.tag.clone(),
                j,
                a: matchup.a,
                b: matchup.b,
                a_is_enl,
                map_index,
                map_seed: cfg.map_seed(map_index),
                map_hash: map_codec_hash(&maps[map_index as usize]),
                game_seed: cfg.game_seed(&matchup.tag, j),
                client_seed: cfg.client_seed(&matchup.tag, j),
                schedule_seed,
                config_hash,
            });
            index += 1;
        }
    }
    Ok(items)
}

/// Generates the Benchmark Seed Set (each map once, ascending k). An
/// infeasible map is a config defect and aborts the tournament.
pub fn map_pool(cfg: &TournamentConfig) -> Result<Vec<MapDefinition>, TournamentError> {
    let mut maps = Vec::with_capacity(cfg.map_count as usize);
    for k in 0..cfg.map_count {
        match arena_core::map::generate_map(cfg.map_seed(k), &cfg.config) {
            Ok(map) => maps.push(map),
            Err(_) => return Err(TournamentError::MapInfeasible { map_index: k }),
        }
    }
    Ok(maps)
}

/// blake3 hex over the D46 map codec bytes (u32 portal count + one
/// `codec::push_portal` record per portal — the IARP header's map
/// encoding) under the m9-mapdef domain.
pub fn map_codec_hash(map: &MapDefinition) -> String {
    let mut bytes = Vec::with_capacity(4 + 13 * map.portals().len());
    bytes.extend_from_slice(&(map.portals().len() as u32).to_le_bytes());
    for portal in map.portals() {
        arena_core::codec::push_portal(portal, &mut bytes);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(MAPDEF_HASH_DOMAIN);
    hasher.update(&bytes);
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_codec_hash_is_the_d46_map_bytes() {
        let map = arena_core::map::generate_map(7, &arena_core::config::GameConfig::default())
            .expect("map");
        // Independent assembly of the same codec stream.
        let mut expect = Vec::new();
        expect.extend_from_slice(&(map.portals().len() as u32).to_le_bytes());
        for portal in map.portals() {
            arena_core::codec::push_portal(portal, &mut expect);
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(MAPDEF_HASH_DOMAIN);
        hasher.update(&expect);
        assert_eq!(map_codec_hash(&map), hasher.finalize().to_hex().to_string());
        // Distinct maps hash distinctly.
        let other =
            arena_core::map::generate_map(8, &arena_core::config::GameConfig::default()).unwrap();
        assert_ne!(map_codec_hash(&map), map_codec_hash(&other));
    }

    // TournamentError implements Display + std::error::Error (the
    // ReplayError/SdkError house style) so it flows through `?` in main
    // and boxed error reporting.
    #[test]
    fn tournament_error_displays_and_is_an_error() {
        let map_error = TournamentError::MapInfeasible { map_index: 3 };
        assert!(map_error.to_string().contains("map 3"), "got: {map_error}");
        let invalid = TournamentError::Invalid("entrant names must be unique".to_string());
        assert!(
            invalid.to_string().contains("entrant names must be unique"),
            "got: {invalid}"
        );
        let boxed: Box<dyn std::error::Error> = Box::new(map_error);
        assert!(
            boxed
                .to_string()
                .starts_with("generated Benchmark Seed Set map")
        );
    }
}

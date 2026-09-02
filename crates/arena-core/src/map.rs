//! Frozen map definition and pre-match seeded generator (R-MAP-02).
//!
//! The definition is the authoritative match input: replays record and reload
//! the full portal list and never re-run the generator. Generation is
//! pre-match tooling with its OWN blake3 stream (`"AMAP"` magic, D12) and is
//! deliberately outside the runtime [`crate::rng::RandomOracle`].

use crate::config::GameConfig;
use crate::geometry::{dist2, in_bounds};
use crate::rng::WordSource;
use crate::rng::uniform_from_words;
use crate::types::{Coord, Owner, Portal, PortalId};

/// Generator stream version (D12); independent of `RNG_VERSION`.
const MAP_GENERATOR_VERSION: u32 = 1;

/// Fresh-draw retries per portal before restarting the whole map.
const MAX_RETRIES_PER_PORTAL: u32 = 1000;

/// Whole-map restarts before declaring the request infeasible.
const MAX_MAP_ATTEMPTS: u32 = 100;

/// Per-attempt seed mixing constant (golden-ratio fraction, D12); generator
/// internals may use wrapping ops — they are not authoritative game math.
const ATTEMPT_SEED_K: u64 = 0x9E37_79B9_7F4A_7C15;

/// Failure modes of [`generate_map`] / [`MapDefinition::validate`].
#[derive(Debug, PartialEq, Eq)]
pub enum MapError {
    /// No layout honoring every constraint was found within the attempt
    /// budget; carries the number of whole-map attempts made.
    Infeasible { attempts: u32 },
    /// The configuration (or, for `validate`, the definition itself) is
    /// structurally invalid: bad parameters, non-dense ids, out-of-bounds
    /// coordinates, or a violated minimum distance.
    InvalidConfig,
}

/// The frozen complete map: portals with dense ids `0..n-1` in placement
/// order (clustered portals first, then uniform ones in the generator).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapDefinition {
    portals: Vec<Portal>,
}

impl MapDefinition {
    /// Builds a definition directly from a portal list — hand-placed
    /// scenario maps and tests. The caller must supply portals with dense
    /// ids `0..n-1` in slice order, matching the generator's output
    /// contract; [`MapDefinition::validate`] checks (but this constructor
    /// does not enforce) that contract plus positions and spacing.
    pub fn new(portals: Vec<Portal>) -> Self {
        MapDefinition { portals }
    }

    pub fn portals(&self) -> &[Portal] {
        &self.portals
    }

    pub fn portal(&self, id: PortalId) -> Option<&Portal> {
        self.portals.get(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.portals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.portals.is_empty()
    }

    /// Checks the definition against `cfg`: config validity, portal count,
    /// dense sequential ids, closed-bounds coordinates, and the pairwise
    /// minimum portal distance.
    pub fn validate(&self, cfg: &GameConfig) -> Result<(), MapError> {
        cfg.validate().map_err(|_| MapError::InvalidConfig)?;

        if self.portals.len() != cfg.portal_count as usize {
            return Err(MapError::InvalidConfig);
        }
        for (index, portal) in self.portals.iter().enumerate() {
            if portal.id.0 != index as u32 {
                return Err(MapError::InvalidConfig);
            }
            if !in_bounds(portal.position, cfg.map_width_cm, cfg.map_height_cm) {
                return Err(MapError::InvalidConfig);
            }
        }

        let min2 = (cfg.portal_min_distance_cm as i64)
            .checked_mul(cfg.portal_min_distance_cm as i64)
            .expect("bounded by GameConfig::validate");
        for i in 0..self.portals.len() {
            for j in (i + 1)..self.portals.len() {
                if dist2(self.portals[i].position, self.portals[j].position) < min2 {
                    return Err(MapError::InvalidConfig);
                }
            }
        }
        Ok(())
    }
}

/// Deterministic clustered-map generator: a pure function of
/// `(map_seed, cfg)` (R-MAP-02). Split per D4: `clustered = count * permille
/// / 1000` (floor), the rest uniform. Layout: `PORTAL_CLUSTER_COUNT` random
/// centers; clustered portals sampled as a 3-word Irwin–Hall offset per axis
/// (gaussian-like, D6) around `center`, clamped into closed map bounds;
/// uniform portals drawn directly in `[0, dim]`. Every candidate must be
/// inside the closed bounds and at least `PORTAL_MIN_DISTANCE` from every
/// placed portal, with up to 1000 fresh draws per portal and up to 100
/// whole-map restarts with remixed seeds.
///
/// Generated portals start `Owner::Neutral` with `hp == 0` (D7).
pub fn generate_map(map_seed: u64, cfg: &GameConfig) -> Result<MapDefinition, MapError> {
    cfg.validate().map_err(|_| MapError::InvalidConfig)?;

    for attempt in 0..MAX_MAP_ATTEMPTS {
        let attempt_seed = map_seed ^ (attempt as u64).wrapping_mul(ATTEMPT_SEED_K);
        if let Some(definition) = generate_attempt(attempt_seed, cfg) {
            return Ok(definition);
        }
    }
    Err(MapError::Infeasible {
        attempts: MAX_MAP_ATTEMPTS,
    })
}

/// Clustered/uniform split (D4). Exposed for tests and diagnostics.
pub fn cluster_split(portal_count: u32, ratio_permille: u32) -> (u32, u32) {
    let clustered = portal_count * ratio_permille / 1000;
    (clustered, portal_count - clustered)
}

fn generate_attempt(attempt_seed: u64, cfg: &GameConfig) -> Option<MapDefinition> {
    let (n_clustered, n_uniform) =
        cluster_split(cfg.portal_count, cfg.clustered_portal_ratio_permille);
    let mut stream = MapWordStream::new(attempt_seed);

    // Step 3: cluster centers, uniform per axis over the closed map range.
    let mut centers = Vec::with_capacity(cfg.portal_cluster_count as usize);
    for _ in 0..cfg.portal_cluster_count {
        let x = map_uniform(&mut stream, closed_range_n(cfg.map_width_cm)) as i32;
        let y = map_uniform(&mut stream, closed_range_n(cfg.map_height_cm)) as i32;
        centers.push(Coord::new(x, y));
    }

    let mut placed: Vec<Coord> = Vec::with_capacity(cfg.portal_count as usize);

    // Step 4: clustered portals (fixed cluster assignment i % count).
    for i in 0..n_clustered {
        let center = centers[(i as usize) % cfg.portal_cluster_count as usize];
        let spread = 4u64 * cfg.portal_min_distance_cm as u64;
        let mut accepted = false;
        for _ in 0..MAX_RETRIES_PER_PORTAL {
            let dx = irwin_hall_offset(&mut stream, spread);
            let dy = irwin_hall_offset(&mut stream, spread);
            let x = clamp_i64(center.x as i64 + dx, 0, cfg.map_width_cm as i64);
            let y = clamp_i64(center.y as i64 + dy, 0, cfg.map_height_cm as i64);
            let candidate = Coord::new(x as i32, y as i32);
            if candidate_accepted(&placed, candidate, cfg) {
                placed.push(candidate);
                accepted = true;
                break;
            }
        }
        if !accepted {
            return None;
        }
    }

    // Step 5: uniform portals.
    for _ in 0..n_uniform {
        let mut accepted = false;
        for _ in 0..MAX_RETRIES_PER_PORTAL {
            let x = map_uniform(&mut stream, closed_range_n(cfg.map_width_cm)) as i32;
            let y = map_uniform(&mut stream, closed_range_n(cfg.map_height_cm)) as i32;
            let candidate = Coord::new(x, y);
            if candidate_accepted(&placed, candidate, cfg) {
                placed.push(candidate);
                accepted = true;
                break;
            }
        }
        if !accepted {
            return None;
        }
    }

    let portals = placed
        .into_iter()
        .enumerate()
        .map(|(index, position)| Portal {
            id: PortalId(index as u32),
            position,
            owner: Owner::Neutral,
            hp: 0,
        })
        .collect();
    Some(MapDefinition { portals })
}

/// In-bounds and pairwise-min-distance acceptance against all placed portals.
fn candidate_accepted(placed: &[Coord], candidate: Coord, cfg: &GameConfig) -> bool {
    if !in_bounds(candidate, cfg.map_width_cm, cfg.map_height_cm) {
        return false;
    }
    let min2 = (cfg.portal_min_distance_cm as i64)
        .checked_mul(cfg.portal_min_distance_cm as i64)
        .expect("bounded by GameConfig::validate");
    placed.iter().all(|&p| dist2(candidate, p) >= min2)
}

/// Closed-range draw count for a dimension: values `[0, dim]` inclusive.
fn closed_range_n(dim_cm: i32) -> u32 {
    (dim_cm as i64 + 1) as u32
}

/// One axis offset: `(u1 + u2 + u3) - (3*spread)/2` with each
/// `u_k = uniform[0, spread]` (integer Irwin–Hall, D6).
fn irwin_hall_offset(stream: &mut MapWordStream, spread: u64) -> i64 {
    // A spread beyond u32::MAX can only come from configs that are wildly
    // infeasible (spread dwarfs the map); cap the draw range so the uniform
    // still works — the clamped result is unchanged in any feasible setting.
    let n = (spread + 1).min(u32::MAX as u64) as u32;
    let u1 = map_uniform(stream, n) as i64;
    let u2 = map_uniform(stream, n) as i64;
    let u3 = map_uniform(stream, n) as i64;
    u1 + u2 + u3 - (3 * spread) as i64 / 2
}

fn clamp_i64(v: i64, lo: i64, hi: i64) -> i64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// The generator's private uniform draw over its own word stream: the exact
/// FROZEN rejection loop shared with the runtime oracle (D12).
fn map_uniform(stream: &mut MapWordStream, n_exclusive: u32) -> u32 {
    assert!(n_exclusive > 0, "map stream draw requires n >= 1");
    uniform_from_words(stream, n_exclusive)
}

/// Private 64-byte-block word stream (D12): block `k` is the XOF of
/// `"AMAP" || u32 version || u64 attempt_seed || u64 k`; words are 4-byte LE
/// in order.
struct MapWordStream {
    attempt_seed: u64,
    block_counter: u64,
    block: Vec<u8>,
    pos: usize,
}

impl MapWordStream {
    fn new(attempt_seed: u64) -> Self {
        let mut stream = MapWordStream {
            attempt_seed,
            block_counter: 0,
            block: vec![0u8; 64],
            pos: 0,
        };
        stream.refill();
        stream
    }

    /// Loads the block addressed by the current counter, then advances the
    /// counter so the next refill hashes the following block.
    fn refill(&mut self) {
        let mut address = Vec::with_capacity(24);
        address.extend_from_slice(b"AMAP");
        address.extend_from_slice(&MAP_GENERATOR_VERSION.to_le_bytes());
        address.extend_from_slice(&self.attempt_seed.to_le_bytes());
        address.extend_from_slice(&self.block_counter.to_le_bytes());
        let mut reader = {
            let mut hasher = blake3::Hasher::new();
            hasher.update(&address);
            hasher.finalize_xof()
        };
        reader.fill(&mut self.block);
        self.pos = 0;
        self.block_counter += 1;
    }
}

impl WordSource for MapWordStream {
    fn next_u32(&mut self) -> u32 {
        if self.pos + 4 > self.block.len() {
            self.refill();
        }
        let bytes: [u8; 4] = self.block[self.pos..self.pos + 4]
            .try_into()
            .expect("64-byte block holds 4-byte words");
        self.pos += 4;
        u32::from_le_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameConfig;

    #[test]
    fn generator_deterministic_same_seed() {
        let cfg = GameConfig::default();
        let first = generate_map(123_456, &cfg).expect("default config generates");
        let second = generate_map(123_456, &cfg).expect("default config generates");
        let a: Vec<(u32, i32, i32)> = first
            .portals()
            .iter()
            .map(|p| (p.id.0, p.position.x, p.position.y))
            .collect();
        let b: Vec<(u32, i32, i32)> = second
            .portals()
            .iter()
            .map(|p| (p.id.0, p.position.x, p.position.y))
            .collect();
        assert_eq!(a, b);
        assert_eq!(first.len(), second.len());
    }

    #[test]
    fn generator_different_seed_different_layout() {
        let cfg = GameConfig::default();
        let first = generate_map(1, &cfg).expect("default config generates");
        let second = generate_map(2, &cfg).expect("default config generates");
        let differs = first
            .portals()
            .iter()
            .zip(second.portals().iter())
            .any(|(a, b)| a.position != b.position);
        assert!(differs, "different seeds must produce different layouts");
    }

    #[test]
    fn generator_default_config_honors_count_bounds_min_distance() {
        let cfg = GameConfig::default();
        let map = generate_map(987_654_321, &cfg).expect("default config generates");
        assert_eq!(map.len(), cfg.portal_count as usize);
        assert_eq!(map.validate(&cfg), Ok(()));
        // Explicit pairwise re-check against every placed portal.
        let min2: i64 = cfg.portal_min_distance_cm as i64 * cfg.portal_min_distance_cm as i64;
        for i in 0..map.len() {
            for j in (i + 1)..map.len() {
                assert!(
                    dist2(map.portals()[i].position, map.portals()[j].position) >= min2,
                    "portals {i},{j} closer than min distance"
                );
            }
        }
        for portal in map.portals() {
            assert!(in_bounds(
                portal.position,
                cfg.map_width_cm,
                cfg.map_height_cm
            ));
        }
    }

    #[test]
    fn generator_ratio_split_counts_floor() {
        assert_eq!(cluster_split(120, 700), (84, 36));
        assert_eq!(cluster_split(7, 500), (3, 4)); // 3.5 floors to 3
        assert_eq!(cluster_split(10, 0), (0, 10));
        assert_eq!(cluster_split(10, 1000), (10, 0));
        // Defaults: 120 portals, 70% clustered.
        let (clustered, uniform) = cluster_split(
            GameConfig::default().portal_count,
            GameConfig::default().clustered_portal_ratio_permille,
        );
        assert_eq!((clustered, uniform), (84, 36));
    }

    #[test]
    fn generator_ratio_extremes_valid() {
        for ratio in [0u32, 1000u32] {
            let mut cfg = GameConfig::default();
            cfg.clustered_portal_ratio_permille = ratio;
            let map = generate_map(555, &cfg).expect("extreme ratio generates");
            assert_eq!(map.validate(&cfg), Ok(()), "ratio {ratio} must be valid");
            assert_eq!(map.len() as u32, cfg.portal_count);
        }
    }

    #[test]
    fn generator_infeasible_config_errors() {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 1000;
        cfg.map_height_cm = 1000;
        cfg.portal_count = 50;
        cfg.portal_min_distance_cm = 1000;
        // A 1000x1000 map fits at most 4 mutually >=1000 cm apart points.
        assert_eq!(
            generate_map(42, &cfg),
            Err(MapError::Infeasible {
                attempts: MAX_MAP_ATTEMPTS
            })
        );
    }

    #[test]
    fn generator_invalid_config_errors() {
        let mut cfg = GameConfig::default();
        cfg.portal_count = 1; // below the minimum of 2
        assert_eq!(generate_map(1, &cfg), Err(MapError::InvalidConfig));
    }

    #[test]
    fn portals_neutral_hp_zero_dense_ids() {
        let cfg = GameConfig::default();
        let map = generate_map(2024, &cfg).expect("default config generates");
        for (index, portal) in map.portals().iter().enumerate() {
            assert_eq!(portal.id, PortalId(index as u32));
            assert_eq!(portal.owner, Owner::Neutral);
            assert_eq!(portal.hp, 0);
        }
        // Accessor consistency.
        assert_eq!(map.portal(PortalId(0)).map(|p| p.id), Some(PortalId(0)));
        assert!(map.portal(PortalId(cfg.portal_count)).is_none());
        assert!(!map.is_empty());
    }

    fn fixture_portal(id: u32, x: i32, y: i32) -> Portal {
        Portal {
            id: PortalId(id),
            position: Coord::new(x, y),
            owner: Owner::Neutral,
            hp: 0,
        }
    }

    #[test]
    fn definition_validate_rejects_bad_fixture() {
        // Two portals, min distance 8000 cm: each fixture violates exactly
        // one rule so the failure reason is unambiguous.
        let mut cfg = GameConfig::default();
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;

        // Violates min distance.
        let too_close = MapDefinition {
            portals: vec![fixture_portal(0, 0, 0), fixture_portal(1, 400, 0)],
        };
        assert_eq!(too_close.validate(&cfg), Err(MapError::InvalidConfig));

        // A valid two-portal fixture for contrast.
        let valid = MapDefinition {
            portals: vec![fixture_portal(0, 0, 0), fixture_portal(1, 600_000, 0)],
        };
        assert_eq!(valid.validate(&cfg), Ok(()));

        // Out-of-bounds coordinate.
        let out_of_bounds = MapDefinition {
            portals: vec![fixture_portal(0, 0, 0), fixture_portal(1, 600_001, 0)],
        };
        assert_eq!(out_of_bounds.validate(&cfg), Err(MapError::InvalidConfig));

        // Non-dense ids.
        let sparse_ids = MapDefinition {
            portals: vec![fixture_portal(0, 0, 0), fixture_portal(2, 600_000, 0)],
        };
        assert_eq!(sparse_ids.validate(&cfg), Err(MapError::InvalidConfig));

        // Wrong count versus config (one portal for portal_count == 2).
        let wrong_count = MapDefinition {
            portals: vec![fixture_portal(0, 0, 0)],
        };
        assert_eq!(wrong_count.validate(&cfg), Err(MapError::InvalidConfig));

        // Invalid config also fails validation.
        let mut bad_cfg = GameConfig::default();
        bad_cfg.portal_count = 1;
        let definition = MapDefinition {
            portals: vec![fixture_portal(0, 0, 0)],
        };
        assert_eq!(definition.validate(&bad_cfg), Err(MapError::InvalidConfig));
    }

    #[test]
    fn definition_validate_accepts_generated_map() {
        let cfg = GameConfig::default();
        let map = generate_map(777, &cfg).expect("default config generates");
        assert_eq!(map.validate(&cfg), Ok(()));
    }

    #[test]
    fn hand_built_constructor_preserves_slice_order() {
        // MapDefinition::new keeps the caller's portal list verbatim; dense
        // ids in slice order are the documented contract.
        let map = MapDefinition::new(vec![
            fixture_portal(0, 10_000, 10_000),
            fixture_portal(1, 25_000, 25_000),
        ]);
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 30_000;
        cfg.map_height_cm = 30_000;
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.portal(PortalId(1)).map(|p| p.position),
            Some(fixture_portal(1, 25_000, 25_000).position)
        );
        assert_eq!(map.validate(&cfg), Ok(()));
    }
}

//! Tournament configuration (D79/D80): entrants, matchups over the
//! Benchmark Seed Set, the per-match `GameConfig`, Elo settings, execution
//! policy (concurrency, sandbox, per-match wall budget). Every seed
//! derives from `master_seed` through a domain-separated blake3 stream
//! (see `schedule.rs`); the M6 `ingress-arena-m6-*` domains are NOT
//! reused, so formal scoring samples stay separated from the tuning pool
//! (P5-RANKING).

use arena_core::config::GameConfig;

use crate::derive_seed;
use crate::tournament::entrant::{Entrant, EntrantId, validate_entrants};

/// One matchup row: entrant `a` vs entrant `b` over `count` single
/// matches (count even — the D55 schedule gives each entrant exactly
/// `count / 2` ENL and RES slots).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TMatchup {
    /// Stable tag; part of every seed derivation payload.
    pub tag: String,
    pub a: EntrantId,
    pub b: EntrantId,
    pub count: u32,
}

impl TMatchup {
    pub fn new(tag: &str, a: EntrantId, b: EntrantId, count: u32) -> Self {
        TMatchup {
            tag: tag.to_string(),
            a,
            b,
            count,
        }
    }
}

/// Elo settings (P5-RANKING: standard Elo, draw = 0.5 each; the math is
/// integer Q32.32 fixed point — see `elo.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EloConfig {
    /// K factor (default 32).
    pub k: u32,
    /// Initial rating (default 1500).
    pub initial: i64,
}

impl Default for EloConfig {
    fn default() -> Self {
        EloConfig {
            k: 32,
            initial: 1500,
        }
    }
}

/// How external clients are spawned (D81).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClientSandbox {
    /// Spawn `<binary> <socket> <seed>` directly.
    Off,
    /// Spawn through `tools/sandbox-runner` with an address-space cap and
    /// a wall-clock watchdog (the rw scratch dir holds the match socket;
    /// the runner's own ro-roots cover the client binary's directory).
    SandboxRunner {
        runner: std::path::PathBuf,
        wall_ms: u64,
        address_space_bytes: u64,
    },
}

impl ClientSandbox {
    pub fn tag(&self) -> &'static str {
        match self {
            ClientSandbox::Off => "off",
            ClientSandbox::SandboxRunner { .. } => "sandbox-runner",
        }
    }
}

/// One complete tournament definition (frozen with each experiment).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TournamentConfig {
    pub name: String,
    pub master_seed: u64,
    pub entrants: Vec<Entrant>,
    pub matchups: Vec<TMatchup>,
    /// Size of the Benchmark Seed Set (distinct maps, D80).
    pub map_count: u32,
    /// `config.total_rounds` must equal this (validated).
    pub rounds_per_match: u32,
    pub config: GameConfig,
    pub elo: EloConfig,
    /// Worker count over the item list (stride partition, default 3 —
    /// the M7 match-semaphore convention).
    pub concurrency: usize,
    pub sandbox: ClientSandbox,
    /// Per-match wall budget: clients are killed at the deadline and the
    /// server settles a deterministic technical outcome (D81).
    pub per_match_budget_ms: u64,
}

impl TournamentConfig {
    pub fn total_matches(&self) -> u64 {
        self.matchups.iter().map(|m| m.count as u64).sum()
    }

    /// The one schedule seed of the tournament (D55 reused verbatim).
    pub fn schedule_seed(&self) -> u64 {
        derive_seed(
            crate::tournament::SCHEDULE_SEED_LABEL,
            &self.master_seed.to_le_bytes(),
        )
    }

    /// Benchmark Seed Set member `k`: bound to the master seed so a new
    /// tournament round samples a fresh, recorded set (D80).
    pub fn map_seed(&self, k: u32) -> u64 {
        let mut payload = Vec::with_capacity(12);
        payload.extend_from_slice(&k.to_le_bytes());
        payload.extend_from_slice(&self.master_seed.to_le_bytes());
        derive_seed(crate::tournament::MAP_SEED_LABEL, &payload)
    }

    /// Game seed of match `j` in matchup `tag`.
    pub fn game_seed(&self, tag: &str, j: u32) -> u64 {
        derive_seed(
            crate::tournament::GAME_SEED_LABEL,
            &tag_j_master(tag, j, self.master_seed),
        )
    }

    /// Client seed of match `j` in matchup `tag`: the external argv seed
    /// and the in-process `BotRng` seed (one derivation, both paths).
    pub fn client_seed(&self, tag: &str, j: u32) -> u64 {
        derive_seed(
            crate::tournament::CLIENT_SEED_LABEL,
            &tag_j_master(tag, j, self.master_seed),
        )
    }

    /// blake3 identity of the canonical 31-word config encoding (D44) —
    /// the SAME value `BatchConfig::config_hash` produces for this
    /// `GameConfig`.
    pub fn config_identity(&self) -> u64 {
        config_identity(&self.config)
    }

    pub fn entrant(&self, id: EntrantId) -> &Entrant {
        &self.entrants[id as usize]
    }

    /// Registry + schedule-shape invariants. Panics nowhere; returns the
    /// first defect.
    pub fn validate(&self) -> Result<(), String> {
        validate_entrants(&self.entrants)?;
        if self.matchups.is_empty() {
            return Err("no matchups".to_string());
        }
        if self.map_count == 0 {
            return Err("map_count must be >= 1".to_string());
        }
        if self.config.total_rounds != self.rounds_per_match {
            return Err(format!(
                "config.total_rounds ({}) != rounds_per_match ({})",
                self.config.total_rounds, self.rounds_per_match
            ));
        }
        for matchup in &self.matchups {
            if matchup.count % 2 == 1 {
                return Err(format!(
                    "matchup {:?} count {} is odd (D55 needs even counts)",
                    matchup.tag, matchup.count
                ));
            }
            if matchup.a == matchup.b {
                return Err(format!(
                    "matchup {:?} pairs an entrant with itself",
                    matchup.tag
                ));
            }
            for id in [matchup.a, matchup.b] {
                if id as usize >= self.entrants.len() {
                    return Err(format!(
                        "matchup {:?} entrant id {id} out of range ({} entrants)",
                        matchup.tag,
                        self.entrants.len()
                    ));
                }
            }
        }
        if self.concurrency == 0 {
            return Err("concurrency must be >= 1".to_string());
        }
        if self.per_match_budget_ms == 0 {
            return Err("per_match_budget_ms must be >= 1".to_string());
        }
        if let ClientSandbox::SandboxRunner { runner, .. } = &self.sandbox {
            if !runner.is_file() {
                return Err(format!("sandbox runner missing: {}", runner.display()));
            }
        }
        Ok(())
    }
}

/// `tag || j_le32 || master_le64` — the shared game/client seed payload.
fn tag_j_master(tag: &str, j: u32, master: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(tag.len() + 12);
    payload.extend_from_slice(tag.as_bytes());
    payload.extend_from_slice(&j.to_le_bytes());
    payload.extend_from_slice(&master.to_le_bytes());
    payload
}

/// blake3-LE-u64 of the D44 canonical 31-word config encoding — extracted
/// from `BatchConfig::config_hash` (pure refactor: the M6 value is pinned
/// by the tests).
pub fn config_identity(cfg: &GameConfig) -> u64 {
    let mut bytes = Vec::with_capacity(arena_core::codec::CONFIG_BYTES);
    arena_core::codec::push_config(cfg, &mut bytes);
    let mut hasher = blake3::Hasher::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    let mut words = [0u8; 8];
    words.copy_from_slice(&hash.as_bytes()[0..8]);
    u64::from_le_bytes(words)
}

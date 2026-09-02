//! The `Bot` interface (D54) and its harness adapter. Bots see exactly what
//! an external client may see: their faction `Observation` (R-VIS, enforced
//! by the type — no enemy private state is reachable), the `GameConfig`
//! constants, and the frozen `MapDefinition` during the initial spawn phase
//! only. In-round `submit_round` inputs never include the map object: bots
//! that need portal geometry read it from `obs.portals`.

use arena_core::config::GameConfig;
use arena_core::harness::{AgentSubmission, RoundStrategy};
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Coord, Faction};

/// Fixed version tag of every M6 bot (P2: fixed-version measuring tools).
pub const BOT_VERSION: &str = "m6.0";

/// The five P2 baseline bots. Declaration order is frozen: it defines the
/// bot-RNG kind tags (D53) and the `BotKind::ALL` registry order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum BotKind {
    Random,
    NearestPortal,
    GreedyField,
    DestroyAnchor,
    Hybrid,
}

impl BotKind {
    /// The full registry, declaration order.
    pub const ALL: [BotKind; 5] = [
        BotKind::Random,
        BotKind::NearestPortal,
        BotKind::GreedyField,
        BotKind::DestroyAnchor,
        BotKind::Hybrid,
    ];

    /// Stable lowercase tag used in matchup names, manifests, and TSV
    /// reports (part of the frozen experiment vocabulary).
    pub fn tag(self) -> &'static str {
        match self {
            BotKind::Random => "random",
            BotKind::NearestPortal => "nearest-portal",
            BotKind::GreedyField => "greedy-field",
            BotKind::DestroyAnchor => "destroy-anchor",
            BotKind::Hybrid => "hybrid",
        }
    }

    /// Short label for matchup tags (e.g. `greedy-field` -> `G`).
    pub fn letter(self) -> &'static str {
        match self {
            BotKind::Random => "R",
            BotKind::NearestPortal => "N",
            BotKind::GreedyField => "G",
            BotKind::DestroyAnchor => "D",
            BotKind::Hybrid => "H",
        }
    }

    /// Parses [`BotKind::tag`] back (report round-trips).
    pub fn from_tag(tag: &str) -> Option<BotKind> {
        match tag {
            "random" => Some(BotKind::Random),
            "nearest-portal" => Some(BotKind::NearestPortal),
            "greedy-field" => Some(BotKind::GreedyField),
            "destroy-anchor" => Some(BotKind::DestroyAnchor),
            "hybrid" => Some(BotKind::Hybrid),
            _ => None,
        }
    }
}

/// A faction's baseline AI. Implementations are deterministic given their
/// construction inputs (`bot_seed`, `faction`, `kind`) and the observation
/// sequence: all randomness flows through the bot's own [`crate::BotRng`]
/// stream (D53).
pub trait Bot: Send {
    fn kind(&self) -> BotKind;

    /// Fixed version string of this bot's algorithm (P2: fixed-version
    /// measuring tools; the version is recorded per match).
    fn version(&self) -> &'static str {
        BOT_VERSION
    }

    /// Full-map initial spawn strategy (pre-Round-1): any legal integer
    /// coordinates in bounds, one entry per roster agent. The runner
    /// validates the result with `validate_initial_spawn_set`; a rejected
    /// set is a bot bug and is recorded as a crash, never silently skipped.
    fn choose_spawns(
        &mut self,
        cfg: &GameConfig,
        map: &MapDefinition,
        roster: &[AgentId],
    ) -> Vec<(AgentId, Coord)>;

    /// One round of submissions for the faction. The returned Vec's order is
    /// the faction's virtual arrival order (`receive_seq` by position);
    /// `bucket` is always stated as 0 (bucket policy is a batch concern
    /// applied by the runner, D18/D56).
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission>;
}

/// Adapts a [`Bot`] to the arena-core [`RoundStrategy`] for `MatchHarness`.
#[allow(dead_code)] // exercised by arena-runner and the integration tests
pub struct BotStrategy(pub Box<dyn Bot>);

impl RoundStrategy for BotStrategy {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        self.0.submit_round(obs)
    }
}

/// Constructs the fixed-version bot of `kind` for one faction of one match.
/// `bot_seed` is per-match (runner-derived, D58); the faction and kind tags
/// keep the two bots' RNG streams disjoint (D53).
pub fn make_bot(
    kind: BotKind,
    faction: Faction,
    bot_seed: u64,
    cfg: &GameConfig,
    map: &MapDefinition,
) -> Box<dyn Bot> {
    match kind {
        BotKind::Random => Box::new(crate::random::RandomBot::new(faction, bot_seed, cfg)),
        BotKind::NearestPortal => Box::new(crate::nearest::NearestPortalBot::new(
            faction, bot_seed, cfg,
        )),
        BotKind::GreedyField => Box::new(crate::greedy_field::GreedyFieldBot::new(
            faction, bot_seed, cfg, map,
        )),
        BotKind::DestroyAnchor => Box::new(crate::destroy_anchor::DestroyAnchorBot::new(
            faction, bot_seed, cfg,
        )),
        BotKind::Hybrid => Box::new(crate::hybrid::HybridBot::new(faction, bot_seed, cfg, map)),
    }
}

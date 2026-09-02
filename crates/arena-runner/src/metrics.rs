//! Per-match metrics (P2.5-METRICS, R-BALANCE-02): one `MatchStats` per
//! match, folded INCREMENTALLY from each round's `RoundReport` (plus the
//! new slice of the public event log and the final state) — 500 reports
//! are never retained. Live-field bookkeeping derives from the executed
//! effects in execution order, so anchor-attack share and layering depth
//! see exactly the world each command saw. Integers only.

use std::collections::BTreeMap;

use arena_bots::bot::{BOT_VERSION, BotKind};
use arena_core::config::GameConfig;
use arena_core::event::Event;
use arena_core::geometry::area2;
use arena_core::map::MapDefinition;
use arena_core::resolve::{CommandEffect, CommandOutcome, RoundReport};
use arena_core::state::GameState;
use arena_core::types::{Faction, FieldId, PortalId, Score2};

use crate::batch::MatchDescriptor;
use crate::policy::BucketPolicy;

/// Match outcome, including the crash path: a panicking bot is recorded as
/// `Crash` and STAYS in the result set (failures count, P2.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    EnlWin,
    ResWin,
    Draw,
    Crash,
}

impl Outcome {
    pub fn tag(self) -> &'static str {
        match self {
            Outcome::EnlWin => "enl-win",
            Outcome::ResWin => "res-win",
            Outcome::Draw => "draw",
            Outcome::Crash => "crash",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Outcome> {
        match tag {
            "enl-win" => Some(Outcome::EnlWin),
            "res-win" => Some(Outcome::ResWin),
            "draw" => Some(Outcome::Draw),
            "crash" => Some(Outcome::Crash),
            _ => None,
        }
    }
}

/// The snowball checkpoints (R-BALANCE-02 / P2.5-METRICS).
pub const CHECKPOINTS: [u32; 4] = [50, 100, 200, 300];

/// One faction's counters inside one match.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct FactionMatchStats {
    // Command mix (from execution_order outcomes).
    pub commands_submitted: u64,
    pub commands_ok: u64,
    pub commands_dropped: u64,
    pub moves: u64,
    pub captures: u64,
    pub attacks: u64,
    pub attack_hits: u64,
    pub anchor_attack_hits: u64,
    pub recharges: u64,
    pub hacks: u64,
    pub keys_gained: u64,
    pub transfers: u64,
    pub links_created: u64,
    pub links_by_reinforcements: u64,
    // Field game.
    pub first_field_round: Option<u32>,
    pub fields_created: u64,
    pub fields_destroyed: u64,
    /// Sum of `area2` over every field ever created (exact integer).
    pub field_area2_sum: u128,
    /// Sum of `destroyed_round - created_round` over destroyed fields.
    pub destroyed_lifetime_rounds_sum: u64,
    pub fields_surviving: u64,
    /// Sum of `total_rounds - created_round` over survivors.
    pub surviving_lifetime_rounds_sum: u64,
    /// Max live fields sharing one portal at any moment.
    pub layering_depth_max: u32,
    /// Summed agent supply at match end.
    pub supply_remaining: u64,
}

impl FactionMatchStats {
    /// Drop rate in permille (0 when nothing was submitted).
    pub fn drop_rate_permille(&self) -> u64 {
        if self.commands_submitted == 0 {
            return 0;
        }
        self.commands_dropped * 1000 / self.commands_submitted
    }

    /// Anchor-attack share in permille (attacks on live enemy-field
    /// vertices over all attack hits; 0 when no attack hit).
    pub fn anchor_share_permille(&self) -> u64 {
        if self.attack_hits == 0 {
            return 0;
        }
        self.anchor_attack_hits * 1000 / self.attack_hits
    }

    /// Key utilization in permille (keys consumed by LINKs over keys from
    /// HACKed effects; 0 when no keys were hacked).
    pub fn key_utilization_permille(&self) -> u64 {
        if self.keys_gained == 0 {
            return 0;
        }
        self.links_created.min(self.keys_gained) as u64 * 1000 / self.keys_gained as u64
    }

    /// Reinforcement impact in permille (links by reinforcement agents —
    /// `id >= 2n` — over all own links; 0 when no links).
    pub fn reinforcement_share_permille(&self) -> u64 {
        if self.links_created == 0 {
            return 0;
        }
        self.links_by_reinforcements * 1000 / self.links_created
    }
}

/// Cumulative score snapshot at one checkpoint round.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ScoreSnapshot {
    pub round: u32,
    pub enl: Score2,
    pub res: Score2,
    /// `None` when the factions were tied at the checkpoint (excluded from
    /// snowball rates; the tie rate is reported separately).
    pub leader: Option<Faction>,
}

/// All metrics of one match.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct MatchStats {
    pub enl: FactionMatchStats,
    pub res: FactionMatchStats,
    /// Snapshots at [`CHECKPOINTS`] in order (zeros until reached).
    pub checkpoints: [ScoreSnapshot; 4],
}

impl MatchStats {
    pub fn new() -> Self {
        MatchStats {
            checkpoints: CHECKPOINTS.map(|round| ScoreSnapshot {
                round,
                enl: 0,
                res: 0,
                leader: None,
            }),
            ..Default::default()
        }
    }

    fn faction(&mut self, faction: Faction) -> &mut FactionMatchStats {
        match faction {
            Faction::Enl => &mut self.enl,
            Faction::Res => &mut self.res,
        }
    }
}

/// The live-field bookkeeping used while folding (per faction: created
/// rounds by field id + per-portal live counts).
#[derive(Default)]
struct LiveFields {
    created: BTreeMap<FieldId, u32>,
    depth: BTreeMap<PortalId, u32>,
}

impl LiveFields {
    fn insert(&mut self, field: FieldId, round: u32, stats: &mut FactionMatchStats) {
        self.created.insert(field, round);
        for portal in [field.a, field.b, field.c] {
            let depth = self.depth.entry(portal).or_insert(0);
            *depth += 1;
            if *depth > stats.layering_depth_max {
                stats.layering_depth_max = *depth;
            }
        }
        stats.fields_created += 1;
    }

    /// The neutralization cascade of `portal` (D40): every live field
    /// containing it dies.
    fn cascade(&mut self, portal: PortalId, round: u32, stats: &mut FactionMatchStats) {
        let dead: Vec<FieldId> = self
            .created
            .keys()
            .copied()
            .filter(|id| id.contains(portal))
            .collect();
        for field in dead {
            let created = self.created.remove(&field).expect("just listed");
            for vertex in [field.a, field.b, field.c] {
                if let Some(depth) = self.depth.get_mut(&vertex) {
                    *depth = depth.saturating_sub(1);
                }
            }
            stats.fields_destroyed += 1;
            stats.destroyed_lifetime_rounds_sum += round.saturating_sub(created) as u64;
        }
    }
}

/// Incremental folder for one whole match: keeps the persistent live-field
/// sets between rounds and the telescoping cumulative scores.
#[derive(Default)]
pub struct MatchFold {
    pub stats: MatchStats,
    live: BTreeMap<Faction, LiveFields>,
    cumulative: BTreeMap<Faction, Score2>,
    checkpoints_seen: usize,
}

impl MatchFold {
    pub fn new() -> Self {
        let mut live: BTreeMap<Faction, LiveFields> = BTreeMap::new();
        live.entry(Faction::Enl).or_default();
        live.entry(Faction::Res).or_default();
        MatchFold {
            stats: MatchStats::new(),
            live,
            ..Default::default()
        }
    }

    /// Folds one settled round (`report` + the round's new events).
    pub fn round(
        &mut self,
        round: u32,
        report: &RoundReport,
        events: &[Event],
        map: &MapDefinition,
        cfg: &GameConfig,
    ) {
        let initial_agents_total = 2 * cfg.initial_agents_per_faction as u32;
        for stream in &report.execution_order {
            let faction = stream.faction;
            for outcome in &stream.outcomes {
                self.stats.faction(faction).commands_submitted += 1;
                match outcome {
                    CommandOutcome::Ok(effect) => {
                        self.stats.faction(faction).commands_ok += 1;
                        self.fold_effect(
                            faction,
                            stream.agent_id.0 >= initial_agents_total,
                            effect,
                            round,
                            map,
                        );
                    }
                    CommandOutcome::Drop(_) => {
                        self.stats.faction(faction).commands_dropped += 1;
                    }
                }
            }
        }

        // Portal turnover from the round's broadcast capture events.
        for event in events {
            if let arena_core::event::EventKind::PortalCaptured { faction, .. } = event.kind {
                self.stats.faction(faction).captures += 1;
            }
        }

        // Telescoping cumulative scores (R-SCORE-01: deltas sum to totals).
        for faction in [Faction::Enl, Faction::Res] {
            let delta = report.score2_delta.get(&faction).copied().unwrap_or(0);
            let total = self.cumulative.entry(faction).or_insert(0);
            *total += delta;
        }
        while self.checkpoints_seen < CHECKPOINTS.len()
            && round == CHECKPOINTS[self.checkpoints_seen]
        {
            let index = self.checkpoints_seen;
            let enl = self.cumulative.get(&Faction::Enl).copied().unwrap_or(0);
            let res = self.cumulative.get(&Faction::Res).copied().unwrap_or(0);
            self.stats.checkpoints[index].enl = enl;
            self.stats.checkpoints[index].res = res;
            self.stats.checkpoints[index].leader = match enl.cmp(&res) {
                std::cmp::Ordering::Greater => Some(Faction::Enl),
                std::cmp::Ordering::Less => Some(Faction::Res),
                std::cmp::Ordering::Equal => None,
            };
            self.checkpoints_seen += 1;
        }
    }

    fn fold_effect(
        &mut self,
        faction: Faction,
        is_reinforcement: bool,
        effect: &CommandEffect,
        round: u32,
        map: &MapDefinition,
    ) {
        match effect {
            CommandEffect::Moved { .. } => self.stats.faction(faction).moves += 1,
            CommandEffect::Captured { .. } => {
                // Counted from the PortalCaptured events (authoritative
                // broadcast source); nothing here.
            }
            CommandEffect::Attacked { hits } => {
                let stats = self.stats.faction(faction);
                stats.attacks += 1;
                stats.attack_hits += hits.len() as u64;
                let enemy = faction.opposite();
                let live = self.live.get(&enemy).expect("both factions tracked");
                for hit in hits {
                    if live.depth.get(&hit.portal).copied().unwrap_or(0) > 0 {
                        stats.anchor_attack_hits += 1;
                    }
                }
                let neutralized: Vec<PortalId> = hits
                    .iter()
                    .filter(|hit| hit.neutralized)
                    .map(|hit| hit.portal)
                    .collect();
                for portal in neutralized {
                    // The cascade kills the DEFENDING faction's fields (a
                    // field's vertices always share its owner faction,
                    // D40) — destruction is credited to the field OWNER's
                    // stats, not the attacker's.
                    let live = self.live.get_mut(&enemy).expect("both factions tracked");
                    let owner_stats = self.stats.faction(enemy);
                    live.cascade(portal, round, owner_stats);
                }
            }
            CommandEffect::Recharged { .. } => self.stats.faction(faction).recharges += 1,
            CommandEffect::Hacked { keys_gained, .. } => {
                let stats = self.stats.faction(faction);
                stats.hacks += 1;
                stats.keys_gained += *keys_gained as u64;
            }
            CommandEffect::Transferred { .. } => self.stats.faction(faction).transfers += 1,
            CommandEffect::Linked { fields, .. } => {
                {
                    let stats = self.stats.faction(faction);
                    stats.links_created += 1;
                    if is_reinforcement {
                        stats.links_by_reinforcements += 1;
                    }
                }
                for field in fields {
                    let position = |portal: PortalId| {
                        map.portal(portal)
                            .expect("field vertices are map portals")
                            .position
                    };
                    let area = area2(position(field.a), position(field.b), position(field.c));
                    let live = self.live.get_mut(&faction).expect("both factions tracked");
                    let stats = self.stats.faction(faction);
                    if stats.first_field_round.is_none() {
                        stats.first_field_round = Some(round);
                    }
                    stats.field_area2_sum += area as u128;
                    live.insert(*field, round, stats);
                }
            }
        }
    }

    /// Closes the match: survivor lifetimes and remaining supply from the
    /// final state.
    pub fn finish(&mut self, state: &GameState, cfg: &GameConfig) {
        for faction in [Faction::Enl, Faction::Res] {
            let live = self.live.get(&faction).expect("both factions tracked");
            let stats = self.stats.faction(faction);
            for created in live.created.values() {
                stats.fields_surviving += 1;
                stats.surviving_lifetime_rounds_sum +=
                    cfg.total_rounds.saturating_sub(*created) as u64;
            }
            stats.supply_remaining = state
                .agents()
                .values()
                .filter(|agent| agent.faction == faction)
                .map(|agent| agent.supply as u64)
                .sum();
        }
    }
}

/// Everything one executed match contributes to the batch (D57): the
/// descriptor echo, the outcome, final scores and state hash, the folded
/// stats, and elapsed wall-clock as METADATA ONLY.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MatchRecord {
    pub index: u64,
    pub matchup_index: u32,
    pub matchup_tag: String,
    pub j: u32,
    pub bot_a: BotKind,
    pub bot_b: BotKind,
    pub bot_a_is_enl: bool,
    pub game_seed: u64,
    pub map_seed: u64,
    pub map_index: u32,
    pub bot_seed: u64,
    pub schedule_seed: u64,
    pub bucket_policy: BucketPolicy,
    pub config_hash: u64,
    /// Owned so parsed records round-trip without static promotion.
    pub bot_a_version: String,
    pub bot_b_version: String,
    pub result: Outcome,
    pub enl_score2: Score2,
    pub res_score2: Score2,
    pub final_state_hash: [u8; 32],
    pub stats: MatchStats,
    pub elapsed_ms: u64,
}

impl MatchRecord {
    /// Builds a record from a successful match.
    pub fn from_match(
        descriptor: &MatchDescriptor,
        state: &GameState,
        stats: MatchStats,
        elapsed_ms: u64,
    ) -> Self {
        let result = match state.final_result() {
            arena_core::types::MatchResult::EnlWin => Outcome::EnlWin,
            arena_core::types::MatchResult::ResWin => Outcome::ResWin,
            arena_core::types::MatchResult::Draw => Outcome::Draw,
        };
        MatchRecord {
            index: descriptor.index,
            matchup_index: descriptor.matchup_index,
            matchup_tag: descriptor.matchup_tag.clone(),
            j: descriptor.j,
            bot_a: descriptor.bot_a,
            bot_b: descriptor.bot_b,
            bot_a_is_enl: descriptor.bot_a_is_enl,
            game_seed: descriptor.game_seed,
            map_seed: descriptor.map_seed,
            map_index: descriptor.map_index,
            bot_seed: descriptor.bot_seed,
            schedule_seed: descriptor.schedule_seed,
            bucket_policy: descriptor.bucket_policy,
            config_hash: descriptor.config_hash,
            bot_a_version: descriptor.bot_a_version.to_string(),
            bot_b_version: descriptor.bot_b_version.to_string(),
            result,
            enl_score2: state.score(Faction::Enl),
            res_score2: state.score(Faction::Res),
            final_state_hash: arena_replay::canon::state_hash(state),
            stats,
            elapsed_ms,
        }
    }

    /// The crash record: the descriptor echo plus a `Crash` outcome with
    /// empty stats. Crashed matches stay in the result set.
    pub fn crashed(descriptor: &MatchDescriptor, elapsed_ms: u64) -> Self {
        MatchRecord {
            index: descriptor.index,
            matchup_index: descriptor.matchup_index,
            matchup_tag: descriptor.matchup_tag.clone(),
            j: descriptor.j,
            bot_a: descriptor.bot_a,
            bot_b: descriptor.bot_b,
            bot_a_is_enl: descriptor.bot_a_is_enl,
            game_seed: descriptor.game_seed,
            map_seed: descriptor.map_seed,
            map_index: descriptor.map_index,
            bot_seed: descriptor.bot_seed,
            schedule_seed: descriptor.schedule_seed,
            bucket_policy: descriptor.bucket_policy,
            config_hash: descriptor.config_hash,
            bot_a_version: descriptor.bot_a_version.to_string(),
            bot_b_version: descriptor.bot_b_version.to_string(),
            result: Outcome::Crash,
            enl_score2: 0,
            res_score2: 0,
            final_state_hash: [0u8; 32],
            stats: MatchStats::new(),
            elapsed_ms,
        }
    }

    /// Equality ignoring the metadata-only elapsed time (reproducibility
    /// checks compare everything else).
    pub fn same_ignoring_elapsed(&self, other: &MatchRecord) -> bool {
        let mut a = self.clone();
        let mut b = other.clone();
        a.elapsed_ms = 0;
        b.elapsed_ms = 0;
        a == b
    }
}

/// The default bot version (re-exported for artifact writers).
pub const DEFAULT_BOT_VERSION: &str = BOT_VERSION;

//! The B2 readiness layer (`docs/balance/ai-balance-plan.md` section 10.1,
//!): the three frozen measurement-tool validation experiments
//! (round-robin of the six R2 policies including mirrors, idle
//! benchmark, blocking probe with full replay capture) and the SEVEN
//! gate evaluators as pure functions over `BalanceMatchRecord`s (plus
//! captured replays for the blocking and mirror gates). Everything here
//! is OFFLINE ANALYSIS: integer raw counts with permille next to — never
//! instead of — the counts, and nothing derived here enters any Game
//! Result, Replay Hash or rule settlement (plan section 9.3).

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use arena_bots::policy::{PolicyId, PolicySpec};
use arena_core::config::GameConfig;
use arena_core::event::EventKind;
use arena_core::geometry::{dist2, point_on_segment, segments_intersect};
use arena_core::types::{Faction, LinkId, Owner, PortalId};
use arena_replay::format::ReplayFile;

use crate::balance::execute::BalanceReplay;
use crate::balance::plan::{ExperimentPlan, PolicyMatchup, SeedSet, Stage};
use crate::balance::record::{BalanceMatchRecord, BalanceOutcome};
use crate::metrics::Outcome;
use crate::policy::BucketPolicy;

// ---------------------------------------------------------------------
// The frozen readiness experiment plans
// ---------------------------------------------------------------------

/// The frozen experiment ids (archive directory names). `-r1` is the
/// first official attempt, superseded by the standby-deadlock fix; `-r2`
/// was superseded because its `matches.tsv` predates the per-row policy
/// parameter columns, leaving the parameters half of the frozen
/// records-integrity gate (gate 7) without archived per-match
/// evidence — both stay on disk as history, never overwritten; `-r3` is
/// the current official run of the same frozen plans. The pilot runs
/// append `-pilot-r0` and are exploratory only — never gate evidence.
pub const B2_RR_EXPERIMENT_ID: &str = "b2-readiness-rr-r3";
pub const B2_IDLE_EXPERIMENT_ID: &str = "b2-readiness-idle-r3";
pub const B2_BLOCK_EXPERIMENT_ID: &str = "b2-readiness-block-r3";

/// The official map counts (frozen before the official run; plan section
/// 8.3 never reduces swap legs — every unit runs both legs).
pub const B2_RR_MAP_COUNT: u32 = 100;
pub const B2_IDLE_MAP_COUNT: u32 = 50;
pub const B2_BLOCK_MAP_COUNT: u32 = 50;

/// The 21 ordered round-robin pairings of the six competitive R2
/// policies (15 distinct pairs + 6 mirrors), declaration order: the
/// `PolicyId::R2_COMPETITIVE` registry order.
pub fn r2_round_robin_matchups() -> Vec<PolicyMatchup> {
    let ids = PolicyId::R2_COMPETITIVE;
    let mut matchups = Vec::new();
    for (index, &a) in ids.iter().enumerate() {
        for &b in ids.iter().skip(index + 1) {
            matchups.push(PolicyMatchup::new(
                &format!("{}--{}", a.tag(), b.tag()),
                a,
                b,
            ));
        }
    }
    for &a in ids.iter() {
        matchups.push(PolicyMatchup::new(
            &format!("{}--{}", a.tag(), a.tag()),
            a,
            a,
        ));
    }
    matchups
}

/// The round-robin readiness plan: all six competitive R2 policies over
/// every pairing including mirrors, Development maps, `Flat(0)`, default
/// config only. Mirror cells on the first [`B2_MIRROR_REPLAY_MAPS`] maps
/// capture BOTH legs' replays (the mirror gate reads spawn sets from
/// replay headers).
pub fn b2_round_robin_plan(map_count: u32, experiment_id: &str) -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Readiness,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: vec![crate::balance::plan::ConfigVariant::new(
            "default",
            GameConfig::default(),
            Vec::new(),
        )],
        policies: arena_bots::policy::r2_specs(),
        matchups: r2_round_robin_matchups(),
        bucket_policy: BucketPolicy::Flat(0),
        gates: vec![
            crate::balance::plan::Gate::Pending {
                gate_id: "sustained-scoring".to_string(),
                statement: ">=4 competitive policies from >=3 families: score2>0 in >=90% and fields_created>=1 in >=80% of non-crash round-robin matches".to_string(),
            },
            crate::balance::plan::Gate::Pending {
                gate_id: "transfer-usage".to_string(),
                statement: ">=1 policy with transfers>0 in >=50% of its non-crash round-robin matches".to_string(),
            },
            crate::balance::plan::Gate::Pending {
                gate_id: "command-drop".to_string(),
                statement: "pooled competitive commands_dropped/commands_submitted < 50 permille".to_string(),
            },
            crate::balance::plan::Gate::Pending {
                gate_id: "mirror-conflict".to_string(),
                statement: "zero mirror legs with identical faction spawn sets".to_string(),
            },
            crate::balance::plan::Gate::Pending {
                gate_id: "records-integrity".to_string(),
                statement: "every match record's policy family/version/parameters match policies.tsv".to_string(),
            },
        ],
    }
}

/// The idle benchmark plan: every competitive R2 policy vs the diagnostic
/// `idle` opponent (plan 10.1 idle gate).
pub fn b2_idle_plan(map_count: u32, experiment_id: &str) -> ExperimentPlan {
    let mut matchups = Vec::new();
    for &policy in PolicyId::R2_COMPETITIVE.iter() {
        matchups.push(PolicyMatchup::new(
            &format!("{}--idle", policy.tag()),
            policy,
            PolicyId::Idle,
        ));
    }
    ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Readiness,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: vec![crate::balance::plan::ConfigVariant::new(
            "default",
            GameConfig::default(),
            Vec::new(),
        )],
        policies: arena_bots::policy::r2_specs(),
        matchups,
        bucket_policy: BucketPolicy::Flat(0),
        gates: vec![crate::balance::plan::Gate::Pending {
            gate_id: "idle-scoring".to_string(),
            statement: "every competitive R2 policy: score2>0 AND fields_created>=1 in 100% of non-crash idle matches".to_string(),
        }],
    }
}

/// The blocking probe plan: `blocker-builder-r2` against a competitive
/// scorer (`adaptive-hybrid-r2`), every leg's replay captured (the
/// blocking proxy runs offline over the replay event log plus the
/// archived Map Definition).
pub fn b2_blocking_plan(map_count: u32, experiment_id: &str) -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: experiment_id.to_string(),
        stage: Stage::Readiness,
        seed_set: SeedSet::Development,
        map_count,
        base_variant: "default".to_string(),
        variants: vec![crate::balance::plan::ConfigVariant::new(
            "default",
            GameConfig::default(),
            Vec::new(),
        )],
        policies: arena_bots::policy::r2_specs(),
        matchups: vec![PolicyMatchup::new(
            "blocker-builder-r2--adaptive-hybrid-r2",
            PolicyId::BlockerBuilderR2,
            PolicyId::AdaptiveHybridR2,
        )],
        bucket_policy: BucketPolicy::Flat(0),
        gates: vec![crate::balance::plan::Gate::Pending {
            gate_id: "active-blocking".to_string(),
            statement: "blocker-builder-r2 has >=1 active block in >=10% of non-crash probe matches and >=5 blocks aggregate".to_string(),
        }],
    }
}

/// Mirror cells on the first N maps capture BOTH legs' replays in the
/// round-robin plan (the mirror gate needs spawn sets from headers).
pub const B2_MIRROR_REPLAY_MAPS: u32 = 12;

/// The round-robin replay sampling: every leg-0 cell plus both legs of
/// mirror cells on the first [`B2_MIRROR_REPLAY_MAPS`] maps.
pub fn rr_replay_sample(cell: &crate::balance::schedule::MatchCell) -> bool {
    cell.leg == 0 || (cell.policy_a == cell.policy_b && cell.map_index < B2_MIRROR_REPLAY_MAPS)
}

/// The blocking probe replay sampling: EVERY leg captured.
pub fn all_legs_replay_sample(_cell: &crate::balance::schedule::MatchCell) -> bool {
    true
}

// ---------------------------------------------------------------------
// Gate evaluators (pure functions; integer raw counts)
// ---------------------------------------------------------------------

/// One gate's outcome: the raw numerator/denominator pair, the derived
/// permille, the frozen threshold and the pass flag. Counts are verbatim
/// — the permille is a display derivation only (plan 9.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GateCount {
    pub gate_id: String,
    pub statement: String,
    pub numerator: u64,
    pub denominator: u64,
    pub permille: u64,
    pub passed: bool,
    pub detail: Vec<String>,
}

impl GateCount {
    fn new(
        gate_id: &str,
        statement: &str,
        numerator: u64,
        denominator: u64,
        threshold_permille: u64,
        passed: bool,
        detail: Vec<String>,
    ) -> Self {
        let permille = if denominator == 0 {
            0
        } else {
            numerator * 1000 / denominator
        };
        let _ = threshold_permille;
        GateCount {
            gate_id: gate_id.to_string(),
            statement: statement.to_string(),
            numerator,
            denominator,
            permille,
            passed,
            detail,
        }
    }

    /// One machine-recomputable line: `gate num den permille pass`.
    pub fn count_line(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.gate_id,
            self.numerator,
            self.denominator,
            self.permille,
            u8::from(self.passed)
        )
    }
}

/// Whether a record is a played, non-crash, non-config-failure match.
fn played(record: &BalanceMatchRecord) -> bool {
    !matches!(
        record.result,
        BalanceOutcome::ConfigFailure | BalanceOutcome::Played(Outcome::Crash)
    )
}

/// The (enl, res) policy tags of a record.
fn factions_of(record: &BalanceMatchRecord) -> (&str, &str) {
    if record.policy_a_is_enl {
        (record.policy_a.tag(), record.policy_b.tag())
    } else {
        (record.policy_b.tag(), record.policy_a.tag())
    }
}

/// One policy's score2 and fields_created inside one record it PLAYED
/// (`None` when the policy is not one of the record's two sides —
/// participation is decided by TAG MEMBERSHIP, never by a zero score).
fn policy_stats(record: &BalanceMatchRecord, tag: &str) -> Option<(u128, u64)> {
    let (enl_tag, res_tag) = factions_of(record);
    if tag == enl_tag {
        Some((record.enl_score2 as u128, record.stats.enl.fields_created))
    } else if tag == res_tag {
        Some((record.res_score2 as u128, record.stats.res.fields_created))
    } else {
        None
    }
}

/// One policy's per-match mechanism presence of a stat (transfers).
fn policy_stat(
    record: &BalanceMatchRecord,
    tag: &str,
    stat: fn(&crate::metrics::FactionMatchStats) -> u64,
) -> u64 {
    let (enl_tag, res_tag) = factions_of(record);
    if tag == enl_tag {
        stat(&record.stats.enl)
    } else if tag == res_tag {
        stat(&record.stats.res)
    } else {
        0
    }
}

/// Gate 1 — sustainable scoring (plan 10.1): per competitive policy, the
/// share of non-crash round-robin matches with `score2 > 0` (>= 90%) and
/// with `fields_created >= 1` (>= 80%); the gate passes when at least 4
/// policies from at least 3 competitive families qualify.
pub fn sustained_scoring_gate(records: &[BalanceMatchRecord], specs: &[PolicySpec]) -> GateCount {
    let mut detail = Vec::new();
    let mut qualifying = BTreeSet::new();
    let mut families = BTreeSet::new();
    for spec in specs.iter().filter(|s| !s.family.is_diagnostic()) {
        let tag = spec.policy_id.tag();
        let mut played_count = 0u64;
        let mut scored = 0u64;
        let mut fielded = 0u64;
        for record in records.iter().filter(|r| played(r)) {
            let Some((score, fields)) = policy_stats(record, tag) else {
                continue; // did not play this match
            };
            played_count += 1;
            scored += u64::from(score > 0);
            fielded += u64::from(fields >= 1);
        }
        if played_count == 0 {
            detail.push(format!("{tag}: no matches"));
            continue;
        }
        let score_share = scored * 1000 / played_count;
        let field_share = fielded * 1000 / played_count;
        let ok = score_share >= 900 && field_share >= 800;
        detail.push(format!(
            "{tag}: scored {scored}/{played_count} ({} permille), fields {fielded}/{played_count} ({} permille){}",
            score_share,
            field_share,
            if ok { " [qualifies]" } else { "" }
        ));
        if ok {
            qualifying.insert(spec.policy_id);
            families.insert(spec.family);
        }
    }
    let passed = qualifying.len() >= 4 && families.len() >= 3;
    GateCount::new(
        "sustained-scoring",
        ">=4 policies from >=3 families: score2>0 >=90% and fields>=1 >=80% of non-crash RR matches",
        qualifying.len() as u64,
        4,
        0,
        passed,
        {
            let mut d = detail;
            d.push(format!("qualifying_families={}", families.len()));
            d
        },
    )
}

/// Gate 2 — idle scoring (plan 10.1): every competitive R2 policy has
/// `score2 > 0` AND `fields_created >= 1` in 100% of its non-crash idle
/// matches (any crash also fails its match).
pub fn idle_gate(records: &[BalanceMatchRecord]) -> GateCount {
    let mut detail = Vec::new();
    let mut total = 0u64;
    let mut passing = 0u64;
    for &policy in PolicyId::R2_COMPETITIVE.iter() {
        let tag = policy.tag();
        let mut matches = 0u64;
        let mut good = 0u64;
        for record in records.iter().filter(|r| played(r)) {
            let Some((score, fields)) = policy_stats(record, tag) else {
                continue; // did not play this match
            };
            matches += 1;
            good += u64::from(score > 0 && fields >= 1);
        }
        if matches == 0 {
            detail.push(format!("{tag}: no idle matches"));
            continue;
        }
        total += matches;
        passing += good;
        if good < matches {
            detail.push(format!(
                "{tag}: {good}/{matches} idle matches score AND field"
            ));
        }
    }
    let passed = total > 0 && passing == total;
    GateCount::new(
        "idle-scoring",
        "every competitive R2 policy: score2>0 AND fields>=1 in 100% of non-crash idle matches",
        passing,
        total,
        0,
        passed,
        detail,
    )
}

/// Gate 3 — Transfer usage (plan 10.1): at least one policy with
/// `transfers > 0` in >= 50% of its non-crash round-robin matches.
pub fn transfer_gate(records: &[BalanceMatchRecord]) -> GateCount {
    let mut detail = Vec::new();
    let mut best: Option<(String, u64, u64)> = None;
    for &policy in PolicyId::R2_COMPETITIVE.iter() {
        let tag = policy.tag();
        let mut matches = 0u64;
        let mut used = 0u64;
        for record in records.iter().filter(|r| played(r)) {
            if policy_stats(record, tag).is_none() {
                continue; // did not play this match
            }
            let transfers = policy_stat(record, tag, |stats| stats.transfers);
            matches += 1;
            used += u64::from(transfers > 0);
        }
        if matches == 0 {
            continue;
        }
        detail.push(format!("{tag}: transfers>0 in {used}/{matches}"));
        let better = best
            .as_ref()
            .map(|(_, used_b, matches_b)| used * matches_b > *used_b * matches)
            .unwrap_or(true);
        if better {
            best = Some((tag.to_string(), used, matches));
        }
    }
    let (tag, used, matches) = best.unwrap_or(("none".to_string(), 0, 0));
    let passed = matches > 0 && used * 2 >= matches;
    let mut d = detail;
    d.insert(0, format!("best: {tag} {used}/{matches}"));
    GateCount::new(
        "transfer-usage",
        ">=1 policy with transfers>0 in >=50% of its non-crash RR matches",
        used,
        matches,
        0,
        passed,
        d,
    )
}

/// Gate 5 — Command Drop (plan 10.1): the POOLED drop rate of
/// competitive matches (both sides competitive policies) stays below
/// 50 permille (the plan's 5%); above the threshold, the failure
/// requires a DropReason breakdown over captured replays (drops.rs
/// machinery — reused from B0).
pub fn drop_gate(records: &[BalanceMatchRecord]) -> GateCount {
    let mut submitted = 0u64;
    let mut dropped = 0u64;
    for record in records.iter().filter(|r| played(r)) {
        let competitive = |tag: &str| {
            PolicyId::from_tag(tag)
                .map(|id| !id.family().is_diagnostic())
                .unwrap_or(false)
        };
        let (enl_tag, res_tag) = factions_of(record);
        if !competitive(enl_tag) || !competitive(res_tag) {
            continue;
        }
        submitted += record.stats.enl.commands_submitted + record.stats.res.commands_submitted;
        dropped += record.stats.enl.commands_dropped + record.stats.res.commands_dropped;
    }
    let permille = if submitted == 0 {
        0
    } else {
        dropped * 1000 / submitted
    };
    GateCount::new(
        "command-drop",
        "pooled competitive commands_dropped/commands_submitted < 50 permille",
        dropped,
        submitted,
        50,
        submitted > 0 && permille < 50,
        vec![format!("drop_rate_permille={permille}")],
    )
}

/// Gate 6 — mirror conflicts (plan 10.1): no mirror leg's two factions
/// chose the identical Initial Spawn Set (read from the replay headers
/// of captured mirror legs); paired mirror asymmetry is reported as raw
/// counts (units where both legs' outcomes agree).
pub fn mirror_gate(records: &[BalanceMatchRecord], replays: &[BalanceReplay]) -> GateCount {
    let by_cell: BTreeMap<u64, &BalanceReplay> =
        replays.iter().map(|r| (r.cell_index, r)).collect();
    let mut legs = 0u64;
    let mut identical = 0u64;
    let mut mirror_units = 0u64;
    let mut symmetric_outcomes = 0u64;
    let mut detail = Vec::new();
    // Outcome symmetry per mirror unit (leg pairs by pair_id).
    let mut unit_outcomes: BTreeMap<String, Vec<&BalanceOutcome>> = BTreeMap::new();
    for record in records
        .iter()
        .filter(|r| played(r) && r.policy_a == r.policy_b)
    {
        unit_outcomes
            .entry(record.pair_id.clone())
            .or_default()
            .push(&record.result);
    }
    for (pair, outcomes) in &unit_outcomes {
        if outcomes.len() == 2 && outcomes[0] == outcomes[1] {
            symmetric_outcomes += 1;
        }
        mirror_units += 1;
        let _ = pair;
    }
    for record in records.iter().filter(|r| r.policy_a == r.policy_b) {
        let Some(replay) = by_cell.get(&record.index) else {
            continue;
        };
        let Ok(file) = arena_replay::replay::decode_log(&replay.bytes) else {
            detail.push(format!("cell {}: replay undecodable", record.index));
            continue;
        };
        legs += 1;
        let same = sorted_spawns(&file.header.enl_spawns) == sorted_spawns(&file.header.res_spawns);
        if same {
            identical += 1;
            detail.push(format!(
                "cell {}: identical faction spawn sets",
                record.index
            ));
        }
    }
    let mut d = detail;
    d.insert(
        0,
        format!(
            "mirror_units={} symmetric_outcome_units={} legs_checked={legs}",
            mirror_units, symmetric_outcomes
        ),
    );
    GateCount::new(
        "mirror-conflict",
        "zero mirror legs with identical faction spawn sets",
        identical,
        legs,
        0,
        legs > 0 && identical == 0,
        d,
    )
}

/// The spawn SET of a faction as a comparable value: its positions
/// sorted (agent ids differ per faction by construction — the mirror
/// conflict is two factions choosing the identical COORDINATES).
fn sorted_spawns(
    spawns: &[(arena_core::types::AgentId, arena_core::types::Coord)],
) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = spawns.iter().map(|(_, coord)| (coord.x, coord.y)).collect();
    out.sort();
    out
}

/// Gate 7 — records integrity (plan 10.1, frozen asgate 7):
/// every match record's policy version AND parameters agree with the
/// plan's policy registry (`policies.tsv` is generated from the same
/// registry, one row per `PolicySpec` — family, version and the
/// full-value parameter serialization).
pub fn records_gate(records: &[BalanceMatchRecord], plan: &ExperimentPlan) -> GateCount {
    let mut checked = 0u64;
    let mut bad = 0u64;
    let mut mismatches = Vec::new();
    for record in records {
        for (tag, family, version, parameters) in [
            (
                record.policy_a.tag(),
                record.policy_a_family.tag(),
                record.policy_a_version.as_str(),
                &record.policy_a_parameters,
            ),
            (
                record.policy_b.tag(),
                record.policy_b_family.tag(),
                record.policy_b_version.as_str(),
                &record.policy_b_parameters,
            ),
        ] {
            let Some(spec) = plan.policies.iter().find(|s| s.policy_id.tag() == tag) else {
                mismatches.push(format!(
                    "record {}: {} not in policies.tsv",
                    record.index, tag
                ));
                continue;
            };
            checked += 1;
            let mut side_bad = false;
            if spec.family.tag() != family || spec.version != version {
                mismatches.push(format!(
                    "record {}: {} family/version {}/{} != {}/{}",
                    record.index,
                    tag,
                    family,
                    version,
                    spec.family.tag(),
                    spec.version
                ));
                side_bad = true;
            }
            // The parameters half of the frozen check: the record's
            // complete parameter set must equal the registry row's
            // (typed equality; the full-value tags are echoed for
            // humans only).
            if spec.parameters != *parameters {
                mismatches.push(format!(
                    "record {}: {} parameters {} != {}",
                    record.index,
                    tag,
                    parameters.tag(),
                    spec.parameters.tag()
                ));
                side_bad = true;
            }
            bad += u64::from(side_bad);
        }
    }
    GateCount::new(
        "records-integrity",
        "every match record's policy family/version/parameters match policies.tsv",
        checked - bad.min(checked),
        checked,
        0,
        mismatches.is_empty() && checked > 0,
        mismatches,
    )
}

// ---------------------------------------------------------------------
// The active-blocking geometric proxy (, frozen definition)
// ---------------------------------------------------------------------

/// Counts `blocker`'s ACTIVE BLOCKS in one replay: own `LinkCreated`
/// events whose closed segment intersects the closed segment of at least
/// one enemy candidate pair — a pair of portals both enemy-owned at that
/// moment, within the closed `MAX_LINK_DISTANCE`, with no live link on
/// the pair and no third portal on the pair's closed segment (shared
/// endpoints do not block). Portal ownership and the live-link set are
/// reconstructed from the map plus the event timeline
/// (`PortalCaptured`/`PortalNeutralized`, `LinkCreated`/`LinkDestroyed`).
pub fn count_active_blocks(replay: &ReplayFile, blocker: Faction) -> u64 {
    let enemy_owner = match blocker.opposite() {
        Faction::Enl => Owner::Enl,
        Faction::Res => Owner::Res,
    };
    let cfg = &replay.header.config;
    let max2 = (cfg.max_link_distance_cm as i64) * (cfg.max_link_distance_cm as i64);
    let portals: Vec<(PortalId, arena_core::types::Coord, Owner)> = replay
        .header
        .map
        .portals()
        .iter()
        .map(|p| (p.id, p.position, p.owner))
        .collect();
    let mut owners: BTreeMap<PortalId, Owner> =
        portals.iter().map(|(id, _, owner)| (*id, *owner)).collect();
    let mut links: BTreeSet<LinkId> = BTreeSet::new();
    let mut blocks = 0u64;
    for block in replay.blocks.iter() {
        let arena_replay::format::Block::Round(round) = block else {
            continue;
        };
        for event in &round.events {
            match &event.kind {
                EventKind::LinkCreated { faction, link, .. } => {
                    if *faction == blocker {
                        if link_blocks_enemy_pair(
                            link,
                            &owners,
                            &links,
                            &portals,
                            enemy_owner,
                            max2,
                        ) {
                            blocks += 1;
                        }
                    }
                    links.insert(*link);
                }
                EventKind::LinkDestroyed { link, .. } => {
                    links.remove(link);
                }
                EventKind::PortalCaptured {
                    faction, portal, ..
                } => {
                    owners.insert(
                        *portal,
                        match faction {
                            Faction::Enl => Owner::Enl,
                            Faction::Res => Owner::Res,
                        },
                    );
                }
                EventKind::PortalNeutralized { portal, .. } => {
                    owners.insert(*portal, Owner::Neutral);
                }
                _ => {}
            }
        }
    }
    blocks
}

/// Whether `link`'s closed segment crosses at least one enemy candidate
/// pair's closed segment (no shared endpoints), under the frozen proxy.
fn link_blocks_enemy_pair(
    link: &LinkId,
    owners: &BTreeMap<PortalId, Owner>,
    links: &BTreeSet<LinkId>,
    portals: &[(PortalId, arena_core::types::Coord, Owner)],
    enemy_owner: Owner,
    max2: i64,
) -> bool {
    let (ls, lt) = (
        position_of(portals, link.min),
        position_of(portals, link.max),
    );
    let enemy: Vec<PortalId> = portals
        .iter()
        .filter(|(id, _, _)| owners.get(id) == Some(&enemy_owner))
        .map(|(id, _, _)| *id)
        .collect();
    for (index, &x) in enemy.iter().enumerate() {
        for &y in enemy.iter().skip(index + 1) {
            if x == y {
                continue;
            }
            let Some(pair_id) = LinkId::new(x, y) else {
                continue;
            };
            if links.contains(&pair_id) {
                continue; // a live link already occupies the pair
            }
            let (px, py) = (position_of(portals, x), position_of(portals, y));
            if dist2(px, py) > max2 {
                continue;
            }
            let portal_on_pair = portals.iter().any(|(id, position, _)| {
                *id != x && *id != y && point_on_segment(*position, px, py)
            });
            if portal_on_pair {
                continue;
            }
            let shared = link.contains(x) || link.contains(y);
            if !shared && segments_intersect(ls, lt, px, py) {
                return true;
            }
        }
    }
    false
}

fn position_of(
    portals: &[(PortalId, arena_core::types::Coord, Owner)],
    id: PortalId,
) -> arena_core::types::Coord {
    portals
        .iter()
        .find(|(pid, _, _)| *pid == id)
        .expect("dense portal ids")
        .1
}

/// Gate 4 — active blocking (plan 10.1): `blocker-builder-r2` performs
/// at least one active block in >= 10% of its non-crash probe matches,
/// with at least 5 blocks in aggregate (over the captured replays).
pub fn blocking_gate(
    records: &[BalanceMatchRecord],
    replays: &[BalanceReplay],
    blocker: PolicyId,
) -> GateCount {
    let by_cell: BTreeMap<u64, &BalanceReplay> =
        replays.iter().map(|r| (r.cell_index, r)).collect();
    let mut matches = 0u64;
    let mut with_block = 0u64;
    let mut aggregate = 0u64;
    let mut detail = Vec::new();
    for record in records.iter().filter(|r| played(r)) {
        let blocker_is_enl = if record.policy_a_is_enl {
            record.policy_a == blocker
        } else {
            record.policy_b == blocker
        };
        let blocker_is_res = if record.policy_a_is_enl {
            record.policy_b == blocker
        } else {
            record.policy_a == blocker
        };
        if !blocker_is_enl && !blocker_is_res {
            continue;
        }
        matches += 1;
        let Some(replay) = by_cell.get(&record.index) else {
            detail.push(format!("match {}: replay not captured", record.index));
            continue;
        };
        let Ok(file) = arena_replay::replay::decode_log(&replay.bytes) else {
            detail.push(format!("match {}: replay undecodable", record.index));
            continue;
        };
        let faction = if blocker_is_enl {
            Faction::Enl
        } else {
            Faction::Res
        };
        let blocks = count_active_blocks(&file, faction);
        aggregate += blocks;
        with_block += u64::from(blocks >= 1);
    }
    let share = if matches == 0 {
        0
    } else {
        with_block * 1000 / matches
    };
    let passed = matches > 0 && with_block * 10 >= matches && aggregate >= 5;
    let mut d = detail;
    d.insert(
        0,
        format!(
            "with_block={with_block}/{matches} ({} permille) aggregate={aggregate}",
            share
        ),
    );
    GateCount::new(
        "active-blocking",
        "blocker-builder-r2: >=1 active block in >=10% of probe matches AND >=5 aggregate",
        with_block,
        matches,
        100,
        passed,
        d,
    )
}

/// Evaluates every gate over one round-robin run (records + mirror
/// replays) — the round-robin experiment's own gate set.
pub fn evaluate_round_robin_gates(
    plan: &ExperimentPlan,
    records: &[BalanceMatchRecord],
    replays: &[BalanceReplay],
) -> Vec<GateCount> {
    vec![
        sustained_scoring_gate(records, &plan.policies),
        transfer_gate(records),
        drop_gate(records),
        mirror_gate(records, replays),
        records_gate(records, plan),
    ]
}

/// The machine-recomputable gates text (integer raw counts only).
pub fn gates_text(gates: &[GateCount]) -> String {
    let mut out =
        String::from("# B2 readiness gates (integer raw counts; permille is display-only)\n\n");
    for gate in gates {
        out.push_str(&format!(
            "## {} — {}\n  counts: numerator={} denominator={} permille={} passed={}\n",
            gate.gate_id,
            gate.statement,
            gate.numerator,
            gate.denominator,
            gate.permille,
            u8::from(gate.passed)
        ));
        for line in &gate.detail {
            out.push_str(&format!("  {line}\n"));
        }
        out.push('\n');
    }
    let passing = gates.iter().filter(|g| g.passed).count();
    out.push_str(&format!(
        "summary: {}/{} gates passed\n",
        passing,
        gates.len()
    ));
    out
}

// ---------------------------------------------------------------------
// Operator entry: `arena-runner balance-b2 <output-root> [base-commit]
// [--pilot]`
// ---------------------------------------------------------------------

use crate::balance::archive::write_balance_artifacts;
use crate::balance::execute::{BalanceRunResult, run_balance_batch_with};
use crate::balance::pairs::{compute_paired_results, variant_summaries};
use crate::run::MAX_WORKERS;

fn blake3_hex(bytes: &[u8]) -> String {
    arena_replay::canon::hex(blake3::hash(bytes).as_bytes())
}

/// Runs one readiness experiment, archives it, evaluates its gates and
/// appends `gates.txt` (with its content hash folded into `hashes.txt`).
fn run_one(
    output_root: &std::path::Path,
    plan: &ExperimentPlan,
    replay_sample: &(dyn Fn(&crate::balance::schedule::MatchCell) -> bool + Sync),
    base_commit: Option<&str>,
    gates: fn(&ExperimentPlan, &BalanceRunResult) -> Vec<GateCount>,
) -> Result<(std::path::PathBuf, Vec<GateCount>), String> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_WORKERS);
    println!(
        "balance-b2: {} — {} matchups x {} maps x 2 legs = {} matches, {workers} workers",
        plan.experiment_id,
        plan.matchups.len(),
        plan.map_count,
        plan.matchups.len() * plan.map_count as usize * 2
    );
    let run = run_balance_batch_with(
        plan,
        workers,
        &crate::balance::execute::default_cell_setup,
        replay_sample,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "balance-b2: {} done in {:?} ({} crashes, {} config failures, {} replays)",
        plan.experiment_id,
        std::time::Duration::from_millis(run.elapsed_ms as u64),
        run.crashes(),
        run.config_failures,
        run.replays.len()
    );
    let pairs = compute_paired_results(plan, &run.records).map_err(|e| e.to_string())?;
    let summaries = variant_summaries(plan, &pairs);
    let dir = write_balance_artifacts(output_root, plan, &run, &pairs, &summaries, base_commit)
        .map_err(|e| e.to_string())?;
    let evaluated = gates(plan, &run);
    let text = gates_text(&evaluated);
    let gates_path = dir.join("gates.txt");
    std::fs::write(&gates_path, &text).map_err(|e| e.to_string())?;
    // Fold the new artifact's hash into hashes.txt (keep every artifact
    // content-hashed).
    let hashes_path = dir.join("hashes.txt");
    let mut hashes = std::fs::read_to_string(&hashes_path).map_err(|e| e.to_string())?;
    hashes.push_str(&format!(
        "blake3:{}  gates.txt\n",
        blake3_hex(text.as_bytes())
    ));
    std::fs::write(&hashes_path, &hashes).map_err(|e| e.to_string())?;
    Ok((dir, evaluated))
}

fn rr_gates(plan: &ExperimentPlan, run: &BalanceRunResult) -> Vec<GateCount> {
    evaluate_round_robin_gates(plan, &run.records, &run.replays)
}

fn idle_gates(_plan: &ExperimentPlan, run: &BalanceRunResult) -> Vec<GateCount> {
    vec![idle_gate(&run.records)]
}

fn blocking_gates(_plan: &ExperimentPlan, run: &BalanceRunResult) -> Vec<GateCount> {
    vec![
        blocking_gate(&run.records, &run.replays, PolicyId::BlockerBuilderR2),
        drop_gate(&run.records),
    ]
}

/// `arena-runner balance-b2 <output-root> [base-commit] [--pilot]`: runs
/// the three frozen readiness experiments into NEW
/// `runs/balance/<experiment-id>/` directories (pilots append
/// `-pilot-r0`) and prints the gate outcomes. Returns the process exit
/// code (0 iff every gate passed).
pub fn run_b2_readiness(
    output_root: &std::path::Path,
    base_commit: Option<&str>,
    pilot: bool,
) -> i32 {
    let suffix = if pilot { "-pilot-r0" } else { "" };
    let (rr_maps, idle_maps, block_maps) = if pilot {
        (8, 4, 4)
    } else {
        (B2_RR_MAP_COUNT, B2_IDLE_MAP_COUNT, B2_BLOCK_MAP_COUNT)
    };
    let rr_plan = b2_round_robin_plan(rr_maps, &format!("{B2_RR_EXPERIMENT_ID}{suffix}"));
    let idle_plan = b2_idle_plan(idle_maps, &format!("{B2_IDLE_EXPERIMENT_ID}{suffix}"));
    let block_plan = b2_blocking_plan(block_maps, &format!("{B2_BLOCK_EXPERIMENT_ID}{suffix}"));
    for plan in [&rr_plan, &idle_plan, &block_plan] {
        if let Err(error) = plan.validate() {
            eprintln!("balance-b2: {error}");
            return 1;
        }
    }
    let mut all_gates: Vec<GateCount> = Vec::new();
    let experiments: Vec<(
        &ExperimentPlan,
        &(dyn Fn(&crate::balance::schedule::MatchCell) -> bool + Sync),
        fn(&ExperimentPlan, &BalanceRunResult) -> Vec<GateCount>,
    )> = vec![
        (&rr_plan, &rr_replay_sample, rr_gates),
        (&idle_plan, &|_cell| false, idle_gates),
        (&block_plan, &all_legs_replay_sample, blocking_gates),
    ];
    for (plan, sample, gates) in experiments {
        match run_one(output_root, plan, sample, base_commit, gates) {
            Ok((dir, mut gates)) => {
                println!("balance-b2: archived {}", dir.display());
                all_gates.append(&mut gates);
            }
            Err(error) => {
                eprintln!("balance-b2: {error}");
                return 1;
            }
        }
    }
    let failing: Vec<&GateCount> = all_gates.iter().filter(|g| !g.passed).collect();
    println!(
        "balance-b2: {}/{} readiness gates passed",
        all_gates.len() - failing.len(),
        all_gates.len()
    );
    for gate in &failing {
        eprintln!(
            "balance-b2: GATE FAILED: {} — {}",
            gate.gate_id, gate.statement
        );
    }
    i32::from(!failing.is_empty())
}

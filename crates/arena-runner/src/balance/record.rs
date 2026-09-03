//! The balance per-match record (B1, plan section 12): balance identity
//! columns plus the SAME outcome/metric payload as the frozen m6
//! `matches.tsv`. The m6 format is untouched — this is a SEPARATE TSV with
//! its own fixed column set; serialization reuses the shared metric-row
//! helpers from `report.rs` so the payload bytes can never drift.

use std::collections::BTreeMap;

use arena_bots::policy::{PolicyFamily, PolicyId, PolicySpec};
use arena_core::types::Score2;

use crate::balance::schedule::MatchCell;
use crate::metrics::{MatchRecord, MatchStats, Outcome};
use crate::policy::BucketPolicy;
use crate::report::{
    checkpoint_stat_rows, faction_stats_row, parse_checkpoints, parse_faction_stats,
    parse_state_hash_hex, u64_of, u128f_of,
};

/// A balance match outcome: the m6 outcomes plus the balance-specific
/// config failure (plan section 7.3: a variant that cannot honor its
/// shared Map Definition or cannot generate a map is RECORDED, never
/// silently skipped — and never masquerades as a score-based result).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BalanceOutcome {
    Played(Outcome),
    ConfigFailure,
}

impl BalanceOutcome {
    pub fn tag(self) -> &'static str {
        match self {
            BalanceOutcome::Played(outcome) => outcome.tag(),
            BalanceOutcome::ConfigFailure => "config-failure",
        }
    }

    pub fn from_tag(tag: &str) -> Option<BalanceOutcome> {
        match tag {
            "config-failure" => Some(BalanceOutcome::ConfigFailure),
            _ => Outcome::from_tag(tag).map(BalanceOutcome::Played),
        }
    }
}

/// Everything one executed balance match contributes: the balance
/// identity (experiment, seed set, variant, pair, leg, map, policies with
/// families and versions) plus the frozen m6 metric payload and elapsed
/// wall-clock as METADATA ONLY.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BalanceMatchRecord {
    pub index: u64,
    pub experiment_id: String,
    pub seed_set_tag: String,
    pub variant_id: String,
    pub pair_tag: String,
    pub pair_id: String,
    pub leg: u8,
    pub map_id: String,
    pub map_index: u32,
    pub policy_a: PolicyId,
    pub policy_a_family: PolicyFamily,
    pub policy_a_version: String,
    pub policy_b: PolicyId,
    pub policy_b_family: PolicyFamily,
    pub policy_b_version: String,
    pub policy_a_is_enl: bool,
    pub map_seed: u64,
    pub game_seed: u64,
    pub bot_seed: u64,
    pub schedule_seed: u64,
    pub bucket_policy: BucketPolicy,
    pub config_hash: u64,
    pub result: BalanceOutcome,
    pub enl_score2: Score2,
    pub res_score2: Score2,
    pub final_state_hash: [u8; 32],
    pub stats: MatchStats,
    pub elapsed_ms: u64,
}

impl BalanceMatchRecord {
    /// Builds a record from one executed match (the m6 `MatchRecord` of
    /// the cell's descriptor; metric folding is shared, not forked).
    pub fn from_match_record(
        cell: &MatchCell,
        spec_a: &PolicySpec,
        spec_b: &PolicySpec,
        record: &MatchRecord,
    ) -> Self {
        BalanceMatchRecord {
            index: cell.index,
            experiment_id: cell.experiment_id.clone(),
            seed_set_tag: cell.seed_set_tag.clone(),
            variant_id: cell.variant_id.clone(),
            pair_tag: cell.pair_tag.clone(),
            pair_id: cell.pair_id.clone(),
            leg: cell.leg,
            map_id: cell.map_id.clone(),
            map_index: cell.map_index,
            policy_a: cell.policy_a,
            policy_a_family: spec_a.family,
            policy_a_version: spec_a.version.to_string(),
            policy_b: cell.policy_b,
            policy_b_family: spec_b.family,
            policy_b_version: spec_b.version.to_string(),
            policy_a_is_enl: cell.policy_a_is_enl,
            map_seed: cell.map_seed,
            game_seed: cell.game_seed,
            bot_seed: cell.bot_seed,
            schedule_seed: cell.schedule_seed,
            bucket_policy: cell.bucket_policy,
            config_hash: cell.config_hash,
            result: BalanceOutcome::Played(record.result),
            enl_score2: record.enl_score2,
            res_score2: record.res_score2,
            final_state_hash: record.final_state_hash,
            stats: record.stats.clone(),
            elapsed_ms: record.elapsed_ms,
        }
    }

    /// The config-failure record: identity echo plus a `ConfigFailure`
    /// outcome with empty stats (stays in the result set).
    pub fn config_failure(cell: &MatchCell, spec_a: &PolicySpec, spec_b: &PolicySpec) -> Self {
        BalanceMatchRecord {
            index: cell.index,
            experiment_id: cell.experiment_id.clone(),
            seed_set_tag: cell.seed_set_tag.clone(),
            variant_id: cell.variant_id.clone(),
            pair_tag: cell.pair_tag.clone(),
            pair_id: cell.pair_id.clone(),
            leg: cell.leg,
            map_id: cell.map_id.clone(),
            map_index: cell.map_index,
            policy_a: cell.policy_a,
            policy_a_family: spec_a.family,
            policy_a_version: spec_a.version.to_string(),
            policy_b: cell.policy_b,
            policy_b_family: spec_b.family,
            policy_b_version: spec_b.version.to_string(),
            policy_a_is_enl: cell.policy_a_is_enl,
            map_seed: cell.map_seed,
            game_seed: cell.game_seed,
            bot_seed: cell.bot_seed,
            schedule_seed: cell.schedule_seed,
            bucket_policy: cell.bucket_policy,
            config_hash: cell.config_hash,
            result: BalanceOutcome::ConfigFailure,
            enl_score2: 0,
            res_score2: 0,
            final_state_hash: [0u8; 32],
            stats: MatchStats::new(),
            elapsed_ms: 0,
        }
    }

    /// Equality ignoring the metadata-only elapsed time.
    pub fn same_ignoring_elapsed(&self, other: &BalanceMatchRecord) -> bool {
        let mut a = self.clone();
        let mut b = other.clone();
        a.elapsed_ms = 0;
        b.elapsed_ms = 0;
        a == b
    }
}

/// The fixed `matches.tsv` column order of the balance layer (identity
/// columns + the frozen m6 metric payload).
pub fn balance_columns() -> Vec<String> {
    let head = [
        "index",
        "experiment_id",
        "seed_set_tag",
        "variant_id",
        "pair_tag",
        "pair_id",
        "leg",
        "map_id",
        "map_index",
        "policy_a",
        "policy_a_family",
        "policy_a_version",
        "policy_b",
        "policy_b_family",
        "policy_b_version",
        "policy_a_is_enl",
        "map_seed",
        "game_seed",
        "bot_seed",
        "schedule_seed",
        "bucket_policy",
        "config_hash",
        "result",
        "enl_score2",
        "res_score2",
        "final_state_hash",
        "elapsed_ms",
    ];
    let mut cols: Vec<String> = head.iter().map(|s| s.to_string()).collect();
    for prefix in ["enl", "res"] {
        for field in crate::report::FACTION_STAT_FIELDS {
            cols.push(format!("{prefix}_{field}"));
        }
    }
    for round in crate::metrics::CHECKPOINTS {
        cols.push(format!("enl_score_r{round}"));
        cols.push(format!("res_score_r{round}"));
        cols.push(format!("leader_r{round}"));
    }
    cols
}

/// Serializes one record into the fixed column order.
pub fn balance_record_to_row(record: &BalanceMatchRecord) -> Vec<String> {
    let mut row: Vec<String> = vec![
        record.index.to_string(),
        record.experiment_id.clone(),
        record.seed_set_tag.clone(),
        record.variant_id.clone(),
        record.pair_tag.clone(),
        record.pair_id.clone(),
        record.leg.to_string(),
        record.map_id.clone(),
        record.map_index.to_string(),
        record.policy_a.tag().to_string(),
        record.policy_a_family.tag().to_string(),
        record.policy_a_version.clone(),
        record.policy_b.tag().to_string(),
        record.policy_b_family.tag().to_string(),
        record.policy_b_version.clone(),
        u8::from(record.policy_a_is_enl).to_string(),
        record.map_seed.to_string(),
        record.game_seed.to_string(),
        record.bot_seed.to_string(),
        record.schedule_seed.to_string(),
        record.bucket_policy.tag(),
        format!("{:016x}", record.config_hash),
        record.result.tag().to_string(),
        record.enl_score2.to_string(),
        record.res_score2.to_string(),
        arena_replay::canon::hex(&record.final_state_hash),
        record.elapsed_ms.to_string(),
    ];
    row.extend(faction_stats_row(&record.stats.enl));
    row.extend(faction_stats_row(&record.stats.res));
    row.extend(checkpoint_stat_rows(&record.stats));
    row
}

/// The whole balance `matches.tsv` document.
pub fn balance_records_to_tsv(records: &[BalanceMatchRecord]) -> String {
    let mut text = String::new();
    text.push_str(&balance_columns().join("\t"));
    text.push('\n');
    for record in records {
        text.push_str(&balance_record_to_row(record).join("\t"));
        text.push('\n');
    }
    text
}

/// Parses a balance `matches.tsv` document back (exact round-trip of
/// [`balance_records_to_tsv`]).
pub fn parse_balance_tsv(text: &str) -> Result<Vec<BalanceMatchRecord>, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty document")?.split('\t').collect();
    let expected = balance_columns();
    if header != expected.iter().map(|s| s.as_str()).collect::<Vec<_>>() {
        return Err("header mismatch".to_string());
    }
    let mut records = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let values: Vec<&str> = line.split('\t').collect();
        if values.len() != expected.len() {
            return Err(format!("row width {} != {}", values.len(), expected.len()));
        }
        let map: BTreeMap<&str, &str> = header
            .iter()
            .zip(values.iter())
            .map(|(k, v)| (*k, *v))
            .collect();
        records.push(parse_balance_record(&map)?);
    }
    Ok(records)
}

fn parse_balance_record(map: &BTreeMap<&str, &str>) -> Result<BalanceMatchRecord, String> {
    let policy = |name: &str| -> Result<PolicyId, String> {
        PolicyId::from_tag(map.get(name).copied().unwrap_or(""))
            .ok_or_else(|| format!("{name} tag"))
    };
    let family = |name: &str| -> Result<PolicyFamily, String> {
        PolicyFamily::from_tag(map.get(name).copied().unwrap_or(""))
            .ok_or_else(|| format!("{name} tag"))
    };
    Ok(BalanceMatchRecord {
        index: u64_of(map, "index")?,
        experiment_id: map.get("experiment_id").copied().unwrap_or("").to_string(),
        seed_set_tag: map.get("seed_set_tag").copied().unwrap_or("").to_string(),
        variant_id: map.get("variant_id").copied().unwrap_or("").to_string(),
        pair_tag: map.get("pair_tag").copied().unwrap_or("").to_string(),
        pair_id: map.get("pair_id").copied().unwrap_or("").to_string(),
        leg: u64_of(map, "leg")? as u8,
        map_id: map.get("map_id").copied().unwrap_or("").to_string(),
        map_index: u64_of(map, "map_index")? as u32,
        policy_a: policy("policy_a")?,
        policy_a_family: family("policy_a_family")?,
        policy_a_version: map
            .get("policy_a_version")
            .copied()
            .unwrap_or("")
            .to_string(),
        policy_b: policy("policy_b")?,
        policy_b_family: family("policy_b_family")?,
        policy_b_version: map
            .get("policy_b_version")
            .copied()
            .unwrap_or("")
            .to_string(),
        policy_a_is_enl: map.get("policy_a_is_enl").copied().unwrap_or("0") == "1",
        map_seed: u64_of(map, "map_seed")?,
        game_seed: u64_of(map, "game_seed")?,
        bot_seed: u64_of(map, "bot_seed")?,
        schedule_seed: u64_of(map, "schedule_seed")?,
        bucket_policy: BucketPolicy::from_tag(map.get("bucket_policy").copied().unwrap_or(""))
            .ok_or("bucket_policy tag")?,
        config_hash: u64::from_str_radix(map.get("config_hash").copied().unwrap_or("0"), 16)
            .map_err(|e| format!("config_hash: {e}"))?,
        result: BalanceOutcome::from_tag(map.get("result").copied().unwrap_or(""))
            .ok_or("result tag")?,
        enl_score2: u128f_of(map, "enl_score2")?,
        res_score2: u128f_of(map, "res_score2")?,
        final_state_hash: parse_state_hash_hex(map.get("final_state_hash").copied().unwrap_or(""))?,
        stats: MatchStats {
            enl: parse_faction_stats(map, "enl")?,
            res: parse_faction_stats(map, "res")?,
            checkpoints: parse_checkpoints(map)?,
        },
        elapsed_ms: u64_of(map, "elapsed_ms")?,
    })
}

//! Batch artifacts and aggregation (D57, P2.5-METRICS): the lossless
//! `matches.tsv` (one row per match; re-parsing reproduces the in-memory
//! `MatchRecord`s exactly), `manifest.txt` (key=value setup metadata), and
//! `summary.txt` (per-matchup aggregates). Hand-rolled std-only text with
//! integer arithmetic everywhere — means print as fixed-point with three
//! decimals, rates as permille-derived fractions.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use arena_bots::bot::BotKind;
use arena_core::types::{Faction, Score2};

use crate::batch::{BatchConfig, Matchup};
use crate::metrics::{CHECKPOINTS, FactionMatchStats, MatchRecord, MatchStats, Outcome};
use crate::policy::BucketPolicy;

// ---------------------------------------------------------------------
// TSV serialization
// ---------------------------------------------------------------------

/// The 23 per-faction metric field names in fixed column order (shared by
/// the m6 `matches.tsv` and the balance record payload — one source of
/// truth so the two formats can never drift).
pub const FACTION_STAT_FIELDS: [&str; 23] = [
    "commands_submitted",
    "commands_ok",
    "commands_dropped",
    "moves",
    "captures",
    "attacks",
    "attack_hits",
    "anchor_attack_hits",
    "recharges",
    "hacks",
    "keys_gained",
    "transfers",
    "links_created",
    "links_by_reinforcements",
    "first_field_round",
    "fields_created",
    "fields_destroyed",
    "field_area2_sum",
    "destroyed_lifetime_rounds_sum",
    "fields_surviving",
    "surviving_lifetime_rounds_sum",
    "layering_depth_max",
    "supply_remaining",
];

/// The fixed `matches.tsv` column order (frozen with the experiment).
pub fn columns() -> Vec<String> {
    let mut cols: Vec<String> = [
        "index",
        "matchup_index",
        "matchup_tag",
        "j",
        "bot_a",
        "bot_b",
        "bot_a_is_enl",
        "bot_a_version",
        "bot_b_version",
        "game_seed",
        "map_seed",
        "map_index",
        "bot_seed",
        "schedule_seed",
        "bucket_policy",
        "config_hash",
        "result",
        "enl_score2",
        "res_score2",
        "final_state_hash",
        "elapsed_ms",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for prefix in ["enl", "res"] {
        for field in FACTION_STAT_FIELDS {
            cols.push(format!("{prefix}_{field}"));
        }
    }
    for round in CHECKPOINTS {
        cols.push(format!("enl_score_r{round}"));
        cols.push(format!("res_score_r{round}"));
        cols.push(format!("leader_r{round}"));
    }
    cols
}

fn faction_row(stats: &FactionMatchStats) -> Vec<String> {
    faction_stats_row(stats)
}

/// Serializes one faction's counters into the fixed per-faction column
/// order (shared by the m6 `matches.tsv` and the balance record — the
/// balance layer reuses this helper so the metric payload bytes can never
/// drift between the two formats).
pub fn faction_stats_row(stats: &FactionMatchStats) -> Vec<String> {
    vec![
        stats.commands_submitted.to_string(),
        stats.commands_ok.to_string(),
        stats.commands_dropped.to_string(),
        stats.moves.to_string(),
        stats.captures.to_string(),
        stats.attacks.to_string(),
        stats.attack_hits.to_string(),
        stats.anchor_attack_hits.to_string(),
        stats.recharges.to_string(),
        stats.hacks.to_string(),
        stats.keys_gained.to_string(),
        stats.transfers.to_string(),
        stats.links_created.to_string(),
        stats.links_by_reinforcements.to_string(),
        stats
            .first_field_round
            .map(|r| r.to_string())
            .unwrap_or_else(|| "-".to_string()),
        stats.fields_created.to_string(),
        stats.fields_destroyed.to_string(),
        stats.field_area2_sum.to_string(),
        stats.destroyed_lifetime_rounds_sum.to_string(),
        stats.fields_surviving.to_string(),
        stats.surviving_lifetime_rounds_sum.to_string(),
        stats.layering_depth_max.to_string(),
        stats.supply_remaining.to_string(),
    ]
}

fn leader_tag(leader: Option<Faction>) -> &'static str {
    match leader {
        Some(Faction::Enl) => "enl",
        Some(Faction::Res) => "res",
        None => "tie",
    }
}

fn checkpoint_rows(stats: &MatchStats) -> Vec<String> {
    checkpoint_stat_rows(stats)
}

/// Serializes the checkpoint snapshots into the fixed column order (the
/// shared helper behind the frozen m6 rows).
pub fn checkpoint_stat_rows(stats: &MatchStats) -> Vec<String> {
    let mut out = Vec::with_capacity(12);
    for snapshot in &stats.checkpoints {
        out.push(snapshot.enl.to_string());
        out.push(snapshot.res.to_string());
        out.push(leader_tag(snapshot.leader).to_string());
    }
    out
}

/// Serializes one record into the fixed column order.
pub fn record_to_row(record: &MatchRecord) -> Vec<String> {
    let mut row: Vec<String> = vec![
        record.index.to_string(),
        record.matchup_index.to_string(),
        record.matchup_tag.clone(),
        record.j.to_string(),
        record.bot_a.tag().to_string(),
        record.bot_b.tag().to_string(),
        u8::from(record.bot_a_is_enl).to_string(),
        record.bot_a_version.to_string(),
        record.bot_b_version.to_string(),
        record.game_seed.to_string(),
        record.map_seed.to_string(),
        record.map_index.to_string(),
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
    row.extend(faction_row(&record.stats.enl));
    row.extend(faction_row(&record.stats.res));
    row.extend(checkpoint_rows(&record.stats));
    row
}

/// The whole `matches.tsv` document (header + one row per record).
pub fn records_to_tsv(records: &[MatchRecord]) -> String {
    let mut text = String::new();
    text.push_str(&columns().join("\t"));
    text.push('\n');
    for record in records {
        text.push_str(&record_to_row(record).join("\t"));
        text.push('\n');
    }
    text
}

/// Parses a `matches.tsv` document back into records (exact round-trip of
/// [`records_to_tsv`]).
pub fn parse_matches_tsv(text: &str) -> Result<Vec<MatchRecord>, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty document")?.split('\t').collect();
    let expected = columns();
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
        records.push(parse_record(&map)?);
    }
    Ok(records)
}

/// Parses one faction's counters back (the inverse of
/// [`faction_stats_row`]); shared by the m6 and balance record parsers.
pub fn parse_faction_stats(
    map: &BTreeMap<&str, &str>,
    prefix: &str,
) -> Result<FactionMatchStats, String> {
    let mut stats = FactionMatchStats::default();
    let u64s = |name: &str, slot: &mut u64| -> Result<(), String> {
        *slot = map
            .get(format!("{prefix}_{name}").as_str())
            .copied()
            .unwrap_or("")
            .parse()
            .map_err(|e| format!("{prefix}_{name}: {e}"))?;
        Ok(())
    };
    for (name, slot) in [
        ("commands_submitted", &mut stats.commands_submitted),
        ("commands_ok", &mut stats.commands_ok),
        ("commands_dropped", &mut stats.commands_dropped),
        ("moves", &mut stats.moves),
        ("captures", &mut stats.captures),
        ("attacks", &mut stats.attacks),
        ("attack_hits", &mut stats.attack_hits),
        ("anchor_attack_hits", &mut stats.anchor_attack_hits),
        ("recharges", &mut stats.recharges),
        ("hacks", &mut stats.hacks),
        ("keys_gained", &mut stats.keys_gained),
        ("transfers", &mut stats.transfers),
        ("links_created", &mut stats.links_created),
        (
            "links_by_reinforcements",
            &mut stats.links_by_reinforcements,
        ),
        ("fields_created", &mut stats.fields_created),
        ("fields_destroyed", &mut stats.fields_destroyed),
        (
            "destroyed_lifetime_rounds_sum",
            &mut stats.destroyed_lifetime_rounds_sum,
        ),
        ("fields_surviving", &mut stats.fields_surviving),
        (
            "surviving_lifetime_rounds_sum",
            &mut stats.surviving_lifetime_rounds_sum,
        ),
        ("supply_remaining", &mut stats.supply_remaining),
    ] {
        u64s(name, slot)?;
    }
    stats.field_area2_sum = map
        .get(format!("{prefix}_field_area2_sum").as_str())
        .copied()
        .unwrap_or("")
        .parse()
        .map_err(|e| format!("{prefix}_field_area2_sum: {e}"))?;
    stats.layering_depth_max = map
        .get(format!("{prefix}_layering_depth_max").as_str())
        .copied()
        .unwrap_or("")
        .parse()
        .map_err(|e| format!("{prefix}_layering_depth_max: {e}"))?;
    stats.first_field_round = match map
        .get(format!("{prefix}_first_field_round").as_str())
        .copied()
        .unwrap_or("")
    {
        "-" => None,
        value => Some(
            value
                .parse()
                .map_err(|e| format!("{prefix}_first_field_round: {e}"))?,
        ),
    };
    Ok(stats)
}

/// Parses the checkpoint columns back (the inverse of
/// [`checkpoint_stat_rows`]).
pub fn parse_checkpoints(
    map: &BTreeMap<&str, &str>,
) -> Result<[crate::metrics::ScoreSnapshot; 4], String> {
    let mut checkpoints = [crate::metrics::ScoreSnapshot {
        round: 0,
        enl: 0,
        res: 0,
        leader: None,
    }; 4];
    for (index, round) in CHECKPOINTS.iter().enumerate() {
        let enl: Score2 = map
            .get(format!("enl_score_r{round}").as_str())
            .copied()
            .unwrap_or("0")
            .parse()
            .map_err(|e| format!("enl_score_r{round}: {e}"))?;
        let res: Score2 = map
            .get(format!("res_score_r{round}").as_str())
            .copied()
            .unwrap_or("0")
            .parse()
            .map_err(|e| format!("res_score_r{round}: {e}"))?;
        let leader = match map
            .get(format!("leader_r{round}").as_str())
            .copied()
            .unwrap_or("tie")
        {
            "enl" => Some(Faction::Enl),
            "res" => Some(Faction::Res),
            _ => None,
        };
        checkpoints[index] = crate::metrics::ScoreSnapshot {
            round: *round,
            enl,
            res,
            leader,
        };
    }
    Ok(checkpoints)
}

/// Parses a lowercase hex State Hash string back into its 32 bytes
/// (`arena_replay::canon::hex` inverse; shared by the m6 and balance
/// record parsers).
pub fn parse_state_hash_hex(hash_hex: &str) -> Result<[u8; 32], String> {
    let mut hash_bytes = [0u8; 32];
    for (index, pair) in hash_hex.as_bytes().chunks(2).enumerate() {
        if pair.len() != 2 {
            return Err("final_state_hash: odd length".to_string());
        }
        let high = (pair[0] as char).to_digit(16).ok_or("hash hex")?;
        let low = (pair[1] as char).to_digit(16).ok_or("hash hex")?;
        hash_bytes[index] = (high * 16 + low) as u8;
    }
    Ok(hash_bytes)
}

fn parse_record(map: &BTreeMap<&str, &str>) -> Result<MatchRecord, String> {
    let faction_stats = |prefix: &str| parse_faction_stats(map, prefix);
    let checkpoints = parse_checkpoints(map)?;
    let hash_bytes = parse_state_hash_hex(map.get("final_state_hash").copied().unwrap_or(""))?;

    Ok(MatchRecord {
        index: map
            .get("index")
            .copied()
            .unwrap_or("0")
            .parse()
            .map_err(|e| format!("index: {e}"))?,
        matchup_index: map
            .get("matchup_index")
            .copied()
            .unwrap_or("0")
            .parse()
            .map_err(|e| format!("matchup_index: {e}"))?,
        matchup_tag: map.get("matchup_tag").copied().unwrap_or("").to_string(),
        j: map
            .get("j")
            .copied()
            .unwrap_or("0")
            .parse()
            .map_err(|e| format!("j: {e}"))?,
        bot_a: BotKind::from_tag(map.get("bot_a").copied().unwrap_or("")).ok_or("bot_a tag")?,
        bot_b: BotKind::from_tag(map.get("bot_b").copied().unwrap_or("")).ok_or("bot_b tag")?,
        bot_a_is_enl: map.get("bot_a_is_enl").copied().unwrap_or("0") == "1",
        bot_a_version: map.get("bot_a_version").copied().unwrap_or("").to_string(),
        bot_b_version: map.get("bot_b_version").copied().unwrap_or("").to_string(),
        game_seed: u64_of(map, "game_seed")?,
        map_seed: u64_of(map, "map_seed")?,
        map_index: map
            .get("map_index")
            .copied()
            .unwrap_or("0")
            .parse()
            .map_err(|e| format!("map_index: {e}"))?,
        bot_seed: u64_of(map, "bot_seed")?,
        schedule_seed: u64_of(map, "schedule_seed")?,
        bucket_policy: BucketPolicy::from_tag(map.get("bucket_policy").copied().unwrap_or(""))
            .ok_or("bucket_policy tag")?,
        config_hash: u64::from_str_radix(map.get("config_hash").copied().unwrap_or("0"), 16)
            .map_err(|e| format!("config_hash: {e}"))?,
        result: Outcome::from_tag(map.get("result").copied().unwrap_or("")).ok_or("result tag")?,
        enl_score2: u128f_of(map, "enl_score2")?,
        res_score2: u128f_of(map, "res_score2")?,
        final_state_hash: hash_bytes,
        stats: MatchStats {
            enl: faction_stats("enl")?,
            res: faction_stats("res")?,
            checkpoints,
        },
        elapsed_ms: u64_of(map, "elapsed_ms")?,
    })
}

pub fn u64_of(map: &BTreeMap<&str, &str>, name: &str) -> Result<u64, String> {
    map.get(name)
        .copied()
        .unwrap_or("0")
        .parse()
        .map_err(|e| format!("{name}: {e}"))
}

pub fn u128f_of(map: &BTreeMap<&str, &str>, name: &str) -> Result<u128, String> {
    map.get(name)
        .copied()
        .unwrap_or("0")
        .parse()
        .map_err(|e| format!("{name}: {e}"))
}

// ---------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------

/// Snowball evidence at one checkpoint: matches with a decided leader and
/// how often that leader won (ties at the checkpoint excluded, their rate
/// reported via `ties`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SnowballCheckpoint {
    pub round: u32,
    pub decided: u64,
    pub leader_won: u64,
    pub ties: u64,
}

impl SnowballCheckpoint {
    /// `P(win | leading)` in permille (0 when nothing was decided).
    pub fn win_rate_permille(&self) -> u64 {
        if self.decided == 0 {
            return 0;
        }
        self.leader_won * 1000 / self.decided
    }
}

/// Sums (and maxima) of one faction's metrics over a matchup.
#[derive(Clone, Default, Debug)]
pub struct FactionAggregates {
    pub commands_submitted: u64,
    pub commands_dropped: u64,
    pub captures: u64,
    pub attacks: u64,
    pub attack_hits: u64,
    pub anchor_attack_hits: u64,
    pub recharges: u64,
    pub hacks: u64,
    pub keys_gained: u64,
    pub links_created: u64,
    pub links_by_reinforcements: u64,
    pub fields_created: u64,
    pub fields_destroyed: u64,
    pub field_area2_sum: u128,
    pub destroyed_lifetime_rounds_sum: u64,
    pub fields_surviving: u64,
    pub surviving_lifetime_rounds_sum: u64,
    pub layering_depth_sum: u64,
    pub layering_depth_max: u32,
    pub supply_remaining: u64,
    pub first_field_round_sum: u64,
    pub first_field_matches: u64,
    pub score2_sum: Score2,
}

impl FactionAggregates {
    fn add(&mut self, stats: &FactionMatchStats, score: Score2) {
        self.commands_submitted += stats.commands_submitted;
        self.commands_dropped += stats.commands_dropped;
        self.captures += stats.captures;
        self.attacks += stats.attacks;
        self.attack_hits += stats.attack_hits;
        self.anchor_attack_hits += stats.anchor_attack_hits;
        self.recharges += stats.recharges;
        self.hacks += stats.hacks;
        self.keys_gained += stats.keys_gained;
        self.links_created += stats.links_created;
        self.links_by_reinforcements += stats.links_by_reinforcements;
        self.fields_created += stats.fields_created;
        self.fields_destroyed += stats.fields_destroyed;
        self.field_area2_sum += stats.field_area2_sum;
        self.destroyed_lifetime_rounds_sum += stats.destroyed_lifetime_rounds_sum;
        self.fields_surviving += stats.fields_surviving;
        self.surviving_lifetime_rounds_sum += stats.surviving_lifetime_rounds_sum;
        self.layering_depth_sum += stats.layering_depth_max as u64;
        self.layering_depth_max = self.layering_depth_max.max(stats.layering_depth_max);
        self.supply_remaining += stats.supply_remaining;
        if let Some(round) = stats.first_field_round {
            self.first_field_round_sum += round as u64;
            self.first_field_matches += 1;
        }
        self.score2_sum += score;
    }

    /// Mean first-field round in milli-rounds (exact; 0 when no fields).
    pub fn mean_first_field_round_milli(&self) -> u64 {
        if self.first_field_matches == 0 {
            return 0;
        }
        self.first_field_round_sum * 1000 / self.first_field_matches
    }
}

/// One matchup row of the summary.
#[derive(Clone, Default, Debug)]
pub struct MatchupSummary {
    pub tag: String,
    pub bot_a: Option<BotKind>,
    pub bot_b: Option<BotKind>,
    pub count: u64,
    pub bot_a_wins: u64,
    pub bot_b_wins: u64,
    pub draws: u64,
    pub crashes: u64,
    pub enl_wins: u64,
    pub res_wins: u64,
    pub enl: FactionAggregates,
    pub res: FactionAggregates,
    pub snowball: [SnowballCheckpoint; 4],
}

impl MatchupSummary {
    /// Bot A's win rate in permille over decided (non-crash) matches.
    pub fn bot_a_win_rate_permille(&self) -> u64 {
        let played = self.count - self.crashes;
        if played == 0 {
            return 0;
        }
        self.bot_a_wins * 1000 / played
    }

    pub fn enl_win_rate_permille(&self) -> u64 {
        let played = self.count - self.crashes;
        if played == 0 {
            return 0;
        }
        self.enl_wins * 1000 / played
    }

    pub fn res_win_rate_permille(&self) -> u64 {
        let played = self.count - self.crashes;
        if played == 0 {
            return 0;
        }
        self.res_wins * 1000 / played
    }
}

/// Whole-batch aggregates (per matchup plus the pooled faction split).
#[derive(Clone, Default, Debug)]
pub struct BatchSummary {
    pub matchups: Vec<MatchupSummary>,
    pub total: u64,
    pub total_enl_wins: u64,
    pub total_res_wins: u64,
    pub total_draws: u64,
    pub total_crashes: u64,
}

impl BatchSummary {
    /// Aggregates records (in descriptor order) grouped by matchup.
    pub fn from_records(records: &[MatchRecord], matchups: &[Matchup]) -> Self {
        let mut summaries = Vec::with_capacity(matchups.len());
        for matchup in matchups {
            summaries.push(MatchupSummary {
                tag: matchup.tag.clone(),
                bot_a: Some(matchup.a),
                bot_b: Some(matchup.b),
                snowball: CHECKPOINTS.map(|round| SnowballCheckpoint {
                    round,
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        let mut summary = BatchSummary {
            matchups: summaries,
            ..Default::default()
        };
        for record in records {
            let row = summary
                .matchups
                .get_mut(record.matchup_index as usize)
                .expect("matchup_index in range");
            row.count += 1;
            summary.total += 1;
            match record.result {
                Outcome::Draw => {
                    row.draws += 1;
                    summary.total_draws += 1;
                }
                Outcome::Crash => {
                    row.crashes += 1;
                    summary.total_crashes += 1;
                }
                Outcome::EnlWin => {
                    row.enl_wins += 1;
                    summary.total_enl_wins += 1;
                    if record.bot_a_is_enl {
                        row.bot_a_wins += 1;
                    } else {
                        row.bot_b_wins += 1;
                    }
                }
                Outcome::ResWin => {
                    row.res_wins += 1;
                    summary.total_res_wins += 1;
                    if record.bot_a_is_enl {
                        row.bot_b_wins += 1;
                    } else {
                        row.bot_a_wins += 1;
                    }
                }
            }
            row.enl.add(&record.stats.enl, record.enl_score2);
            row.res.add(&record.stats.res, record.res_score2);
            // Snowball (crashes excluded; draws at the end count as
            // not-won).
            for (index, snapshot) in record.stats.checkpoints.iter().enumerate() {
                let checkpoint = &mut row.snowball[index];
                match snapshot.leader {
                    None => checkpoint.ties += 1,
                    Some(leader) => {
                        checkpoint.decided += 1;
                        let won = match record.result {
                            Outcome::EnlWin => leader == Faction::Enl,
                            Outcome::ResWin => leader == Faction::Res,
                            _ => false,
                        };
                        if won {
                            checkpoint.leader_won += 1;
                        }
                    }
                }
            }
        }
        summary
    }
}

// ---------------------------------------------------------------------
// Text formatting helpers (integer fixed-point only)
// ---------------------------------------------------------------------

/// `part / whole` as a fraction string with three decimals.
pub fn fraction_str(part: u64, whole: u64) -> String {
    if whole == 0 {
        return "-".to_string();
    }
    let milli = part * 1000 / whole;
    format!("{}.{:03}", milli / 1000, milli % 1000)
}

/// Mean of a u128 sum over `count` as a fixed-point string.
pub fn mean_u128_str(sum: u128, count: u64) -> String {
    if count == 0 {
        return "-".to_string();
    }
    let milli = sum * 1000 / count as u128;
    format!("{}.{:03}", milli / 1000, milli % 1000)
}

/// Mean of a u64 sum over `count` as a fixed-point string.
pub fn mean_u64_str(sum: u64, count: u64) -> String {
    if count == 0 {
        return "-".to_string();
    }
    let milli = (sum as u128) * 1000 / count as u128;
    format!("{}.{:03}", milli / 1000, milli % 1000)
}

/// Permille value as a fraction string ("0.512").
pub fn permille_str(permille: u64) -> String {
    format!("{}.{:03}", permille / 1000, permille % 1000)
}

// ---------------------------------------------------------------------
// Artifact writers
// ---------------------------------------------------------------------

/// The `manifest.txt` body: setup + run metadata as key=value lines.
pub fn manifest_text(config: &BatchConfig, workers: usize, elapsed_ms: u128) -> String {
    let mut text = String::new();
    let line = |text: &mut String, key: &str, value: String| {
        text.push_str(key);
        text.push('=');
        text.push_str(&value);
        text.push('\n');
    };
    line(&mut text, "batch_id", config.batch_id.clone());
    line(
        &mut text,
        "master_seed",
        format!("{:#018x}", config.master_seed),
    );
    line(
        &mut text,
        "schedule_seed",
        format!("{:#018x}", config.schedule_seed()),
    );
    line(&mut text, "map_count", config.map_count.to_string());
    line(
        &mut text,
        "matchups",
        config
            .matchups
            .iter()
            .map(|m| format!("{}={}x{}", m.tag, m.a.tag(), m.count))
            .collect::<Vec<_>>()
            .join(","),
    );
    line(
        &mut text,
        "total_matches",
        config.total_matches().to_string(),
    );
    line(&mut text, "bucket_policy", config.bucket_policy.tag());
    line(
        &mut text,
        "config_hash",
        format!("{:#018x}", config.config_hash()),
    );
    line(
        &mut text,
        "total_rounds",
        config.config.total_rounds.to_string(),
    );
    line(
        &mut text,
        "map_seed_derivation",
        format!("LE-u64(blake3({:?} || k_le32))", "ingress-arena-m6-map"),
    );
    line(
        &mut text,
        "game_seed_derivation",
        format!(
            "LE-u64(blake3({:?} || tag || j_le32))",
            "ingress-arena-m6-game"
        ),
    );
    line(
        &mut text,
        "bot_seed_derivation",
        format!(
            "LE-u64(blake3({:?} || tag || j_le32))",
            "ingress-arena-m6-bot"
        ),
    );
    line(
        &mut text,
        "schedule_derivation",
        format!(
            "LE-u64(blake3({:?} || master_seed_le64)); want-bit = blake3(b\"m6-sched\" || sched || tag || j)[0] & 1",
            "ingress-arena-m6-sched"
        ),
    );
    line(
        &mut text,
        "bot_version",
        crate::metrics::DEFAULT_BOT_VERSION.to_string(),
    );
    line(&mut text, "workers", workers.to_string());
    line(&mut text, "elapsed_ms", elapsed_ms.to_string());
    text
}

/// The `summary.txt` body: per-matchup aggregate tables.
pub fn summary_text(summary: &BatchSummary, portal_count: u64) -> String {
    let mut out = String::new();
    out.push_str("# arena-runner batch summary\n\n");
    out.push_str(&format!(
        "total={} enl_wins={} res_wins={} draws={} crashes={}\n\n",
        summary.total,
        summary.total_enl_wins,
        summary.total_res_wins,
        summary.total_draws,
        summary.total_crashes
    ));
    for row in &summary.matchups {
        let count = row.count;
        let played = count - row.crashes;
        out.push_str(&format!(
            "## {} ({} vs {}, {} matches)\n",
            row.tag,
            row.bot_a.map(|k| k.tag()).unwrap_or("?"),
            row.bot_b.map(|k| k.tag()).unwrap_or("?"),
            count
        ));
        out.push_str(&format!(
            "  bot_a_wins={} bot_b_wins={} draws={} crashes={}\n",
            row.bot_a_wins, row.bot_b_wins, row.draws, row.crashes
        ));
        out.push_str(&format!(
            "  bot_a_win_rate={} enl_win_rate={} res_win_rate={} (over {} played)\n",
            permille_str(row.bot_a_win_rate_permille()),
            permille_str(row.enl_win_rate_permille()),
            permille_str(row.res_win_rate_permille()),
            played
        ));
        for (label, agg) in [("enl", &row.enl), ("res", &row.res)] {
            out.push_str(&format!(
                "  {label}: mean_score2={} mean_first_field_round={} fields_created={} fields_destroyed={} turnover={} layering_mean={} layering_max={} anchor_share={} key_util={} reinforcement_share={} hacks={} attacks={} recharges={} captures={} supply_remaining={} drop_rate={}\n",
                mean_u128_str(agg.score2_sum, played),
                mean_u64_str(agg.first_field_round_sum, agg.first_field_matches),
                agg.fields_created,
                agg.fields_destroyed,
                // R-BALANCE-02 turnover is a PER-MATCH rate: the matchup-wide
                // capture sum over (played matches x portals in one map).
                // Without the `played` factor the value would scale linearly
                // with the matchup's match count and be incomparable across
                // matchups (crashed matches fold zero stats, so `played`
                // matches the denominator of every other mean in this line).
                fraction_str(agg.captures, played.saturating_mul(portal_count)),
                fraction_str(agg.layering_depth_sum, played),
                agg.layering_depth_max,
                fraction_str(agg.anchor_attack_hits, agg.attack_hits),
                fraction_str(agg.links_created, agg.keys_gained),
                fraction_str(agg.links_by_reinforcements, agg.links_created),
                agg.hacks,
                agg.attacks,
                agg.recharges,
                agg.captures,
                mean_u64_str(agg.supply_remaining, played),
                fraction_str(agg.commands_dropped, agg.commands_submitted),
            ));
        }
        let lifetimes =
            row.enl.destroyed_lifetime_rounds_sum + row.res.destroyed_lifetime_rounds_sum;
        let destroyed = row.enl.fields_destroyed + row.res.fields_destroyed;
        let survivors = row.enl.fields_surviving + row.res.fields_surviving;
        let survivor_lifetime =
            row.enl.surviving_lifetime_rounds_sum + row.res.surviving_lifetime_rounds_sum;
        out.push_str(&format!(
            "  fields: mean_area2={} mean_lifetime_destroyed={} mean_lifetime_survivors={} mean_lifetime_all={}\n",
            mean_u128_str(
                row.enl.field_area2_sum + row.res.field_area2_sum,
                row.enl.fields_created + row.res.fields_created
            ),
            mean_u64_str(lifetimes, destroyed),
            mean_u64_str(survivor_lifetime, survivors),
            mean_u64_str(lifetimes + survivor_lifetime, destroyed + survivors),
        ));
        for checkpoint in &row.snowball {
            out.push_str(&format!(
                "  snowball_r{}: P(win|leading)={} decided={} ties={}\n",
                checkpoint.round,
                permille_str(checkpoint.win_rate_permille()),
                checkpoint.decided,
                checkpoint.ties
            ));
        }
        out.push('\n');
    }
    out
}

/// Writes `manifest.txt`, `matches.tsv`, and `summary.txt` into `dir`
/// (created on demand).
pub fn write_artifacts(
    dir: &Path,
    config: &BatchConfig,
    records: &[MatchRecord],
    workers: usize,
    elapsed_ms: u128,
) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(
        dir.join("manifest.txt"),
        manifest_text(config, workers, elapsed_ms),
    )?;
    fs::write(dir.join("matches.tsv"), records_to_tsv(records))?;
    let summary = BatchSummary::from_records(records, &config.matchups);
    fs::write(
        dir.join("summary.txt"),
        summary_text(&summary, config.config.portal_count as u64),
    )?;
    Ok(())
}

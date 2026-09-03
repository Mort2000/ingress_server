//! Drop-reason tally over a FROZEN batch archive (B0 evidence pass,
//! `docs/ai-balance-plan.md` principle 8 — raw data first).
//!
//! `docs/balance-report-r1.md` quotes a per-reason breakdown of the
//! dropped commands in the Greedy x DestroyAnchor matchup. The frozen
//! archive (`runs/m6-r1/matches.tsv`) stores per-faction drop TOTALS only,
//! so the breakdown is re-derived here: every archived match is
//! deterministically re-executed from its own per-match record (game seed,
//! bot seed, map seed, bucket policy and both BotKinds all read back from
//! the archive) and the `DropReason` of every `CommandOutcome::Drop` is
//! tallied per BotKind.
//!
//! Faithfulness is verified, not assumed: each re-execution's final State
//! Hash and per-faction submitted/ok/dropped counts are compared against
//! the archived row, and ANY mismatch aborts the whole tally without
//! writing artifacts. This module is offline analysis only (plan section
//! 9): integer counts everywhere, permille derived values printed next to
//! — never instead of — the raw counts, and nothing here feeds back into
//! any match, Game Result, Replay Hash or rule settlement. Frozen archives
//! are read strictly read-only; new artifacts land only in a NEW
//! `runs/balance/<experiment-id>/` directory (never over a historical
//! run).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use arena_bots::bot::{BotKind, BotStrategy, make_bot};
use arena_core::config::GameConfig;
use arena_core::harness::MatchHarness;
use arena_core::map::{MapDefinition, generate_map};
use arena_core::resolve::{CommandOutcome, DropReason};
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::Faction;

use crate::metrics::{MatchRecord, Outcome};
use crate::policy::PolicyStrategy;
use crate::report::parse_matches_tsv;
use crate::run::MAX_WORKERS;

/// Every `DropReason` variant as its exact Rust identifier, in enum
/// declaration order — the canonical row order of the tally artifacts (the
/// identifiers `docs/balance-report-r1.md` cites are these tags).
pub const DROP_REASON_TAGS: [&str; 24] = [
    "MoveOutOfBounds",
    "MoveBudgetExceeded",
    "PortalNotFound",
    "OutOfOperationRange",
    "CaptureTargetNotNeutral",
    "AttackNoEnemyPortalsInRange",
    "RechargePortalNotOwned",
    "RechargePositionNotSatisfied",
    "RechargeInsufficientSupply",
    "HackCooldownActive",
    "HackEpochLimitReached",
    "TransferTargetNotFound",
    "TransferTargetWrongFaction",
    "TransferOutOfRange",
    "TransferInsufficientSupply",
    "TransferInsufficientKeys",
    "TransferExceedsReceiverSupply",
    "LinkSelfLoop",
    "LinkPortalNotOwned",
    "LinkMissingTargetKey",
    "LinkDistanceExceeded",
    "LinkAlreadyExists",
    "LinkBlockedByPortal",
    "LinkIntersectsLink",
];

/// The `DROP_REASON_TAGS` entry of `reason` (payload fields are ignored —
/// the tally counts variants, not arguments).
pub fn drop_reason_tag(reason: &DropReason) -> &'static str {
    match reason {
        DropReason::MoveOutOfBounds { .. } => "MoveOutOfBounds",
        DropReason::MoveBudgetExceeded { .. } => "MoveBudgetExceeded",
        DropReason::PortalNotFound { .. } => "PortalNotFound",
        DropReason::OutOfOperationRange { .. } => "OutOfOperationRange",
        DropReason::CaptureTargetNotNeutral { .. } => "CaptureTargetNotNeutral",
        DropReason::AttackNoEnemyPortalsInRange => "AttackNoEnemyPortalsInRange",
        DropReason::RechargePortalNotOwned { .. } => "RechargePortalNotOwned",
        DropReason::RechargePositionNotSatisfied { .. } => "RechargePositionNotSatisfied",
        DropReason::RechargeInsufficientSupply { .. } => "RechargeInsufficientSupply",
        DropReason::HackCooldownActive { .. } => "HackCooldownActive",
        DropReason::HackEpochLimitReached { .. } => "HackEpochLimitReached",
        DropReason::TransferTargetNotFound { .. } => "TransferTargetNotFound",
        DropReason::TransferTargetWrongFaction { .. } => "TransferTargetWrongFaction",
        DropReason::TransferOutOfRange { .. } => "TransferOutOfRange",
        DropReason::TransferInsufficientSupply { .. } => "TransferInsufficientSupply",
        DropReason::TransferInsufficientKeys { .. } => "TransferInsufficientKeys",
        DropReason::TransferExceedsReceiverSupply { .. } => "TransferExceedsReceiverSupply",
        DropReason::LinkSelfLoop { .. } => "LinkSelfLoop",
        DropReason::LinkPortalNotOwned { .. } => "LinkPortalNotOwned",
        DropReason::LinkMissingTargetKey { .. } => "LinkMissingTargetKey",
        DropReason::LinkDistanceExceeded { .. } => "LinkDistanceExceeded",
        DropReason::LinkAlreadyExists { .. } => "LinkAlreadyExists",
        DropReason::LinkBlockedByPortal { .. } => "LinkBlockedByPortal",
        DropReason::LinkIntersectsLink { .. } => "LinkIntersectsLink",
    }
}

/// One faction's recomputed command outcome counts inside one match.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct FactionDrops {
    pub submitted: u64,
    pub ok: u64,
    pub dropped: u64,
    /// Variant tag -> count (BTreeMap: deterministic regardless of
    /// insertion order).
    pub reasons: BTreeMap<&'static str, u64>,
}

impl FactionDrops {
    fn observe(&mut self, outcome: &CommandOutcome) {
        self.submitted += 1;
        match outcome {
            CommandOutcome::Ok(_) => self.ok += 1,
            CommandOutcome::Drop(reason) => {
                self.dropped += 1;
                *self.reasons.entry(drop_reason_tag(reason)).or_insert(0) += 1;
            }
        }
    }
}

/// The recomputed drop tally of one archived match (descriptor echo plus
/// both factions' counts and the final State Hash for verification).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchDrops {
    pub index: u64,
    pub matchup_tag: String,
    pub bot_a: BotKind,
    pub bot_b: BotKind,
    pub bot_a_is_enl: bool,
    pub enl: FactionDrops,
    pub res: FactionDrops,
    pub final_state_hash: [u8; 32],
}

/// Deterministically re-executes the match described by `record` (fresh
/// m6.0 bots, per-record seeds, same construction order as
/// [`crate::run::run_match`]) and folds every executed command outcome.
/// Crash records cannot be re-verified (the original panic is not
/// archived) and are refused here.
pub fn reexecute_archived_match(
    record: &MatchRecord,
    cfg: &GameConfig,
    map: &MapDefinition,
) -> Result<MatchDrops, String> {
    if record.result == Outcome::Crash {
        return Err(format!(
            "match {}: archived Crash outcome — the panic is not re-executable",
            record.index
        ));
    }
    if crate::tournament::config::config_identity(cfg) != record.config_hash {
        return Err(format!(
            "match {}: config identity mismatch — the archive was not produced with this GameConfig",
            record.index
        ));
    }
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Same construction order as run::run_match: one bot per faction
        // from the shared bot_seed, ENL spawns chosen first.
        let (enl_kind, res_kind) = if record.bot_a_is_enl {
            (record.bot_a, record.bot_b)
        } else {
            (record.bot_b, record.bot_a)
        };
        let mut enl_bot = make_bot(enl_kind, Faction::Enl, record.bot_seed, cfg, map);
        let mut res_bot = make_bot(res_kind, Faction::Res, record.bot_seed, cfg, map);
        let (enl_roster, res_roster) = GameState::initial_roster(cfg);
        let enl_spawns = validate_initial_spawn_set(
            &enl_roster,
            &enl_bot.choose_spawns(cfg, map, &enl_roster),
            cfg,
        )
        .map_err(|e| format!("match {}: ENL spawn set: {e:?}", record.index))?;
        let res_spawns = validate_initial_spawn_set(
            &res_roster,
            &res_bot.choose_spawns(cfg, map, &res_roster),
            cfg,
        )
        .map_err(|e| format!("match {}: RES spawn set: {e:?}", record.index))?;
        let mut harness = MatchHarness::new(
            record.game_seed,
            *cfg,
            map,
            &enl_spawns,
            &res_spawns,
            Box::new(PolicyStrategy::new(
                Box::new(BotStrategy(enl_bot)),
                Faction::Enl,
                record.bucket_policy,
            )),
            Box::new(PolicyStrategy::new(
                Box::new(BotStrategy(res_bot)),
                Faction::Res,
                record.bucket_policy,
            )),
        )
        .map_err(|e| format!("match {}: harness: {e:?}", record.index))?;
        let mut drops = MatchDrops {
            index: record.index,
            matchup_tag: record.matchup_tag.clone(),
            bot_a: record.bot_a,
            bot_b: record.bot_b,
            bot_a_is_enl: record.bot_a_is_enl,
            enl: FactionDrops::default(),
            res: FactionDrops::default(),
            final_state_hash: [0u8; 32],
        };
        for _ in 0..cfg.total_rounds {
            let report = harness.run_round();
            for stream in &report.execution_order {
                let faction_drops = match stream.faction {
                    Faction::Enl => &mut drops.enl,
                    Faction::Res => &mut drops.res,
                };
                for outcome in &stream.outcomes {
                    faction_drops.observe(outcome);
                }
            }
        }
        drops.final_state_hash = arena_replay::canon::state_hash(harness.state());
        Ok(drops)
    }));
    match outcome {
        Ok(Ok(drops)) => Ok(drops),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(format!(
            "match {}: re-execution panicked (archived result was {:?})",
            record.index, record.result
        )),
    }
}

/// Compares a re-execution against its archived row: identical final
/// State Hash and identical per-faction submitted/ok/dropped counts. The
/// per-reason tally is only trusted once this passes.
pub fn verify_match_drops(record: &MatchRecord, drops: &MatchDrops) -> Result<(), String> {
    if drops.final_state_hash != record.final_state_hash {
        return Err(format!(
            "match {}: final State Hash mismatch (re-executed {}, archived {})",
            record.index,
            arena_replay::canon::hex(&drops.final_state_hash),
            arena_replay::canon::hex(&record.final_state_hash)
        ));
    }
    for (label, got, want) in [
        ("enl", &drops.enl, &record.stats.enl),
        ("res", &drops.res, &record.stats.res),
    ] {
        let triple = (
            got.submitted,
            got.ok,
            got.dropped,
            want.commands_submitted,
            want.commands_ok,
            want.commands_dropped,
        );
        if (triple.0, triple.1, triple.2) != (triple.3, triple.4, triple.5) {
            return Err(format!(
                "match {}: {label} command counts mismatch (re-executed submitted/ok/dropped {}/{}/{}, archived {}/{}/{})",
                record.index, triple.0, triple.1, triple.2, triple.3, triple.4, triple.5
            ));
        }
    }
    Ok(())
}

/// One BotKind's matchup-wide tally (mirror slots of the same kind pool —
/// the summary line says so explicitly).
#[derive(Clone, Debug)]
pub struct BotDropSummary {
    pub bot: BotKind,
    /// Matches this kind participated in (a mirror match counts once).
    pub matches: u64,
    /// Archived per-faction `commands_submitted` sum (matches.tsv).
    pub archived_submitted: u64,
    /// Archived per-faction `commands_dropped` sum (matches.tsv).
    pub archived_dropped: u64,
    /// Re-executed variant tag -> count.
    pub reasons: BTreeMap<&'static str, u64>,
}

impl BotDropSummary {
    /// Sum of the re-executed per-reason counts.
    pub fn reason_sum(&self) -> u64 {
        self.reasons.values().sum()
    }
}

/// One matchup's pooled tally.
#[derive(Clone, Debug)]
pub struct MatchupDropSummary {
    pub tag: String,
    pub matches: u64,
    /// Archived Crash records skipped (not re-executable).
    pub crashes_skipped: u64,
    /// Distinct BotKinds in first-appearance (record) order.
    pub bots: Vec<BotDropSummary>,
}

/// Aggregates verified per-match tallies into per-matchup, per-BotKind
/// summaries, in record order (deterministic regardless of worker count).
/// Archived totals come from `records` (the matches.tsv parse), per-reason
/// counts from the re-executions; `verify_match_drops` has already forced
/// them to agree per match.
pub fn aggregate_drop_reasons(
    records: &[MatchRecord],
    drops: &[Option<MatchDrops>],
) -> Result<Vec<MatchupDropSummary>, String> {
    if records.len() != drops.len() {
        return Err(format!(
            "aggregate: {} records but {} tallies",
            records.len(),
            drops.len()
        ));
    }
    let mut matchups: Vec<MatchupDropSummary> = Vec::new();
    let mut by_tag: BTreeMap<String, usize> = BTreeMap::new();
    for (record, tally) in records.iter().zip(drops.iter()) {
        let index = *by_tag.entry(record.matchup_tag.clone()).or_insert_with(|| {
            matchups.push(MatchupDropSummary {
                tag: record.matchup_tag.clone(),
                matches: 0,
                crashes_skipped: 0,
                bots: Vec::new(),
            });
            matchups.len() - 1
        });
        let matchup = &mut matchups[index];
        matchup.matches += 1;
        let Some(tally) = tally else {
            matchup.crashes_skipped += 1;
            continue;
        };
        // Mirror slots of one BotKind pool into a single entry.
        let mut kinds_seen: BTreeSet<BotKind> = BTreeSet::new();
        let factions: [(BotKind, &FactionDrops, u64, u64); 2] = [
            (
                if record.bot_a_is_enl {
                    record.bot_a
                } else {
                    record.bot_b
                },
                &tally.enl,
                record.stats.enl.commands_submitted,
                record.stats.enl.commands_dropped,
            ),
            (
                if record.bot_a_is_enl {
                    record.bot_b
                } else {
                    record.bot_a
                },
                &tally.res,
                record.stats.res.commands_submitted,
                record.stats.res.commands_dropped,
            ),
        ];
        for (kind, faction_drops, archived_submitted, archived_dropped) in factions {
            let entry = if let Some(position) = matchup.bots.iter().position(|bot| bot.bot == kind)
            {
                &mut matchup.bots[position]
            } else {
                matchup.bots.push(BotDropSummary {
                    bot: kind,
                    matches: 0,
                    archived_submitted: 0,
                    archived_dropped: 0,
                    reasons: BTreeMap::new(),
                });
                matchup.bots.last_mut().expect("just pushed")
            };
            if kinds_seen.insert(kind) {
                entry.matches += 1;
            }
            entry.archived_submitted += archived_submitted;
            entry.archived_dropped += archived_dropped;
            for (tag, count) in &faction_drops.reasons {
                *entry.reasons.entry(tag).or_insert(0) += count;
            }
        }
    }
    // Belt-and-braces cross-check (per-match verification already implies
    // it): per-reason sums must equal the archived drop totals.
    for matchup in &matchups {
        for bot in &matchup.bots {
            let sum = bot.reason_sum();
            if sum != bot.archived_dropped {
                return Err(format!(
                    "matchup {} bot {}: per-reason sum {} != archived commands_dropped {}",
                    matchup.tag,
                    bot.bot.tag(),
                    sum,
                    bot.archived_dropped
                ));
            }
        }
    }
    Ok(matchups)
}

/// The `drop-reasons.tsv` body: one row per (matchup, BotKind, reason) in
/// canonical tag order — zero rows included, so an absent reason is
/// recorded as an archived zero rather than an omission — plus one TOTAL
/// row per (matchup, BotKind).
pub fn drop_reasons_tsv(matchups: &[MatchupDropSummary]) -> String {
    let mut text = String::from("matchup_tag\tbot\tdrop_reason\tcount\n");
    for matchup in matchups {
        for bot in &matchup.bots {
            for tag in DROP_REASON_TAGS {
                let count = bot.reasons.get(tag).copied().unwrap_or(0);
                text.push_str(&format!(
                    "{}\t{}\t{}\t{}\n",
                    matchup.tag,
                    bot.bot.tag(),
                    tag,
                    count
                ));
            }
            text.push_str(&format!(
                "{}\t{}\tTOTAL\t{}\n",
                matchup.tag,
                bot.bot.tag(),
                bot.reason_sum()
            ));
        }
    }
    text
}

/// The `summary.txt` body: per-matchup, per-BotKind human-readable counts
/// with permille shares printed NEXT TO the raw counts (never instead of
/// them) and the archived-totals reconciliation.
pub fn drop_reasons_summary(matchups: &[MatchupDropSummary], source_matches: u64) -> String {
    let mut out = String::new();
    out.push_str("# arena-runner drop-reason tally (offline analysis, B0 evidence pass)\n");
    out.push_str(&format!(
        "# source matches: {source_matches}; integer counts; permille shares are derived, the counts are the record\n\n"
    ));
    for matchup in matchups {
        out.push_str(&format!("## {} ({} matches", matchup.tag, matchup.matches));
        if matchup.crashes_skipped > 0 {
            out.push_str(&format!(
                ", {} archived Crash records skipped",
                matchup.crashes_skipped
            ));
        }
        out.push_str(")\n");
        for bot in &matchup.bots {
            out.push_str(&format!(
                "### {} ({} matches, mirror slots pooled)\n",
                bot.bot.tag(),
                bot.matches
            ));
            let drop_rate_permille = if bot.archived_submitted == 0 {
                0
            } else {
                bot.archived_dropped * 1000 / bot.archived_submitted
            };
            out.push_str(&format!(
                "  submitted={} (matches.tsv) dropped={} (matches.tsv, drop rate {} permille)\n",
                bot.archived_submitted, bot.archived_dropped, drop_rate_permille
            ));
            for tag in DROP_REASON_TAGS {
                let count = bot.reasons.get(tag).copied().unwrap_or(0);
                if count == 0 {
                    continue;
                }
                let share_permille = if bot.archived_dropped == 0 {
                    0
                } else {
                    count * 1000 / bot.archived_dropped
                };
                out.push_str(&format!(
                    "  {tag}: {count} ({share_permille} permille of this bot's drops)\n"
                ));
            }
            let sum = bot.reason_sum();
            out.push_str(&format!(
                "  TOTAL per-reason sum: {sum} (archived commands_dropped: {}; reconciliation {})\n",
                bot.archived_dropped,
                if sum == bot.archived_dropped {
                    "OK"
                } else {
                    "MISMATCH"
                }
            ));
        }
        out.push('\n');
    }
    out
}

/// The `manifest.txt` body: full provenance of the re-derivation.
#[allow(clippy::too_many_arguments)]
pub fn drop_reasons_manifest(
    experiment_id: &str,
    source_dir: &Path,
    source_matches: usize,
    source_tsv_blake3: &str,
    source_manifest: &BTreeMap<String, String>,
    base_commit: Option<&str>,
    workers: usize,
    elapsed_ms: u128,
    crashes_skipped: u64,
) -> String {
    let mut text = String::new();
    let line = |text: &mut String, key: &str, value: String| {
        text.push_str(key);
        text.push('=');
        text.push_str(&value);
        text.push('\n');
    };
    line(&mut text, "experiment_id", experiment_id.to_string());
    line(&mut text, "stage", "B0-evidence-pass".to_string());
    line(
        &mut text,
        "purpose",
        "re-derive per-DropReason command counts for docs/balance-report-r1.md (matches.tsv archives drop totals only)".to_string(),
    );
    line(&mut text, "source_dir", source_dir.display().to_string());
    line(&mut text, "source_matches", source_matches.to_string());
    line(
        &mut text,
        "source_matches_tsv_blake3",
        source_tsv_blake3.to_string(),
    );
    for key in ["master_seed", "schedule_seed", "bot_version", "config_hash"] {
        if let Some(value) = source_manifest.get(key) {
            line(&mut text, &format!("source_{key}"), value.clone());
        }
    }
    line(
        &mut text,
        "reexecute_mode",
        "in-process MatchHarness, fresh m6.0 bots, per-record game_seed/bot_seed/map_seed and bucket policy read back from matches.tsv".to_string(),
    );
    line(&mut text, "config", "GameConfig::default()".to_string());
    line(
        &mut text,
        "tally_source",
        "RoundReport execution_order CommandOutcome::Drop variants".to_string(),
    );
    line(
        &mut text,
        "verification",
        "every match: recomputed final State Hash and per-faction submitted/ok/dropped compared against the archived row; any mismatch aborts".to_string(),
    );
    line(
        &mut text,
        "base_commit",
        base_commit.unwrap_or("unrecorded").to_string(),
    );
    line(
        &mut text,
        "reproduce",
        format!(
            "cargo run --release -p arena-runner -- drop-reasons {} runs <base_commit>",
            source_dir.display()
        ),
    );
    line(&mut text, "workers", workers.to_string());
    line(
        &mut text,
        "crash_records_skipped",
        crashes_skipped.to_string(),
    );
    line(&mut text, "elapsed_ms", elapsed_ms.to_string());
    line(
        &mut text,
        "note",
        "offline analysis only; integer raw counts are the record; nothing here enters any Game Result, Replay Hash or rule settlement".to_string(),
    );
    text
}

/// blake3 of a file's bytes, lowercase hex.
fn blake3_hex_of_file(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(arena_replay::canon::hex(blake3::hash(&bytes).as_bytes()))
}

/// Operator entry point behind the `arena-runner drop-reasons`
/// subcommand: reads the frozen archive at `frozen_run_dir`, re-executes
/// and verifies every match, and writes the tally artifacts into a NEW
/// `<output_root>/balance/<experiment-id>/` directory (existing
/// directories are refused — historical runs are never overwritten).
/// Returns the process exit code.
pub fn run_drop_reasons(
    frozen_run_dir: &Path,
    output_root: &Path,
    base_commit: Option<&str>,
) -> i32 {
    let start = Instant::now();
    let tsv_path = frozen_run_dir.join("matches.tsv");
    let text = match fs::read_to_string(&tsv_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("drop-reasons: cannot read {}: {error}", tsv_path.display());
            return 1;
        }
    };
    let records = match parse_matches_tsv(&text) {
        Ok(records) => records,
        Err(error) => {
            eprintln!("drop-reasons: parse {}: {error}", tsv_path.display());
            return 1;
        }
    };
    let source_tsv_blake3 = match blake3_hex_of_file(&tsv_path) {
        Ok(hash) => hash,
        Err(error) => {
            eprintln!("drop-reasons: hash {}: {error}", tsv_path.display());
            return 1;
        }
    };
    // Echo the frozen batch's own provenance when the manifest is present.
    let mut source_manifest = BTreeMap::new();
    if let Ok(manifest) = fs::read_to_string(frozen_run_dir.join("manifest.txt")) {
        for line in manifest.lines() {
            if let Some((key, value)) = line.split_once('=') {
                source_manifest.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }
    let cfg = GameConfig::default();
    if crate::tournament::config::config_identity(&cfg)
        != records
            .first()
            .map(|record| record.config_hash)
            .unwrap_or_default()
    {
        eprintln!(
            "drop-reasons: {} was not produced with GameConfig::default() — this tally only reconstructs the frozen default-config batches",
            frozen_run_dir.display()
        );
        return 1;
    }
    // Map pool: one MapDefinition per distinct archived map_seed, built
    // once up front and shared (worker count cannot move a match).
    let map_seeds: Vec<u64> = records
        .iter()
        .map(|record| record.map_seed)
        .collect::<BTreeSet<u64>>()
        .into_iter()
        .collect();
    let mut map_index_of: BTreeMap<u64, usize> = BTreeMap::new();
    let mut maps: Vec<Arc<MapDefinition>> = Vec::with_capacity(map_seeds.len());
    for (index, seed) in map_seeds.iter().enumerate() {
        match generate_map(*seed, &cfg) {
            Ok(map) => {
                map_index_of.insert(*seed, index);
                maps.push(Arc::new(map));
            }
            Err(error) => {
                eprintln!(
                    "drop-reasons: archived map seed {seed:#x} does not regenerate: {error:?}"
                );
                return 1;
            }
        }
    }

    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_WORKERS)
        .clamp(1, records.len().max(1));
    // One Result slot per record (stride partition, run_batch convention);
    // workers never share mutable state — each match tallies independently
    // and the merge below is in record order, so the worker count cannot
    // move the result.
    let mut slots: Vec<Option<Result<Option<MatchDrops>, String>>> =
        records.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for worker in 0..workers {
            let records = &records;
            let maps = &maps;
            let map_index_of = &map_index_of;
            handles.push(scope.spawn(move || {
                let mut ours: Vec<(usize, Result<Option<MatchDrops>, String>)> = Vec::new();
                let mut index = worker;
                while index < records.len() {
                    let record = &records[index];
                    let result = if record.result == Outcome::Crash {
                        Ok(None)
                    } else {
                        let map =
                            maps[*map_index_of.get(&record.map_seed).expect("seed pooled")].clone();
                        reexecute_archived_match(record, &cfg, &map)
                            .and_then(|drops| verify_match_drops(record, &drops).map(|_| drops))
                            .map(Some)
                    };
                    ours.push((index, result));
                    index += workers;
                }
                ours
            }));
        }
        for handle in handles {
            for (slot, result) in handle.join().expect("drop-reasons worker panicked") {
                slots[slot] = Some(result);
            }
        }
    });
    let mut errors: Vec<String> = Vec::new();
    let mut tallies: Vec<Option<MatchDrops>> = Vec::with_capacity(records.len());
    for slot in slots {
        match slot.expect("every record tallied exactly once") {
            Ok(drops) => tallies.push(drops),
            Err(error) => errors.push(error),
        }
    }
    if !errors.is_empty() {
        for error in errors.iter().take(10) {
            eprintln!("drop-reasons: verification failed: {error}");
        }
        eprintln!(
            "drop-reasons: {} of {} matches failed re-execution verification — nothing written",
            errors.len(),
            records.len()
        );
        return 1;
    }
    let crashes_skipped = tallies.iter().filter(|tally| tally.is_none()).count() as u64;
    let matchups = match aggregate_drop_reasons(&records, &tallies) {
        Ok(matchups) => matchups,
        Err(error) => {
            eprintln!("drop-reasons: aggregation cross-check failed: {error}");
            return 1;
        }
    };

    // New artifacts only: refuse an existing experiment directory.
    let experiment_id = format!(
        "b0-drop-reasons-{}",
        frozen_run_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive".to_string())
    );
    let dir = output_root.join("balance").join(&experiment_id);
    if dir.exists() {
        eprintln!(
            "drop-reasons: {} already exists — historical runs are never overwritten",
            dir.display()
        );
        return 1;
    }
    if let Err(error) = fs::create_dir_all(&dir) {
        eprintln!("drop-reasons: cannot create {}: {error}", dir.display());
        return 1;
    }
    let manifest = drop_reasons_manifest(
        &experiment_id,
        frozen_run_dir,
        records.len(),
        &source_tsv_blake3,
        &source_manifest,
        base_commit,
        workers,
        start.elapsed().as_millis(),
        crashes_skipped,
    );
    let tsv = drop_reasons_tsv(&matchups);
    let summary = drop_reasons_summary(&matchups, records.len() as u64);
    let files = [
        ("manifest.txt", manifest),
        ("drop-reasons.tsv", tsv),
        ("summary.txt", summary.clone()),
    ];
    for (name, body) in &files {
        if let Err(error) = fs::write(dir.join(name), body) {
            eprintln!(
                "drop-reasons: cannot write {}: {error}",
                dir.join(name).display()
            );
            return 1;
        }
    }
    // Content hashes of the artifacts (runs/ is untracked; the committed
    // report cites these — plan section 12).
    let mut hashes = String::new();
    for (name, _) in &files {
        match blake3_hex_of_file(&dir.join(name)) {
            Ok(hash) => hashes.push_str(&format!("blake3:{hash}  {name}\n")),
            Err(error) => {
                eprintln!(
                    "drop-reasons: cannot hash {}: {error}",
                    dir.join(name).display()
                );
                return 1;
            }
        }
    }
    if let Err(error) = fs::write(dir.join("hashes.txt"), &hashes) {
        eprintln!(
            "drop-reasons: cannot write {}: {error}",
            dir.join("hashes.txt").display()
        );
        return 1;
    }

    println!(
        "drop-reasons: {} matches re-executed and verified ({} crashes skipped) in {:?}",
        records.len(),
        crashes_skipped,
        start.elapsed()
    );
    println!("drop-reasons: artifacts in {}", dir.display());
    print!("{}", summary);
    for line in hashes.lines() {
        println!("drop-reasons: {line}");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_reason_tags_are_unique_and_in_declaration_order() {
        let mut sorted = DROP_REASON_TAGS;
        sorted.sort_unstable();
        let mut unique = sorted.to_vec();
        unique.dedup();
        assert_eq!(
            sorted.len(),
            unique.len(),
            "every DropReason tag is distinct"
        );
        // The match in drop_reason_tag is exhaustive (the compiler checks
        // that); pin one representative payload-carrying variant per
        // command family so payloads really are ignored.
        assert_eq!(
            drop_reason_tag(&DropReason::CaptureTargetNotNeutral {
                portal: arena_core::types::PortalId(0),
                owner: arena_core::types::Owner::Enl
            }),
            "CaptureTargetNotNeutral"
        );
        assert_eq!(
            drop_reason_tag(&DropReason::LinkAlreadyExists {
                link: arena_core::types::LinkId::new(
                    arena_core::types::PortalId(0),
                    arena_core::types::PortalId(1)
                )
                .expect("distinct portals")
            }),
            "LinkAlreadyExists"
        );
        assert_eq!(
            drop_reason_tag(&DropReason::AttackNoEnemyPortalsInRange),
            "AttackNoEnemyPortalsInRange"
        );
    }

    #[test]
    fn tsv_and_summary_carry_raw_counts_alongside_shares() {
        let mut reasons = BTreeMap::new();
        reasons.insert("CaptureTargetNotNeutral", 7u64);
        let matchups = vec![MatchupDropSummary {
            tag: "Greedy-DestroyAnchor".to_string(),
            matches: 2,
            crashes_skipped: 0,
            bots: vec![BotDropSummary {
                bot: BotKind::GreedyField,
                matches: 2,
                archived_submitted: 10,
                archived_dropped: 7,
                reasons,
            }],
        }];
        let tsv = drop_reasons_tsv(&matchups);
        assert_eq!(
            tsv.lines().next().expect("header"),
            "matchup_tag\tbot\tdrop_reason\tcount"
        );
        // Zero rows are explicit (absence is archived, not omitted).
        assert!(tsv.contains("Greedy-DestroyAnchor\tgreedy-field\tLinkSelfLoop\t0\n"));
        assert!(tsv.contains("Greedy-DestroyAnchor\tgreedy-field\tCaptureTargetNotNeutral\t7\n"));
        assert!(tsv.contains("Greedy-DestroyAnchor\tgreedy-field\tTOTAL\t7\n"));
        let summary = drop_reasons_summary(&matchups, 2);
        assert!(summary.contains("dropped=7 (matches.tsv"));
        assert!(summary.contains("CaptureTargetNotNeutral: 7 ("));
        assert!(
            summary.contains(
                "TOTAL per-reason sum: 7 (archived commands_dropped: 7; reconciliation OK)"
            )
        );
    }
}

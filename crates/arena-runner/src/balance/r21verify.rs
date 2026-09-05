//! The r2.1 post-audit verification batch (the 2026-09-05 bot action
//! audit's recommended step 4, SMALL scale only): the two audited
//! pairings plus the idle readiness re-check of every competitive R2
//! policy, at `4 Development maps x 2 faction-swap legs = 8` matches per
//! combination — every combination stays under 10 matches until the
//! operator reviews the results and clears larger batches.
//!
//! Beyond the standard balance archive, every match also folds the
//! AUDIT'S OWN metrics in-process (from the resolver's round reports,
//! before anything is discarded): the agent-round activity rate (the
//! no-task proxy the audit measured at 66–68% for defender/blocker and
//! 10% for disruptor), the zero-effect vs effective Recharge split with
//! restored HP, and the Transfer count. Those land in `verify.tsv` /
//! `verify.txt` — offline analysis only, integer raw counts, never part
//! of any Game Result or Replay Hash.

#![allow(clippy::too_many_arguments)]

use std::collections::BTreeMap;

use arena_bots::policy::{PolicyId, PolicySpec, make_policy_player};
use arena_core::config::GameConfig;
use arena_core::event::{Event, EventKind};
use arena_core::harness::MatchHarness;
use arena_core::map::{MapDefinition, generate_map};
use arena_core::resolve::{CommandEffect, CommandOutcome, RoundReport};
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::Faction;
use arena_replay::recorder::ReplayWriter;

use crate::balance::execute::BalanceReplay;
use crate::balance::plan::{ExperimentPlan, PolicyMatchup, SeedSet, Stage};
use crate::balance::record::{BalanceMatchRecord, BalanceOutcome};
use crate::balance::schedule::{MatchCell, expand};
use crate::batch::MatchDescriptor;
use crate::metrics::{MatchFold, MatchRecord, Outcome};
use crate::policy::PolicyStrategy;
use crate::policy::BucketPolicy;

/// The frozen experiment id (archive directory name). `-r1` is the first
/// run of the r2.1 policies; the batch is EXPLORATORY evidence pending
/// operator review, like the B2 `-pilot-r0` runs — never gate evidence.
pub const R21_VERIFY_EXPERIMENT_ID: &str = "r21-verify-r1";

/// The frozen map count: 4 maps x 2 legs = 8 matches per combination,
/// under the operator's 10-per-combination review cap.
pub const R21_VERIFY_MAP_COUNT: u32 = 4;

/// The audited pairings (report.md's two matches) plus the idle readiness
/// re-check of every competitive R2 policy.
pub fn r21_verify_matchups() -> Vec<PolicyMatchup> {
    let mut matchups = vec![
        PolicyMatchup::new(
            "builder-defender-r2--builder-disruptor-r2",
            PolicyId::BuilderDefenderR2,
            PolicyId::BuilderDisruptorR2,
        ),
        PolicyMatchup::new(
            "logistics-builder-r2--blocker-builder-r2",
            PolicyId::LogisticsBuilderR2,
            PolicyId::BlockerBuilderR2,
        ),
    ];
    for &policy in PolicyId::R2_COMPETITIVE.iter() {
        matchups.push(PolicyMatchup::new(
            &format!("{}--idle", policy.tag()),
            policy,
            PolicyId::Idle,
        ));
    }
    matchups
}

/// The verification plan: default config, Development maps, `Flat(0)`,
/// the r2.1 policy registry.
pub fn r21_verify_plan() -> ExperimentPlan {
    ExperimentPlan {
        experiment_id: R21_VERIFY_EXPERIMENT_ID.to_string(),
        stage: Stage::Readiness,
        seed_set: SeedSet::Development,
        map_count: R21_VERIFY_MAP_COUNT,
        base_variant: "default".to_string(),
        variants: vec![crate::balance::plan::ConfigVariant::new(
            "default",
            GameConfig::default(),
            Vec::new(),
        )],
        policies: arena_bots::policy::r2_specs(),
        matchups: r21_verify_matchups(),
        bucket_policy: BucketPolicy::Flat(0),
        gates: vec![crate::balance::plan::Gate::Pending {
            gate_id: "operator-review".to_string(),
            statement: "exploratory batch: every combination <10 matches; larger runs wait for the operator's review of verify.txt".to_string(),
        }],
    }
}

// ---------------------------------------------------------------------
// The audit fold (in-process, from the resolver round reports)
// ---------------------------------------------------------------------

/// One faction's audit metrics (all raw integer counts).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct FactionAudit {
    /// Live agent-rounds elapsed (initial roster + granted reinforcements).
    pub agent_rounds_total: u64,
    /// Agent-rounds with a non-empty submitted command stream (the audit's
    /// no-task inverse: an R2 policy only emits commands for granted
    /// tasks, so an empty stream is an unassigned agent-round).
    pub agent_rounds_active: u64,
    /// Executed Recharges that restored HP (`hp_after > hp_before`).
    pub recharges_effective: u64,
    /// Executed Recharges that restored nothing (the audit's R451-454
    /// 100->100 pattern).
    pub recharges_zero: u64,
    /// Sum of `hp_after - hp_before` over effective Recharges.
    pub recharge_hp_restored: u64,
    /// Executed Transfers.
    pub transfers: u64,
}

impl FactionAudit {
    /// Passive agent-rounds (no submitted commands) — the audit's no-task
    /// numerator.
    pub fn agent_rounds_passive(&self) -> u64 {
        self.agent_rounds_total.saturating_sub(self.agent_rounds_active)
    }

    /// Passive share in permille (0 when nothing elapsed; display only —
    /// the counts above are the record).
    pub fn passive_permille(&self) -> u64 {
        if self.agent_rounds_total == 0 {
            return 0;
        }
        self.agent_rounds_passive() * 1000 / self.agent_rounds_total
    }
}

/// The whole match's audit fold: one [`FactionAudit`] per faction plus the
/// live-roster counter reinforcement events grow.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct MatchAudit {
    pub enl: FactionAudit,
    pub res: FactionAudit,
    live_per_faction: u32,
}

impl MatchAudit {
    /// The empty fold of a match with `cfg`'s initial roster.
    pub fn new(cfg: &GameConfig) -> MatchAudit {
        MatchAudit {
            live_per_faction: cfg.initial_agents_per_faction,
            ..Default::default()
        }
    }

    fn faction(&mut self, faction: Faction) -> &mut FactionAudit {
        match faction {
            Faction::Enl => &mut self.enl,
            Faction::Res => &mut self.res,
        }
    }

    /// Folds one settled round: the denominator is the roster as it stood
    /// when the round BEGAN (reinforcements land in the settlement of the
    /// boundary round and can only act from the next round — resolve.rs
    /// appends them after `current_round % interval == 0` settles), so the
    /// roster growth from `new_events` applies from the NEXT folded round;
    /// activity comes from the echoed submissions, and the recharge/transfer
    /// split from the executed outcomes.
    pub fn round(&mut self, report: &RoundReport, new_events: &[Event]) {
        for faction in [Faction::Enl, Faction::Res] {
            self.faction(faction).agent_rounds_total += self.live_per_faction as u64;
        }
        for event in new_events {
            if let EventKind::Reinforcement {
                count_per_faction, ..
            } = event.kind
            {
                self.live_per_faction = self.live_per_faction.saturating_add(count_per_faction);
            }
        }
        for stream in &report.submissions {
            if !stream.commands.is_empty() {
                self.faction(stream.faction).agent_rounds_active += 1;
            }
        }
        for stream in &report.execution_order {
            for outcome in &stream.outcomes {
                match outcome {
                    CommandOutcome::Ok(CommandEffect::Recharged {
                        hp_before, hp_after, ..
                    }) => {
                        let stats = self.faction(stream.faction);
                        if hp_after > hp_before {
                            stats.recharges_effective += 1;
                            stats.recharge_hp_restored += (hp_after - hp_before) as u64;
                        } else {
                            stats.recharges_zero += 1;
                        }
                    }
                    CommandOutcome::Ok(CommandEffect::Transferred { .. }) => {
                        self.faction(stream.faction).transfers += 1;
                    }
                    _ => {}
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// Execution: the m6 play loop + ReplayWriter + the audit fold
// ---------------------------------------------------------------------

/// One cell's full outcome: the standard balance record, the captured
/// replay (every leg — the batch is small enough to keep them all) and
/// the audit fold.
struct CellResult {
    record: BalanceMatchRecord,
    replay: Option<Vec<u8>>,
    audit: MatchAudit,
}

/// Drives one cell's match directly (the drops.rs precedent: the frozen
/// `run::play` path cannot see per-round reports) — standard MatchFold
/// for the record, ReplayWriter for the archive, MatchAudit for
/// verify.tsv. The whole cell runs under `catch_unwind` (the m6 crash
/// contract: a panic is this cell's Crash, never a batch abort); a
/// panicked cell has no settled audit.
fn run_cell(
    cell: &MatchCell,
    spec_a: &PolicySpec,
    spec_b: &PolicySpec,
    cfg: &GameConfig,
    map: &MapDefinition,
) -> CellResult {
    let started = std::time::Instant::now();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (enl_spec, res_spec) = if cell.policy_a_is_enl {
            (spec_a, spec_b)
        } else {
            (spec_b, spec_a)
        };
        let mut enl_player = make_policy_player(enl_spec, Faction::Enl, cell.bot_seed, cfg, map);
        let mut res_player = make_policy_player(res_spec, Faction::Res, cell.bot_seed, cfg, map);
        let (enl_roster, res_roster) = GameState::initial_roster(cfg);
        let enl_spawns = validate_initial_spawn_set(
            &enl_roster,
            &enl_player.choose_spawns(cfg, map, &enl_roster),
            cfg,
        )
        .expect("bot spawn set");
        let res_spawns = validate_initial_spawn_set(
            &res_roster,
            &res_player.choose_spawns(cfg, map, &res_roster),
            cfg,
        )
        .expect("bot spawn set");
        let mut harness = MatchHarness::new(
            cell.game_seed,
            *cfg,
            map,
            &enl_spawns,
            &res_spawns,
            Box::new(PolicyStrategy::new(
                enl_player.into_strategy(),
                Faction::Enl,
                cell.bucket_policy,
            )),
            Box::new(PolicyStrategy::new(
                res_player.into_strategy(),
                Faction::Res,
                cell.bucket_policy,
            )),
        )
        .expect("match builds from validated inputs");
        let mut fold = MatchFold::new();
        let mut audit = MatchAudit::new(cfg);
        let mut replay = ReplayWriter::new(cell.game_seed, cfg, map, &enl_spawns, &res_spawns);
        let mut events_seen = 0usize;
        for _ in 0..cfg.total_rounds {
            let report = harness.run_round();
            let state = harness.state();
            let new_events = &state.events()[events_seen..];
            fold.round(state.round() - 1, &report, new_events, map, cfg);
            audit.round(&report, new_events);
            events_seen = state.events().len();
            replay
                .record_round(&report, state)
                .expect("replay recorder round");
        }
        let state = harness.into_state();
        fold.finish(&state, cfg);
        let replay = replay.finish(&state).expect("replay writer finish");
        (fold.stats, audit, replay, state)
    }));
    let descriptor = MatchDescriptor {
        index: cell.index,
        matchup_index: 0,
        matchup_tag: cell.pair_tag.clone(),
        j: cell.map_index * 2 + cell.leg as u32,
        bot_a: cell.policy_a.tag().to_string(),
        bot_b: cell.policy_b.tag().to_string(),
        bot_a_is_enl: cell.policy_a_is_enl,
        game_seed: cell.game_seed,
        map_seed: cell.map_seed,
        map_index: cell.map_index,
        bot_seed: cell.bot_seed,
        schedule_seed: cell.schedule_seed,
        bucket_policy: cell.bucket_policy,
        config_hash: cell.config_hash,
        bot_a_version: spec_a.version,
        bot_b_version: spec_b.version,
    };
    let elapsed = started.elapsed().as_millis() as u64;
    match caught {
        Ok((stats, audit, replay, state)) => {
            let record = MatchRecord::from_match(&descriptor, &state, stats, elapsed);
            CellResult {
                record: BalanceMatchRecord::from_match_record(cell, spec_a, spec_b, &record),
                replay: Some(replay),
                audit,
            }
        }
        Err(_panic) => CellResult {
            record: BalanceMatchRecord::from_match_record(
                cell,
                spec_a,
                spec_b,
                &MatchRecord::crashed(&descriptor, elapsed),
            ),
            replay: None,
            audit: MatchAudit::default(),
        },
    }
}

// ---------------------------------------------------------------------
// verify.tsv / verify.txt (integer raw counts; rates display-only)
// ---------------------------------------------------------------------

/// One (matchup, policy) aggregation row of `verify.tsv`.
#[derive(Clone, Copy, Default, Debug)]
struct PolicyVerifyRow {
    legs: u64,
    wins: u64,
    draws: u64,
    losses: u64,
    crashes: u64,
    audit: FactionAudit,
    first_field_round_min: Option<u32>,
    first_field_round_max: Option<u32>,
    fields_created_sum: u64,
    score2_sum: u128,
}

/// The fixed `verify.tsv` columns.
fn verify_columns() -> Vec<&'static str> {
    vec![
        "pair_tag",
        "policy",
        "legs",
        "wins",
        "draws",
        "losses",
        "crashes",
        "agent_rounds_total",
        "agent_rounds_active",
        "agent_rounds_passive",
        "passive_permille",
        "recharges_effective",
        "recharges_zero",
        "recharge_hp_restored",
        "transfers",
        "first_field_round_min",
        "first_field_round_max",
        "fields_created_sum",
        "score2_sum",
    ]
}

/// Aggregates one run's records+audits into the per-(matchup, policy)
/// rows, ordered by matchup declaration then policy A before policy B.
fn verify_rows(
    plan: &ExperimentPlan,
    records: &[BalanceMatchRecord],
    audits: &BTreeMap<u64, MatchAudit>,
) -> Vec<(String, String, PolicyVerifyRow)> {
    let by_index: BTreeMap<u64, (&BalanceMatchRecord, MatchAudit)> = records
        .iter()
        .map(|record| {
            (
                record.index,
                (record, *audits.get(&record.index).unwrap_or(&MatchAudit::default())),
            )
        })
        .collect();
    let mut out = Vec::new();
    for matchup in &plan.matchups {
        for (spec, other) in [(matchup.a, matchup.b), (matchup.b, matchup.a)] {
            let mut row = PolicyVerifyRow::default();
            for record in records.iter().filter(|r| r.pair_tag == matchup.tag) {
                // A leg participates from both (policy, other) views; the
                // row counts each leg once per view.
                let this_is_a = record.policy_a == spec;
                let this_is_b = record.policy_b == spec;
                if !this_is_a && !this_is_b {
                    continue;
                }
                let (_, audit) = by_index
                    .get(&record.index)
                    .expect("every record has its audit");
                let this_is_enl = if this_is_a {
                    record.policy_a_is_enl
                } else {
                    !record.policy_a_is_enl
                };
                let faction_audit = if this_is_enl { audit.enl } else { audit.res };
                row.legs += 1;
                let (score, stats, won) = if this_is_enl {
                    (
                        record.enl_score2,
                        &record.stats.enl,
                        matches!(record.result, BalanceOutcome::Played(Outcome::EnlWin)),
                    )
                } else {
                    (
                        record.res_score2,
                        &record.stats.res,
                        matches!(record.result, BalanceOutcome::Played(Outcome::ResWin)),
                    )
                };
                match record.result {
                    BalanceOutcome::Played(Outcome::Crash) => row.crashes += 1,
                    BalanceOutcome::Played(Outcome::Draw) => row.draws += 1,
                    BalanceOutcome::Played(_) if won => row.wins += 1,
                    BalanceOutcome::Played(_) => row.losses += 1,
                    BalanceOutcome::ConfigFailure => {}
                }
                row.audit.agent_rounds_total += faction_audit.agent_rounds_total;
                row.audit.agent_rounds_active += faction_audit.agent_rounds_active;
                row.audit.recharges_effective += faction_audit.recharges_effective;
                row.audit.recharges_zero += faction_audit.recharges_zero;
                row.audit.recharge_hp_restored += faction_audit.recharge_hp_restored;
                row.audit.transfers += faction_audit.transfers;
                row.first_field_round_min = row
                    .first_field_round_min
                    .iter()
                    .chain(stats.first_field_round.iter())
                    .copied()
                    .min();
                row.first_field_round_max = row
                    .first_field_round_max
                    .iter()
                    .chain(stats.first_field_round.iter())
                    .copied()
                    .max();
                row.fields_created_sum += stats.fields_created;
                row.score2_sum += score as u128;
                let _ = other;
            }
            if row.legs > 0 {
                out.push((matchup.tag.clone(), spec.tag().to_string(), row));
            }
        }
    }
    out
}

/// The whole `verify.tsv` document.
fn verify_tsv(
    plan: &ExperimentPlan,
    records: &[BalanceMatchRecord],
    audits: &BTreeMap<u64, MatchAudit>,
) -> String {
    let mut text = String::new();
    text.push_str(&verify_columns().join("\t"));
    text.push('\n');
    for (pair, policy, row) in verify_rows(plan, records, audits) {
        let cells = [
            pair,
            policy,
            row.legs.to_string(),
            row.wins.to_string(),
            row.draws.to_string(),
            row.losses.to_string(),
            row.crashes.to_string(),
            row.audit.agent_rounds_total.to_string(),
            row.audit.agent_rounds_active.to_string(),
            row.audit.agent_rounds_passive().to_string(),
            row.audit.passive_permille().to_string(),
            row.audit.recharges_effective.to_string(),
            row.audit.recharges_zero.to_string(),
            row.audit.recharge_hp_restored.to_string(),
            row.audit.transfers.to_string(),
            row.first_field_round_min
                .map(|r| r.to_string())
                .unwrap_or_else(|| "-".to_string()),
            row.first_field_round_max
                .map(|r| r.to_string())
                .unwrap_or_else(|| "-".to_string()),
            row.fields_created_sum.to_string(),
            row.score2_sum.to_string(),
        ];
        text.push_str(&cells.join("\t"));
        text.push('\n');
    }
    text
}

/// The human-readable `verify.txt`: the same rows plus the audited r2.0
/// reference points (report.md) for direct comparison.
fn verify_text(
    plan: &ExperimentPlan,
    records: &[BalanceMatchRecord],
    audits: &BTreeMap<u64, MatchAudit>,
) -> String {
    let mut out = String::from(
        "# r2.1 verification metrics (integer raw counts; permille display-only)\n\
         # audit r2.0 reference points (.scratch/bot-action-audit-20260905/report.md):\n\
         #   defender no-task 1662/2500 agent-rounds (664 permille)\n\
         #   blocker   no-task 1692/2500 agent-rounds (676 permille)\n\
         #   disruptor no-task  258/2500 agent-rounds (103 permille)\n\
         #   defender recharges: 8 executed, 4 zero-effect (100->100)\n\
         #   logistics: 10 transfers, each deferring a same-round link\n\n",
    );
    for (pair, policy, row) in verify_rows(plan, records, audits) {
        out.push_str(&format!(
            "{pair} / {policy}: legs={} W-D-L={}--{}--{} crashes={}\n  \
             agent-rounds total={} active={} passive={} ({} permille)\n  \
             recharges effective={} zero={} restored_hp={} transfers={}\n  \
             first_field=[{:?}..{:?}] fields={} score2_sum={}\n",
            row.legs,
            row.wins,
            row.draws,
            row.losses,
            row.crashes,
            row.audit.agent_rounds_total,
            row.audit.agent_rounds_active,
            row.audit.agent_rounds_passive(),
            row.audit.passive_permille(),
            row.audit.recharges_effective,
            row.audit.recharges_zero,
            row.audit.recharge_hp_restored,
            row.audit.transfers,
            row.first_field_round_min,
            row.first_field_round_max,
            row.fields_created_sum,
            row.score2_sum,
        ));
    }
    out
}

// ---------------------------------------------------------------------
// Operator entry: `arena-runner balance-r21-verify <output-root>
// [base-commit]`
// ---------------------------------------------------------------------

/// Runs the verification batch on a scoped worker pool (stride partition;
/// worker count cannot move a match), archives it with the standard
/// balance artifacts plus `verify.tsv`/`verify.txt` (hashed), and prints
/// the verification summary. Returns the process exit code (0 unless the
/// run itself failed — this batch has NO pass/fail gates; the verdict is
/// the operator's review).
pub fn run_r21_verify(output_root: &std::path::Path, base_commit: Option<&str>) -> i32 {
    if arena_bots::r2::R2_VERSION != "r2.1" {
        eprintln!("balance-r21-verify requires the frozen r2.1 source; current policy version is {}. Use r22-micro for the r2.2 paired probes.", arena_bots::r2::R2_VERSION);
        return 2;
    }
    let plan = r21_verify_plan();
    if let Err(error) = plan.validate() {
        eprintln!("balance-r21-verify: {error}");
        return 1;
    }
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(crate::run::MAX_WORKERS);
    let cells = match expand(&plan) {
        Ok(cells) => cells,
        Err(error) => {
            eprintln!("balance-r21-verify: {error}");
            return 1;
        }
    };
    let base = plan.base();
    let mut pool = Vec::with_capacity(plan.map_count as usize);
    for k in 0..plan.map_count {
        let seed = crate::balance::plan::map_seed(plan.seed_set, k);
        match generate_map(seed, &base.config) {
            Ok(map) => pool.push(std::sync::Arc::new(map)),
            Err(_) => {
                eprintln!("balance-r21-verify: development map {k} is infeasible (config defect)");
                return 1;
            }
        }
    }
    let spec_of = |id: PolicyId| -> PolicySpec {
        plan.policies
            .iter()
            .find(|s| s.policy_id == id)
            .expect("plan validation registers every matchup policy")
            .clone()
    };
    println!(
        "balance-r21-verify: {} matchups x {} maps x 2 legs = {} matches (every combination {} matches, under the 10-cap), {workers} workers",
        plan.matchups.len(),
        plan.map_count,
        cells.len(),
        plan.map_count * 2
    );
    let start = std::time::Instant::now();
    let mut slots: Vec<Option<(BalanceMatchRecord, Option<BalanceReplay>, MatchAudit)>> =
        cells.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for worker in 0..workers {
            let cells = &cells;
            let pool = &pool;
            handles.push(scope.spawn(move || {
                let mut ours = Vec::new();
                let mut index = worker;
                while index < cells.len() {
                    let cell = &cells[index];
                    let spec_a = spec_of(cell.policy_a);
                    let spec_b = spec_of(cell.policy_b);
                    let map = pool[cell.map_index as usize].clone();
                    let result = run_cell(cell, &spec_a, &spec_b, &base.config, &map);
                    let replay = result.replay.map(|bytes| BalanceReplay {
                        cell_index: cell.index,
                        variant_id: cell.variant_id.clone(),
                        pair_id: cell.pair_id.clone(),
                        leg: cell.leg,
                        map_id: cell.map_id.clone(),
                        bytes,
                    });
                    ours.push((index, (result.record, replay, result.audit)));
                    index += workers;
                }
                ours
            }));
        }
        for handle in handles {
            for (slot_index, slot) in handle.join().expect("verification worker panicked") {
                slots[slot_index] = Some(slot);
            }
        }
    });
    let mut records = Vec::with_capacity(cells.len());
    let mut replays = Vec::new();
    let mut audits = BTreeMap::new();
    let mut crashes = 0u64;
    for slot in slots {
        let (record, replay, audit) = slot.expect("every cell ran exactly once");
        if record.result == BalanceOutcome::Played(Outcome::Crash) {
            crashes += 1;
        }
        audits.insert(record.index, audit);
        if let Some(replay) = replay {
            replays.push(replay);
        }
        records.push(record);
    }
    let run = crate::balance::execute::BalanceRunResult {
        records: records.clone(),
        replays,
        config_failures: 0,
        elapsed_ms: start.elapsed().as_millis(),
        workers,
    };
    let pairs = match crate::balance::pairs::compute_paired_results(&plan, &run.records) {
        Ok(pairs) => pairs,
        Err(error) => {
            eprintln!("balance-r21-verify: {error}");
            return 1;
        }
    };
    let summaries = crate::balance::pairs::variant_summaries(&plan, &pairs);
    let dir = match crate::balance::archive::write_balance_artifacts(
        output_root,
        &plan,
        &run,
        &pairs,
        &summaries,
        base_commit,
    ) {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("balance-r21-verify: archive: {error}");
            return 1;
        }
    };
    let tsv = verify_tsv(&plan, &records, &audits);
    let text = verify_text(&plan, &records, &audits);
    let blake3_hex = |bytes: &[u8]| arena_replay::canon::hex(blake3::hash(bytes).as_bytes());
    for (name, body) in [("verify.tsv", tsv.clone()), ("verify.txt", text.clone())] {
        if let Err(error) = std::fs::write(dir.join(name), &body) {
            eprintln!("balance-r21-verify: {name}: {error}");
            return 1;
        }
        let hashes_path = dir.join("hashes.txt");
        let mut hashes = match std::fs::read_to_string(&hashes_path) {
            Ok(hashes) => hashes,
            Err(error) => {
                eprintln!("balance-r21-verify: hashes.txt: {error}");
                return 1;
            }
        };
        hashes.push_str(&format!("blake3:{}  {}\n", blake3_hex(body.as_bytes()), name));
        if let Err(error) = std::fs::write(&hashes_path, &hashes) {
            eprintln!("balance-r21-verify: hashes.txt: {error}");
            return 1;
        }
    }
    println!(
        "balance-r21-verify: archived {} ({} matches, {} crashes, {} replays, {:?})",
        dir.display(),
        run.records.len(),
        crashes,
        run.replays.len(),
        std::time::Duration::from_millis(run.elapsed_ms as u64)
    );
    println!("{text}");
    i32::from(crashes > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::resolve::SubmittedCommandStream;

    /// The batch shape honors the operator's review cap: every
    /// combination runs under 10 matches.
    #[test]
    fn every_combination_stays_under_ten_matches() {
        let plan = r21_verify_plan();
        assert_eq!(plan.map_count, 4);
        assert_eq!(plan.map_count * 2, 8);
        let cells = expand(&plan).expect("plan expands");
        let mut per_pair: BTreeMap<String, u64> = BTreeMap::new();
        for cell in &cells {
            *per_pair.entry(cell.pair_tag.clone()).or_default() += 1;
        }
        assert_eq!(per_pair.len(), plan.matchups.len());
        assert!(per_pair.values().all(|&legs| legs < 10));
        // The two audited pairings and the six idle probes are all present.
        for tag in [
            "builder-defender-r2--builder-disruptor-r2",
            "logistics-builder-r2--blocker-builder-r2",
            "greedy-coordinated-r2--idle",
            "adaptive-hybrid-r2--idle",
        ] {
            assert!(per_pair.contains_key(tag), "{tag} missing");
        }
    }

    /// The audit fold counts activity, reinforcement growth and the
    /// recharge effect split from hand-built round reports.
    #[test]
    fn audit_fold_counts_activity_recharges_and_transfers() {
        use arena_core::types::{AgentId, Command};
        let cfg = GameConfig::default();
        let mut audit = MatchAudit::new(&cfg);
        assert_eq!(audit.live_per_faction, cfg.initial_agents_per_faction);
        let submission = |faction: Faction, agent: u32, commands: Vec<Command>| {
            SubmittedCommandStream {
                round: 1,
                faction,
                agent_id: AgentId(agent),
                received_at_ms: 0,
                receive_seq: agent,
                commands,
            }
        };
        let report = RoundReport {
            submissions: vec![
                submission(Faction::Enl, 0, vec![Command::Move {
                    destination: arena_core::types::Coord::new(1, 1),
                }]),
                submission(Faction::Enl, 1, vec![]),
                submission(Faction::Res, 0, vec![]),
            ],
            slots: BTreeMap::new(),
            execution_order: vec![
                arena_core::resolve::ExecutedStream {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    receive_bucket: 0,
                    receive_seq: 0,
                    outcomes: vec![
                        CommandOutcome::Ok(CommandEffect::Recharged {
                            portal: arena_core::types::PortalId(0),
                            hp_before: 100,
                            hp_after: 100,
                        }),
                        CommandOutcome::Ok(CommandEffect::Recharged {
                            portal: arena_core::types::PortalId(1),
                            hp_before: 70,
                            hp_after: 85,
                        }),
                    ],
                },
                arena_core::resolve::ExecutedStream {
                    faction: Faction::Res,
                    agent_id: AgentId(3),
                    receive_bucket: 0,
                    receive_seq: 3,
                    outcomes: vec![CommandOutcome::Ok(CommandEffect::Transferred {
                        target: AgentId(4),
                        supply: 5,
                        keys: BTreeMap::new(),
                    })],
                },
            ],
            ignored_duplicates: Vec::new(),
            unlocatable: Vec::new(),
            score2_delta: BTreeMap::new(),
        };
        audit.round(&report, &[]);
        // Roster: the default config's 3 initial agents per faction, no
        // reinforcements yet.
        assert_eq!(audit.enl.agent_rounds_total, 3);
        assert_eq!(audit.res.agent_rounds_total, 3);
        // ENL agent 0 submitted a Move, agent 1 passed, RES agent 0 passed.
        assert_eq!(audit.enl.agent_rounds_active, 1);
        assert_eq!(audit.res.agent_rounds_active, 0);
        assert_eq!(audit.enl.agent_rounds_passive(), 2);
        assert_eq!(audit.enl.passive_permille(), 666);
        // One zero-effect and one effective (15 HP) recharge; one transfer.
        assert_eq!(audit.enl.recharges_zero, 1);
        assert_eq!(audit.enl.recharges_effective, 1);
        assert_eq!(audit.enl.recharge_hp_restored, 15);
        assert_eq!(audit.res.transfers, 1);
        // A reinforcement event grows both factions' rosters.
        let reinforcement = Event {
            seq: 0,
            round: 2,
            kind: EventKind::Reinforcement {
                index: 1,
                position: arena_core::types::Coord::new(0, 0),
                count_per_faction: 1,
                first_agent_id: AgentId(10),
            },
        };
        audit.round(
            &RoundReport {
                submissions: vec![submission(Faction::Enl, 0, vec![Command::Attack])],
                slots: BTreeMap::new(),
                execution_order: vec![],
                ignored_duplicates: Vec::new(),
                unlocatable: Vec::new(),
                score2_delta: BTreeMap::new(),
            },
            &[reinforcement],
        );
        // The reinforcement landed in THIS round's settlement (resolver:
        // appended after the boundary round settles), so the round just
        // folded still counted the pre-round roster of 3 — the new agent
        // joins the denominator from the next round only.
        assert_eq!(audit.enl.agent_rounds_total, 6);
        assert_eq!(audit.res.agent_rounds_total, 6);
        audit.round(
            &RoundReport {
                submissions: vec![],
                slots: BTreeMap::new(),
                execution_order: vec![],
                ignored_duplicates: Vec::new(),
                unlocatable: Vec::new(),
                score2_delta: BTreeMap::new(),
            },
            &[],
        );
        assert_eq!(audit.enl.agent_rounds_total, 10);
        assert_eq!(audit.res.agent_rounds_total, 10);
    }

    /// The default config's full-match denominator: 3 initial agents, +1
    /// per faction at the settlement of rounds 100/200/300/400 (the
    /// resolver's `current_round % interval == 0` boundary), 500 rounds —
    /// exactly 2500 actionable agent-rounds per faction, NOT 2504 (the
    /// pre-fix fold counted each arrival in its arrival round).
    #[test]
    fn default_match_denominator_is_2500_per_faction() {
        use arena_core::types::AgentId;
        let cfg = GameConfig::default();
        assert_eq!(cfg.total_rounds, 500);
        assert_eq!(cfg.initial_agents_per_faction, 3);
        assert_eq!(cfg.reinforcement_interval_rounds, 100);
        assert_eq!(cfg.reinforcement_agents_per_faction, 1);
        let mut audit = MatchAudit::new(&cfg);
        let empty = RoundReport {
            submissions: vec![],
            slots: BTreeMap::new(),
            execution_order: vec![],
            ignored_duplicates: Vec::new(),
            unlocatable: Vec::new(),
            score2_delta: BTreeMap::new(),
        };
        for round in 1..=cfg.total_rounds {
            let mut new_events: &[Event] = &[];
            let mut owned;
            if round % cfg.reinforcement_interval_rounds == 0 && round < cfg.total_rounds {
                owned = vec![Event {
                    seq: round as u64,
                    round,
                    kind: EventKind::Reinforcement {
                        index: round / cfg.reinforcement_interval_rounds,
                        position: arena_core::types::Coord::new(0, 0),
                        count_per_faction: cfg.reinforcement_agents_per_faction,
                        first_agent_id: AgentId(10 + round),
                    },
                }];
                new_events = &owned;
            }
            audit.round(&empty, new_events);
        }
        assert_eq!(audit.enl.agent_rounds_total, 2500);
        assert_eq!(audit.res.agent_rounds_total, 2500);
    }

    /// verify_rows keeps the two sides of every pairing apart (the audit
    /// metrics are per-policy) and counts each leg once per side.
    #[test]
    fn verify_rows_split_sides_and_count_legs_once_each() {
        let plan = r21_verify_plan();
        let cells = expand(&plan).expect("plan expands");
        // Fake records for the two audited pairings only: every cell
        // played, ENL won, with synthetic audits.
        let records: Vec<BalanceMatchRecord> = cells
            .iter()
            .filter(|cell| !cell.pair_tag.ends_with("--idle"))
            .map(|cell| {
                let spec_a = plan
                    .policies
                    .iter()
                    .find(|s| s.policy_id == cell.policy_a)
                    .unwrap();
                let spec_b = plan
                    .policies
                    .iter()
                    .find(|s| s.policy_id == cell.policy_b)
                    .unwrap();
                let mut record = BalanceMatchRecord::config_failure(cell, spec_a, spec_b);
                record.result = BalanceOutcome::Played(if cell.policy_a_is_enl {
                    Outcome::EnlWin
                } else {
                    Outcome::ResWin
                });
                record
            })
            .collect();
        let mut audits = BTreeMap::new();
        for record in &records {
            audits.insert(
                record.index,
                MatchAudit {
                    enl: FactionAudit {
                        agent_rounds_total: 100,
                        agent_rounds_active: 90,
                        ..Default::default()
                    },
                    res: FactionAudit {
                        agent_rounds_total: 100,
                        agent_rounds_active: 50,
                        ..Default::default()
                    },
                    live_per_faction: 0,
                },
            );
        }
        let rows = verify_rows(&plan, &records, &audits);
        // Two matchups x two sides.
        assert_eq!(rows.len(), 4);
        let (pair, policy, row) = &rows[0];
        assert_eq!(pair, "builder-defender-r2--builder-disruptor-r2");
        assert_eq!(policy, "builder-defender-r2");
        assert_eq!(row.legs, 8);
        // Policy A is ENL in leg 0 and RES in leg 1: half of each side's
        // audit mix into its row (the synthetic audits are side-keyed).
        assert_eq!(row.audit.agent_rounds_total, 800);
        assert_eq!(
            row.audit.agent_rounds_active,
            4 * 90 + 4 * 50,
            "legs where A is ENL count the ENL audit, the others the RES audit"
        );
    }
}

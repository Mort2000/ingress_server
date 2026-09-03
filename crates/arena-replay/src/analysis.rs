//! Offline conflict analysis over a verified IARP (A-LATENCY, D70 realized).
//!
//! A **conflict** is two opposing-faction command streams in the SAME round
//! whose commands target the same game object with opposing intent
//! (R-RESOLVE-04):
//!
//! * **double CAPTURE** — both streams `CAPTURE` the same portal;
//! * **crossing LINKs** — one stream's `LINK(A,B)` segment geometrically
//!   intersects the other's `LINK(C,D)` (closed predicate, no shared
//!   portal endpoint — the D39 "illegal intersection" composition, so a
//!   shared-endpoint pair is NOT a conflict: both links can legally exist);
//! * **ATTACK vs RECHARGE** — an executed `ATTACK` damages a portal of the
//!   defender's faction while a defender stream `RECHARGE`s that portal in
//!   the same round.
//!
//! `TRANSFER` never counts (no opposing intent over a game object), and
//! same-faction command pairs never count.
//!
//! The pass FIRST verifies the whole log with [`crate::replay::replay`]
//! (the P6-GATE reference — an unverified archive is an error, never an
//! analysis input), then re-executes every round once more to recover the
//! per-command outcomes the resolver produced (the IARP round record
//! stores the intake facts and events, but per-command DROP reasons live
//! only in the re-executed `RoundReport`). Determinism guarantees the
//! re-executed outcomes are exactly the ones the recorded hashes pin.
//!
//! For each decisive conflict the report names the winner — the contender
//! whose effect executed — and classifies WHY order decided it: the winner
//! sat in a strictly earlier Priority Bucket, or both shared a bucket and
//! the winner won the within-bucket ordering (R-RESOLVE-03: execution is
//! bucket-ascending; inside a bucket the faction alternation starts at the
//! round's start faction and each faction's queue orders by
//! `(received_at_ms, receive_seq)`, so the earlier-executed contender
//! always holds `bucket <= loser.bucket`). A conflict where BOTH relevant
//! commands executed (an attack that damaged a portal the defender then
//! repaired) or NEITHER did (both dropped for their own reasons) is
//! **indecisive** and excluded from the win-rate aggregates.

use std::collections::BTreeMap;

use arena_core::geometry::segments_intersect;
use arena_core::resolve::{
    CommandEffect, CommandOutcome, DropReason, RoundReport, SubmittedCommandStream,
};
use arena_core::state::GameState;
use arena_core::types::{AgentId, Command, Coord, Faction, LinkId, PortalId};

use crate::format::{ReplayError, ReplayFile};
use crate::replay::{initial_spawns, replay};

/// Which kind of opposing intent the two streams had (R-RESOLVE-04).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ConflictKind {
    /// Both streams CAPTUREd the same neutral portal.
    DoubleCapture,
    /// The two LINK commands' segments geometrically cross (no shared
    /// endpoint) — only one of the links can exist.
    CrossingLinks,
    /// An executed ATTACK damaged a portal the opposing stream recharged
    /// in the same round.
    AttackVsRecharge,
}

impl ConflictKind {
    pub fn name(self) -> &'static str {
        match self {
            ConflictKind::DoubleCapture => "double-capture",
            ConflictKind::CrossingLinks => "crossing-links",
            ConflictKind::AttackVsRecharge => "attack-vs-recharge",
        }
    }
}

/// The game object the streams fought over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConflictObject {
    Portal(PortalId),
    /// The two proposed (canonical) links.
    Links(LinkId, LinkId),
}

/// One contender of a conflict: the stream's identity plus the receive
/// metadata and execution slot that decided the outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Side {
    pub faction: Faction,
    pub agent: AgentId,
    /// Authoritative priority bucket of the stream (R-RESOLVE-03).
    pub bucket: u64,
    pub received_at_ms: u64,
    pub receive_seq: u32,
    /// 0-based position in the round's fixed execution order.
    pub execution_position: usize,
}

/// Why the winner's ordering won.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BucketAdvantage {
    /// The winner sat in a strictly earlier Priority Bucket — the bucket
    /// layer of R-RESOLVE-03 decided the conflict.
    WinnerEarlierBucket,
    /// Both contenders shared a bucket; the winner won the within-bucket
    /// ordering — the faction alternation starting at the round's start
    /// faction (D19/D20), then `(received_at_ms, receive_seq)` inside
    /// each faction's queue.
    WinnerSameBucketFirstArrival,
}

/// One decisive same-round conflict with a known winner.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Conflict {
    pub round: u32,
    pub kind: ConflictKind,
    pub object: ConflictObject,
    /// The contender whose effect executed (the R-RESOLVE-04 "first
    /// executed" side; always the earlier execution position).
    pub winner: Side,
    pub loser: Side,
    /// Why the winner's order won.
    pub advantage: BucketAdvantage,
    /// The loser's DROP reason, when its command dropped.
    pub loser_drop: Option<DropReason>,
    /// Whether the loser's DROP was caused by the winner's own effect —
    /// it acted on the pre-emptied state (the exact R-RESOLVE-04 shape:
    /// `CaptureTargetNotNeutral`, `LinkIntersectsLink`, or a
    /// `RechargePortalNotOwned` on the just-neutralized portal).
    pub loser_pre_emptied: bool,
}

/// Aggregate win-rate bookkeeping over the decisive conflicts. Counts are
/// integers; rates are derived as permille (the house no-float rule).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ConflictAggregate {
    /// Decisive conflicts (exactly one contender's effect survived).
    pub decisive: u64,
    /// Conflicts whose opposing intents both executed or both dropped —
    /// recorded for honesty, excluded from every rate below.
    pub indecisive: u64,
    /// Decisive conflicts per kind.
    pub by_kind: BTreeMap<ConflictKind, u64>,
    /// Winner sat in a strictly earlier bucket.
    pub winner_earlier_bucket: u64,
    /// Winner shared the loser's bucket and arrived first.
    pub winner_same_bucket_first_arrival: u64,
    /// Decisive conflicts where the loser dropped ON the winner's effect
    /// (acted on the pre-emptied state).
    pub loser_pre_empted: u64,
}

/// The analysis result for one replay.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConflictReport {
    pub game_seed: u64,
    /// Rounds the verified log carried.
    pub rounds_analyzed: u32,
    /// Decisive conflicts, sorted by (round, kind, winner agent).
    pub conflicts: Vec<Conflict>,
    pub aggregate: ConflictAggregate,
}

impl ConflictReport {
    /// Integer permille of `part` within `total` (0 when total is 0).
    pub fn permille(part: u64, total: u64) -> u64 {
        if total == 0 { 0 } else { (part * 1000) / total }
    }

    /// One-line human summary (the runner's `conflict-stats` subcommand
    /// prints this; std-only, integers and permille only).
    pub fn summary(&self) -> String {
        let a = &self.aggregate;
        let kind = |kind: ConflictKind| a.by_kind.get(&kind).copied().unwrap_or(0);
        format!(
            "rounds={} decisive={} indecisive={} | winner_earlier_bucket={} ({}‰) \
             winner_same_bucket_first_arrival={} ({}‰) loser_pre_empted={} ({}‰) | \
             by_kind: double-capture={} crossing-links={} attack-vs-recharge={}",
            self.rounds_analyzed,
            a.decisive,
            a.indecisive,
            a.winner_earlier_bucket,
            Self::permille(a.winner_earlier_bucket, a.decisive),
            a.winner_same_bucket_first_arrival,
            Self::permille(a.winner_same_bucket_first_arrival, a.decisive),
            a.loser_pre_empted,
            Self::permille(a.loser_pre_empted, a.decisive),
            kind(ConflictKind::DoubleCapture),
            kind(ConflictKind::CrossingLinks),
            kind(ConflictKind::AttackVsRecharge),
        )
    }
}

/// Verifies the log end to end, then analyzes every round for conflicts.
/// An unverified archive is a [`ReplayError`], never an analysis result.
pub fn analyze_conflicts(log: &ReplayFile) -> Result<ConflictReport, ReplayError> {
    // Full verification first (P6-GATE reference; the D88 export
    // convention): every per-round hash, delta, event, and intake fact
    // plus the trailer.
    replay(log)?;

    let reports = reexecute(log)?;
    let detector = Detector {
        portal_pos: log
            .header
            .map
            .portals()
            .iter()
            .map(|portal| portal.position)
            .collect(),
    };
    let mut conflicts = Vec::new();
    let mut aggregate = ConflictAggregate::default();
    for (record, report) in log.round_records().zip(&reports) {
        detector.round(
            record.round,
            &record.submissions,
            report,
            &mut conflicts,
            &mut aggregate,
        );
    }

    conflicts.sort_by_key(|conflict| (conflict.round, conflict.kind, conflict.winner.agent));
    // The per-kind map is rebuilt from the sorted list so the aggregate
    // cannot drift from the detailed records.
    aggregate.by_kind.clear();
    for conflict in &conflicts {
        *aggregate.by_kind.entry(conflict.kind).or_insert(0) += 1;
    }
    Ok(ConflictReport {
        game_seed: log.header.game_seed,
        rounds_analyzed: reports.len() as u32,
        conflicts,
        aggregate,
    })
}

/// Convenience: decode + verify + analyze in one step.
pub fn analyze_conflicts_bytes(bytes: &[u8]) -> Result<ConflictReport, ReplayError> {
    analyze_conflicts(&crate::format::decode(bytes)?)
}

/// Re-executes the match exactly like [`crate::replay::replay`] (which has
/// already verified it) but keeps every `RoundReport` so the analysis can
/// read the per-command outcomes.
fn reexecute(log: &ReplayFile) -> Result<Vec<RoundReport>, ReplayError> {
    let header = &log.header;
    let enl = initial_spawns(&header.config, Faction::Enl, &header.enl_spawns)?;
    let res = initial_spawns(&header.config, Faction::Res, &header.res_spawns)?;
    let mut state = GameState::new(header.config, &header.map, &enl, &res).map_err(|_| {
        ReplayError::SetupInvalid {
            what: "state construction",
        }
    })?;
    let resolver = arena_core::resolve::Resolver::with_seed(header.game_seed);
    let mut reports = Vec::new();
    for record in log.round_records() {
        let report = resolver.run_round(&mut state, &record.submissions);
        state.advance_round();
        reports.push(report);
    }
    Ok(reports)
}

/// One executed stream aligned with its submission's commands and receive
/// metadata at its fixed execution position.
#[derive(Clone, Copy)]
struct StreamView<'a> {
    side: Side,
    commands: &'a [Command],
    outcomes: &'a [CommandOutcome],
}

impl<'a> StreamView<'a> {
    fn outcomes(&self) -> impl Iterator<Item = (&'a Command, &'a CommandOutcome)> {
        self.commands.iter().zip(self.outcomes.iter())
    }
}

/// The per-command facts one conflict detector pairing needs.
#[derive(Clone, Copy)]
struct Intent<'a> {
    stream: StreamView<'a>,
    /// The command's DROP reason, if it dropped.
    drop: Option<&'a DropReason>,
    /// Whether the command's effect executed (Ok).
    executed: bool,
}

impl<'a> Intent<'a> {
    fn new(stream: StreamView<'a>, outcome: &'a CommandOutcome) -> Intent<'a> {
        Intent {
            stream,
            drop: match outcome {
                CommandOutcome::Drop(reason) => Some(reason),
                CommandOutcome::Ok(_) => None,
            },
            executed: matches!(outcome, CommandOutcome::Ok(_)),
        }
    }
}

struct Detector {
    portal_pos: Vec<Coord>,
}

impl Detector {
    fn round(
        &self,
        round: u32,
        submissions: &[SubmittedCommandStream],
        report: &RoundReport,
        conflicts: &mut Vec<Conflict>,
        aggregate: &mut ConflictAggregate,
    ) {
        // Align every executed stream with its (first) submission: intake
        // ignores later duplicates, so the first submission per agent is
        // the executable one.
        let streams: Vec<StreamView<'_>> = report
            .execution_order
            .iter()
            .enumerate()
            .filter_map(|(position, executed)| {
                let submission = submissions
                    .iter()
                    .find(|submission| submission.agent_id == executed.agent_id)?;
                Some(StreamView {
                    side: Side {
                        faction: executed.faction,
                        agent: executed.agent_id,
                        bucket: executed.receive_bucket,
                        received_at_ms: submission.received_at_ms,
                        receive_seq: submission.receive_seq,
                        execution_position: position,
                    },
                    commands: &submission.commands,
                    outcomes: &executed.outcomes,
                })
            })
            .collect();

        self.detect_double_captures(round, &streams, conflicts, aggregate);
        self.detect_crossing_links(round, &streams, conflicts, aggregate);
        self.detect_attack_vs_recharge(round, &streams, conflicts, aggregate);
    }

    fn detect_double_captures(
        &self,
        round: u32,
        streams: &[StreamView<'_>],
        conflicts: &mut Vec<Conflict>,
        aggregate: &mut ConflictAggregate,
    ) {
        // Capture is a major action: at most one per stream, so per portal
        // every stream contributes at most one intent.
        let mut by_portal: BTreeMap<PortalId, Vec<Intent<'_>>> = BTreeMap::new();
        for stream in streams {
            for (command, outcome) in stream.outcomes() {
                if let Command::Capture { portal } = command {
                    by_portal
                        .entry(*portal)
                        .or_default()
                        .push(Intent::new(*stream, outcome));
                }
            }
        }
        for (portal, intents) in by_portal {
            if intents.len() < 2 {
                continue;
            }
            for left in 0..intents.len() {
                for right in (left + 1)..intents.len() {
                    if intents[left].stream.side.faction == intents[right].stream.side.faction {
                        continue; // opposing factions only
                    }
                    self.record_pair(
                        round,
                        ConflictKind::DoubleCapture,
                        ConflictObject::Portal(portal),
                        intents[left],
                        intents[right],
                        conflicts,
                        aggregate,
                    );
                }
            }
        }
    }

    fn detect_crossing_links(
        &self,
        round: u32,
        streams: &[StreamView<'_>],
        conflicts: &mut Vec<Conflict>,
        aggregate: &mut ConflictAggregate,
    ) {
        // LINK is auxiliary: a stream may carry several. Self-loops
        // (`LinkId::new -> None`) and unknown portals never create a link
        // and cannot cross anything — skipped at collection.
        let links: Vec<(Intent<'_>, LinkId, Coord, Coord)> = streams
            .iter()
            .flat_map(|stream| {
                stream.outcomes().filter_map(move |(command, outcome)| {
                    let Command::Link {
                        source_portal,
                        target_portal,
                    } = command
                    else {
                        return None;
                    };
                    let link = LinkId::new(*source_portal, *target_portal)?;
                    Some((
                        Intent::new(*stream, outcome),
                        link,
                        *self.portal_pos.get(link.min.0 as usize)?,
                        *self.portal_pos.get(link.max.0 as usize)?,
                    ))
                })
            })
            .collect();
        for left in 0..links.len() {
            for right in (left + 1)..links.len() {
                let (a, link_a, a_from, a_to) = links[left];
                let (b, link_b, b_from, b_to) = links[right];
                if a.stream.side.faction == b.stream.side.faction {
                    continue;
                }
                // A shared portal endpoint is not a conflict: both links
                // can legally exist (D39's `!shared_portal_endpoint`).
                if link_a.contains(link_b.min) || link_a.contains(link_b.max) {
                    continue;
                }
                if !segments_intersect(a_from, a_to, b_from, b_to) {
                    continue;
                }
                self.record_pair(
                    round,
                    ConflictKind::CrossingLinks,
                    ConflictObject::Links(link_a, link_b),
                    a,
                    b,
                    conflicts,
                    aggregate,
                );
            }
        }
    }

    fn detect_attack_vs_recharge(
        &self,
        round: u32,
        streams: &[StreamView<'_>],
        conflicts: &mut Vec<Conflict>,
        aggregate: &mut ConflictAggregate,
    ) {
        // Executed attacks paired with the portals they hit (a DROPPED
        // attack names no portal, so it cannot be part of this conflict by
        // construction — the detection premise is the executed damage).
        let mut attacks: Vec<(StreamView<'_>, PortalId)> = Vec::new();
        let mut recharges: Vec<(Intent<'_>, PortalId)> = Vec::new();
        for stream in streams {
            for (command, outcome) in stream.outcomes() {
                match command {
                    Command::Attack => {
                        if let CommandOutcome::Ok(CommandEffect::Attacked { hits }) = outcome {
                            for hit in hits {
                                attacks.push((*stream, hit.portal));
                            }
                        }
                    }
                    Command::Recharge { portal } => {
                        recharges.push((Intent::new(*stream, outcome), *portal));
                    }
                    _ => {}
                }
            }
        }
        for (attacker, portal) in &attacks {
            for (defender, recharged) in &recharges {
                if *portal != *recharged || attacker.side.faction == defender.stream.side.faction {
                    continue;
                }
                self.record_pair(
                    round,
                    ConflictKind::AttackVsRecharge,
                    ConflictObject::Portal(*portal),
                    // The attacker's effect executed by construction.
                    Intent {
                        stream: *attacker,
                        drop: None,
                        executed: true,
                    },
                    *defender,
                    conflicts,
                    aggregate,
                );
            }
        }
    }

    /// Shared tail for every detected pairing: classify decisive vs
    /// indecisive, order the contenders by execution position, and file
    /// the conflict.
    fn record_pair(
        &self,
        round: u32,
        kind: ConflictKind,
        object: ConflictObject,
        a: Intent<'_>,
        b: Intent<'_>,
        conflicts: &mut Vec<Conflict>,
        aggregate: &mut ConflictAggregate,
    ) {
        match (a.executed, b.executed) {
            // Both effects executed, or neither did: no winner.
            (true, true) | (false, false) => aggregate.indecisive += 1,
            _ => {
                let (winner, loser) =
                    if a.stream.side.execution_position <= b.stream.side.execution_position {
                        (a, b)
                    } else {
                        (b, a)
                    };
                debug_assert!(winner.executed && !loser.executed);
                let loser_pre_emptied = matches!(
                    loser.drop,
                    Some(
                        DropReason::CaptureTargetNotNeutral { .. }
                            | DropReason::LinkIntersectsLink { .. }
                            | DropReason::RechargePortalNotOwned { .. }
                    )
                );
                // R-RESOLVE-03: execution is bucket-ascending, so the
                // earlier-executed winner never sits in a later bucket.
                debug_assert!(winner.stream.side.bucket <= loser.stream.side.bucket);
                let advantage = if winner.stream.side.bucket < loser.stream.side.bucket {
                    BucketAdvantage::WinnerEarlierBucket
                } else {
                    BucketAdvantage::WinnerSameBucketFirstArrival
                };
                conflicts.push(Conflict {
                    round,
                    kind,
                    object,
                    winner: winner.stream.side,
                    loser: loser.stream.side,
                    advantage,
                    loser_drop: loser.drop.cloned(),
                    loser_pre_emptied,
                });
                aggregate.decisive += 1;
                match advantage {
                    BucketAdvantage::WinnerEarlierBucket => {
                        aggregate.winner_earlier_bucket += 1;
                    }
                    BucketAdvantage::WinnerSameBucketFirstArrival => {
                        aggregate.winner_same_bucket_first_arrival += 1;
                    }
                }
                if loser_pre_emptied {
                    aggregate.loser_pre_empted += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_shape_is_printable() {
        let report = ConflictReport {
            game_seed: 1,
            rounds_analyzed: 10,
            conflicts: Vec::new(),
            aggregate: ConflictAggregate::default(),
        };
        assert!(report.summary().starts_with("rounds=10 decisive=0"));
        assert_eq!(ConflictReport::permille(1, 4), 250);
        assert_eq!(ConflictReport::permille(1, 0), 0);
    }
}

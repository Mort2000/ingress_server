//! Resolver: intake, fair ordering, and per-command execution
//! (R-RESOLVE-01..04, P1-RESOLVE). The resolver decides validity against
//! the latest authoritative state; [`crate::state::GameState`] owns every
//! mutation.

use std::collections::{BTreeMap, BTreeSet};

use crate::event::EventKind;
use crate::geometry::{
    dist2, in_bounds, move_cost, point_on_segment, segments_intersect, within_closed_range,
};
use crate::rng::RandomOracle;
use crate::state::{GameState, faction_as_owner};
use crate::types::{AgentId, Command, Coord, Faction, FieldId, LinkId, Owner, PortalId, Score2};

/// One agent's ordered Command Stream as handed to the resolver by the
/// transport (or, in-process, by the harness). `received_at_ms` is the
/// authoritative monotonic receive time of the complete frame and
/// `receive_seq` is the per-faction strictly increasing arrival counter
/// (R-RESOLVE-01); neither is client-supplied.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SubmittedCommandStream {
    pub round: u32,
    pub faction: Faction,
    pub agent_id: AgentId,
    pub received_at_ms: u64,
    pub receive_seq: u32,
    pub commands: Vec<Command>,
}

/// Maps a monotonic receive time onto its Priority Bucket (R-RESOLVE-03):
/// `floor(received_at / PRIORITY_BUCKET)`. `bucket_ms` must be non-zero
/// (guaranteed by `GameConfig::validate`).
pub fn receive_bucket(received_at_ms: u64, bucket_ms: u32) -> u64 {
    received_at_ms / bucket_ms as u64
}

/// Ordering metadata for one executable stream; the caller keeps the index
/// into its own slice (`merge_order` returns those indices).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OrderEntry {
    pub faction: Faction,
    pub received_at_ms: u64,
    pub receive_seq: u32,
}

/// Pure, index-preserving merge of executable streams (R-RESOLVE-03, D19):
/// buckets ascending; inside a bucket each faction's queue is ordered by
/// `(received_at_ms, receive_seq)`; alternation starts at the round's start
/// faction (odd round => ENL, even => RES, D20) and restarts fresh in every
/// bucket; when one queue empties the other drains in order.
pub fn merge_order(entries: &[OrderEntry], round: u32, bucket_ms: u32) -> Vec<usize> {
    let start = start_faction_for_round(round);
    let mut buckets: BTreeMap<u64, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let bucket = receive_bucket(entry.received_at_ms, bucket_ms);
        let queues = buckets.entry(bucket).or_default();
        match entry.faction {
            Faction::Enl => queues.0.push(index),
            Faction::Res => queues.1.push(index),
        }
    }

    let mut order = Vec::with_capacity(entries.len());
    for (_bucket, (mut enl_queue, mut res_queue)) in buckets {
        let by_arrival = |a: &usize, b: &usize| {
            (entries[*a].received_at_ms, entries[*a].receive_seq)
                .cmp(&(entries[*b].received_at_ms, entries[*b].receive_seq))
        };
        enl_queue.sort_by(by_arrival);
        res_queue.sort_by(by_arrival);
        let (mut first, mut second) = if start == Faction::Enl {
            (enl_queue, res_queue)
        } else {
            (res_queue, enl_queue)
        };
        // Alternation starts from the start faction and restarts fresh in
        // every bucket; an emptied queue simply stops taking turns.
        while !first.is_empty() || !second.is_empty() {
            if let Some(index) = first.first().copied() {
                order.push(index);
                first.remove(0);
            }
            if let Some(index) = second.first().copied() {
                order.push(index);
                second.remove(0);
            }
        }
    }
    order
}

/// The faction whose queue settles first this round (D20): odd rounds
/// notify RES first, so settlement starts from ENL; even rounds reverse.
fn start_faction_for_round(round: u32) -> Faction {
    if round % 2 == 1 {
        Faction::Enl
    } else {
        Faction::Res
    }
}

/// Structural (submission-time, world-blind) stream rejection (D27): the
/// whole stream is rejected and nothing executes; the slot stays frozen and
/// no re-submission is possible (R-ACTION-01/02).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StreamRejection {
    /// More than `MAX_COMMANDS_PER_AGENT_ROUND` commands (majors count too).
    TooManyCommands { count: u32, max: u32 },
    /// More than one Major Action in one stream.
    MultipleMajorActions { count: u32 },
}

/// Structural stream check (D27, extracted as `pub` in M7 / D68 so the
/// external server can answer SUBMISSION_RESULT immediately with the SAME
/// rule text the Resolver applies): count-only and world-blind — the
/// command cap first, then the Major Action allowance. Pure refactor: the
/// [`Resolver::run_round`] intake calls this and produces identical
/// verdicts.
pub fn structural_check(commands: &[Command], max_commands: u32) -> Result<(), StreamRejection> {
    if commands.len() > max_commands as usize {
        return Err(StreamRejection::TooManyCommands {
            count: commands.len() as u32,
            max: max_commands,
        });
    }
    let majors = commands.iter().filter(|c| c.is_major()).count() as u32;
    if majors > 1 {
        return Err(StreamRejection::MultipleMajorActions { count: majors });
    }
    Ok(())
}

/// Envelope-locatability failures (D21): the submission cannot be pinned to
/// an agent slot, so it freezes nothing. All other transport concerns are M7.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unlocatable {
    WrongRound { submitted: u32, current: u32 },
    UnknownAgent(AgentId),
    FactionMismatch { claimed: Faction, actual: Faction },
}

/// Execution-time DROP reasons (R-RESOLVE-02); one variant per distinct
/// rule failure (D24). A DROP never rolls back earlier successes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DropReason {
    MoveOutOfBounds {
        destination: Coord,
    },
    MoveBudgetExceeded {
        requested: u64,
        remaining: u64,
    },
    PortalNotFound {
        portal: PortalId,
    },
    OutOfOperationRange {
        portal: PortalId,
    },
    CaptureTargetNotNeutral {
        portal: PortalId,
        owner: Owner,
    },
    AttackNoEnemyPortalsInRange,
    RechargePortalNotOwned {
        portal: PortalId,
        owner: Owner,
    },
    RechargePositionNotSatisfied {
        portal: PortalId,
    },
    RechargeInsufficientSupply {
        required: u32,
        available: u32,
    },
    /// HACK cooldown (R-HACK-02): `rounds_remaining` more rounds until the
    /// pair may hack again.
    HackCooldownActive {
        portal: PortalId,
        last_hack_round: u32,
        rounds_remaining: u32,
    },
    /// HACK per-epoch limit reached for this (agent, portal) pair
    /// (R-HACK-02).
    HackEpochLimitReached {
        portal: PortalId,
        limit: u32,
    },
    /// TRANSFER target does not exist (R-TRANSFER-01).
    TransferTargetNotFound {
        target: AgentId,
    },
    /// TRANSFER target belongs to the other faction (R-TRANSFER-01).
    TransferTargetWrongFaction {
        target: AgentId,
    },
    /// TRANSFER target beyond the closed `TRANSFER_RANGE` from the sender's
    /// current position (R-TRANSFER-01).
    TransferOutOfRange {
        target: AgentId,
    },
    /// Sender supply below the transferred amount (R-TRANSFER-01).
    TransferInsufficientSupply {
        requested: u32,
        available: u32,
    },
    /// Sender key balance for `portal` below the transferred count; portals
    /// are checked in ascending `PortalId` and the first shortfall reports
    /// (D35).
    TransferInsufficientKeys {
        portal: PortalId,
        requested: u32,
        available: u32,
    },
    /// `receiver_supply + transfer_supply` would exceed `MAX_SUPPLY` (u64
    /// sum, R-TRANSFER-01); `capacity` is the receiver's remaining headroom.
    TransferExceedsReceiverSupply {
        target: AgentId,
        requested: u32,
        capacity: u32,
    },
    /// LINK source and target are the same portal (R-LINK-01 rule 5).
    LinkSelfLoop {
        portal: PortalId,
    },
    /// LINK source or target is not owned by the acting faction (R-LINK-01
    /// rule 1); reported for the source first.
    LinkPortalNotOwned {
        portal: PortalId,
        owner: Owner,
    },
    /// The agent holds no key of the LINK target (R-LINK-01 rule 3).
    LinkMissingTargetKey {
        portal: PortalId,
    },
    /// Source-target distance exceeds the closed `MAX_LINK_DISTANCE`
    /// (R-LINK-01 rule 4; squared i64 compare, no square root).
    LinkDistanceExceeded {
        source: PortalId,
        target: PortalId,
    },
    /// The undirected pair already has a link, in either command direction
    /// (R-LINK-01 rule 6).
    LinkAlreadyExists {
        link: LinkId,
    },
    /// Another portal lies on the closed source-target segment, endpoints
    /// exempt (R-LINK-01 rule 7); the first offender in ascending `PortalId`
    /// reports.
    LinkBlockedByPortal {
        link: LinkId,
        portal: PortalId,
    },
    /// The new link illegally intersects an existing link (R-LINK-01 rule 8,
    /// R-LINK-02): a geometric intersection without a shared portal
    /// endpoint. The first offender in ascending `LinkId` reports.
    LinkIntersectsLink {
        link: LinkId,
        existing: LinkId,
    },
}

/// One enemy portal hit by an ATTACK, in ascending `PortalId` order (D25).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttackHit {
    pub portal: PortalId,
    pub hp_after: u32,
    pub neutralized: bool,
}

/// The world effect of one successfully executed command.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandEffect {
    Moved {
        from: Coord,
        to: Coord,
        cost: u64,
    },
    Captured {
        portal: PortalId,
    },
    Attacked {
        hits: Vec<AttackHit>,
    },
    Recharged {
        portal: PortalId,
        hp_before: u32,
        hp_after: u32,
    },
    /// A successful HACK (R-HACK-01): `keys_gained` keys in full (never
    /// clamped), `supply_granted` supply after the `MAX_SUPPLY` clamp
    /// (`0` when the agent already sat at the cap — a clamp is not a
    /// failure), and `supply_after` the post-grant balance. Never
    /// broadcasts (R-EVENT-01).
    Hacked {
        portal: PortalId,
        keys_gained: u32,
        supply_granted: u32,
        supply_after: u32,
    },
    /// A successful TRANSFER (R-TRANSFER-01): the moved `supply` and the
    /// moved non-zero key entries (echoing exactly what changed hands).
    /// Never broadcasts (R-EVENT-01).
    Transferred {
        target: AgentId,
        supply: u32,
        keys: BTreeMap<PortalId, u32>,
    },
    /// A successful LINK (R-LINK-01): the canonical undirected `link` and
    /// the fields it closed, ascending `FieldId` (R-FIELD-02 layering can
    /// close several at once).
    Linked {
        link: LinkId,
        fields: Vec<FieldId>,
    },
}

/// Per-command result: commands inside a stream are processed sequentially
/// and independently validated against the latest state (R-ACTION-01).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandOutcome {
    Ok(CommandEffect),
    Drop(DropReason),
}

/// Everything that happened to one executed stream, in its fixed order slot.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExecutedStream {
    pub faction: Faction,
    pub agent_id: AgentId,
    pub receive_bucket: u64,
    pub receive_seq: u32,
    pub outcomes: Vec<CommandOutcome>,
}

/// Final state of one agent's round slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotStatus {
    /// Structurally valid, non-empty, and executed; carries its bucket and
    /// per-faction arrival counter.
    Executed { bucket: u64, receive_seq: u32 },
    /// Structurally valid empty stream: an explicit PASS. Frozen, never
    /// queued (R-ACTION-01).
    Pass,
    /// Structurally invalid: frozen as rejected, never queued (ADR-0004).
    Rejected(StreamRejection),
    /// No submission existed when input closed: frozen PASS (D22).
    NotSubmitted,
}

/// Full settlement record for one round. `submissions` echoes the EXACT
/// input slice in submission order — including unlocatable and
/// ignored-duplicate submissions, each with its authoritative receive
/// metadata (D48, P1-REPLAY: the replay file stores this slice verbatim and
/// the replay engine re-executes it). `execution_order` is the fixed
/// stream order chosen before any command ran and is never reordered by
/// results (R-RESOLVE-03). `score2_delta` is the round-end field scoring
/// per faction (R-SCORE-01; P1-REPLAY per-round record).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RoundReport {
    pub submissions: Vec<SubmittedCommandStream>,
    pub slots: BTreeMap<AgentId, SlotStatus>,
    pub execution_order: Vec<ExecutedStream>,
    pub ignored_duplicates: Vec<AgentId>,
    pub unlocatable: Vec<Unlocatable>,
    pub score2_delta: BTreeMap<Faction, Score2>,
}

/// Round resolver, bound to the match's random oracle (D33): HACK draws its
/// key yield from a `RandomOracle` constructed with the match `game_seed`.
/// The oracle has no cursor, so the resolver stays stateless between rounds;
/// `GameState` never holds the seed or the oracle (D15).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Resolver {
    oracle: RandomOracle,
}

impl Resolver {
    /// Authoritative constructor: binds the resolver to the match seed.
    pub fn with_seed(game_seed: u64) -> Self {
        Resolver {
            oracle: RandomOracle::new(game_seed),
        }
    }

    /// Seed-0 compatibility constructor retained for the M2 fixtures; real
    /// matches construct via [`Resolver::with_seed`] from the Replay Header
    /// seed (D15).
    pub fn new() -> Self {
        Resolver::with_seed(0)
    }

    /// Settles one round (P1-RESOLVE steps 1-6):
    /// intake in submission order (locatability gate, freeze-first,
    /// structural check), auto-PASS for unfrozen agents, fair merge of the
    /// executable streams, sequential per-command execution against the
    /// latest state, then the round-end field scoring (R-SCORE-01, recorded
    /// as `score2_delta`) and the reinforcement boundary (R-SPAWN-02).
    /// Mutates `state`; never advances the round (the harness
    /// owns [`GameState::advance_round`]).
    ///
    /// # Panics
    ///
    /// A second call while round `state.round()` is already settled panics
    /// (D47): settling twice observably corrupts scores (`settle_scores`
    /// would run again). Advance the round between calls. Direct
    /// `settle_scores` calls are not guarded — it stays a primitive.
    pub fn run_round(
        &self,
        state: &mut GameState,
        submissions: &[SubmittedCommandStream],
    ) -> RoundReport {
        state.begin_settle_round();
        let mut report = RoundReport {
            submissions: submissions.to_vec(),
            slots: BTreeMap::new(),
            execution_order: Vec::new(),
            ignored_duplicates: Vec::new(),
            unlocatable: Vec::new(),
            score2_delta: BTreeMap::new(),
        };
        let bucket_ms = state.config().priority_bucket_ms;
        let current_round = state.round();
        let max_commands = state.config().max_commands_per_agent_round;
        let mut frozen: BTreeSet<AgentId> = BTreeSet::new();
        let mut queued: Vec<usize> = Vec::new();

        // --- Intake, in submission order ---------------------------------
        for (index, submission) in submissions.iter().enumerate() {
            // D21 locatability gate: envelope rejects freeze nothing.
            if submission.round != current_round {
                report.unlocatable.push(Unlocatable::WrongRound {
                    submitted: submission.round,
                    current: current_round,
                });
                continue;
            }
            let Some(agent) = state.agent(submission.agent_id) else {
                report
                    .unlocatable
                    .push(Unlocatable::UnknownAgent(submission.agent_id));
                continue;
            };
            if agent.faction != submission.faction {
                report.unlocatable.push(Unlocatable::FactionMismatch {
                    claimed: submission.faction,
                    actual: agent.faction,
                });
                continue;
            }

            // First locatable submission freezes the slot (ADR-0004).
            if frozen.contains(&submission.agent_id) {
                report.ignored_duplicates.push(submission.agent_id);
                continue;
            }
            frozen.insert(submission.agent_id);

            // Structural check (D27/D68): the shared, world-blind rule.
            if let Err(rejection) = structural_check(&submission.commands, max_commands) {
                report
                    .slots
                    .insert(submission.agent_id, SlotStatus::Rejected(rejection));
                continue;
            }
            if submission.commands.is_empty() {
                report.slots.insert(submission.agent_id, SlotStatus::Pass);
                continue;
            }
            queued.push(index);
        }

        // --- Fair merge, fixed before any execution -----------------------
        let entries: Vec<OrderEntry> = queued
            .iter()
            .map(|&i| OrderEntry {
                faction: submissions[i].faction,
                received_at_ms: submissions[i].received_at_ms,
                receive_seq: submissions[i].receive_seq,
            })
            .collect();
        for entry_index in merge_order(&entries, current_round, bucket_ms) {
            // merge_order addresses the `entries` slice; translate back to
            // the submission index through the queue built during intake.
            let submission = &submissions[queued[entry_index]];
            let outcomes = execute_stream(state, &self.oracle, submission);
            let bucket = receive_bucket(submission.received_at_ms, bucket_ms);
            report.slots.insert(
                submission.agent_id,
                SlotStatus::Executed {
                    bucket,
                    receive_seq: submission.receive_seq,
                },
            );
            report.execution_order.push(ExecutedStream {
                faction: submission.faction,
                agent_id: submission.agent_id,
                receive_bucket: bucket,
                receive_seq: submission.receive_seq,
                outcomes,
            });
        }

        // --- Round close: unfrozen agents become PASS (D22) ----------------
        let roster: Vec<AgentId> = state.agents().keys().copied().collect();
        for agent_id in roster {
            if !frozen.contains(&agent_id) {
                report.slots.insert(agent_id, SlotStatus::NotSubmitted);
            }
        }

        // --- Round-end settlement (P1-RESOLVE step 6) ----------------------
        // Field scoring first (R-SCORE-01: the delta lands in the report),
        // then the reinforcement boundary (R-SPAWN-02/D42). Agents appended
        // here join AFTER this round's slots and scoring — they act, score
        // nothing by construction, and first appear in the next round.
        report.score2_delta = state.settle_scores();
        let interval = state.config().reinforcement_interval_rounds;
        let total_rounds = state.config().total_rounds;
        if current_round % interval == 0 && current_round < total_rounds {
            let index = current_round / interval;
            let width_n = (state.config().map_width_cm as u32)
                .checked_add(1)
                .expect("map dimensions are capped by GameConfig::validate");
            let height_n = (state.config().map_height_cm as u32)
                .checked_add(1)
                .expect("map dimensions are capped by GameConfig::validate");
            // The interval index — NOT the hack epoch — is the stable index
            // of the reinforcement RNG domain (R-RNG-01, D42).
            let position = self.oracle.reinforcement_position(index, width_n, height_n);
            let count = state.config().reinforcement_agents_per_faction;
            let first_agent_id = state.add_reinforcement_agents(position, count);
            state.push_event(EventKind::Reinforcement {
                index,
                position,
                count_per_faction: count,
                first_agent_id,
            });
        }

        report
    }
}

impl Default for Resolver {
    /// Documented compatibility default: seed 0 (same as [`Resolver::new`]).
    fn default() -> Self {
        Resolver::new()
    }
}

/// Executes a stream's commands sequentially, each validated against the
/// latest authoritative state (R-ACTION-01: no interleaving, no rollback).
fn execute_stream(
    state: &mut GameState,
    oracle: &RandomOracle,
    submission: &SubmittedCommandStream,
) -> Vec<CommandOutcome> {
    submission
        .commands
        .iter()
        .map(|command| {
            execute_command(
                state,
                oracle,
                submission.agent_id,
                submission.faction,
                command,
            )
        })
        .collect()
}

fn execute_command(
    state: &mut GameState,
    oracle: &RandomOracle,
    agent_id: AgentId,
    faction: Faction,
    command: &Command,
) -> CommandOutcome {
    match command {
        Command::Move { destination } => execute_move(state, agent_id, *destination),
        Command::Attack => execute_attack(state, agent_id, faction),
        Command::Capture { portal } => execute_capture(state, agent_id, *portal),
        Command::Recharge { portal } => execute_recharge(state, agent_id, faction, *portal),
        Command::Hack { portal } => execute_hack(state, oracle, agent_id, *portal),
        Command::Transfer {
            target_agent,
            supply,
            keys,
        } => execute_transfer(state, agent_id, faction, *target_agent, *supply, keys),
        Command::Link {
            source_portal,
            target_portal,
        } => execute_link(state, agent_id, faction, *source_portal, *target_portal),
    }
}

/// MOVE (R-MOVE-01): bounds FIRST (an illegal destination must DROP, never
/// panic — M1 review carry-forward), then per-segment cost from the latest
/// position against the remaining budget. A DROP leaves both position and
/// budget untouched; later commands use the original position.
fn execute_move(state: &mut GameState, agent_id: AgentId, destination: Coord) -> CommandOutcome {
    let (width, height) = (state.config().map_width_cm, state.config().map_height_cm);
    if !in_bounds(destination, width, height) {
        return CommandOutcome::Drop(DropReason::MoveOutOfBounds { destination });
    }
    let from = state
        .agent(agent_id)
        .expect("intake verified the agent exists")
        .position;
    let cost = move_cost(from, destination);
    let budget = state.config().move_budget_cm as u64;
    let spent = state
        .tracker(agent_id)
        .expect("every agent has a tracker")
        .move_spent;
    let remaining = budget
        .checked_sub(spent)
        .expect("move_spent never exceeds MOVE_BUDGET");
    if cost > remaining {
        return CommandOutcome::Drop(DropReason::MoveBudgetExceeded {
            requested: cost,
            remaining,
        });
    }
    state.apply_move(agent_id, destination, cost);
    CommandOutcome::Ok(CommandEffect::Moved {
        from,
        to: destination,
        cost,
    })
}

/// CAPTURE (R-CAPTURE-01): portal exists, agent in closed operation range,
/// target Neutral. The major allowance is consumed even on DROP (D28).
fn execute_capture(state: &mut GameState, agent_id: AgentId, portal: PortalId) -> CommandOutcome {
    state.consume_major(agent_id);
    let Some(found) = state.portal(portal) else {
        return CommandOutcome::Drop(DropReason::PortalNotFound { portal });
    };
    let (portal_position, owner) = (found.position, found.owner);
    let agent_position = state
        .agent(agent_id)
        .expect("intake verified the agent exists")
        .position;
    let operation_range = state.config().portal_operation_range_cm;
    if !within_closed_range(agent_position, portal_position, operation_range) {
        return CommandOutcome::Drop(DropReason::OutOfOperationRange { portal });
    }
    if owner != Owner::Neutral {
        return CommandOutcome::Drop(DropReason::CaptureTargetNotNeutral { portal, owner });
    }
    state.apply_capture(agent_id, portal);
    CommandOutcome::Ok(CommandEffect::Captured { portal })
}

/// ATTACK (R-ATTACK-01/02): fixed damage to every enemy-owned portal within
/// closed operation range, in ascending `PortalId` order; per portal emit
/// `PORTAL_ATTACKED` then `PORTAL_NEUTRALIZED` when HP reaches 0 (D25).
/// Zero targets DROP (D23). Agents never take damage.
fn execute_attack(state: &mut GameState, agent_id: AgentId, faction: Faction) -> CommandOutcome {
    state.consume_major(agent_id);
    let position = state
        .agent(agent_id)
        .expect("intake verified the agent exists")
        .position;
    let operation_range = state.config().portal_operation_range_cm;
    let enemy_owner = faction_as_owner(faction.opposite());
    // Dense slice order == ascending PortalId.
    let targets: Vec<PortalId> = state
        .portals()
        .iter()
        .filter(|p| {
            p.owner == enemy_owner && within_closed_range(position, p.position, operation_range)
        })
        .map(|p| p.id)
        .collect();
    if targets.is_empty() {
        return CommandOutcome::Drop(DropReason::AttackNoEnemyPortalsInRange);
    }
    let damage = state.config().attack_damage;
    let mut hits = Vec::with_capacity(targets.len());
    for portal in targets {
        let (hp_after, neutralized) = state.apply_attack_damage(portal, damage);
        state.push_event(EventKind::PortalAttacked {
            faction,
            agent_id,
            portal,
            damage,
            hp_after,
        });
        if neutralized {
            let cascade = state.neutralize_portal(portal);
            state.push_event(EventKind::PortalNeutralized {
                faction,
                agent_id,
                portal,
            });
            for link in cascade.links {
                state.push_event(EventKind::LinkDestroyed {
                    faction,
                    agent_id,
                    link,
                });
            }
            for field in cascade.fields {
                state.push_event(EventKind::FieldDestroyed {
                    faction,
                    agent_id,
                    field,
                });
            }
        }
        hits.push(AttackHit {
            portal,
            hp_after,
            neutralized,
        });
    }
    CommandOutcome::Ok(CommandEffect::Attacked { hits })
}

/// RECHARGE (R-RECHARGE-01). Check order is contractual: portal exists ->
/// owned by own faction -> position (closed operation range OR at least one
/// key of that portal, never consumed) -> supply >= RECHARGE_COST. Emits no
/// event (R-EVENT-01). The major allowance is consumed even on DROP (D28).
fn execute_recharge(
    state: &mut GameState,
    agent_id: AgentId,
    faction: Faction,
    portal: PortalId,
) -> CommandOutcome {
    state.consume_major(agent_id);
    let Some(found) = state.portal(portal) else {
        return CommandOutcome::Drop(DropReason::PortalNotFound { portal });
    };
    let (portal_position, owner) = (found.position, found.owner);
    if owner != faction_as_owner(faction) {
        return CommandOutcome::Drop(DropReason::RechargePortalNotOwned { portal, owner });
    }
    let (agent_position, key_count, supply) = {
        let agent = state
            .agent(agent_id)
            .expect("intake verified the agent exists");
        (
            agent.position,
            agent.keys.get(&portal).copied().unwrap_or(0),
            agent.supply,
        )
    };
    let operation_range = state.config().portal_operation_range_cm;
    if !within_closed_range(agent_position, portal_position, operation_range) && key_count < 1 {
        return CommandOutcome::Drop(DropReason::RechargePositionNotSatisfied { portal });
    }
    let required = state.config().recharge_cost_supply;
    if supply < required {
        return CommandOutcome::Drop(DropReason::RechargeInsufficientSupply {
            required,
            available: supply,
        });
    }
    let amount = state.config().recharge_amount;
    let (hp_before, hp_after) = state.apply_recharge(agent_id, portal, amount);
    CommandOutcome::Ok(CommandEffect::Recharged {
        portal,
        hp_before,
        hp_after,
    })
}

/// HACK (R-HACK-01/02, D34). Check order is contractual: portal exists ->
/// closed operation range from the CURRENT position (no owner check —
/// neutral, own, and enemy portals are all legal targets) -> cooldown ->
/// epoch limit. Only a hack passing every check draws its single sample; a
/// DROPped hack mutates nothing, draws nothing, and advances no ordinal
/// (R-RNG-01). The supply grant clamps at `MAX_SUPPLY` (a clamp is not a
/// failure); keys never clamp; portal hp/owner never change; no event
/// broadcasts (R-EVENT-01).
fn execute_hack(
    state: &mut GameState,
    oracle: &RandomOracle,
    agent_id: AgentId,
    portal: PortalId,
) -> CommandOutcome {
    let Some(found) = state.portal(portal) else {
        return CommandOutcome::Drop(DropReason::PortalNotFound { portal });
    };
    let portal_position = found.position;
    let agent_position = state
        .agent(agent_id)
        .expect("intake verified the agent exists")
        .position;
    let operation_range = state.config().portal_operation_range_cm;
    if !within_closed_range(agent_position, portal_position, operation_range) {
        return CommandOutcome::Drop(DropReason::OutOfOperationRange { portal });
    }
    let tracker = state
        .hack_tracker(agent_id, portal)
        .copied()
        .unwrap_or_default();
    let round = state.round();
    let cooldown = state.config().hack_cooldown_rounds;
    if let Some(last) = tracker.last_hack_round {
        let elapsed = round
            .checked_sub(last)
            .expect("last_hack_round is never in the future");
        if elapsed < cooldown {
            return CommandOutcome::Drop(DropReason::HackCooldownActive {
                portal,
                last_hack_round: last,
                rounds_remaining: cooldown - elapsed,
            });
        }
    }
    let limit = state.config().hack_limit_per_epoch;
    if tracker.hack_count_epoch >= limit {
        return CommandOutcome::Drop(DropReason::HackEpochLimitReached { portal, limit });
    }

    // The single random draw (D3/D31): the ordinal is the pair's LIFETIME
    // count of successful hacks, so dropped attempts and epoch resets never
    // shift a later address.
    let min = state.config().hack_key_yield_min;
    let max = state.config().hack_key_yield_max;
    let span = max
        .checked_sub(min)
        .and_then(|diff| diff.checked_add(1))
        .expect("min >= 1 keeps the yield span within u32 (GameConfig::validate)");
    let address = oracle.hack_address(round, agent_id, portal, tracker.hack_ordinal);
    let keys_gained = min
        .checked_add(oracle.uniform(&address, span))
        .expect("draw < span keeps the yield within u32");
    let supply_yield = state.config().hack_supply_yield;
    let (supply_granted, supply_after) =
        state.apply_hack(agent_id, portal, keys_gained, supply_yield);
    CommandOutcome::Ok(CommandEffect::Hacked {
        portal,
        keys_gained,
        supply_granted,
        supply_after,
    })
}

/// TRANSFER (R-TRANSFER-01, D35). Check order is contractual: target exists
/// -> same faction -> closed `TRANSFER_RANGE` from CURRENT positions (either
/// side may already have moved this round) -> sender supply -> sender key
/// balance per portal in ascending `PortalId` (zero-count entries are
/// inert; the first shortfall reports) -> receiver capacity in u64. Any
/// failure DROPs with zero mutation; the sender is the stream owner, so no
/// payload field can redirect it. Self-transfers and zero-quantity
/// transfers are legal no-ops (still range/faction checked). Never
/// broadcasts (R-EVENT-01).
fn execute_transfer(
    state: &mut GameState,
    sender: AgentId,
    faction: Faction,
    target: AgentId,
    supply: u32,
    keys: &BTreeMap<PortalId, u32>,
) -> CommandOutcome {
    let Some(target_agent) = state.agent(target) else {
        return CommandOutcome::Drop(DropReason::TransferTargetNotFound { target });
    };
    if target_agent.faction != faction {
        return CommandOutcome::Drop(DropReason::TransferTargetWrongFaction { target });
    }
    let (target_position, target_supply) = (target_agent.position, target_agent.supply);
    let sender_agent = state
        .agent(sender)
        .expect("intake verified the agent exists");
    let transfer_range = state.config().transfer_range_cm;
    if !within_closed_range(sender_agent.position, target_position, transfer_range) {
        return CommandOutcome::Drop(DropReason::TransferOutOfRange { target });
    }
    if sender_agent.supply < supply {
        return CommandOutcome::Drop(DropReason::TransferInsufficientSupply {
            requested: supply,
            available: sender_agent.supply,
        });
    }
    for (&portal, &count) in keys {
        if count == 0 {
            continue;
        }
        let available = sender_agent.keys.get(&portal).copied().unwrap_or(0);
        if available < count {
            return CommandOutcome::Drop(DropReason::TransferInsufficientKeys {
                portal,
                requested: count,
                available,
            });
        }
    }
    let max_supply = state.config().max_supply;
    let sum = (target_supply as u64)
        .checked_add(supply as u64)
        .expect("u64 sum of two u32 values");
    if sum > max_supply as u64 {
        let capacity = (max_supply as u64)
            .checked_sub(target_supply as u64)
            .expect("supply never exceeds MAX_SUPPLY") as u32;
        return CommandOutcome::Drop(DropReason::TransferExceedsReceiverSupply {
            target,
            requested: supply,
            capacity,
        });
    }
    state.apply_transfer(sender, target, supply, keys);
    let moved: BTreeMap<PortalId, u32> = keys
        .iter()
        .filter(|&(_, &count)| count > 0)
        .map(|(&portal, &count)| (portal, count))
        .collect();
    CommandOutcome::Ok(CommandEffect::Transferred {
        target,
        supply,
        keys: moved,
    })
}

/// LINK (R-LINK-01/02, D38/D39). Check order is contractual — the first
/// failing rule reports, one DROP variant per rule failure (D24):
/// source exists -> target exists -> distinct portals -> source owned ->
/// target owned -> agent within CLOSED `portal_operation_range` of the
/// SOURCE (the direction only pins the standing requirement and the key
/// spent) -> at least one key of the TARGET -> source-target distance
/// within the closed `MAX_LINK_DISTANCE` (squared i64 compare) -> the
/// undirected pair unused -> no OTHER portal on the closed segment
/// (endpoints exempt, first offender ascending `PortalId`) -> no illegal
/// intersection with an existing link (D39: `segments_intersect && !shared
/// portal endpoint`; the closed predicate already counts collinear overlap;
/// shared endpoints are legal and field interiors never block — R-LINK-02).
///
/// Success consumes exactly one target key, inserts the undirected edge,
/// derives the fields it closes (cached `area2`), and broadcasts
/// `LINK_CREATED` then one `FIELD_CREATED` per field ascending.
fn execute_link(
    state: &mut GameState,
    agent_id: AgentId,
    faction: Faction,
    source: PortalId,
    target: PortalId,
) -> CommandOutcome {
    let Some(source_portal) = state.portal(source) else {
        return CommandOutcome::Drop(DropReason::PortalNotFound { portal: source });
    };
    let Some(target_portal) = state.portal(target) else {
        return CommandOutcome::Drop(DropReason::PortalNotFound { portal: target });
    };
    let (source_position, source_owner) = (source_portal.position, source_portal.owner);
    let (target_position, target_owner) = (target_portal.position, target_portal.owner);
    let Some(link) = LinkId::new(source, target) else {
        return CommandOutcome::Drop(DropReason::LinkSelfLoop { portal: source });
    };
    let own_owner = faction_as_owner(faction);
    if source_owner != own_owner {
        return CommandOutcome::Drop(DropReason::LinkPortalNotOwned {
            portal: source,
            owner: source_owner,
        });
    }
    if target_owner != own_owner {
        return CommandOutcome::Drop(DropReason::LinkPortalNotOwned {
            portal: target,
            owner: target_owner,
        });
    }
    let agent_position = state
        .agent(agent_id)
        .expect("intake verified the agent exists")
        .position;
    let operation_range = state.config().portal_operation_range_cm;
    if !within_closed_range(agent_position, source_position, operation_range) {
        return CommandOutcome::Drop(DropReason::OutOfOperationRange { portal: source });
    }
    let key_count = state
        .agent(agent_id)
        .expect("intake verified the agent exists")
        .keys
        .get(&target)
        .copied()
        .unwrap_or(0);
    if key_count < 1 {
        return CommandOutcome::Drop(DropReason::LinkMissingTargetKey { portal: target });
    }
    let max_distance = state.config().max_link_distance_cm as i64;
    let max2 = max_distance
        .checked_mul(max_distance)
        .expect("bounded by GameConfig::validate");
    if dist2(source_position, target_position) > max2 {
        return CommandOutcome::Drop(DropReason::LinkDistanceExceeded { source, target });
    }
    if state.link_graph().link(link).is_some() {
        return CommandOutcome::Drop(DropReason::LinkAlreadyExists { link });
    }
    // Rule 7: any OTHER portal on the closed segment blocks; dense slice
    // order is ascending PortalId, so the first hit is the first offender.
    for portal in state.portals() {
        if portal.id == source || portal.id == target {
            continue;
        }
        if point_on_segment(portal.position, source_position, target_position) {
            return CommandOutcome::Drop(DropReason::LinkBlockedByPortal {
                link,
                portal: portal.id,
            });
        }
    }
    // Rule 8 (R-LINK-02): geometric intersection without a shared portal
    // endpoint. BTreeMap order is ascending LinkId, so the first hit is the
    // first offender. T-touch and interior-endpoint cases were already
    // rejected by the portal-on-segment rule above.
    for (&existing, _) in state.link_graph().links() {
        if link.contains(existing.min) || link.contains(existing.max) {
            continue; // shared portal endpoint is legal
        }
        let existing_min = state
            .portal(existing.min)
            .expect("dense portal ids")
            .position;
        let existing_max = state
            .portal(existing.max)
            .expect("dense portal ids")
            .position;
        if segments_intersect(source_position, target_position, existing_min, existing_max) {
            return CommandOutcome::Drop(DropReason::LinkIntersectsLink { link, existing });
        }
    }

    let (link, fields) = state.apply_link(agent_id, source, target);
    state.push_event(EventKind::LinkCreated {
        faction,
        agent_id,
        link,
    });
    for field in &fields {
        state.push_event(EventKind::FieldCreated {
            faction,
            agent_id,
            field: *field,
        });
    }
    CommandOutcome::Ok(CommandEffect::Linked { link, fields })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameConfig;
    use crate::map::MapDefinition;
    use crate::spawn::validate_initial_spawn_set;
    use crate::types::{Coord, Owner, Portal};

    // ------------------------------------------------------------------
    // Fixtures
    // ------------------------------------------------------------------

    fn base_config() -> GameConfig {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 30_000;
        cfg.map_height_cm = 30_000;
        cfg.portal_min_distance_cm = 4_000;
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;
        cfg.initial_agents_per_faction = 1;
        cfg
    }

    fn portal_fixture(id: u32, x: i32, y: i32, owner: Owner, hp: u32) -> Portal {
        Portal {
            id: PortalId(id),
            position: Coord::new(x, y),
            owner,
            hp,
        }
    }

    fn fixture_state(
        cfg: &GameConfig,
        portals: Vec<Portal>,
        enl: &[Coord],
        res: &[Coord],
    ) -> GameState {
        let mut full = cfg.clone();
        full.portal_count = portals.len() as u32;
        full.portal_cluster_count = portals.len().max(1) as u32;
        let map = MapDefinition::new(portals);
        let (enl_roster, res_roster) = GameState::initial_roster(&full);
        assert_eq!(
            enl_roster.len(),
            enl.len(),
            "fixture: ENL coordinates must cover the roster"
        );
        assert_eq!(
            res_roster.len(),
            res.len(),
            "fixture: RES coordinates must cover the roster"
        );
        let enl_entries: Vec<(AgentId, Coord)> = enl_roster
            .iter()
            .copied()
            .zip(enl.iter().copied())
            .collect();
        let res_entries: Vec<(AgentId, Coord)> = res_roster
            .iter()
            .copied()
            .zip(res.iter().copied())
            .collect();
        let enl_spawns = validate_initial_spawn_set(&enl_roster, &enl_entries, &full)
            .expect("fixture ENL spawns");
        let res_spawns = validate_initial_spawn_set(&res_roster, &res_entries, &full)
            .expect("fixture RES spawns");
        GameState::new(full, &map, &enl_spawns, &res_spawns).expect("fixture state")
    }

    /// Two neutral portals; ENL agent 0 sits exactly at P0's closed operation
    /// range boundary, RES agent 1 is 4_000 cm from P0.
    fn standard_state() -> GameState {
        fixture_state(
            &base_config(),
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000)],
            &[Coord::new(6_000, 10_000)],
        )
    }

    fn stream(
        round: u32,
        faction: Faction,
        agent: AgentId,
        received_at_ms: u64,
        seq: u32,
        commands: Vec<Command>,
    ) -> SubmittedCommandStream {
        SubmittedCommandStream {
            round,
            faction,
            agent_id: agent,
            received_at_ms,
            receive_seq: seq,
            commands,
        }
    }

    fn enl(commands: Vec<Command>) -> SubmittedCommandStream {
        stream(1, Faction::Enl, AgentId(0), 0, 0, commands)
    }

    fn res(commands: Vec<Command>) -> SubmittedCommandStream {
        stream(1, Faction::Res, AgentId(1), 0, 0, commands)
    }

    fn run(state: &mut GameState, submissions: &[SubmittedCommandStream]) -> RoundReport {
        Resolver::new().run_round(state, submissions)
    }

    fn moved_outcome(outcome: &CommandOutcome) -> (Coord, Coord, u64) {
        match outcome {
            CommandOutcome::Ok(CommandEffect::Moved { from, to, cost }) => (*from, *to, *cost),
            other => panic!("expected Moved, got {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // Checklist 5: global event sequence across rounds
    // ------------------------------------------------------------------

    #[test]
    fn event_seq_is_global_zero_based_strictly_increasing_across_rounds() {
        let mut state = standard_state();
        let capture = |faction, agent| {
            stream(
                1,
                faction,
                agent,
                0,
                0,
                vec![Command::Capture {
                    portal: PortalId(0),
                }],
            )
        };
        let _ = run(&mut state, &[capture(Faction::Enl, AgentId(0))]);
        state.advance_round();
        let _ = run(
            &mut state,
            &[stream(
                2,
                Faction::Res,
                AgentId(1),
                0,
                0,
                vec![Command::Attack],
            )],
        );
        let events = state.events();
        assert_eq!(events.len(), 2);
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event.seq, index as u64);
        }
        // Round-tagged correctly per generating round; the sequence keeps
        // growing across the round boundary (checklist 5).
        assert_eq!(events[0].round, 1);
        assert_eq!(events[1].round, 2);
    }

    // ------------------------------------------------------------------
    // Intake (checklist 6-12)
    // ------------------------------------------------------------------

    #[test]
    fn first_locatable_submission_freezes_and_duplicates_are_ignored() {
        let mut state = standard_state();
        let first = enl(vec![Command::Move {
            destination: Coord::new(12_000, 5_000),
        }]);
        let second = stream(
            1,
            Faction::Enl,
            AgentId(0),
            0,
            1,
            vec![Command::Move {
                destination: Coord::new(20_000, 5_000),
            }],
        );
        let report = run(&mut state, &[first, second]);

        assert_eq!(report.ignored_duplicates, vec![AgentId(0)]);
        assert!(report.unlocatable.is_empty());
        assert_eq!(report.execution_order.len(), 1);
        // The original commands executed, not the duplicate.
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(12_000, 5_000))
        );
        assert_eq!(
            report.slots.get(&AgentId(0)),
            Some(&SlotStatus::Executed {
                bucket: 0,
                receive_seq: 0
            })
        );
    }

    #[test]
    fn unlocatable_submissions_do_not_freeze_the_slot() {
        // Wrong round.
        let mut state = standard_state();
        let wrong_round = stream(5, Faction::Enl, AgentId(0), 0, 0, vec![Command::Attack]);
        let valid = enl(vec![Command::Capture {
            portal: PortalId(0),
        }]);
        let report = run(&mut state, &[wrong_round, valid]);
        assert_eq!(
            report.unlocatable,
            vec![Unlocatable::WrongRound {
                submitted: 5,
                current: 1
            }]
        );
        assert_eq!(
            report.execution_order.len(),
            1,
            "later valid submission still freezes"
        );

        // Unknown agent.
        let mut state = standard_state();
        let unknown = stream(1, Faction::Res, AgentId(99), 0, 0, vec![Command::Attack]);
        let valid = res(vec![Command::Capture {
            portal: PortalId(0),
        }]);
        let report = run(&mut state, &[unknown, valid]);
        assert_eq!(
            report.unlocatable,
            vec![Unlocatable::UnknownAgent(AgentId(99))]
        );
        assert!(!report.slots.contains_key(&AgentId(99)));
        assert_eq!(report.execution_order.len(), 1);

        // Faction mismatch: RES claims the ENL agent.
        let mut state = standard_state();
        let mismatch = stream(1, Faction::Res, AgentId(0), 0, 0, vec![Command::Attack]);
        let valid = enl(vec![Command::Capture {
            portal: PortalId(0),
        }]);
        let report = run(&mut state, &[mismatch, valid]);
        assert_eq!(
            report.unlocatable,
            vec![Unlocatable::FactionMismatch {
                claimed: Faction::Res,
                actual: Faction::Enl
            }]
        );
        assert_eq!(report.execution_order.len(), 1);
    }

    // M5 (D48, P1-REPLAY): the report echoes the EXACT input slice in
    // submission order — including unlocatable and ignored-duplicate
    // submissions, each with its authoritative receive metadata. The replay
    // file stores this slice verbatim and the replay engine re-executes it.
    #[test]
    fn round_report_submissions_echo_exact_input_slice() {
        let mut state = standard_state();
        let wrong_round = stream(5, Faction::Enl, AgentId(0), 0, 0, vec![Command::Attack]);
        let first = enl(vec![Command::Capture {
            portal: PortalId(0),
        }]);
        // A second locatable submission for the same agent: ignored, but
        // still part of the echoed slice.
        let duplicate = SubmittedCommandStream {
            round: 1,
            faction: Faction::Enl,
            agent_id: AgentId(0),
            received_at_ms: 7,
            receive_seq: 9,
            commands: vec![Command::Attack],
        };
        let report = run(&mut state, &[wrong_round.clone(), first, duplicate.clone()]);
        assert_eq!(report.submissions.len(), 3, "every submission echoed");
        assert_eq!(report.submissions[0], wrong_round, "unlocatable echoed");
        assert_eq!(
            report.submissions[2], duplicate,
            "ignored duplicate echoed with its receive metadata"
        );
        assert_eq!(
            report.submissions[2].received_at_ms, 7,
            "receive metadata travels"
        );
        assert_eq!(report.submissions[2].receive_seq, 9);
        // The echo is the input slice, in submission order, untouched.
        let ids: Vec<AgentId> = report.submissions.iter().map(|s| s.agent_id).collect();
        assert_eq!(ids, vec![AgentId(0), AgentId(0), AgentId(0)]);
    }

    #[test]
    fn command_count_boundary_rejects_whole_stream() {
        let mut cfg = base_config();
        cfg.initial_agents_per_faction = 2;
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 6_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        let stay = Coord::new(10_000, 5_000);
        let eleven = stream(
            1,
            Faction::Enl,
            AgentId(0),
            0,
            0,
            vec![Command::Move { destination: stay }; 11],
        );
        let ten = stream(
            1,
            Faction::Enl,
            AgentId(1),
            0,
            1,
            vec![
                Command::Move {
                    destination: Coord::new(10_000, 6_000)
                };
                10
            ],
        );
        let late_fix = stream(1, Faction::Enl, AgentId(0), 0, 2, vec![Command::Attack]);
        let report = run(&mut state, &[eleven, ten, late_fix]);

        // The 11-command stream is rejected, frozen, and never executes.
        assert_eq!(
            report.slots.get(&AgentId(0)),
            Some(&SlotStatus::Rejected(StreamRejection::TooManyCommands {
                count: 11,
                max: 10
            }))
        );
        assert_eq!(report.execution_order.len(), 1);
        assert_eq!(report.execution_order[0].agent_id, AgentId(1));
        // The rejected agent cannot fix things up (ADR-0004).
        assert_eq!(report.ignored_duplicates, vec![AgentId(0)]);
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(10_000, 5_000)),
            "no command of the rejected stream executed"
        );
        // Exactly 10 commands (the boundary) all execute.
        assert_eq!(report.execution_order[0].outcomes.len(), 10);
        assert!(
            report.execution_order[0]
                .outcomes
                .iter()
                .all(|o| matches!(o, CommandOutcome::Ok(CommandEffect::Moved { .. })))
        );
    }

    #[test]
    fn multiple_majors_rejects_and_zero_majors_is_fine() {
        let mut cfg = base_config();
        cfg.initial_agents_per_faction = 2;
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 6_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        let two_majors = stream(
            1,
            Faction::Enl,
            AgentId(0),
            0,
            0,
            vec![
                Command::Capture {
                    portal: PortalId(0),
                },
                Command::Attack,
            ],
        );
        let zero_majors = stream(
            1,
            Faction::Enl,
            AgentId(1),
            0,
            1,
            vec![
                Command::Move {
                    destination: Coord::new(10_001, 6_000),
                },
                Command::Move {
                    destination: Coord::new(10_000, 6_000),
                },
            ],
        );
        let report = run(&mut state, &[two_majors, zero_majors]);

        assert_eq!(
            report.slots.get(&AgentId(0)),
            Some(&SlotStatus::Rejected(
                StreamRejection::MultipleMajorActions { count: 2 }
            ))
        );
        // Nothing of the rejected stream executed.
        assert_eq!(
            state.portal(PortalId(0)).map(|p| p.owner),
            Some(Owner::Neutral)
        );
        assert!(
            !state
                .tracker(AgentId(0))
                .map(|t| t.major_used)
                .unwrap_or(false)
        );
        // The zero-major stream runs fine.
        assert_eq!(report.execution_order.len(), 1);
        assert_eq!(report.execution_order[0].agent_id, AgentId(1));
        assert_eq!(report.execution_order[0].outcomes.len(), 2);
    }

    #[test]
    fn empty_stream_is_explicit_pass_and_freezes() {
        let mut state = standard_state();
        let empty = enl(vec![]);
        let later = enl(vec![Command::Capture {
            portal: PortalId(0),
        }]);
        let report = run(&mut state, &[empty, later]);

        assert_eq!(report.slots.get(&AgentId(0)), Some(&SlotStatus::Pass));
        assert!(
            report.execution_order.is_empty(),
            "PASS never enters the queue"
        );
        assert_eq!(report.ignored_duplicates, vec![AgentId(0)]);
        assert_eq!(
            state.portal(PortalId(0)).map(|p| p.owner),
            Some(Owner::Neutral)
        );
    }

    #[test]
    fn agent_without_submission_auto_passes_at_round_close() {
        let mut state = standard_state();
        let report = run(&mut state, &[]);
        assert_eq!(
            report.slots.get(&AgentId(0)),
            Some(&SlotStatus::NotSubmitted)
        );
        assert_eq!(
            report.slots.get(&AgentId(1)),
            Some(&SlotStatus::NotSubmitted)
        );
        assert!(report.execution_order.is_empty());
    }

    #[test]
    fn all_drop_stream_keeps_its_ordering_slot() {
        let mut state = standard_state();
        let bad_move = enl(vec![Command::Move {
            destination: Coord::new(30_001, 0),
        }]);
        let capture = res(vec![Command::Capture {
            portal: PortalId(0),
        }]);
        let report = run(&mut state, &[bad_move, capture]);

        // Odd round: ENL settles first even though its whole stream DROPs.
        assert_eq!(report.execution_order.len(), 2);
        assert_eq!(report.execution_order[0].agent_id, AgentId(0));
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::MoveOutOfBounds {
                destination: Coord::new(30_001, 0)
            })
        );
        assert_eq!(report.execution_order[1].agent_id, AgentId(1));
        assert_eq!(
            report.execution_order[1].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Captured {
                portal: PortalId(0)
            })
        );
    }

    // ------------------------------------------------------------------
    // Ordering (checklist 13-18)
    // ------------------------------------------------------------------

    #[test]
    fn receive_bucket_floor_boundaries() {
        // Default PRIORITY_BUCKET is 5 ms.
        assert_eq!(receive_bucket(0, 5), 0);
        assert_eq!(receive_bucket(4, 5), 0);
        assert_eq!(receive_bucket(5, 5), 1);
        assert_eq!(receive_bucket(9, 5), 1); // 2*bucket_ms - 1
        assert_eq!(receive_bucket(10, 5), 2);
        // A one-millisecond bucket degenerates to the identity.
        assert_eq!(receive_bucket(0, 1), 0);
        assert_eq!(receive_bucket(1, 1), 1);
    }

    #[test]
    fn merge_order_sorts_buckets_ascending_first() {
        let entries = [
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 10,
                receive_seq: 0,
            },
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 3,
                receive_seq: 1,
            },
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 7,
                receive_seq: 2,
            },
        ];
        // Buckets 2, 0, 1: the bucket-0 entry runs before bucket 1 and 2
        // regardless of faction alternation.
        assert_eq!(merge_order(&entries, 1, 5), vec![1, 2, 0]);
    }

    #[test]
    fn merge_order_start_faction_follows_round_parity_both_directions() {
        let entries = [
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 0,
                receive_seq: 0,
            },
            OrderEntry {
                faction: Faction::Res,
                received_at_ms: 0,
                receive_seq: 0,
            },
        ];
        // Odd round: ENL queue starts. Even round: RES queue starts.
        assert_eq!(merge_order(&entries, 1, 5), vec![0, 1]);
        assert_eq!(merge_order(&entries, 3, 5), vec![0, 1]);
        assert_eq!(merge_order(&entries, 2, 5), vec![1, 0]);
        assert_eq!(merge_order(&entries, 4, 5), vec![1, 0]);
    }

    #[test]
    fn merge_order_unequal_queue_lengths_drain_in_order() {
        let entries = [
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 0,
                receive_seq: 0,
            },
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 1,
                receive_seq: 1,
            },
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 2,
                receive_seq: 2,
            },
            OrderEntry {
                faction: Faction::Res,
                received_at_ms: 0,
                receive_seq: 0,
            },
        ];
        // Odd round: ENL-0 -> RES-0 -> ENL-1 -> ENL-2.
        assert_eq!(merge_order(&entries, 1, 5), vec![0, 3, 1, 2]);
        // Even round: RES-0 -> ENL-0 -> ENL-1 -> ENL-2.
        assert_eq!(merge_order(&entries, 2, 5), vec![3, 0, 1, 2]);
    }

    #[test]
    fn merge_order_in_bucket_sequence_and_fresh_alternation_per_bucket() {
        // Within a bucket, (received_at_ms, receive_seq) orders each faction:
        // the entry with ms 2 (seq 5) precedes the entry with ms 4 (seq 1).
        let entries = [
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 4,
                receive_seq: 1,
            }, // 0: ENL bucket 0, late
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 2,
                receive_seq: 5,
            }, // 1: ENL bucket 0, early
            OrderEntry {
                faction: Faction::Res,
                received_at_ms: 0,
                receive_seq: 3,
            }, // 2: RES bucket 0
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 5,
                receive_seq: 0,
            }, // 3: ENL bucket 1
            OrderEntry {
                faction: Faction::Res,
                received_at_ms: 9,
                receive_seq: 7,
            }, // 4: RES bucket 1
        ];
        // Odd round: bucket 0 alternates ENL -> RES -> ENL (the early ENL
        // entry first), bucket 1 restarts with ENL.
        assert_eq!(merge_order(&entries, 1, 5), vec![1, 2, 0, 3, 4]);
        // Even round: every bucket restarts with RES instead.
        assert_eq!(merge_order(&entries, 2, 5), vec![2, 1, 0, 4, 3]);
    }

    #[test]
    fn merge_order_is_pure_and_index_preserving() {
        let entries = [
            OrderEntry {
                faction: Faction::Res,
                received_at_ms: 12,
                receive_seq: 0,
            },
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 3,
                receive_seq: 4,
            },
            OrderEntry {
                faction: Faction::Res,
                received_at_ms: 3,
                receive_seq: 1,
            },
            OrderEntry {
                faction: Faction::Enl,
                received_at_ms: 21,
                receive_seq: 0,
            },
        ];
        let first = merge_order(&entries, 1, 5);
        let second = merge_order(&entries, 1, 5);
        assert_eq!(first, second, "metadata-only function is pure");
        let mut sorted = first.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            vec![0, 1, 2, 3],
            "result is a permutation of the indices"
        );
        assert!(merge_order(&[], 1, 5).is_empty());
    }

    // ------------------------------------------------------------------
    // MOVE (checklist 19-23)
    // ------------------------------------------------------------------

    #[test]
    fn move_success_bills_each_segment_from_the_latest_position() {
        let mut state = standard_state();
        let report = run(
            &mut state,
            &[enl(vec![
                Command::Move {
                    destination: Coord::new(18_000, 5_000),
                },
                Command::Move {
                    destination: Coord::new(18_000, 9_000),
                },
                Command::Move {
                    destination: Coord::new(13_000, 9_000),
                },
            ])],
        );
        let stream_out = &report.execution_order[0];
        let costs: Vec<u64> = stream_out
            .outcomes
            .iter()
            .map(|o| moved_outcome(o).2)
            .collect();
        assert_eq!(costs, vec![8_000, 4_000, 5_000]);
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(13_000, 9_000))
        );
        assert_eq!(
            state.tracker(AgentId(0)).map(|t| t.move_spent),
            Some(17_000)
        );
    }

    #[test]
    fn move_out_of_bounds_drops_without_panic_or_side_effects() {
        let mut state = standard_state();
        let destination = Coord::new(30_001, 0);
        let report = run(
            &mut state,
            &[enl(vec![
                Command::Move { destination },
                Command::Capture {
                    portal: PortalId(0),
                },
            ])],
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert_eq!(
            outcomes[0],
            CommandOutcome::Drop(DropReason::MoveOutOfBounds { destination })
        );
        // Position AND budget untouched; the later CAPTURE ran from the
        // original position (in range) and succeeded.
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(10_000, 5_000))
        );
        assert_eq!(state.tracker(AgentId(0)).map(|t| t.move_spent), Some(0));
        assert_eq!(
            outcomes[1],
            CommandOutcome::Ok(CommandEffect::Captured {
                portal: PortalId(0)
            })
        );
    }

    #[test]
    fn move_budget_drop_leaves_budget_untouched_for_later_moves() {
        let mut state = standard_state();
        let report = run(
            &mut state,
            &[enl(vec![
                Command::Move {
                    destination: Coord::new(10_000, 24_999),
                }, // 19_999
                Command::Move {
                    destination: Coord::new(10_000, 29_999),
                }, // 5_000 > 1 remaining
                Command::Move {
                    destination: Coord::new(10_000, 25_000),
                }, // 1 <= 1 remaining
            ])],
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert_eq!(
            outcomes[1],
            CommandOutcome::Drop(DropReason::MoveBudgetExceeded {
                requested: 5_000,
                remaining: 1
            })
        );
        assert!(matches!(outcomes[0], CommandOutcome::Ok(_)));
        assert!(matches!(outcomes[2], CommandOutcome::Ok(_)));
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(10_000, 25_000))
        );
        assert_eq!(
            state.tracker(AgentId(0)).map(|t| t.move_spent),
            Some(20_000)
        );
    }

    #[test]
    fn move_costs_and_closed_map_edges_corners() {
        let mut cfg = base_config();
        cfg.move_budget_cm = 45_000;
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(0, 0)],
            &[Coord::new(29_000, 29_000)],
        );
        // Round 1: the (0,0)->(1,1) cost-2 example, a zero-cost same-point
        // move, and both closed x-axis edges.
        let report = run(
            &mut state,
            &[stream(
                1,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![
                    Command::Move {
                        destination: Coord::new(1, 1),
                    },
                    Command::Move {
                        destination: Coord::new(1, 1),
                    },
                    Command::Move {
                        destination: Coord::new(0, 5_000),
                    },
                    Command::Move {
                        destination: Coord::new(30_000, 5_000),
                    },
                ],
            )],
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert_eq!(moved_outcome(&outcomes[0]).2, 2);
        assert_eq!(moved_outcome(&outcomes[1]).2, 0);
        assert_eq!(moved_outcome(&outcomes[2]).2, 5_000);
        assert_eq!(moved_outcome(&outcomes[3]).2, 30_000);
        state.advance_round();

        // Round 2: the (0,0) corner is a legal destination.
        let _ = run(
            &mut state,
            &[stream(
                2,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![Command::Move {
                    destination: Coord::new(0, 0),
                }],
            )],
        );
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(0, 0))
        );
        state.advance_round();

        // Round 3: the far corner (30_000, 30_000).
        let _ = run(
            &mut state,
            &[stream(
                3,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![Command::Move {
                    destination: Coord::new(30_000, 30_000),
                }],
            )],
        );
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.position),
            Some(Coord::new(30_000, 30_000))
        );
    }

    #[test]
    fn move_then_capture_uses_the_new_position() {
        let mut state = fixture_state(
            &base_config(),
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(20_000, 20_000)],
            &[Coord::new(6_000, 10_000)],
        );
        // From (20_000, 20_000) the portal is out of range; after the move
        // to (10_000, 5_000) it is exactly at the closed boundary.
        let report = run(
            &mut state,
            &[enl(vec![
                Command::Move {
                    destination: Coord::new(10_000, 5_000),
                },
                Command::Capture {
                    portal: PortalId(0),
                },
            ])],
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert!(matches!(outcomes[0], CommandOutcome::Ok(_)));
        assert_eq!(
            outcomes[1],
            CommandOutcome::Ok(CommandEffect::Captured {
                portal: PortalId(0)
            })
        );
        assert_eq!(state.portal(PortalId(0)).map(|p| p.owner), Some(Owner::Enl));
    }

    // ------------------------------------------------------------------
    // CAPTURE (checklist 24-27)
    // ------------------------------------------------------------------

    #[test]
    fn capture_at_exact_closed_range_boundary_succeeds() {
        let mut state = standard_state();
        let report = run(
            &mut state,
            &[enl(vec![Command::Capture {
                portal: PortalId(0),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Captured {
                portal: PortalId(0)
            })
        );
        let portal = state.portal(PortalId(0)).expect("portal exists");
        assert_eq!(portal.owner, Owner::Enl);
        assert_eq!(portal.hp, 100, "capture sets PORTAL_MAX_HP");
        assert_eq!(
            state.events(),
            &[crate::event::Event {
                seq: 0,
                round: 1,
                kind: crate::event::EventKind::PortalCaptured {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    portal: PortalId(0)
                }
            }]
        );
    }

    #[test]
    fn capture_failure_modes_have_distinct_reasons_and_still_consume_major() {
        let mut cfg = base_config();
        cfg.initial_agents_per_faction = 3;
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Res, 100),
                portal_fixture(1, 4_000, 4_000, Owner::Enl, 100),
            ],
            &[
                Coord::new(16_000, 16_000), // agent 0: 8_485 cm from P0 -> out of range
                Coord::new(4_000, 8_000),   // agent 1: 4_000 cm from P1 (own portal)
                Coord::new(6_000, 10_000),  // agent 2: 4_000 cm from P0 (enemy portal)
            ],
            &[
                Coord::new(29_000, 29_000),
                Coord::new(29_000, 29_000),
                Coord::new(29_000, 29_000),
            ],
        );
        let report = run(
            &mut state,
            &[
                stream(
                    1,
                    Faction::Enl,
                    AgentId(0),
                    0,
                    0,
                    vec![Command::Capture {
                        portal: PortalId(0),
                    }],
                ),
                stream(
                    1,
                    Faction::Enl,
                    AgentId(1),
                    0,
                    1,
                    vec![Command::Capture {
                        portal: PortalId(1),
                    }],
                ),
                stream(
                    1,
                    Faction::Enl,
                    AgentId(2),
                    0,
                    2,
                    vec![Command::Capture {
                        portal: PortalId(0),
                    }],
                ),
            ],
        );
        assert_eq!(report.execution_order.len(), 3);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::OutOfOperationRange {
                portal: PortalId(0)
            })
        );
        assert_eq!(
            report.execution_order[1].outcomes[0],
            CommandOutcome::Drop(DropReason::CaptureTargetNotNeutral {
                portal: PortalId(1),
                owner: Owner::Enl
            })
        );
        assert_eq!(
            report.execution_order[2].outcomes[0],
            CommandOutcome::Drop(DropReason::CaptureTargetNotNeutral {
                portal: PortalId(0),
                owner: Owner::Res
            })
        );
        // D28: the major allowance is spent even on DROP.
        for agent in [AgentId(0), AgentId(1), AgentId(2)] {
            assert!(state.tracker(agent).map(|t| t.major_used).unwrap_or(false));
        }
    }

    #[test]
    fn capture_unknown_portal_is_execution_time_not_structural() {
        let mut state = standard_state();
        let report = run(
            &mut state,
            &[enl(vec![Command::Capture {
                portal: PortalId(7),
            }])],
        );
        // The stream itself executed (D27): unknown ids are world-state DROPs.
        assert_eq!(
            report.slots.get(&AgentId(0)),
            Some(&SlotStatus::Executed {
                bucket: 0,
                receive_seq: 0
            })
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::PortalNotFound {
                portal: PortalId(7)
            })
        );
    }

    #[test]
    fn capture_conflict_first_executor_wins() {
        let mut state = standard_state();
        let report = run(
            &mut state,
            &[
                enl(vec![Command::Capture {
                    portal: PortalId(0),
                }]),
                res(vec![Command::Capture {
                    portal: PortalId(0),
                }]),
            ],
        );
        // Odd round: ENL settles first and wins the portal (R-RESOLVE-04).
        assert_eq!(report.execution_order.len(), 2);
        assert_eq!(report.execution_order[0].faction, Faction::Enl);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Captured {
                portal: PortalId(0)
            })
        );
        assert_eq!(
            report.execution_order[1].outcomes[0],
            CommandOutcome::Drop(DropReason::CaptureTargetNotNeutral {
                portal: PortalId(0),
                owner: Owner::Enl
            })
        );
        assert_eq!(state.portal(PortalId(0)).map(|p| p.owner), Some(Owner::Enl));
        assert_eq!(
            state.events().len(),
            1,
            "only the winning capture broadcast"
        );
    }

    // ------------------------------------------------------------------
    // ATTACK (checklist 28-34)
    // ------------------------------------------------------------------

    /// P0 (10_000,10_000) and P2 (4_000,16_000) are ENL and within the RES
    /// spawn's range; P1 (25_000,25_000) is ENL but far; P3 (10_000,16_000)
    /// is RES-owned and in range.
    fn attack_fixture(damage: u32) -> (GameConfig, GameState) {
        let mut cfg = base_config();
        cfg.attack_damage = damage;
        let state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Enl, 100),
                portal_fixture(1, 25_000, 25_000, Owner::Enl, 100),
                portal_fixture(2, 4_000, 16_000, Owner::Enl, 100),
                portal_fixture(3, 10_000, 16_000, Owner::Res, 100),
            ],
            &[Coord::new(20_000, 5_000)],
            &[Coord::new(7_000, 13_000)],
        );
        (cfg, state)
    }

    #[test]
    fn attack_hits_every_enemy_portal_in_closed_range_only() {
        let (_cfg, mut state) = attack_fixture(20);
        let report = run(&mut state, &[res(vec![Command::Attack])]);
        let outcome = &report.execution_order[0].outcomes[0];
        let hits = match outcome {
            CommandOutcome::Ok(CommandEffect::Attacked { hits }) => hits,
            other => panic!("expected Attacked, got {other:?}"),
        };
        assert_eq!(
            hits,
            &vec![
                AttackHit {
                    portal: PortalId(0),
                    hp_after: 80,
                    neutralized: false
                },
                AttackHit {
                    portal: PortalId(2),
                    hp_after: 80,
                    neutralized: false
                },
            ]
        );
        // Beyond range, own, and neutral portals untouched.
        assert_eq!(state.portal(PortalId(1)).map(|p| p.hp), Some(100));
        assert_eq!(state.portal(PortalId(3)).map(|p| p.hp), Some(100));
        assert_eq!(state.events().len(), 2);
    }

    #[test]
    fn attack_damage_at_least_hp_neutralizes_with_adjacent_events() {
        let (_cfg, mut state) = attack_fixture(100);
        let report = run(&mut state, &[res(vec![Command::Attack])]);
        let hits = match &report.execution_order[0].outcomes[0] {
            CommandOutcome::Ok(CommandEffect::Attacked { hits }) => hits,
            other => panic!("expected Attacked, got {other:?}"),
        };
        // Ascending PortalId with neutralization flags.
        assert_eq!(
            hits,
            &vec![
                AttackHit {
                    portal: PortalId(0),
                    hp_after: 0,
                    neutralized: true
                },
                AttackHit {
                    portal: PortalId(2),
                    hp_after: 0,
                    neutralized: true
                },
            ]
        );
        for portal in [PortalId(0), PortalId(2)] {
            assert_eq!(
                state.portal(portal).map(|p| (p.owner, p.hp)),
                Some((Owner::Neutral, 0))
            );
        }
        // Events per portal stay adjacent: ATTACKED then NEUTRALIZED (D25).
        let kinds: Vec<_> = state.events().iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                crate::event::EventKind::PortalAttacked {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    portal: PortalId(0),
                    damage: 100,
                    hp_after: 0
                },
                crate::event::EventKind::PortalNeutralized {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    portal: PortalId(0)
                },
                crate::event::EventKind::PortalAttacked {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    portal: PortalId(2),
                    damage: 100,
                    hp_after: 0
                },
                crate::event::EventKind::PortalNeutralized {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    portal: PortalId(2)
                },
            ]
        );
    }

    #[test]
    fn attack_default_numbers_five_attacks_of_twenty() {
        let (_cfg, mut state) = attack_fixture(20);
        for round in 1..=5u32 {
            let _ = run(
                &mut state,
                &[stream(
                    round,
                    Faction::Res,
                    AgentId(1),
                    0,
                    0,
                    vec![Command::Attack],
                )],
            );
            if round < 5 {
                state.advance_round();
            }
        }
        // Four attacks leave HP 20 with the owner intact...
        // (checked mid-loop below via the final arithmetic instead)
        assert_eq!(
            state.portal(PortalId(0)).map(|p| (p.owner, p.hp)),
            Some((Owner::Neutral, 0))
        );
        assert_eq!(
            state.portal(PortalId(2)).map(|p| (p.owner, p.hp)),
            Some((Owner::Neutral, 0))
        );
        // 2 portals x 5 attacks + 2 neutralizations.
        assert_eq!(state.events().len(), 12);
        let attacked: Vec<u32> = state
            .events()
            .iter()
            .filter(|e| matches!(e.kind, crate::event::EventKind::PortalAttacked { .. }))
            .map(|e| e.round)
            .collect();
        assert_eq!(attacked, vec![1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
    }

    #[test]
    fn attack_four_strikes_leave_hp_twenty_fifth_neutralizes() {
        let (_cfg, mut state) = attack_fixture(20);
        for round in 1..=4u32 {
            let _ = run(
                &mut state,
                &[stream(
                    round,
                    Faction::Res,
                    AgentId(1),
                    0,
                    0,
                    vec![Command::Attack],
                )],
            );
            state.advance_round();
        }
        assert_eq!(
            state.portal(PortalId(0)).map(|p| (p.owner, p.hp)),
            Some((Owner::Enl, 20))
        );
        assert!(
            state
                .events()
                .iter()
                .all(|e| !matches!(e.kind, crate::event::EventKind::PortalNeutralized { .. }))
        );
        let _ = run(
            &mut state,
            &[stream(
                5,
                Faction::Res,
                AgentId(1),
                0,
                0,
                vec![Command::Attack],
            )],
        );
        assert_eq!(
            state.portal(PortalId(0)).map(|p| (p.owner, p.hp)),
            Some((Owner::Neutral, 0))
        );
    }

    #[test]
    fn attack_without_targets_drops_consumes_major_emits_nothing() {
        let mut state = standard_state();
        let report = run(&mut state, &[res(vec![Command::Attack])]);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::AttackNoEnemyPortalsInRange)
        );
        assert!(
            state
                .tracker(AgentId(1))
                .map(|t| t.major_used)
                .unwrap_or(false)
        );
        assert!(state.events().is_empty());
    }

    #[test]
    fn neutralized_portal_is_capturable_later_in_the_same_round() {
        let mut cfg = base_config();
        cfg.attack_damage = 100;
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Enl, 100),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000)],
            &[Coord::new(6_000, 10_000)],
        );
        state.advance_round(); // round 2: RES settles first
        let report = run(
            &mut state,
            &[
                stream(2, Faction::Res, AgentId(1), 0, 0, vec![Command::Attack]),
                stream(
                    2,
                    Faction::Enl,
                    AgentId(0),
                    0,
                    0,
                    vec![Command::Capture {
                        portal: PortalId(0),
                    }],
                ),
            ],
        );
        assert_eq!(report.execution_order.len(), 2);
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Attacked { .. })
        ));
        // R-ATTACK-02 immediacy: the update is visible to the next command.
        assert_eq!(
            report.execution_order[1].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Captured {
                portal: PortalId(0)
            })
        );
        assert_eq!(
            state.portal(PortalId(0)).map(|p| (p.owner, p.hp)),
            Some((Owner::Enl, 100))
        );
        let kinds: Vec<_> = state.events().iter().map(|e| e.kind).collect();
        assert!(matches!(
            kinds[0],
            crate::event::EventKind::PortalAttacked { .. }
        ));
        assert!(matches!(
            kinds[1],
            crate::event::EventKind::PortalNeutralized { .. }
        ));
        assert!(matches!(
            kinds[2],
            crate::event::EventKind::PortalCaptured { .. }
        ));
    }

    #[test]
    fn attack_never_damages_agents() {
        let (_cfg, mut state) = attack_fixture(20);
        let enl_before = state.agent(AgentId(0)).map(|a| (a.position, a.supply));
        let res_before = state.agent(AgentId(1)).map(|a| (a.position, a.supply));
        let _ = run(&mut state, &[res(vec![Command::Attack])]);
        assert_eq!(
            state.agent(AgentId(0)).map(|a| (a.position, a.supply)),
            enl_before
        );
        assert_eq!(
            state.agent(AgentId(1)).map(|a| (a.position, a.supply)),
            res_before
        );
    }

    // ------------------------------------------------------------------
    // RECHARGE (checklist 35-39)
    // ------------------------------------------------------------------

    fn recharge_fixture(owner: Owner, hp: u32, enl_position: Coord) -> GameState {
        fixture_state(
            &base_config(),
            vec![
                portal_fixture(0, 10_000, 10_000, owner, hp),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[enl_position],
            &[Coord::new(6_000, 10_000)],
        )
    }

    #[test]
    fn recharge_own_portal_in_range_heals_and_spends_supply() {
        let mut state = recharge_fixture(Owner::Enl, 40, Coord::new(10_000, 5_000));
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Recharged {
                portal: PortalId(0),
                hp_before: 40,
                hp_after: 60
            })
        );
        assert_eq!(state.portal(PortalId(0)).map(|p| p.hp), Some(60));
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(19));
        assert!(
            state.events().is_empty(),
            "RECHARGE never broadcasts (R-EVENT-01)"
        );
    }

    #[test]
    fn recharge_clamps_at_max_hp_and_still_spends_supply() {
        let mut state = recharge_fixture(Owner::Enl, 100, Coord::new(10_000, 5_000));
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Recharged {
                portal: PortalId(0),
                hp_before: 100,
                hp_after: 100
            })
        );
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(19));
    }

    #[test]
    fn recharge_remote_with_key_succeeds_and_keeps_the_key() {
        let mut state = recharge_fixture(Owner::Enl, 40, Coord::new(20_000, 20_000));
        state.grant_keys(AgentId(0), PortalId(0), 1);
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Recharged { hp_after: 60, .. })
        ));
        // Remote recharge requires holding a key but never consumes it.
        assert_eq!(
            state
                .agent(AgentId(0))
                .map(|a| a.keys.get(&PortalId(0)).copied()),
            Some(Some(1))
        );
    }

    #[test]
    fn recharge_failure_modes() {
        // Enemy or neutral portal: RechargePortalNotOwned.
        let mut state = recharge_fixture(Owner::Res, 100, Coord::new(10_000, 5_000));
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::RechargePortalNotOwned {
                portal: PortalId(0),
                owner: Owner::Res
            })
        );

        let mut state = recharge_fixture(Owner::Neutral, 0, Coord::new(10_000, 5_000));
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::RechargePortalNotOwned {
                portal: PortalId(0),
                owner: Owner::Neutral
            })
        );

        // Insufficient supply (in range, own portal).
        let mut cfg = base_config();
        cfg.initial_supply = 1;
        cfg.recharge_cost_supply = 5;
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Enl, 100),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000)],
            &[Coord::new(6_000, 10_000)],
        );
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::RechargeInsufficientSupply {
                required: 5,
                available: 1
            })
        );

        // Neither in range nor holding a key (own portal).
        let mut state = recharge_fixture(Owner::Enl, 100, Coord::new(20_000, 20_000));
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::RechargePositionNotSatisfied {
                portal: PortalId(0)
            })
        );
    }

    #[test]
    fn recharge_check_order_is_fixed_and_exact_supply_succeeds() {
        // exists -> owner: an unknown portal beats an enemy owner, a far
        // position, and missing supply.
        let mut cfg = base_config();
        cfg.initial_supply = 1;
        cfg.recharge_cost_supply = 5;
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Res, 100),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(20_000, 20_000)],
            &[Coord::new(6_000, 10_000)],
        );
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(9),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::PortalNotFound {
                portal: PortalId(9)
            })
        );

        // owner -> position -> supply: enemy-owned portal, far agent, and
        // insufficient supply all fail, but the owner check answers first.
        // (D47: one settle per round — the round advances between the two
        // DROP scenarios of this shared state.)
        state.advance_round();
        let report = run(
            &mut state,
            &[stream(
                2,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![Command::Recharge {
                    portal: PortalId(0),
                }],
            )],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::RechargePortalNotOwned {
                portal: PortalId(0),
                owner: Owner::Res
            })
        );

        // position -> supply: own portal, far agent, insufficient supply.
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Enl, 100),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(20_000, 20_000)],
            &[Coord::new(6_000, 10_000)],
        );
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::RechargePositionNotSatisfied {
                portal: PortalId(0)
            })
        );

        // Supply exactly equal to the cost succeeds and leaves zero.
        let mut exact_cfg = base_config();
        exact_cfg.initial_supply = 5;
        exact_cfg.recharge_cost_supply = 5;
        let mut state = fixture_state(
            &exact_cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Enl, 40),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000)],
            &[Coord::new(6_000, 10_000)],
        );
        let report = run(
            &mut state,
            &[enl(vec![Command::Recharge {
                portal: PortalId(0),
            }])],
        );
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Recharged { hp_after: 60, .. })
        ));
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(0));
    }

    // ------------------------------------------------------------------
    // Round lifecycle details
    // ------------------------------------------------------------------

    #[test]
    fn trackers_reset_only_between_rounds_even_on_drop() {
        let mut state = standard_state();
        // One successful move and one dropped capture in the same round.
        let _ = run(
            &mut state,
            &[enl(vec![
                Command::Move {
                    destination: Coord::new(12_000, 5_000),
                },
                Command::Capture {
                    portal: PortalId(9),
                }, // DROPs
            ])],
        );
        let tracker = state.tracker(AgentId(0)).expect("tracker exists");
        assert_eq!(tracker.move_spent, 2_000);
        assert!(tracker.major_used);
        state.advance_round();
        assert_eq!(state.tracker(AgentId(0)), Some(&Default::default()));
    }

    // ------------------------------------------------------------------
    // M3: HACK (checklist 4-11)
    // ------------------------------------------------------------------

    use crate::rng::RandomOracle;
    use crate::state::HackTracker;

    /// Golden-vector seed shared with the frozen RNG tests (rng.rs): the
    /// hack address (round 10, agent 7, portal 17, seq 1) has a pinned XOF.
    const GOLDEN_SEED: u64 = 0x0123_4567_89AB_CDEF;

    fn hack_cmd(portal: u32) -> Command {
        Command::Hack {
            portal: PortalId(portal),
        }
    }

    fn transfer_cmd(target: u32, supply: u32, keys: &[(u32, u32)]) -> Command {
        Command::Transfer {
            target_agent: AgentId(target),
            supply,
            keys: keys.iter().map(|&(p, c)| (PortalId(p), c)).collect(),
        }
    }

    fn run_with(
        game_seed: u64,
        state: &mut GameState,
        submissions: &[SubmittedCommandStream],
    ) -> RoundReport {
        Resolver::with_seed(game_seed).run_round(state, submissions)
    }

    /// Independent oracle derivation of the D3 key yield for the default
    /// config (`1 + uniform(addr, 3)`) at a fully named address.
    fn oracle_keys(game_seed: u64, round: u32, agent: u32, portal: u32, ordinal: u32) -> u32 {
        let oracle = RandomOracle::new(game_seed);
        let address = oracle.hack_address(round, AgentId(agent), PortalId(portal), ordinal);
        1 + oracle.uniform(&address, 3)
    }

    fn agent_snapshot(state: &GameState, id: AgentId) -> (Coord, u32, BTreeMap<PortalId, u32>) {
        let agent = state.agent(id).expect("agent exists");
        (agent.position, agent.supply, agent.keys.clone())
    }

    fn hacked(outcome: &CommandOutcome) -> (u32, u32, u32, u32) {
        match outcome {
            CommandOutcome::Ok(CommandEffect::Hacked {
                portal,
                keys_gained,
                supply_granted,
                supply_after,
            }) => (portal.0, *keys_gained, *supply_granted, *supply_after),
            other => panic!("expected Hacked, got {other:?}"),
        }
    }

    fn hack_stream(round: u32, agent: AgentId, portal: u32) -> SubmittedCommandStream {
        stream(
            round,
            Faction::Enl,
            agent,
            0,
            0,
            vec![Command::Hack {
                portal: PortalId(portal),
            }],
        )
    }

    fn advance_to(state: &mut GameState, round: u32) {
        while state.round() < round {
            state.advance_round();
        }
    }

    /// Three portals around the ENL spawn (10_000, 5_000): P0 neutral at
    /// (10_000, 10_000) (closed boundary), P1 ENL at (6_000, 6_000), P2 RES
    /// at (14_000, 6_000) — all within PORTAL_OPERATION_RANGE.
    fn hackable_state() -> GameState {
        fixture_state(
            &base_config(),
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 6_000, 6_000, Owner::Enl, 100),
                portal_fixture(2, 14_000, 6_000, Owner::Res, 100),
            ],
            &[Coord::new(10_000, 5_000)],
            &[Coord::new(24_000, 24_000)],
        )
    }

    // Checklist 4: HACK succeeds on neutral, own, AND enemy portals; keys
    // follow the oracle draw; supply grows by HACK_SUPPLY_YIELD; portals and
    // the event log stay untouched.
    #[test]
    fn hack_succeeds_for_neutral_own_and_enemy_portals() {
        let mut state = hackable_state();
        let report = run(
            &mut state,
            &[stream(
                1,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![hack_cmd(0), hack_cmd(1), hack_cmd(2)],
            )],
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert_eq!(outcomes.len(), 3);
        // Supply 20 -> 21 -> 22 -> 23, one yield per success, keys per draw.
        assert_eq!(hacked(&outcomes[0]), (0, oracle_keys(0, 1, 0, 0, 0), 1, 21));
        assert_eq!(hacked(&outcomes[1]), (1, oracle_keys(0, 1, 0, 1, 0), 1, 22));
        assert_eq!(hacked(&outcomes[2]), (2, oracle_keys(0, 1, 0, 2, 0), 1, 23));
        // Portal hp/owner NEVER change on a hack (R-HACK-01).
        for (owner, hp) in [(Owner::Neutral, 0), (Owner::Enl, 100), (Owner::Res, 100)] {
            let portal = state
                .portals()
                .iter()
                .find(|p| p.owner == owner)
                .expect("portal exists");
            assert_eq!((portal.owner, portal.hp), (owner, hp));
        }
        assert_eq!(
            state.agent(AgentId(0)).map(|a| a.supply),
            Some(23),
            "HACK yields supply"
        );
        // Every pair's tracker advanced exactly once.
        for portal in 0..=2u32 {
            assert_eq!(
                state.hack_tracker(AgentId(0), PortalId(portal)),
                Some(&HackTracker {
                    last_hack_round: Some(1),
                    hack_count_epoch: 1,
                    hack_ordinal: 1,
                })
            );
        }
        assert!(
            state.events().is_empty(),
            "HACK never broadcasts (R-EVENT-01)"
        );
    }

    // Checklist 5: golden vectors through the resolver — a fully hardcoded
    // literal plus independent in-test oracle derivations.
    #[test]
    fn hack_golden_vectors_through_the_resolver() {
        let mut cfg = base_config();
        cfg.initial_agents_per_faction = 8; // AgentId(7) is ENL
        let portals = golden_grid_portals();
        assert_eq!(portals.len(), 18, "PortalId(17) must exist");
        // Agents 0-6 sit far away; agent 7 spawns at the closed operation
        // range boundary of P17 (21_000, 25_000).
        let enl_spawns = golden_enl_spawns();
        let res_spawns = vec![Coord::new(2_000, 2_000); 8];
        let mut state = fixture_state(&cfg, portals, &enl_spawns, &res_spawns);

        // Round 7: ordinal 0.
        advance_to(&mut state, 7);
        let report = run_with(GOLDEN_SEED, &mut state, &[hack_stream(7, AgentId(7), 17)]);
        assert_eq!(
            hacked(&report.execution_order[0].outcomes[0]),
            (17, oracle_keys(GOLDEN_SEED, 7, 7, 17, 0), 1, 21)
        );

        // Round 10 (cooldown clean: 10 - 7 == 3): ordinal 1 — the frozen
        // golden address of rng.rs (uniform == 2, so keys == 3).
        advance_to(&mut state, 10);
        let report = run_with(GOLDEN_SEED, &mut state, &[hack_stream(10, AgentId(7), 17)]);
        let (_, keys_gained, _, supply_after) = hacked(&report.execution_order[0].outcomes[0]);
        assert_eq!(keys_gained, 3, "hardcoded golden literal");
        assert_eq!(keys_gained, oracle_keys(GOLDEN_SEED, 10, 7, 17, 1));
        assert_eq!(supply_after, 22);

        // The lifetime ordinal advanced to 2 and the draws replay byte-stable.
        assert_eq!(
            state.hack_tracker(AgentId(7), PortalId(17)),
            Some(&HackTracker {
                last_hack_round: Some(10),
                hack_count_epoch: 2,
                hack_ordinal: 2,
            })
        );

        // D33 wiring: the seed-0 compatibility resolver draws from its own
        // stream, never from the match seed's (same two-hack scenario).
        let mut state2 = fixture_state(
            &{
                let mut cfg = base_config();
                cfg.initial_agents_per_faction = 8;
                cfg
            },
            golden_grid_portals(),
            &golden_enl_spawns(),
            &vec![Coord::new(2_000, 2_000); 8],
        );
        advance_to(&mut state2, 7);
        let _ = run(&mut state2, &[hack_stream(7, AgentId(7), 17)]); // ordinal 0
        advance_to(&mut state2, 10);
        let report = run(&mut state2, &[hack_stream(10, AgentId(7), 17)]);
        let (_, keys_gained, _, _) = hacked(&report.execution_order[0].outcomes[0]);
        assert_eq!(
            keys_gained,
            oracle_keys(0, 10, 7, 17, 1),
            "the seed-0 resolver draws exactly the seed-0 oracle stream"
        );
    }

    /// The 6x3 grid of the golden test, factored for reuse: 18 portals,
    /// pairwise distance >= 4_000 cm, so dense `PortalId(17)` exists at
    /// (29_000, 25_000).
    fn golden_grid_portals() -> Vec<Portal> {
        let mut portals = Vec::new();
        let mut id = 0u32;
        for &y in &[5_000i32, 15_000, 25_000] {
            for &x in &[1_000i32, 7_000, 13_000, 19_000, 25_000, 29_000] {
                portals.push(portal_fixture(id, x, y, Owner::Neutral, 0));
                id += 1;
            }
        }
        portals
    }

    fn golden_enl_spawns() -> Vec<Coord> {
        let mut spawns = vec![Coord::new(1_000, 1_000); 7];
        spawns.push(Coord::new(29_000, 20_000));
        spawns
    }

    // Checklist 6: out-of-operation-range HACK DROPs, reusing the CAPTURE
    // range reason, drawing nothing; the exact boundary succeeds (closed).
    #[test]
    fn hack_range_drop_and_closed_boundary() {
        // standard_state puts the ENL agent exactly 5_000 cm from P0: the
        // closed boundary of PORTAL_OPERATION_RANGE.
        let mut state = standard_state();
        let report = run(&mut state, &[enl(vec![hack_cmd(0)])]);
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Hacked { .. })
        ));

        // One centimetre further: DROP, no sample, no tracker, no keys.
        let mut far = fixture_state(
            &base_config(),
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 4_999)], // 5_001 cm from P0
            &[Coord::new(6_000, 10_000)],
        );
        let report = run(&mut far, &[enl(vec![hack_cmd(0)])]);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::OutOfOperationRange {
                portal: PortalId(0)
            })
        );
        assert!(
            far.agent(AgentId(0))
                .map(|a| a.keys.is_empty() && a.supply == far.config().initial_supply)
                .unwrap_or(false),
            "a dropped hack grants nothing"
        );
        assert_eq!(
            far.hack_tracker(AgentId(0), PortalId(0)),
            None,
            "a dropped hack advances no tracker (no sample drawn)"
        );
    }

    // Checklist 7: the R-HACK-02 spec example — hack Round 10, Rounds 11/12
    // DROP with shrinking rounds_remaining, Round 13 succeeds again.
    #[test]
    fn hack_cooldown_spec_example_round_ten_to_thirteen() {
        let mut state = standard_state();
        advance_to(&mut state, 10);
        let report = run(&mut state, &[hack_stream(10, AgentId(0), 0)]);
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Hacked { .. })
        ));

        for (round, remaining) in [(11u32, 2u32), (12, 1)] {
            advance_to(&mut state, round);
            let report = run(&mut state, &[hack_stream(round, AgentId(0), 0)]);
            assert_eq!(
                report.execution_order[0].outcomes[0],
                CommandOutcome::Drop(DropReason::HackCooldownActive {
                    portal: PortalId(0),
                    last_hack_round: 10,
                    rounds_remaining: remaining,
                }),
                "round {round}"
            );
        }

        advance_to(&mut state, 13);
        let report = run(&mut state, &[hack_stream(13, AgentId(0), 0)]);
        assert_eq!(
            hacked(&report.execution_order[0].outcomes[0]),
            (0, oracle_keys(0, 13, 0, 0, 1), 1, 22),
            "Round 13 is the first legal re-hack, drawing ordinal 1"
        );
    }

    // Checklist 8: the per-epoch limit (HACK_LIMIT_PER_EPOCH = 5) reached on
    // the cooldown-forced cadence 1, 4, 7, 10, 13; a round failing BOTH
    // cooldown and limit reports cooldown (D34 order).
    #[test]
    fn hack_epoch_limit_and_cooldown_priority() {
        let mut state = standard_state();
        for round in [1u32, 4, 7, 10, 13] {
            advance_to(&mut state, round);
            let report = run(&mut state, &[hack_stream(round, AgentId(0), 0)]);
            assert!(
                matches!(
                    report.execution_order[0].outcomes[0],
                    CommandOutcome::Ok(CommandEffect::Hacked { .. })
                ),
                "round {round} is legal"
            );
        }
        assert_eq!(
            state
                .hack_tracker(AgentId(0), PortalId(0))
                .map(|t| t.hack_count_epoch),
            Some(5)
        );

        // Round 14 fails cooldown (14 - 13 == 1) AND the limit; cooldown
        // reports first.
        advance_to(&mut state, 14);
        let report = run(&mut state, &[hack_stream(14, AgentId(0), 0)]);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::HackCooldownActive {
                portal: PortalId(0),
                last_hack_round: 13,
                rounds_remaining: 2,
            })
        );

        // Round 16 is cooldown-clean (16 - 13 == 3) but over the limit.
        advance_to(&mut state, 16);
        let report = run(&mut state, &[hack_stream(16, AgentId(0), 0)]);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::HackEpochLimitReached {
                portal: PortalId(0),
                limit: 5,
            })
        );
        assert_eq!(
            state
                .hack_tracker(AgentId(0), PortalId(0))
                .map(|t| t.hack_ordinal),
            Some(5),
            "five successful hacks drew ordinals 0..=4"
        );
    }

    // Checklist 9: epoch reset immediacy — a hack on the last round of an
    // epoch does not block the first round of the next one, and the ordinal
    // keeps counting across the boundary (R-HACK-02, D31/D32).
    #[test]
    fn hack_epoch_reset_last_round_to_first_round() {
        let mut cfg = base_config();
        cfg.epoch_length = 4; // rounds 1-4 = epoch 0, 5-8 = epoch 1
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000)],
            &[Coord::new(6_000, 10_000)],
        );
        advance_to(&mut state, 4);
        let report = run(&mut state, &[hack_stream(4, AgentId(0), 0)]);
        assert_eq!(
            hacked(&report.execution_order[0].outcomes[0]),
            (0, oracle_keys(0, 4, 0, 0, 0), 1, 21)
        );

        // Round 5 opens epoch 1: the eager sweep wiped the cooldown anchor
        // (5 - 4 == 1 would have blocked) and the epoch count, but the draw
        // uses ordinal 1 — the lifetime counter survived.
        advance_to(&mut state, 5);
        let report = run(&mut state, &[hack_stream(5, AgentId(0), 0)]);
        assert_eq!(
            hacked(&report.execution_order[0].outcomes[0]),
            (0, oracle_keys(0, 5, 0, 0, 1), 1, 22),
            "Round 5 draws with ordinal 1, not a reset ordinal 0"
        );
        assert_eq!(
            state.hack_tracker(AgentId(0), PortalId(0)),
            Some(&HackTracker {
                last_hack_round: Some(5),
                hack_count_epoch: 1,
                hack_ordinal: 2,
            })
        );
    }

    // Checklist 10: a dropped hack consumes no sample and shifts no later
    // address (R-RNG-01) — identical ordinals, keys, and supplies with or
    // without the dropped attempt in between.
    #[test]
    fn dropped_hacks_consume_no_sample() {
        fn scenario(with_dropped_attempt: bool) -> GameState {
            let mut state = standard_state();
            advance_to(&mut state, 5);
            let report = run(&mut state, &[hack_stream(5, AgentId(0), 0)]);
            assert!(matches!(
                report.execution_order[0].outcomes[0],
                CommandOutcome::Ok(CommandEffect::Hacked { .. })
            ));
            if with_dropped_attempt {
                advance_to(&mut state, 6);
                let report = run(&mut state, &[hack_stream(6, AgentId(0), 0)]);
                assert_eq!(
                    report.execution_order[0].outcomes[0],
                    CommandOutcome::Drop(DropReason::HackCooldownActive {
                        portal: PortalId(0),
                        last_hack_round: 5,
                        rounds_remaining: 2,
                    })
                );
            }
            advance_to(&mut state, 8);
            let report = run(&mut state, &[hack_stream(8, AgentId(0), 0)]);
            let (_, keys_gained, _, _) = hacked(&report.execution_order[0].outcomes[0]);
            assert_eq!(
                keys_gained,
                oracle_keys(0, 8, 0, 0, 1),
                "Round 8 draws ordinal 1 in both runs"
            );
            state
        }

        let clean = scenario(false);
        let dropped = scenario(true);
        assert_eq!(
            clean.hack_tracker(AgentId(0), PortalId(0)),
            dropped.hack_tracker(AgentId(0), PortalId(0)),
            "same ordinals with and without the dropped attempt"
        );
        assert_eq!(
            agent_snapshot(&clean, AgentId(0)),
            agent_snapshot(&dropped, AgentId(0))
        );
    }

    // Checklist 11: a hack at MAX_SUPPLY succeeds with full keys, zero
    // granted supply, and a fully advanced tracker (the clamp is not a
    // failure, D34).
    #[test]
    fn hack_supply_clamp_at_max() {
        let mut cfg = base_config();
        cfg.initial_supply = 100; // == MAX_SUPPLY
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000)],
            &[Coord::new(6_000, 10_000)],
        );
        let report = run(&mut state, &[enl(vec![hack_cmd(0)])]);
        let expected_keys = oracle_keys(0, 1, 0, 0, 0);
        assert_eq!(
            hacked(&report.execution_order[0].outcomes[0]),
            (0, expected_keys, 0, 100)
        );
        assert_eq!(
            state
                .agent(AgentId(0))
                .map(|a| a.keys.get(&PortalId(0)).copied()),
            Some(Some(expected_keys)),
            "keys are never clamped"
        );
        assert_eq!(
            state.hack_tracker(AgentId(0), PortalId(0)),
            Some(&HackTracker {
                last_hack_round: Some(1),
                hack_count_epoch: 1,
                hack_ordinal: 1,
            }),
            "a clamped hack still updates cooldown/count/ordinal"
        );
    }

    // ------------------------------------------------------------------
    // M3: TRANSFER (checklist 12-16)
    // ------------------------------------------------------------------

    /// ENL A0 at (10_000, 5_000); P0 (10_000, 10_000) and P1 (25_000, 25_000).
    /// Both spawn slices must have equal length (one roster size per config).
    fn transfer_state(enl_spawns: &[Coord], res_spawns: &[Coord]) -> GameState {
        let mut cfg = base_config();
        cfg.initial_agents_per_faction = enl_spawns.len() as u32;
        fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            enl_spawns,
            res_spawns,
        )
    }

    // Checklist 12: the happy path moves supply and every key denomination
    // atomically, keeps the sender inventory canonical, merges on the
    // receiver, and broadcasts nothing.
    #[test]
    fn transfer_moves_supply_and_keys_atomically() {
        let mut state = transfer_state(
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 8_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        state.grant_keys(AgentId(0), PortalId(0), 3);
        state.grant_keys(AgentId(0), PortalId(1), 2);
        state.grant_keys(AgentId(1), PortalId(0), 1);
        let report = run(
            &mut state,
            &[enl(vec![transfer_cmd(1, 5, &[(0, 2), (1, 2)])])],
        );
        let mut expected_keys = BTreeMap::new();
        expected_keys.insert(PortalId(0), 2u32);
        expected_keys.insert(PortalId(1), 2u32);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Transferred {
                target: AgentId(1),
                supply: 5,
                keys: expected_keys,
            }),
            "the effect echoes the moved non-zero entries"
        );
        // Sender: supply 15, only the non-depleted key entry remains.
        let sender = agent_snapshot(&state, AgentId(0));
        assert_eq!(sender.1, 15);
        assert_eq!(
            sender.2,
            BTreeMap::from([(PortalId(0), 1u32)]),
            "entries depleted to 0 are removed (canonical inventory)"
        );
        // Receiver: supply 25, P0 merged 1+2, P1 fresh 2.
        let receiver = agent_snapshot(&state, AgentId(1));
        assert_eq!(receiver.1, 25);
        assert_eq!(
            receiver.2,
            BTreeMap::from([(PortalId(0), 3u32), (PortalId(1), 2u32)])
        );
        assert!(state.events().is_empty(), "TRANSFER never broadcasts");
        assert!(
            !state
                .tracker(AgentId(0))
                .map(|t| t.major_used)
                .unwrap_or(false),
            "TRANSFER is auxiliary: no major consumed"
        );
    }

    // Checklist 13: the DROP catalog — each failure leaves both agents
    // byte-identical.
    #[test]
    fn transfer_drop_catalog_zero_state_change() {
        // A0 sender (10_000, 5_000); A1 same faction in range (10_000, 8_000);
        // A2 same faction FAR (29_000, 29_000); A3/A4/A5 RES.
        let mut state = transfer_state(
            &[
                Coord::new(10_000, 5_000),
                Coord::new(10_000, 8_000),
                Coord::new(29_000, 29_000),
            ],
            &[
                Coord::new(6_000, 10_000),
                Coord::new(6_000, 11_000),
                Coord::new(6_000, 12_000),
            ],
        );
        state.grant_keys(AgentId(0), PortalId(0), 3);
        let before: Vec<_> = [0u32, 1, 2, 3, 4, 5]
            .iter()
            .map(|&id| agent_snapshot(&state, AgentId(id)))
            .collect();

        let cases: Vec<(Command, DropReason)> = vec![
            (
                transfer_cmd(99, 0, &[]),
                DropReason::TransferTargetNotFound {
                    target: AgentId(99),
                },
            ),
            (
                transfer_cmd(3, 1, &[]),
                DropReason::TransferTargetWrongFaction { target: AgentId(3) },
            ),
            (
                transfer_cmd(2, 1, &[]),
                DropReason::TransferOutOfRange { target: AgentId(2) },
            ),
            (
                transfer_cmd(1, 21, &[]),
                DropReason::TransferInsufficientSupply {
                    requested: 21,
                    available: 20,
                },
            ),
            (
                transfer_cmd(1, 1, &[(0, 5)]),
                DropReason::TransferInsufficientKeys {
                    portal: PortalId(0),
                    requested: 5,
                    available: 3,
                },
            ),
            (
                // Both denominations are short; the FIRST in ascending
                // PortalId order reports (D35).
                transfer_cmd(1, 0, &[(1, 5), (0, 5)]),
                DropReason::TransferInsufficientKeys {
                    portal: PortalId(0),
                    requested: 5,
                    available: 3,
                },
            ),
        ];
        for (command, reason) in cases {
            // D47: one settle per round — each DROP case gets a fresh round.
            let round = state.round();
            let report = run(
                &mut state,
                &[stream(round, Faction::Enl, AgentId(0), 0, 0, vec![command])],
            );
            assert_eq!(
                report.execution_order[0].outcomes[0],
                CommandOutcome::Drop(reason),
            );
            let after: Vec<_> = [0u32, 1, 2, 3, 4, 5]
                .iter()
                .map(|&id| agent_snapshot(&state, AgentId(id)))
                .collect();
            assert_eq!(before, after, "a dropped transfer mutates nothing");
            assert!(state.events().is_empty());
            state.advance_round();
        }
    }

    // Checklist 13 (boundary): TRANSFER_RANGE is closed — exact 5_000 cm
    // succeeds, 5_001 cm DROPs.
    #[test]
    fn transfer_range_is_closed() {
        let mut boundary = transfer_state(
            &[Coord::new(10_000, 5_000), Coord::new(15_000, 5_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        let report = run(&mut boundary, &[enl(vec![transfer_cmd(1, 0, &[])])]);
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Transferred { .. })
        ));

        let mut beyond = transfer_state(
            &[Coord::new(10_000, 5_000), Coord::new(15_001, 5_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        let report = run(&mut beyond, &[enl(vec![transfer_cmd(1, 0, &[])])]);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::TransferOutOfRange { target: AgentId(1) })
        );
    }

    // Checklist 13 (capacity): receiver supply + transfer must stay within
    // MAX_SUPPLY (u64 sum); exactly filling the capacity succeeds.
    #[test]
    fn transfer_receiver_capacity_boundary() {
        let mut cfg = base_config();
        cfg.initial_agents_per_faction = 2;
        cfg.initial_supply = 99; // both agents one below MAX_SUPPLY
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 8_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        // 99 + 2 > 100: DROP with the receiver's remaining headroom.
        let report = run(&mut state, &[enl(vec![transfer_cmd(1, 2, &[])])]);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::TransferExceedsReceiverSupply {
                target: AgentId(1),
                requested: 2,
                capacity: 1,
            })
        );
        // 99 + 1 == 100: exactly full is legal. (D47: fresh round for the
        // second scenario of this shared state.)
        state.advance_round();
        let report = run(
            &mut state,
            &[stream(
                2,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![transfer_cmd(1, 1, &[])],
            )],
        );
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Transferred { .. })
        ));
        assert_eq!(state.agent(AgentId(1)).map(|a| a.supply), Some(100));
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(98));
    }

    // Checklist 14: transfers measure CURRENT positions — the sender may
    // MOVE closer earlier in the same stream, and a target that already
    // moved this round is measured from its latest position.
    #[test]
    fn transfer_measures_the_senders_latest_position_after_a_move() {
        let mut state = transfer_state(
            &[Coord::new(10_000, 0), Coord::new(10_000, 6_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        // From the spawn the gap is 6_000 cm (out of range); after the move
        // to (10_000, 1_000) it is exactly 5_000 cm (closed range).
        let report = run(
            &mut state,
            &[enl(vec![
                Command::Move {
                    destination: Coord::new(10_000, 1_000),
                },
                transfer_cmd(1, 3, &[]),
            ])],
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert!(matches!(outcomes[0], CommandOutcome::Ok(_)));
        assert!(matches!(
            outcomes[1],
            CommandOutcome::Ok(CommandEffect::Transferred { .. })
        ));
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(17));
        assert_eq!(state.agent(AgentId(1)).map(|a| a.supply), Some(23));
    }

    #[test]
    fn transfer_measures_a_target_that_moved_earlier_this_round() {
        let mut state = transfer_state(
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 8_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        // Same faction, same bucket: A1 (seq 0) settles before A0 (seq 1)
        // and walks out of TRANSFER_RANGE.
        let report = run(
            &mut state,
            &[
                stream(
                    1,
                    Faction::Enl,
                    AgentId(1),
                    0,
                    0,
                    vec![Command::Move {
                        destination: Coord::new(20_000, 20_000),
                    }],
                ),
                stream(
                    1,
                    Faction::Enl,
                    AgentId(0),
                    0,
                    1,
                    vec![transfer_cmd(1, 1, &[])],
                ),
            ],
        );
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(_)
        ));
        assert_eq!(
            report.execution_order[1].outcomes[0],
            CommandOutcome::Drop(DropReason::TransferOutOfRange { target: AgentId(1) }),
            "the range check sees A1's post-move position"
        );
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(20));
        assert_eq!(state.agent(AgentId(1)).map(|a| a.supply), Some(20));
    }

    // Checklist 15: self-transfers and zero-quantity transfers are legal
    // no-ops — but target/faction/range still validate, and zero-count key
    // entries never create phantom inventory.
    #[test]
    fn transfer_no_ops_self_and_zero_quantity() {
        let mut state = transfer_state(
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 8_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        state.grant_keys(AgentId(0), PortalId(0), 2);
        let before = (
            agent_snapshot(&state, AgentId(0)),
            agent_snapshot(&state, AgentId(1)),
        );

        // Self-transfer: range trivially satisfied, balances checked against
        // oneself, nets out to no change.
        let report = run(&mut state, &[enl(vec![transfer_cmd(0, 2, &[(0, 2)])])]);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Transferred {
                target: AgentId(0),
                supply: 2,
                keys: BTreeMap::from([(PortalId(0), 2u32)]),
            })
        );

        // Zero-supply transfer with a zero-count key entry: succeeds and
        // creates no 0-entry in the receiver inventory. (D47: the round
        // advances between the two no-op scenarios of this shared state.)
        state.advance_round();
        let report = run(
            &mut state,
            &[stream(
                2,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![transfer_cmd(1, 0, &[(0, 0)])],
            )],
        );
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Transferred {
                target: AgentId(1),
                supply: 0,
                keys: BTreeMap::new(),
            })
        );

        // Validations still run: a zero-quantity transfer to a far same-
        // faction agent still DROPs on range.
        let mut far_state = transfer_state(
            &[Coord::new(10_000, 5_000), Coord::new(29_000, 29_000)],
            &[Coord::new(6_000, 10_000), Coord::new(6_000, 11_000)],
        );
        let report = run(&mut far_state, &[enl(vec![transfer_cmd(1, 0, &[])])]);
        assert_eq!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Drop(DropReason::TransferOutOfRange { target: AgentId(1) })
        );

        assert_eq!(
            (
                agent_snapshot(&state, AgentId(0)),
                agent_snapshot(&state, AgentId(1))
            ),
            before,
            "no-ops change no inventory"
        );
        assert!(
            state
                .agent(AgentId(1))
                .map(|a| a.keys.is_empty())
                .unwrap_or(false),
            "no phantom zero-count entries"
        );
    }

    // Checklist 16: auxiliary stacking — several HACKs and TRANSFERs plus one
    // major in a single stream; the second hack of the same round DROPs on
    // cooldown (current_round - last == 0).
    #[test]
    fn auxiliary_stacking_hacks_transfers_and_one_major() {
        let mut cfg = base_config();
        cfg.initial_agents_per_faction = 2;
        let mut state = fixture_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
                portal_fixture(2, 6_000, 6_000, Owner::Neutral, 0),
            ],
            &[Coord::new(10_000, 5_000), Coord::new(10_000, 8_000)],
            &[Coord::new(24_000, 24_000), Coord::new(24_000, 25_000)],
        );
        let report = run(
            &mut state,
            &[stream(
                1,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![
                    hack_cmd(0),
                    hack_cmd(0), // same-round re-hack: cooldown 1 - 1 == 0
                    hack_cmd(2),
                    transfer_cmd(1, 1, &[]),
                    Command::Capture {
                        portal: PortalId(0),
                    },
                ],
            )],
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert_eq!(outcomes.len(), 5);
        let (p0_keys, _, _, _) = match &outcomes[0] {
            CommandOutcome::Ok(CommandEffect::Hacked { keys_gained, .. }) => {
                (*keys_gained, 0, 0, 0)
            }
            other => panic!("expected first hack to succeed, got {other:?}"),
        };
        assert_eq!(
            outcomes[1],
            CommandOutcome::Drop(DropReason::HackCooldownActive {
                portal: PortalId(0),
                last_hack_round: 1,
                rounds_remaining: 3,
            })
        );
        let p2_keys = match &outcomes[2] {
            CommandOutcome::Ok(CommandEffect::Hacked { keys_gained, .. }) => *keys_gained,
            other => panic!("expected third hack to succeed, got {other:?}"),
        };
        assert!(matches!(
            outcomes[3],
            CommandOutcome::Ok(CommandEffect::Transferred { .. })
        ));
        assert!(matches!(
            outcomes[4],
            CommandOutcome::Ok(CommandEffect::Captured { .. })
        ));
        // The one major landed despite four auxiliary commands around it.
        assert!(
            state
                .tracker(AgentId(0))
                .map(|t| t.major_used)
                .unwrap_or(false)
        );
        assert_eq!(state.portal(PortalId(0)).map(|p| p.owner), Some(Owner::Enl));
        // Two successful hacks + one transferred supply.
        assert_eq!(state.agent(AgentId(0)).map(|a| a.supply), Some(21));
        assert_eq!(state.agent(AgentId(1)).map(|a| a.supply), Some(21));
        assert_eq!(
            state
                .agent(AgentId(0))
                .map(|a| a.keys.get(&PortalId(0)).copied()),
            Some(Some(p0_keys))
        );
        assert_eq!(
            state
                .agent(AgentId(0))
                .map(|a| a.keys.get(&PortalId(2)).copied()),
            Some(Some(p2_keys))
        );
    }

    #[test]
    fn buckets_beat_faction_alternation_in_execution() {
        // ENL is the round's start faction, but its stream arrives one bucket
        // later than the RES stream: bucket order wins.
        let mut state = standard_state();
        let late_enl = stream(
            1,
            Faction::Enl,
            AgentId(0),
            5,
            0,
            vec![Command::Capture {
                portal: PortalId(0),
            }],
        );
        let early_res = stream(
            1,
            Faction::Res,
            AgentId(1),
            0,
            0,
            vec![Command::Capture {
                portal: PortalId(0),
            }],
        );
        let report = run(&mut state, &[late_enl, early_res]);
        assert_eq!(report.execution_order[0].faction, Faction::Res);
        assert_eq!(
            report.execution_order[1].outcomes[0],
            CommandOutcome::Drop(DropReason::CaptureTargetNotNeutral {
                portal: PortalId(0),
                owner: Owner::Res
            })
        );
        assert_eq!(
            report.slots.get(&AgentId(0)),
            Some(&SlotStatus::Executed {
                bucket: 1,
                receive_seq: 0
            })
        );
    }

    // ------------------------------------------------------------------
    // M4: LINK execution, cascade events, settlement, reinforcement
    // (checklist 18-39)
    // ------------------------------------------------------------------

    use crate::types::{FieldId, LinkId, MatchResult};

    fn enl_portal(id: u32, x: i32, y: i32) -> Portal {
        portal_fixture(id, x, y, Owner::Enl, 100)
    }

    fn link_cmd(a: u32, b: u32) -> Command {
        Command::Link {
            source_portal: PortalId(a),
            target_portal: PortalId(b),
        }
    }

    /// State-level edge seeding for geometry-focused fixtures: grants the
    /// target key and applies the link directly (no resolver checks).
    fn seed_link(state: &mut GameState, agent: AgentId, a: u32, b: u32) {
        state.grant_keys(agent, PortalId(b), 1);
        state.apply_link(agent, PortalId(a), PortalId(b));
    }

    fn linked(outcome: &CommandOutcome) -> (LinkId, Vec<FieldId>) {
        match outcome {
            CommandOutcome::Ok(CommandEffect::Linked { link, fields }) => (*link, fields.clone()),
            other => panic!("expected Linked, got {other:?}"),
        }
    }

    fn drop_reason(outcome: &CommandOutcome) -> DropReason {
        match outcome {
            CommandOutcome::Drop(reason) => *reason,
            other => panic!("expected Drop, got {other:?}"),
        }
    }

    /// ENL square-ish default fixture: ENL agent 0 at `enl_pos`, RES agent 1
    /// far away, `portals` appended after two neutral dummies... no — the
    /// caller supplies every portal; only the rosters are fixed at 1+1.
    fn link_state(
        cfg: &GameConfig,
        portals: Vec<Portal>,
        enl_pos: Coord,
        res_pos: Coord,
    ) -> GameState {
        fixture_state(cfg, portals, &[enl_pos], &[res_pos])
    }

    // Checklist 18: the full success path — capture, hack the target key,
    // move back into source range, LINK.
    #[test]
    fn link_success_full_path() {
        let mut state = link_state(
            &base_config(),
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 14_000, 10_000, Owner::Neutral, 0),
            ],
            Coord::new(10_000, 5_000),
            Coord::new(29_000, 29_000),
        );

        // Round 1: capture P0 (closed boundary range), step under P1.
        let _ = run(
            &mut state,
            &[enl(vec![
                Command::Capture {
                    portal: PortalId(0),
                },
                Command::Move {
                    destination: Coord::new(14_000, 5_000),
                },
            ])],
        );
        state.advance_round();
        // Round 2: hack P1's key (range 5_000 exactly), then capture P1.
        let report = run(
            &mut state,
            &[stream(
                2,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![
                    hack_cmd(1),
                    Command::Capture {
                        portal: PortalId(1),
                    },
                ],
            )],
        );
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Hacked { .. })
        ));
        let keys_before = state
            .agent(AgentId(0))
            .map(|a| a.keys[&PortalId(1)])
            .unwrap();
        assert!(keys_before >= 1, "the hack yielded at least one key");
        state.advance_round();

        // Round 3: return to P0's range and link P0 -> P1.
        let report = run(
            &mut state,
            &[stream(
                3,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![
                    Command::Move {
                        destination: Coord::new(10_000, 5_000),
                    },
                    link_cmd(0, 1),
                ],
            )],
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert!(matches!(outcomes[0], CommandOutcome::Ok(_)), "the move ran");
        let (link, fields) = linked(&outcomes[1]);
        assert_eq!(
            link,
            LinkId::new(PortalId(0), PortalId(1)).expect("canonical")
        );
        assert!(fields.is_empty(), "no triangle exists");
        // Exactly one target key consumed, and the edge is in the graph.
        assert_eq!(
            state
                .agent(AgentId(0))
                .map(|a| a.keys.get(&PortalId(1)).copied()),
            Some(Some(keys_before - 1))
        );
        assert!(state.link_graph().link(link).is_some());
        // LINK_CREATED broadcast; the capture events precede it.
        assert!(matches!(
            state.events()[state.events().len() - 1].kind,
            crate::event::EventKind::LinkCreated { .. }
        ));
    }

    // Checklist 19: unknown portals DROP at execution time (D27) — the
    // stream still occupies its ordering slot.
    #[test]
    fn link_unknown_portals_drop_at_execution_time() {
        let mut state = standard_state();
        let report = run(&mut state, &[enl(vec![link_cmd(9, 0), link_cmd(0, 9)])]);
        assert_eq!(
            report.slots.get(&AgentId(0)),
            Some(&SlotStatus::Executed {
                bucket: 0,
                receive_seq: 0
            }),
            "the stream executed and kept its slot"
        );
        let outcomes = &report.execution_order[0].outcomes;
        assert_eq!(
            drop_reason(&outcomes[0]),
            DropReason::PortalNotFound {
                portal: PortalId(9)
            },
            "unknown source reports first"
        );
        assert_eq!(
            drop_reason(&outcomes[1]),
            DropReason::PortalNotFound {
                portal: PortalId(9)
            },
            "then the unknown target"
        );
        assert!(state.events().is_empty());
    }

    // Checklist 20: a self loop reports before any ownership check.
    #[test]
    fn link_self_loop_drops() {
        let mut state = link_state(
            &base_config(),
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Res, 100),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            Coord::new(10_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        let report = run(&mut state, &[enl(vec![link_cmd(0, 0)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkSelfLoop {
                portal: PortalId(0)
            },
            "self loop beats the enemy-owner failure"
        );
    }

    // Checklist 21: ownership reports the source first, then the target,
    // with the owner in the payload.
    #[test]
    fn link_portal_not_owned_reports_source_then_target() {
        // Neutral source (P0), ENL target (P1).
        let mut state = link_state(
            &base_config(),
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                enl_portal(1, 14_000, 10_000),
            ],
            Coord::new(10_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkPortalNotOwned {
                portal: PortalId(0),
                owner: Owner::Neutral
            }
        );

        // ENL source (P0), enemy target (P1).
        let mut state = link_state(
            &base_config(),
            vec![
                enl_portal(0, 10_000, 10_000),
                portal_fixture(1, 14_000, 10_000, Owner::Res, 100),
            ],
            Coord::new(10_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkPortalNotOwned {
                portal: PortalId(1),
                owner: Owner::Res
            }
        );
    }

    // Checklist 22: the closed operation range applies to the SOURCE only —
    // standing near the target does not satisfy the rule.
    #[test]
    fn link_range_is_closed_and_checks_source_only() {
        let portals = || vec![enl_portal(0, 10_000, 10_000), enl_portal(1, 14_000, 10_000)];
        // Agent exactly 5_000 cm below P0 (the closed boundary): succeeds.
        let mut state = link_state(
            &base_config(),
            portals(),
            Coord::new(10_000, 5_000),
            Coord::new(29_000, 29_000),
        );
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            linked(&report.execution_order[0].outcomes[0]).0,
            LinkId::new(PortalId(0), PortalId(1)).expect("canonical")
        );

        // Agent exactly 5_000 cm below P1 — within the TARGET's closed range
        // but ~6_403 cm from the SOURCE: directional DROP.
        let mut state = link_state(
            &base_config(),
            portals(),
            Coord::new(14_000, 5_000),
            Coord::new(29_000, 29_000),
        );
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::OutOfOperationRange {
                portal: PortalId(0)
            }
        );
    }

    // Checklist 23: a missing target key mutates nothing.
    #[test]
    fn link_missing_target_key_drops_without_mutation() {
        let mut state = link_state(
            &base_config(),
            vec![enl_portal(0, 10_000, 10_000), enl_portal(1, 14_000, 10_000)],
            Coord::new(10_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        state.grant_keys(AgentId(0), PortalId(0), 2); // source keys only
        let before = agent_snapshot(&state, AgentId(0));
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkMissingTargetKey {
                portal: PortalId(1)
            }
        );
        assert_eq!(agent_snapshot(&state, AgentId(0)), before);
        assert!(state.link_graph().links().is_empty());
    }

    // Checklist 24: MAX_LINK_DISTANCE is closed — the exact boundary
    // succeeds, one unit further DROPs (squared compare, no square root).
    #[test]
    fn link_distance_closed_boundary() {
        let mut cfg = base_config();
        cfg.max_link_distance_cm = 10_000;
        // Distance exactly 10_000.
        let mut state = link_state(
            &cfg,
            vec![enl_portal(0, 10_000, 10_000), enl_portal(1, 20_000, 10_000)],
            Coord::new(10_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Linked { .. })
        ));

        // One unit further (10_001): DROP.
        let mut state = link_state(
            &cfg,
            vec![enl_portal(0, 10_000, 10_000), enl_portal(1, 20_001, 10_000)],
            Coord::new(10_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkDistanceExceeded {
                source: PortalId(0),
                target: PortalId(1)
            }
        );
    }

    // Checklist 25: with A-B existing, both LINK(A,B) and LINK(B,A) DROP
    // naming the same canonical LinkId.
    #[test]
    fn link_duplicate_pair_drops_both_directions() {
        let mut state = link_state(
            &base_config(),
            vec![enl_portal(0, 10_000, 10_000), enl_portal(1, 14_000, 10_000)],
            Coord::new(10_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        seed_link(&mut state, AgentId(0), 0, 1);
        // Keys for both directions: each command names a different target.
        state.grant_keys(AgentId(0), PortalId(1), 2);
        state.grant_keys(AgentId(0), PortalId(0), 2);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1), link_cmd(1, 0)])]);
        let canonical = LinkId::new(PortalId(0), PortalId(1)).expect("canonical");
        for outcome in &report.execution_order[0].outcomes {
            assert_eq!(
                drop_reason(outcome),
                DropReason::LinkAlreadyExists { link: canonical }
            );
        }
    }

    // Checklist 26: a portal strictly inside the closed segment blocks (the
    // first offender in ascending PortalId reports); the segment endpoints
    // themselves are exempt.
    #[test]
    fn link_blocked_by_portal_on_closed_segment() {
        let mut cfg = base_config();
        cfg.portal_min_distance_cm = 1_000;
        // P2 (7_000) and P3 (12_000) both lie strictly inside P0-P1.
        let mut state = link_state(
            &cfg,
            vec![
                enl_portal(0, 4_000, 10_000),
                enl_portal(1, 16_000, 10_000),
                enl_portal(2, 7_000, 10_000),
                enl_portal(3, 12_000, 10_000),
            ],
            Coord::new(4_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkBlockedByPortal {
                link: LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
                portal: PortalId(2),
            },
            "the first offender ascending (P2 before P3)"
        );

        // The short link between the two interior portals is legal: its own
        // endpoints are exempt and no other portal lies on [7_000, 12_000].
        // (D47: fresh round for the second scenario of this shared state.)
        state.grant_keys(AgentId(0), PortalId(3), 1);
        state.advance_round();
        let report = run(
            &mut state,
            &[stream(
                2,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![link_cmd(2, 3)],
            )],
        );
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Linked { .. })
        ));
    }

    // Checklist 27: a proper X crossing of an existing link DROPs naming the
    // first existing link ascending.
    #[test]
    fn link_crossing_existing_link_drops() {
        let mut state = link_state(
            &base_config(),
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 16_000, 16_000),
                enl_portal(2, 4_000, 16_000),
                enl_portal(3, 16_000, 4_000),
            ],
            Coord::new(4_000, 16_000), // the agent stands ON the source P2
            Coord::new(29_000, 29_000),
        );
        seed_link(&mut state, AgentId(0), 0, 1); // the diagonal (0,1)
        state.grant_keys(AgentId(0), PortalId(3), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(2, 3)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkIntersectsLink {
                link: LinkId::new(PortalId(2), PortalId(3)).expect("canonical"),
                existing: LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
            }
        );
    }

    // Checklist 28: a fan of links sharing one portal endpoint is entirely
    // legal; collinear overlap WITHOUT a shared portal is illegal.
    #[test]
    fn link_shared_endpoint_legal_collinear_overlap_illegal() {
        let mut cfg = base_config();
        cfg.portal_min_distance_cm = 1_000;
        let mut state = link_state(
            &cfg,
            vec![
                enl_portal(0, 4_000, 10_000),
                enl_portal(1, 12_000, 12_000),
                enl_portal(2, 16_000, 4_000),
                enl_portal(3, 12_000, 4_000),
            ],
            Coord::new(4_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        // Fan from P0: (0,1), (0,2), (0,3) — every pair shares P0.
        state.grant_keys(AgentId(0), PortalId(1), 1);
        state.grant_keys(AgentId(0), PortalId(2), 1);
        state.grant_keys(AgentId(0), PortalId(3), 1);
        let report = run(
            &mut state,
            &[enl(vec![link_cmd(0, 1), link_cmd(0, 2), link_cmd(0, 3)])],
        );
        for outcome in &report.execution_order[0].outcomes {
            assert!(
                matches!(outcome, CommandOutcome::Ok(CommandEffect::Linked { .. })),
                "shared-portal fans are legal: {outcome:?}"
            );
        }

        // Collinear overlap without a shared portal: existing (0,4) spans
        // y = 10_000 from x 4_000 to 16_000; the new link (5,6) lies strictly
        // inside that span on the same line.
        let mut state = link_state(
            &cfg,
            vec![
                enl_portal(0, 0, 10_000),
                enl_portal(1, 24_000, 10_000),
                enl_portal(2, 8_000, 10_000),
                enl_portal(3, 14_000, 10_000),
            ],
            Coord::new(8_000, 10_000),
            Coord::new(29_000, 29_000),
        );
        seed_link(&mut state, AgentId(0), 0, 1);
        state.grant_keys(AgentId(0), PortalId(3), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(2, 3)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkIntersectsLink {
                link: LinkId::new(PortalId(2), PortalId(3)).expect("canonical"),
                existing: LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
            },
            "collinear overlap with no shared portal is an illegal intersection"
        );
    }

    // Checklist 29: a link through a field interior is legal (R-LINK-02) —
    // only portal-on-segment and link crossings block.
    #[test]
    fn link_inside_existing_field_is_legal() {
        let mut state = link_state(
            &base_config(),
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 20_000, 4_000),
                enl_portal(2, 4_000, 20_000),
                enl_portal(3, 8_000, 8_000),
                enl_portal(4, 12_000, 8_000),
            ],
            Coord::new(8_000, 8_000),
            Coord::new(29_000, 29_000),
        );
        seed_link(&mut state, AgentId(0), 0, 1);
        seed_link(&mut state, AgentId(0), 0, 2);
        seed_link(&mut state, AgentId(0), 1, 2);
        let field = FieldId::new(PortalId(0), PortalId(1), PortalId(2)).expect("canonical");
        assert!(state.link_graph().fields().contains_key(&field));

        state.grant_keys(AgentId(0), PortalId(4), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(3, 4)])]);
        let (link, fields) = linked(&report.execution_order[0].outcomes[0]);
        assert_eq!(
            link,
            LinkId::new(PortalId(3), PortalId(4)).expect("canonical")
        );
        assert!(
            fields.is_empty(),
            "an interior chord closes no triangle here"
        );
        assert!(
            state.link_graph().fields().contains_key(&field),
            "the surrounding field survives"
        );
    }

    // Checklist 30: stacked violations report the first frozen reason —
    // owner before key before distance before duplicate before geometry.
    #[test]
    fn link_check_order_is_contractual() {
        // Owner beats key, distance, AND duplicate: the source is enemy
        // owned, the agent holds no key, the pair is over max distance, and
        // an edge already exists (seeded past the resolver's checks).
        let mut cfg = base_config();
        cfg.max_link_distance_cm = 5_000;
        let build = |owner_p0: Owner| {
            link_state(
                &cfg,
                vec![
                    portal_fixture(0, 10_000, 10_000, owner_p0, 100),
                    enl_portal(1, 20_000, 10_000),
                ],
                Coord::new(10_000, 10_000),
                Coord::new(29_000, 29_000),
            )
        };
        let mut state = build(Owner::Res);
        seed_link(&mut state, AgentId(0), 0, 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkPortalNotOwned {
                portal: PortalId(0),
                owner: Owner::Res
            }
        );

        // Key beats distance and duplicate: everything ENL owned, no key.
        let mut state = build(Owner::Enl);
        seed_link(&mut state, AgentId(0), 0, 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkMissingTargetKey {
                portal: PortalId(1)
            }
        );

        // Distance beats duplicate: key granted, pair 10_000 > 5_000 apart.
        let mut state = build(Owner::Enl);
        seed_link(&mut state, AgentId(0), 0, 1);
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkDistanceExceeded {
                source: PortalId(0),
                target: PortalId(1)
            }
        );

        // Duplicate beats geometry: the pair is legal by distance but the
        // edge exists AND a crossing edge exists — the duplicate reports.
        let mut state = link_state(
            &base_config(),
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 16_000, 16_000),
                enl_portal(2, 4_000, 16_000),
                enl_portal(3, 16_000, 4_000),
            ],
            Coord::new(4_000, 4_000),
            Coord::new(29_000, 29_000),
        );
        seed_link(&mut state, AgentId(0), 0, 1);
        seed_link(&mut state, AgentId(0), 2, 3);
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkAlreadyExists {
                link: LinkId::new(PortalId(0), PortalId(1)).expect("canonical")
            }
        );

        // Portal-on-segment beats intersection: the new link BOTH passes
        // through portal 4 and crosses existing (2,3) — the portal reports.
        let mut state = link_state(
            &base_config(),
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 16_000, 16_000),
                enl_portal(2, 10_000, 6_000),
                enl_portal(3, 6_000, 10_000),
                enl_portal(4, 10_000, 10_000),
            ],
            Coord::new(4_000, 4_000),
            Coord::new(29_000, 29_000),
        );
        seed_link(&mut state, AgentId(0), 2, 3);
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(0, 1)])]);
        assert_eq!(
            drop_reason(&report.execution_order[0].outcomes[0]),
            DropReason::LinkBlockedByPortal {
                link: LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
                portal: PortalId(4),
            }
        );
    }

    // Checklist 31 (R-RESOLVE-04 row 2): the first LINK in execution order
    // is created; the later intersecting LINK DROPs.
    #[test]
    fn same_round_intersecting_links_r_resolve_04_row2() {
        let mut cfg = base_config();
        cfg.initial_agents_per_faction = 2;
        let mut state = fixture_state(
            &cfg,
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 16_000, 16_000),
                enl_portal(2, 4_000, 16_000),
                enl_portal(3, 16_000, 4_000),
            ],
            &[Coord::new(4_000, 4_000), Coord::new(4_000, 16_000)],
            &[Coord::new(29_000, 29_000), Coord::new(29_000, 28_000)],
        );
        state.grant_keys(AgentId(0), PortalId(1), 1);
        state.grant_keys(AgentId(1), PortalId(3), 1);
        let first = stream(1, Faction::Enl, AgentId(0), 0, 0, vec![link_cmd(0, 1)]);
        let second = stream(1, Faction::Enl, AgentId(1), 0, 1, vec![link_cmd(2, 3)]);
        let report = run(&mut state, &[first, second]);

        assert_eq!(report.execution_order.len(), 2);
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Linked { .. })
        ));
        assert_eq!(
            drop_reason(&report.execution_order[1].outcomes[0]),
            DropReason::LinkIntersectsLink {
                link: LinkId::new(PortalId(2), PortalId(3)).expect("canonical"),
                existing: LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
            }
        );
    }

    // Checklist 32: a closing link emits LINK_CREATED then FIELD_CREATED per
    // field ascending, and the effect lists the new field ids — layered
    // fields included.
    #[test]
    fn link_completing_triangle_emits_events_in_frozen_order() {
        let mut state = link_state(
            &base_config(),
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 20_000, 4_000),
                enl_portal(2, 4_000, 20_000),
                enl_portal(3, 20_000, 20_000),
            ],
            Coord::new(4_000, 20_000),
            Coord::new(29_000, 29_000),
        );
        // Edges so that P2-P3 closes TWO layered fields: (0,2,3) and (1,2,3).
        seed_link(&mut state, AgentId(0), 0, 1);
        seed_link(&mut state, AgentId(0), 0, 2);
        seed_link(&mut state, AgentId(0), 1, 2);
        seed_link(&mut state, AgentId(0), 0, 3);
        seed_link(&mut state, AgentId(0), 1, 3);
        state.grant_keys(AgentId(0), PortalId(3), 1);
        let events_before = state.events().len();

        let report = run(&mut state, &[enl(vec![link_cmd(2, 3)])]);
        let (link, fields) = linked(&report.execution_order[0].outcomes[0]);
        assert_eq!(
            link,
            LinkId::new(PortalId(2), PortalId(3)).expect("canonical")
        );
        assert_eq!(
            fields,
            vec![
                FieldId::new(PortalId(0), PortalId(2), PortalId(3)).expect("canonical"),
                FieldId::new(PortalId(1), PortalId(2), PortalId(3)).expect("canonical"),
            ],
            "layered fields ascend in the effect"
        );

        let fresh: Vec<_> = state.events()[events_before..]
            .iter()
            .map(|e| e.kind)
            .collect();
        assert_eq!(
            fresh,
            vec![
                crate::event::EventKind::LinkCreated {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    link: LinkId::new(PortalId(2), PortalId(3)).expect("canonical"),
                },
                crate::event::EventKind::FieldCreated {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    field: FieldId::new(PortalId(0), PortalId(2), PortalId(3)).expect("canonical"),
                },
                crate::event::EventKind::FieldCreated {
                    faction: Faction::Enl,
                    agent_id: AgentId(0),
                    field: FieldId::new(PortalId(1), PortalId(2), PortalId(3)).expect("canonical"),
                },
            ],
            "LINK_CREATED then FIELD_CREATED per field ascending"
        );
    }

    // Checklist 33: neutralization emits PORTAL_NEUTRALIZED, LINK_DESTROYED
    // ascending, FIELD_DESTROYED ascending; unaffected neighbor fields
    // survive; a layered field over two removed edges emits ONE destroy.
    #[test]
    fn attack_neutralization_cascades_in_frozen_event_order() {
        let mut cfg = base_config();
        cfg.attack_damage = 100;
        let mut state = link_state(
            &cfg,
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 20_000, 4_000),
                enl_portal(2, 4_000, 20_000),
                enl_portal(3, 20_000, 20_000),
            ],
            Coord::new(29_000, 29_000),
            // RES sits 4_000 from P1 only.
            Coord::new(20_000, 8_000),
        );
        // Field (0,1,2) and the disjoint-edge field (1,2,3) via edges
        // (1,2),(1,3),(2,3).
        seed_link(&mut state, AgentId(0), 0, 1);
        seed_link(&mut state, AgentId(0), 0, 2);
        seed_link(&mut state, AgentId(0), 1, 2);
        seed_link(&mut state, AgentId(0), 1, 3);
        seed_link(&mut state, AgentId(0), 2, 3);
        assert_eq!(state.link_graph().fields().len(), 2);

        let report = run(&mut state, &[res(vec![Command::Attack])]);
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Attacked { .. })
        ));
        let kinds: Vec<_> = state.events().iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                crate::event::EventKind::PortalAttacked {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    portal: PortalId(1),
                    damage: 100,
                    hp_after: 0,
                },
                crate::event::EventKind::PortalNeutralized {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    portal: PortalId(1),
                },
                crate::event::EventKind::LinkDestroyed {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    link: LinkId::new(PortalId(0), PortalId(1)).expect("canonical"),
                },
                crate::event::EventKind::LinkDestroyed {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    link: LinkId::new(PortalId(1), PortalId(2)).expect("canonical"),
                },
                crate::event::EventKind::LinkDestroyed {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    link: LinkId::new(PortalId(1), PortalId(3)).expect("canonical"),
                },
                // The layered field (0,1,2) spans TWO removed edges ((0,1)
                // and (1,2)) — one FIELD_DESTROYED, not two. (1,2,3) also
                // dies with edges (1,2)/(1,3).
                crate::event::EventKind::FieldDestroyed {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    field: FieldId::new(PortalId(0), PortalId(1), PortalId(2)).expect("canonical"),
                },
                crate::event::EventKind::FieldDestroyed {
                    faction: Faction::Res,
                    agent_id: AgentId(1),
                    field: FieldId::new(PortalId(1), PortalId(2), PortalId(3)).expect("canonical"),
                },
            ]
        );
        assert!(
            state.link_graph().fields().is_empty(),
            "both fields touched P1"
        );
        // The edges not incident to P1 survive.
        assert!(
            state
                .link_graph()
                .link(LinkId::new(PortalId(0), PortalId(2)).expect("c"))
                .is_some()
        );
        assert!(
            state
                .link_graph()
                .link(LinkId::new(PortalId(2), PortalId(3)).expect("c"))
                .is_some()
        );
    }

    // Checklist 34: round-end settlement semantics — a field destroyed in
    // its creation round scores 0; surviving fields score; the final round
    // still settles.
    #[test]
    fn settle_scores_after_all_streams() {
        let square = || {
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 20_000, 4_000),
                enl_portal(2, 4_000, 20_000),
            ]
        };
        // (a) Created and destroyed in the SAME round: scores 0.
        let mut cfg = base_config();
        cfg.attack_damage = 100;
        let mut state = link_state(
            &cfg,
            square(),
            Coord::new(20_000, 4_000),
            Coord::new(20_000, 8_000), // RES 4_000 from P1
        );
        seed_link(&mut state, AgentId(0), 0, 1);
        seed_link(&mut state, AgentId(0), 0, 2);
        state.grant_keys(AgentId(0), PortalId(2), 1);
        let report = run(
            &mut state,
            &[
                enl(vec![link_cmd(1, 2)]),  // closes (0,1,2) — ENL settles first
                res(vec![Command::Attack]), // neutralizes P1, field dies
            ],
        );
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Linked { .. })
        ));
        assert_eq!(
            report.score2_delta.get(&Faction::Enl),
            Some(&0),
            "created-then-destroyed fields score nothing"
        );
        assert_eq!(state.score(Faction::Enl), 0);

        // (b) A surviving field scores its full area2 (256_000_000).
        let mut state = link_state(
            &base_config(),
            square(),
            Coord::new(4_000, 20_000),
            Coord::new(29_000, 29_000),
        );
        seed_link(&mut state, AgentId(0), 0, 1);
        seed_link(&mut state, AgentId(0), 0, 2);
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let report = run(&mut state, &[enl(vec![link_cmd(2, 1)])]);
        assert_eq!(report.score2_delta.get(&Faction::Enl), Some(&256_000_000));
        assert_eq!(state.score(Faction::Enl), 256_000_000);
        state.advance_round();
        let report = run(&mut state, &[]);
        assert_eq!(
            report.score2_delta.get(&Faction::Enl),
            Some(&256_000_000),
            "the surviving field scores again next round"
        );

        // (c) The FINAL round still settles: total_rounds == 2, field closed
        // in round 2, scored once.
        let mut cfg = base_config();
        cfg.total_rounds = 2;
        let mut state = link_state(
            &cfg,
            square(),
            Coord::new(4_000, 20_000),
            Coord::new(29_000, 29_000),
        );
        seed_link(&mut state, AgentId(0), 0, 1);
        seed_link(&mut state, AgentId(0), 0, 2);
        state.grant_keys(AgentId(0), PortalId(1), 1);
        let _ = run(&mut state, &[]); // round 1 settles nothing
        assert_eq!(state.score(Faction::Enl), 0);
        // D47: the round advances between the two settles (previously this
        // settled round 2 twice — benign only because nothing scored yet).
        state.advance_round();
        let report = run(
            &mut state,
            &[stream(
                2,
                Faction::Enl,
                AgentId(0),
                0,
                0,
                vec![link_cmd(2, 1)],
            )],
        );
        assert_eq!(
            report.score2_delta.get(&Faction::Enl),
            Some(&256_000_000),
            "the last round's new field scores once (R-SCORE-01)"
        );
        assert_eq!(state.score(Faction::Enl), 256_000_000);
        assert_eq!(state.final_result(), MatchResult::EnlWin);
    }

    // Checklist 35: the report's delta map equals the per-faction sum of
    // surviving field areas.
    #[test]
    fn round_report_carries_score2_delta() {
        let mut state = link_state(
            &base_config(),
            vec![
                enl_portal(0, 4_000, 4_000),
                enl_portal(1, 20_000, 4_000),
                enl_portal(2, 4_000, 20_000),
                enl_portal(3, 20_000, 20_000),
                enl_portal(4, 28_000, 4_000),
                portal_fixture(5, 4_000, 30_000, Owner::Res, 100),
            ],
            Coord::new(4_000, 4_000),
            Coord::new(4_000, 30_000),
        );
        // ENL field (0,1,2): 256_000_000.
        seed_link(&mut state, AgentId(0), 0, 1);
        seed_link(&mut state, AgentId(0), 0, 2);
        seed_link(&mut state, AgentId(0), 1, 2);
        // RES field (3,4,5): cross((20,20)->(28,4), (20,20)->(4,30))
        // = 8_000*10_000 - 16_000*16_000 -> 176_000_000.
        seed_link(&mut state, AgentId(1), 3, 4);
        seed_link(&mut state, AgentId(1), 3, 5);
        seed_link(&mut state, AgentId(1), 4, 5);

        let report = run(&mut state, &[]);
        let mut expected = BTreeMap::new();
        expected.insert(Faction::Enl, 256_000_000u128);
        expected.insert(Faction::Res, 176_000_000u128);
        assert_eq!(report.score2_delta, expected);
        assert_eq!(state.score(Faction::Enl), 256_000_000u128);
        assert_eq!(state.score(Faction::Res), 176_000_000u128);
    }

    // Checklist 36: reinforcement lands after the boundary round's whole
    // settlement — absent from its slots and snapshot, actable in the next.
    #[test]
    fn reinforcement_boundary_timing() {
        let mut cfg = base_config();
        cfg.total_rounds = 6;
        cfg.reinforcement_interval_rounds = 2;
        cfg.reinforcement_agents_per_faction = 1;
        let mut state = link_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            Coord::new(10_000, 5_000),
            Coord::new(6_000, 10_000),
        );

        // Round 1: no boundary (1 % 2 != 0), nothing spawns.
        let _ = run(&mut state, &[]);
        assert!(state.events().is_empty());
        assert_eq!(state.agents().len(), 2);
        state.advance_round();

        // Round 2: the boundary — agents spawn AFTER the round's slots.
        let report = run(&mut state, &[]);
        assert_eq!(
            report.slots.keys().copied().collect::<Vec<_>>(),
            vec![AgentId(0), AgentId(1)],
            "the new agents take no boundary-round slot"
        );
        assert_eq!(report.score2_delta.get(&Faction::Enl), Some(&0));
        assert!(matches!(
            state.events()[0].kind,
            crate::event::EventKind::Reinforcement { .. }
        ));
        assert_eq!(state.events()[0].round, 2);
        assert_eq!(state.agents().len(), 4);
        let position = state
            .agent(AgentId(2))
            .map(|a| a.position)
            .expect("spawned");
        assert_eq!(state.agent(AgentId(3)).map(|a| a.position), Some(position));
        state.advance_round();

        // Round 3: the reinforcement agent CAN act in its first round.
        let report = run(
            &mut state,
            &[stream(
                3,
                Faction::Enl,
                AgentId(2),
                0,
                0,
                vec![Command::Move {
                    destination: position,
                }],
            )],
        );
        assert!(matches!(
            report.execution_order[0].outcomes[0],
            CommandOutcome::Ok(CommandEffect::Moved { .. })
        ));
    }

    // Checklist 37: both factions' new agents sit at the oracle-recomputed
    // shared position (golden cross-check,).
    #[test]
    fn reinforcement_position_matches_oracle_golden() {
        let game_seed = 0xA11CE;
        let mut cfg = base_config();
        cfg.total_rounds = 10;
        cfg.reinforcement_interval_rounds = 3;
        let mut state = link_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            Coord::new(10_000, 5_000),
            Coord::new(6_000, 10_000),
        );
        // Rounds 3, 6, 9 are boundaries; 9 < 10 spawns the last block.
        let mut positions = Vec::new();
        for round in 1..=9u32 {
            let _ = run_with(game_seed, &mut state, &[]);
            if round % 3 == 0 && round < 10 {
                let index = round / 3;
                let oracle = RandomOracle::new(game_seed);
                let expected = oracle.reinforcement_position(index, 30_001, 30_001);
                let first = 2 + 2 * (index - 1); // ENL newcomer of this block
                assert_eq!(
                    state.agent(AgentId(first)).map(|a| a.position),
                    Some(expected),
                    "round {round} ENL newcomer at the oracle position"
                );
                assert_eq!(
                    state.agent(AgentId(first + 1)).map(|a| a.position),
                    Some(expected),
                    "round {round} RES newcomer shares it"
                );
                positions.push(expected);
            }
            state.advance_round();
        }
        assert_eq!(positions.len(), 3);
        assert!(
            positions[0] != positions[1] || positions[1] != positions[2],
            "different interval indices draw different addresses"
        );
    }

    // Checklist 38: no ghost spawn past the final round.
    #[test]
    fn reinforcement_skipped_past_final_round() {
        let mut cfg = base_config();
        cfg.total_rounds = 4;
        cfg.reinforcement_interval_rounds = 4;
        let mut state = link_state(
            &cfg,
            vec![
                portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
                portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
            ],
            Coord::new(10_000, 5_000),
            Coord::new(6_000, 10_000),
        );
        for _round in 1..=4u32 {
            let _ = run(&mut state, &[]);
            state.advance_round();
        }
        assert!(state.events().is_empty(), "round 4 == total: no spawn");
        assert_eq!(state.agents().len(), 2);
        assert_eq!(state.next_agent_id(), AgentId(2));
    }

    // Checklist 39: the default schedule grows each faction 3 -> 7 with
    // consecutive global ids, ENL block first.
    #[test]
    fn reinforcement_roster_growth_three_to_seven() {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 30_000;
        cfg.map_height_cm = 30_000;
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;
        cfg.portal_min_distance_cm = 4_000;
        let map = MapDefinition::new(vec![
            portal_fixture(0, 10_000, 10_000, Owner::Neutral, 0),
            portal_fixture(1, 25_000, 25_000, Owner::Neutral, 0),
        ]);
        let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
        let enl_entries: Vec<(AgentId, Coord)> = enl_roster
            .iter()
            .copied()
            .zip([Coord::new(0, 0), Coord::new(1_000, 0), Coord::new(2_000, 0)])
            .collect();
        let res_entries: Vec<(AgentId, Coord)> = res_roster
            .iter()
            .copied()
            .zip([
                Coord::new(0, 1_000),
                Coord::new(1_000, 1_000),
                Coord::new(2_000, 1_000),
            ])
            .collect();
        let enl = validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg).expect("spawns");
        let res = validate_initial_spawn_set(&res_roster, &res_entries, &cfg).expect("spawns");
        let mut state = GameState::new(cfg, &map, &enl, &res).expect("state");

        // Boundaries 100/200/300/400 under the default schedule: k = 1 each.
        let mut expected_enl: Vec<AgentId> = enl_roster.clone();
        let mut expected_res: Vec<AgentId> = res_roster.clone();
        for (boundary, next_id) in [(100u32, 6u32), (200, 8), (300, 10), (400, 12)] {
            while state.round() < boundary {
                let _ = run(&mut state, &[]);
                state.advance_round();
            }
            let report = run(&mut state, &[]); // the boundary round itself
            assert_eq!(state.round(), boundary);
            // The boundary round's slots cover only the PRE-growth roster;
            // the newcomers join after settlement.
            assert_eq!(
                report.slots.keys().copied().max().expect("slots exist"),
                AgentId(next_id - 1),
                "boundary {boundary}: newcomers take no slot"
            );
            assert_eq!(
                state.agent(AgentId(next_id)).map(|a| a.faction),
                Some(Faction::Enl),
                "boundary {boundary}: ENL block continues the counter"
            );
            assert_eq!(
                state.agent(AgentId(next_id + 1)).map(|a| a.faction),
                Some(Faction::Res)
            );
            expected_enl.push(AgentId(next_id));
            expected_res.push(AgentId(next_id + 1));
            let enl_count = state
                .agents()
                .values()
                .filter(|a| a.faction == Faction::Enl)
                .count();
            let res_count = state
                .agents()
                .values()
                .filter(|a| a.faction == Faction::Res)
                .count();
            assert_eq!(enl_count, expected_enl.len(), "boundary {boundary} ENL");
            assert_eq!(res_count, expected_res.len(), "boundary {boundary} RES");
            state.advance_round();
        }
        assert_eq!(expected_enl.len(), 7, "3 -> 7 across four boundaries");
        assert_eq!(expected_res.len(), 7);
        // Consecutive global blocks: ids 6..=13 with ENL first in each pair.
        let mut all: Vec<u32> = state.agents().keys().map(|id| id.0).collect();
        all.sort_unstable();
        assert_eq!(all, (0..14).collect::<Vec<_>>());
        assert_eq!(state.next_agent_id(), AgentId(14));
    }

    // ------------------------------------------------------------------
    // M7 (D68): the extracted `structural_check` pins the exact intake
    // verdicts of `run_round` — the pure-refactor guarantee.
    // ------------------------------------------------------------------

    #[test]
    fn structural_check_matches_run_round_verdicts_at_the_boundaries() {
        let stay = || Command::Move {
            destination: Coord::new(10_000, 5_000),
        };
        let moves = |count: usize| (0..count).map(|_| stay()).collect::<Vec<_>>();
        // Command-count boundary: 10 ok, 11 rejected (default cap 10).
        assert_eq!(structural_check(&moves(10), 10), Ok(()));
        assert_eq!(
            structural_check(&moves(11), 10),
            Err(StreamRejection::TooManyCommands { count: 11, max: 10 })
        );
        // Major-count boundary: two majors rejected.
        let capture = Command::Capture {
            portal: PortalId(0),
        };
        assert_eq!(
            structural_check(&[capture.clone(), Command::Attack], 10),
            Err(StreamRejection::MultipleMajorActions { count: 2 })
        );
        // Both defects coexist: the count check reports FIRST (D27 order).
        let mut both = vec![capture.clone(), Command::Attack];
        both.extend(moves(10));
        assert_eq!(
            structural_check(&both, 10),
            Err(StreamRejection::TooManyCommands { count: 12, max: 10 }),
            "count check precedes the major allowance"
        );
        // Empty is structurally fine (an explicit PASS) — run_round freezes
        // it as Pass.
        assert_eq!(structural_check(&[], 10), Ok(()));

        // End-to-end: the resolver's slot statuses equal the helper verdicts.
        let res_moves: Vec<Command> = (0..11)
            .map(|_| Command::Move {
                destination: Coord::new(6_000, 10_000),
            })
            .collect();
        let mut state = standard_state();
        let report = run(
            &mut state,
            &[
                stream(
                    1,
                    Faction::Enl,
                    AgentId(0),
                    0,
                    0,
                    vec![capture.clone(), Command::Attack],
                ),
                stream(1, Faction::Res, AgentId(1), 0, 1, res_moves.clone()),
            ],
        );
        let enl_verdict = structural_check(&[capture, Command::Attack], 10).unwrap_err();
        let res_verdict = structural_check(&res_moves, 10).unwrap_err();
        assert_eq!(
            report.slots.get(&AgentId(0)),
            Some(&SlotStatus::Rejected(enl_verdict))
        );
        assert_eq!(
            report.slots.get(&AgentId(1)),
            Some(&SlotStatus::Rejected(res_verdict))
        );
    }
}

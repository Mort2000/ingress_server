//! Resolver: intake, fair ordering, and per-command execution
//! (R-RESOLVE-01..04, P1-RESOLVE). The resolver decides validity against
//! the latest authoritative state; [`crate::state::GameState`] owns every
//! mutation.

use std::collections::{BTreeMap, BTreeSet};

use crate::event::EventKind;
use crate::geometry::{in_bounds, move_cost, within_closed_range};
use crate::state::{GameState, faction_as_owner};
use crate::types::{AgentId, Command, Coord, Faction, Owner, PortalId};

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
    MoveOutOfBounds { destination: Coord },
    MoveBudgetExceeded { requested: u64, remaining: u64 },
    PortalNotFound { portal: PortalId },
    OutOfOperationRange { portal: PortalId },
    CaptureTargetNotNeutral { portal: PortalId, owner: Owner },
    AttackNoEnemyPortalsInRange,
    RechargePortalNotOwned { portal: PortalId, owner: Owner },
    RechargePositionNotSatisfied { portal: PortalId },
    RechargeInsufficientSupply { required: u32, available: u32 },
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

/// Full settlement record for one round. `execution_order` is the fixed
/// stream order chosen before any command ran and is never reordered by
/// results (R-RESOLVE-03).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RoundReport {
    pub slots: BTreeMap<AgentId, SlotStatus>,
    pub execution_order: Vec<ExecutedStream>,
    pub ignored_duplicates: Vec<AgentId>,
    pub unlocatable: Vec<Unlocatable>,
}

/// Stateless round resolver.
#[derive(Default)]
pub struct Resolver;

impl Resolver {
    pub fn new() -> Self {
        Resolver
    }

    /// Settles one round (P1-RESOLVE steps 1-5 minus the M3+ items):
    /// intake in submission order (locatability gate, freeze-first,
    /// structural check), auto-PASS for unfrozen agents, fair merge of the
    /// executable streams, then sequential per-command execution against the
    /// latest state. Mutates `state`; never advances the round (the harness
    /// owns [`GameState::advance_round`]).
    pub fn run_round(
        &self,
        state: &mut GameState,
        submissions: &[SubmittedCommandStream],
    ) -> RoundReport {
        let mut report = RoundReport {
            slots: BTreeMap::new(),
            execution_order: Vec::new(),
            ignored_duplicates: Vec::new(),
            unlocatable: Vec::new(),
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

            // Structural check (D27): count-only, world-blind. Command count
            // first, then the Major Action allowance.
            if submission.commands.len() > max_commands as usize {
                report.slots.insert(
                    submission.agent_id,
                    SlotStatus::Rejected(StreamRejection::TooManyCommands {
                        count: submission.commands.len() as u32,
                        max: max_commands,
                    }),
                );
                continue;
            }
            let majors = submission.commands.iter().filter(|c| c.is_major()).count() as u32;
            if majors > 1 {
                report.slots.insert(
                    submission.agent_id,
                    SlotStatus::Rejected(StreamRejection::MultipleMajorActions { count: majors }),
                );
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
            let outcomes = execute_stream(state, submission);
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

        report
    }
}

/// Executes a stream's commands sequentially, each validated against the
/// latest authoritative state (R-ACTION-01: no interleaving, no rollback).
fn execute_stream(
    state: &mut GameState,
    submission: &SubmittedCommandStream,
) -> Vec<CommandOutcome> {
    submission
        .commands
        .iter()
        .map(|command| execute_command(state, submission.agent_id, submission.faction, command))
        .collect()
}

fn execute_command(
    state: &mut GameState,
    agent_id: AgentId,
    faction: Faction,
    command: &Command,
) -> CommandOutcome {
    match command {
        Command::Move { destination } => execute_move(state, agent_id, *destination),
        Command::Attack => execute_attack(state, agent_id, faction),
        Command::Capture { portal } => execute_capture(state, agent_id, *portal),
        Command::Recharge { portal } => execute_recharge(state, agent_id, faction, *portal),
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
            state.neutralize_portal(portal);
            state.push_event(EventKind::PortalNeutralized {
                faction,
                agent_id,
                portal,
            });
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
                hp_after: 55
            })
        );
        assert_eq!(state.portal(PortalId(0)).map(|p| p.hp), Some(55));
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
            CommandOutcome::Ok(CommandEffect::Recharged { hp_after: 55, .. })
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
            CommandOutcome::Ok(CommandEffect::Recharged { hp_after: 55, .. })
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
}

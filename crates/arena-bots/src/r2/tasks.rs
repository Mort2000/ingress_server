//! Agent-task allocation with per-round target reservation (plan sections
//! 5.1 and 6.3): the decision layer proposes ranked [`TaskRequest`]s and
//! this module decides who does what — at most one task per agent per
//! round, at most one agent per one-shot task per round, with ONE
//! explicit exception: a coordinated attack whose declared capacity
//! admits several agents. Nothing here keys on `AgentId`: reinforcement
//! agents enter through exactly the same allocation path (no hardcoded
//! "first three build" roles), so a role is a grant, not an identity.
//!
//! [`allocate`] is a pure deterministic function of `(agents, requests,
//! fresh board)` — no RNG, no wall clock — so policy-level determinism
//! reduces to the determinism of the request ranking (plan section 6.3).

use std::collections::BTreeMap;

use arena_core::types::{AgentId, PortalId};

/// One one-shot tactical task for one agent (plan 6.3). Two tasks are the
/// same reservation when their variants and targets are equal. Link
/// directions share one undirected edge reservation; the grant retains
/// its original direction so execution consumes the correct target Key.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Task {
    /// Take a neutral portal (Major Action).
    Capture { portal: PortalId },
    /// HACK a portal for keys and Supply (the key-acquisition task).
    KeyFarm { portal: PortalId },
    /// Create the link `source -> target` (consumes a Key of `target`).
    Link { source: PortalId, target: PortalId },
    /// Attack an enemy portal. The ONLY task that may carry coordinated
    /// capacity > 1 (plan 6.3's explicit exception).
    Attack { portal: PortalId },
    /// Recharge an own anchor (Major Action, consumes Supply).
    Recharge { portal: PortalId },
    /// Hold station at `portal` — walk there and wait (the link-source
    /// pre-positioning task: the stationed agent receives a key by
    /// Transfer and fires the edge, instead of the distant key-holder
    /// walking the whole way itself).
    Standby { portal: PortalId },
    /// Rendezvous with `receiver` to deliver one key of `key` (the runner
    /// task: approach the station; the owning policy pays the payload
    /// once the receiver is inside TRANSFER_RANGE).
    Transfer { receiver: AgentId, key: PortalId },
}

impl Task {
    fn reservation(self) -> Self {
        match self {
            Self::Link { source, target } => Self::Link {
                source: source.min(target),
                target: source.max(target),
            },
            task => task,
        }
    }

    /// Stable lowercase tag for diagnostics and decision records.
    pub fn kind_tag(&self) -> &'static str {
        match self {
            Task::Capture { .. } => "capture",
            Task::KeyFarm { .. } => "key-farm",
            Task::Link { .. } => "link",
            Task::Attack { .. } => "attack",
            Task::Recharge { .. } => "recharge",
            Task::Standby { .. } => "standby",
            Task::Transfer { .. } => "transfer",
        }
    }

    /// The portal this task occupies, when it has one (Transfer targets an
    /// agent instead).
    pub fn target_portal(&self) -> Option<PortalId> {
        match self {
            Task::Capture { portal }
            | Task::KeyFarm { portal }
            | Task::Attack { portal }
            | Task::Recharge { portal }
            | Task::Standby { portal } => Some(*portal),
            Task::Link { source, .. } => Some(*source),
            Task::Transfer { .. } => None,
        }
    }
}

/// One agent's ranked request for a task: `priority` orders requests (the
/// value model's integer scores slot in directly) and
/// `assignee_capacity` declares how many agents the task admits — `1` for
/// every one-shot task, `> 1` ONLY for an explicit coordinated attack
/// (plan section 6.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TaskRequest {
    pub task: Task,
    pub priority: i64,
    pub assignee_capacity: u32,
}

impl TaskRequest {
    /// A one-shot request at `priority`.
    pub fn one_shot(task: Task, priority: i64) -> TaskRequest {
        TaskRequest {
            task,
            priority,
            assignee_capacity: 1,
        }
    }

    /// A coordinated-attack request: the single sanctioned
    /// multi-assignee exception, with its declared capacity.
    pub fn coordinated(task: Task, priority: i64, capacity: u32) -> TaskRequest {
        TaskRequest {
            task,
            priority,
            assignee_capacity: capacity.max(1),
        }
    }
}

/// One round's reservation ledger. Constructed fresh each round (claims
/// never leak across rounds — plan 6.3's per-round uniqueness); the board
/// itself never looks at agent positions or the observation, so it stays
/// a pure contention resolver.
#[derive(Clone, Debug)]
pub struct TaskBoard {
    round: u32,
    remaining: BTreeMap<Task, u32>,
    assignees: BTreeMap<Task, Vec<AgentId>>,
}

impl TaskBoard {
    /// The empty board of `round`.
    pub fn new(round: u32) -> TaskBoard {
        TaskBoard {
            round,
            remaining: BTreeMap::new(),
            assignees: BTreeMap::new(),
        }
    }

    /// The round this board belongs to.
    pub fn round(&self) -> u32 {
        self.round
    }

    /// Tries to claim `task` for `agent`. The FIRST claim fixes the
    /// task's capacity for the round (`capacity_if_new`: 1 for one-shot
    /// tasks, the declared headcount for a coordinated attack); later
    /// claims succeed only while capacity remains.
    pub fn claim(&mut self, agent: AgentId, task: Task, capacity_if_new: u32) -> bool {
        let task = task.reservation();
        let capacity = capacity_if_new.max(1);
        let remaining = self.remaining.entry(task).or_insert(capacity);
        if *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        self.assignees.entry(task).or_default().push(agent);
        true
    }

    /// How many more agents the task still admits this round (0 for a
    /// task never claimed).
    pub fn remaining_capacity(&self, task: &Task) -> u32 {
        self.remaining
            .get(&task.reservation())
            .copied()
            .unwrap_or(0)
    }

    /// The agents granted this task so far this round, in grant order.
    pub fn assignees(&self, task: &Task) -> &[AgentId] {
        self.assignees
            .get(&task.reservation())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Whether any agent holds this task this round.
    pub fn is_assigned(&self, task: &Task) -> bool {
        !self.assignees(task).is_empty()
    }
}

/// Grants at most one task per agent, resolving all contention through
/// `board`: every `(agent, request)` pair is considered once, in
/// (priority descending, agent id ascending, task ascending) order —
/// highest-value work is granted first, equal priorities go to the
/// lower agent id, and an agent falls through to its next-ranked request
/// when its preferred task is exhausted. A pure deterministic function
/// of its inputs; agents not present in `requests` receive nothing.
pub fn allocate(
    board: &mut TaskBoard,
    agents: &[AgentId],
    requests: &BTreeMap<AgentId, Vec<TaskRequest>>,
) -> BTreeMap<AgentId, Task> {
    allocate_compatible(board, agents, requests, |_, _, _| true)
}

/// The observation-aware core can reject mutually incompatible grants
/// before reserving a task; rejected agents still try lower-ranked work.
pub(crate) fn allocate_compatible(
    board: &mut TaskBoard,
    agents: &[AgentId],
    requests: &BTreeMap<AgentId, Vec<TaskRequest>>,
    compatible: impl Fn(AgentId, Task, &BTreeMap<AgentId, Task>) -> bool,
) -> BTreeMap<AgentId, Task> {
    let mut flattened: Vec<(i64, AgentId, TaskRequest)> = Vec::new();
    for &agent in agents {
        if let Some(agent_requests) = requests.get(&agent) {
            for request in agent_requests {
                flattened.push((request.priority, agent, *request));
            }
        }
    }
    flattened.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.task.cmp(&b.2.task))
    });

    let mut granted: BTreeMap<AgentId, Task> = BTreeMap::new();
    for (_, agent, request) in flattened {
        if granted.contains_key(&agent) {
            continue; // one task per agent per round
        }
        if compatible(agent, request.task, &granted)
            && board.claim(agent, request.task, request.assignee_capacity)
        {
            granted.insert(agent, request.task);
        }
    }
    granted
}

#[cfg(test)]
mod tests {
    use super::*;

    // Unit pins of the reservation semantics the integration fixtures
    // exercise through allocate().

    #[test]
    fn first_claim_fixes_capacity_even_if_declared_later() {
        let task = Task::Recharge {
            portal: PortalId(1),
        };
        let mut board = TaskBoard::new(2);
        // The first claim's capacity is the round's contract: a later,
        // larger declaration cannot widen a one-shot reservation.
        assert!(board.claim(AgentId(4), task, 1));
        assert!(!board.claim(AgentId(5), task, 3));
        assert_eq!(board.assignees(&task), &[AgentId(4)]);
    }

    #[test]
    fn zero_capacity_clamps_to_one_shot() {
        let task = Task::KeyFarm {
            portal: PortalId(0),
        };
        let mut board = TaskBoard::new(2);
        assert!(board.claim(AgentId(0), task, 0));
        assert!(!board.claim(AgentId(1), task, 0));
        assert!(board.is_assigned(&task));
    }

    #[test]
    fn task_tags_and_targets_cover_all_variants() {
        let tasks = [
            Task::Capture {
                portal: PortalId(0),
            },
            Task::KeyFarm {
                portal: PortalId(1),
            },
            Task::Link {
                source: PortalId(1),
                target: PortalId(2),
            },
            Task::Attack {
                portal: PortalId(3),
            },
            Task::Recharge {
                portal: PortalId(4),
            },
            Task::Standby {
                portal: PortalId(5),
            },
            Task::Transfer {
                receiver: AgentId(7),
                key: PortalId(2),
            },
        ];
        let mut tags: Vec<&str> = tasks.iter().map(|t| t.kind_tag()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), 7, "distinct tags per variant");
        assert_eq!(tasks[0].target_portal(), Some(PortalId(0)));
        assert_eq!(tasks[2].target_portal(), Some(PortalId(1)));
        assert_eq!(tasks[5].target_portal(), Some(PortalId(5)));
        assert_eq!(tasks[6].target_portal(), None);
    }
}

//! Session, round contexts, and submission. The lifetime chain
//! (ADR-0006/D66): `RoundContext<'round>` borrows `&'round Session`;
//! `ObservationView<'round>` borrows the context; the view therefore
//! cannot outlive `on_round`, cannot be stored in bot state, and cannot
//! reach `std::thread::spawn` (needs `'static`). Scoped threads sharing a
//! COPY of the (Send + Sync) view remain usable and join by construction.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use arena_core::config::GameConfig;
use arena_core::types::{AgentId, Command, Faction};

use arena_protocol::decode::Envelope;

use crate::SdkError;
use crate::conn::Connection;
use crate::phase::PhaseGate;
use crate::shm::{RegionHeader, ShmReader};
use crate::view::{MapView, ObservationView};

/// One authenticated client's shared state for a match: the two read-only
/// SHM mappings, the phase gate, and the rule constants. The connection
/// stays OUTSIDE the session (submissions need `&mut`; observations share
/// `&`), which is exactly the read/write split of P3.
#[derive(Debug)]
pub struct Session {
    pub(crate) public: ShmReader,
    pub(crate) private: ShmReader,
    pub(crate) phase: PhaseGate,
    /// Latest private SubmissionResult per agent (the server's first /
    /// duplicate / structural verdicts).
    results: Mutex<BTreeMap<AgentId, (u8, String)>>,
    /// The faction assigned by SERVER_HELLO.
    pub faction: Faction,
    /// The rule constants from INITIAL_SPAWN_START (MatchInfo).
    pub config: GameConfig,
    pub match_id: String,
}

impl Session {
    /// Builds a session over EXISTING regions (the participant loop does
    /// this after SERVER_HELLO/INITIAL_SPAWN_START; tests do it over
    /// fixtures). The phase starts at Connecting.
    pub fn new(match_id: &str, faction: Faction, config: GameConfig) -> Result<Self, SdkError> {
        let (public_name, enl_name, res_name) = crate::shm::region_names(match_id);
        let private_name = match faction {
            Faction::Enl => enl_name,
            Faction::Res => res_name,
        };
        let public = ShmReader::open(&public_name, RegionHeader::public())?;
        let private_kind = match faction {
            Faction::Enl => crate::shm::REGION_KIND_ENL,
            Faction::Res => crate::shm::REGION_KIND_RES,
        };
        let private = ShmReader::open(&private_name, RegionHeader::private(private_kind))?;
        Ok(Session {
            public,
            private,
            phase: PhaseGate::new(),
            results: Mutex::new(BTreeMap::new()),
            faction,
            config,
            match_id: match_id.to_string(),
        })
    }

    /// The phase gate (transitions are driven by the participant loop).
    pub fn phase(&self) -> &crate::phase::PhaseGate {
        &self.phase
    }

    /// Constructs the round context after a ROUND_START_SIGNAL. Refuses
    /// with [`SdkError::PhaseViolation`] unless the phase machine has the
    /// round open — this is the between-rounds SHM-read refusal (P3-RPC).
    pub fn round_context<'round>(
        &'round self,
        conn: &'round mut Connection,
        round: u32,
    ) -> Result<RoundContext<'round>, SdkError> {
        self.phase.assert_read_licensed()?;
        let agents = self.own_agent_ids()?;
        Ok(RoundContext {
            session: self,
            conn,
            round,
            deadline: Instant::now() + Duration::from_millis(self.config.decision_window_ms as u64),
            remaining_agents: agents,
            closed: None,
        })
    }

    /// The own-faction agent ids of the currently published private
    /// snapshot (reinforcements included).
    pub fn own_agent_ids(&self) -> Result<Vec<AgentId>, SdkError> {
        self.phase.assert_read_licensed()?;
        let snapshot = self.private.read()?;
        let decoded = arena_protocol::decode::decode_private(snapshot.payload)
            .map_err(|e| SdkError::Protocol(e.to_string()))?;
        Ok(decoded.agents.into_iter().map(|agent| agent.id).collect())
    }

    /// Records a server SubmissionResult (private feedback surface).
    pub(crate) fn record_result(&self, agent: AgentId, status: u8, detail: String) {
        self.results
            .lock()
            .expect("uncontended")
            .insert(agent, (status, detail));
    }

    /// The latest server verdict for one agent's submission, if any.
    pub fn last_submission_result(&self, agent: AgentId) -> Option<(u8, String)> {
        self.results
            .lock()
            .expect("uncontended")
            .get(&agent)
            .cloned()
    }
}

/// Acknowledgement of a locally accepted submission. The server's verdict
/// arrives asynchronously as a SUBMISSION_RESULT frame and is surfaced via
/// [`Session::last_submission_result`] — a submitted stream is never
/// re-sent (first submission freezes, ADR-0004).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubmissionAck {
    /// The frame was written; the slot is now client-side closed.
    Sent { round: u32, agent: AgentId },
}

/// One round's participation context (P3-RPC SDK contract). Constructed
/// per ROUND_START_SIGNAL; closes on the last agent submission, an
/// explicit commit, or the local deadline — afterwards every borrow or
/// submission is [`SdkError::ContextClosed`].
pub struct RoundContext<'round> {
    pub(crate) session: &'round Session,
    pub(crate) conn: &'round mut Connection,
    round: u32,
    deadline: Instant,
    /// Agents that have not yet submitted this round.
    remaining_agents: Vec<AgentId>,
    closed: Option<ContextClose>,
}

/// Why a context closed (commit is idempotent after completion; a
/// deadline-closed context refuses everything).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContextClose {
    /// All agents submitted, or a COMMIT_ROUND was already sent: the
    /// faction's input is complete.
    Completed,
    /// The local deadline passed.
    Deadline,
}

impl<'round> RoundContext<'round> {
    /// The round number this context licenses.
    pub fn round(&self) -> u32 {
        self.round
    }

    /// Milliseconds until the local deadline (the server deadline is
    /// authoritative; the SDK clock starts at ROUND_START_SIGNAL receipt).
    pub fn remaining_ms(&self) -> u64 {
        self.deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as u64
    }

    /// Whether the context has closed (last submit, commit, deadline).
    pub fn is_closed(&self) -> bool {
        self.closed.is_some()
    }

    /// The zero-copy observation of the round-start snapshots. Refuses
    /// after close ([`SdkError::ContextClosed`]) and outside the licensed
    /// phase ([`SdkError::PhaseViolation`]).
    pub fn observation(&self) -> Result<ObservationView<'round>, SdkError> {
        if self.closed.is_some() {
            return Err(SdkError::ContextClosed);
        }
        self.session.phase.assert_read_licensed()?;
        let public = self.session.public.read()?;
        let private = self.session.private.read()?;
        // The view captures the phase gate with the same lifetime as the
        // lent payload: every accessor re-asserts the read is licensed, so
        // a view copy held past ROUND_CLOSED refuses (Settlement Phase).
        ObservationView::new(public.payload, private.payload, &self.session.phase)
    }

    /// Submits one agent's ordered command stream as one frame. The first
    /// submission of the LAST unsubmitted agent closes the context.
    pub fn submit(
        &mut self,
        agent: AgentId,
        commands: Vec<Command>,
    ) -> Result<SubmissionAck, SdkError> {
        if self.closed.is_some() {
            return Err(SdkError::ContextClosed);
        }
        if Instant::now() >= self.deadline {
            self.closed = Some(ContextClose::Deadline);
            return Err(SdkError::ContextClosed);
        }
        self.conn.send(&Envelope::SubmitCommandStream {
            round: self.round,
            agent_id: agent,
            commands,
        })?;
        self.remaining_agents.retain(|&open| open != agent);
        if self.remaining_agents.is_empty() {
            self.closed = Some(ContextClose::Completed);
        }
        Ok(SubmissionAck::Sent {
            round: self.round,
            agent,
        })
    }

    /// Explicitly freezes this faction's remaining agents as passes
    /// (COMMIT_ROUND); closes the context. IDEMPOTENT after completion
    /// (submitting every agent already completed this faction's input):
    /// the SDK sends COMMIT_ROUND at most once and a redundant commit is
    /// accepted as a no-op. After a DEADLINE closure it is an error like
    /// every other use.
    pub fn commit(&mut self) -> Result<(), SdkError> {
        if self.closed == Some(ContextClose::Completed) {
            return Ok(());
        }
        if self.closed == Some(ContextClose::Deadline) {
            return Err(SdkError::ContextClosed);
        }
        self.conn
            .send(&Envelope::CommitRound { round: self.round })?;
        self.closed = Some(ContextClose::Completed);
        Ok(())
    }
}

/// The pre-match context: the complete frozen map is readable (generation
/// 0 of the public region) while the spawn window runs.
pub struct SpawnContext<'spawn> {
    pub(crate) session: &'spawn Session,
}

impl<'spawn> SpawnContext<'spawn> {
    /// The frozen map view (one seqlock read of the public region).
    pub fn map_view(&self) -> Result<MapView<'spawn>, SdkError> {
        self.session.phase.assert_read_licensed()?;
        let snapshot = self.session.public.read()?;
        MapView::new(snapshot.payload, &self.session.phase)
    }

    /// The rule constants for this match.
    pub fn config(&self) -> &GameConfig {
        &self.session.config
    }

    /// This faction's initial roster ids (derived from the config, D17 —
    /// the private generation-0 snapshot carries exactly these).
    pub fn roster(&self) -> Vec<AgentId> {
        let n = self.session.config.initial_agents_per_faction;
        let offset = match self.session.faction {
            Faction::Enl => 0,
            Faction::Res => n,
        };
        (offset..offset + n).map(AgentId).collect()
    }

    /// The assigned faction.
    pub fn faction(&self) -> Faction {
        self.session.faction
    }
}

impl Session {
    /// Constructs the spawn context after INITIAL_SPAWN_START.
    pub fn spawn_context(&self) -> Result<SpawnContext<'_>, SdkError> {
        self.phase.assert_read_licensed()?;
        Ok(SpawnContext { session: self })
    }
}

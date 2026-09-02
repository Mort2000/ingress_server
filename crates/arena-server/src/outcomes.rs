//! Match outcomes and per-faction metrics (P3-GATE, D70). Technical
//! terminations are never dressed up as score results (CONTEXT Match
//! Outcome).

use std::collections::BTreeMap;

use arena_core::types::{Faction, MatchResult, Score2};

/// What the runner records for one match.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MatchOutcome {
    /// All rounds completed; the plain score comparison decides.
    Normal {
        result: MatchResult,
        score_enl: Score2,
        score_res: Score2,
        rounds_completed: u32,
    },
    /// One faction's Initial Spawn Set was internally invalid or missed
    /// the pre-match deadline; `winner` is `None` when both failed
    /// (unscored, excluded from Elo).
    SpawnForfeit {
        winner: Option<Faction>,
        reason_losing: String,
    },
    /// One client exited / EOF'd / over-limited after the match started;
    /// `winner` is `None` when the server lost both (unscored
    /// infrastructure error).
    ClientForfeit {
        winner: Option<Faction>,
        at_round: u32,
        reason_losing: String,
    },
    /// Startup infrastructure error (version mismatch, handshake window
    /// expired, transport fault before INITIAL_SPAWN_START): always
    /// unscored, never a SPAWN_FORFEIT (P3-RPC).
    SetupError { reason: String },
}

impl MatchOutcome {
    /// True when the match produced no scoreable result.
    pub fn is_unscored(&self) -> bool {
        match self {
            MatchOutcome::Normal { .. } => false,
            MatchOutcome::SpawnForfeit { winner, .. }
            | MatchOutcome::ClientForfeit { winner, .. } => winner.is_none(),
            MatchOutcome::SetupError { .. } => true,
        }
    }

    /// Stable outcome tag for journals and exit codes.
    pub fn tag(&self) -> &'static str {
        match self {
            MatchOutcome::Normal { result, .. } => match result {
                MatchResult::EnlWin => "enl_win",
                MatchResult::ResWin => "res_win",
                MatchResult::Draw => "draw",
            },
            MatchOutcome::SpawnForfeit { winner, .. } => match winner {
                Some(Faction::Enl) => "spawn_forfeit_res_loses",
                Some(Faction::Res) => "spawn_forfeit_enl_loses",
                None => "spawn_forfeit_unscored",
            },
            MatchOutcome::ClientForfeit { winner, .. } => match winner {
                Some(Faction::Enl) => "client_forfeit_res_loses",
                Some(Faction::Res) => "client_forfeit_enl_loses",
                None => "client_forfeit_unscored",
            },
            MatchOutcome::SetupError { .. } => "setup_error",
        }
    }

    /// `GameEnd.outcome` byte (arena-protocol codes).
    pub fn game_end_code(&self) -> u8 {
        match self {
            MatchOutcome::Normal { result, .. } => match result {
                MatchResult::EnlWin => arena_protocol::decode::END_ENL_WIN,
                MatchResult::ResWin => arena_protocol::decode::END_RES_WIN,
                MatchResult::Draw => arena_protocol::decode::END_DRAW,
            },
            MatchOutcome::SpawnForfeit { .. } => arena_protocol::decode::END_SPAWN_FORFEIT,
            MatchOutcome::ClientForfeit { .. } => arena_protocol::decode::END_CLIENT_FORFEIT,
            MatchOutcome::SetupError { .. } => arena_protocol::decode::END_SETUP_ERROR,
        }
    }
}

/// Per-faction submission metrics (P3-GATE: latency, bucket distribution,
/// timeout rate; conflict-win-rate is deferred to offline replay analysis
/// — recorded as a deviation in progress.md).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FactionMetrics {
    /// `received_at_ms` (ms since round start) histogram, 1 ms bins.
    pub latency_histogram_ms: BTreeMap<u64, u64>,
    /// Priority-bucket distribution of EXECUTED streams (from RoundReport).
    pub bucket_distribution: BTreeMap<u64, u64>,
    /// Slot count frozen as NotSubmitted by a MISSED deadline (the true
    /// timeout rate numerator).
    pub not_submitted_slots: u64,
    /// Slot count frozen by a first submission (accepted or structural).
    pub submitted_slots: u64,
    /// Slots frozen as deliberate empty passes by a prompt COMMIT_ROUND.
    pub committed_passes: u64,
    /// Rounds this faction was notified first (parity journal fact).
    pub notified_first_rounds: u64,
}

impl FactionMetrics {
    /// Total frames counted in the latency histogram.
    pub fn total_submissions(&self) -> u64 {
        self.latency_histogram_ms.values().sum()
    }

    /// Missed-deadline share of all slots (P3-GATE timeout rate); prompt
    /// COMMIT_ROUND passes do not count as timeouts.
    pub fn timeout_rate_permille(&self) -> u64 {
        let total = self.not_submitted_slots + self.submitted_slots + self.committed_passes;
        if total == 0 {
            return 0;
        }
        self.not_submitted_slots * 1000 / total
    }
}

/// Key=value serialization of the whole result file (hand-rolled,
/// std-only, like the M6 artifacts).
pub fn result_file_text(
    outcome: &MatchOutcome,
    metrics: &[FactionMetrics; 2],
    rounds_completed: u32,
) -> String {
    let mut out = String::new();
    out.push_str("outcome=");
    out.push_str(outcome.tag());
    out.push('\n');
    out.push_str(&format!("rounds_completed={rounds_completed}\n"));
    match outcome {
        MatchOutcome::Normal {
            score_enl,
            score_res,
            ..
        } => {
            out.push_str(&format!("score_enl={score_enl}\n"));
            out.push_str(&format!("score_res={score_res}\n"));
        }
        MatchOutcome::SpawnForfeit { reason_losing, .. }
        | MatchOutcome::ClientForfeit { reason_losing, .. } => {
            out.push_str(&format!("reason_losing={reason_losing}\n"))
        }
        MatchOutcome::SetupError { reason } => {
            out.push_str(&format!("reason={reason}\n"));
        }
    }
    for (faction, tag) in [(Faction::Enl, "enl"), (Faction::Res, "res")] {
        let index = faction_index(faction);
        let m = &metrics[index];
        out.push_str(&format!(
            "{tag}_submissions={}\n{tag}_not_submitted={}\n{tag}_committed_passes={}\n{tag}_timeout_rate_permille={}\n{tag}_notified_first_rounds={}\n",
            m.total_submissions(),
            m.not_submitted_slots,
            m.committed_passes,
            m.timeout_rate_permille(),
            m.notified_first_rounds
        ));
        let mut buckets = String::new();
        for (bucket, count) in &m.bucket_distribution {
            buckets.push_str(&format!("{bucket}:{count},"));
        }
        out.push_str(&format!("{tag}_buckets={buckets}\n"));
        let mut latencies = String::new();
        for (ms, count) in m.latency_histogram_ms.iter().take(200) {
            latencies.push_str(&format!("{ms}:{count},"));
        }
        out.push_str(&format!("{tag}_latency_ms={latencies}\n"));
    }
    out
}

/// Index of a faction in the two-slot arrays (ENL first).
pub fn faction_index(faction: Faction) -> usize {
    match faction {
        Faction::Enl => 0,
        Faction::Res => 1,
    }
}

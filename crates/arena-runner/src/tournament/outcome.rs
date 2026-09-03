//! Outcome normalization (D82, CONTEXT "Match Outcome"): the tournament's
//! one vocabulary over both execution paths. A technical termination
//! (spawn forfeit, client forfeit) is NEVER dressed up as a score-based
//! result, and infrastructure faults stay unscored; only `Normal` moves
//! ratings and W/D/L counters (`elo_relevant`).

use arena_core::types::{Faction, MatchResult, Score2};
use arena_server::MatchOutcome;

use crate::tournament::entrant::EntrantId;

/// Where a technical/unscored outcome happened (archived with every
/// match; `meta.json` also carries the private reason text).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    SpawnForfeit,
    ClientForfeit,
    SetupError,
    /// Runner-side fault: a panic or io error inside the tournament
    /// executor itself (never the game's verdict).
    RunnerFault,
}

impl Stage {
    pub fn tag(self) -> &'static str {
        match self {
            Stage::SpawnForfeit => "spawn_forfeit",
            Stage::ClientForfeit => "client_forfeit",
            Stage::SetupError => "setup_error",
            Stage::RunnerFault => "runner_fault",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Stage> {
        match tag {
            "spawn_forfeit" => Some(Stage::SpawnForfeit),
            "client_forfeit" => Some(Stage::ClientForfeit),
            "setup_error" => Some(Stage::SetupError),
            "runner_fault" => Some(Stage::RunnerFault),
            _ => None,
        }
    }
}

/// The tournament-level Match Outcome taxonomy (D82).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TournamentOutcome {
    /// All rounds completed; the plain score comparison decides.
    Normal {
        result: MatchResult,
        score_enl: Score2,
        score_res: Score2,
        rounds_completed: u32,
        final_state_hash: [u8; 32],
    },
    /// One side failed a spawn/window/transport contract; the OTHER
    /// entrant wins. Never counts as W/D/L.
    TechnicalWin {
        winner: EntrantId,
        stage: Stage,
        reason: String,
    },
    /// Both sides failed, or infrastructure failed: excluded from Elo.
    Unscored { stage: Stage, reason: String },
}

impl TournamentOutcome {
    /// Stable outcome tag for `index.tsv` (`Normal` reports its result).
    pub fn outcome_tag(&self) -> &'static str {
        match self {
            TournamentOutcome::Normal { result, .. } => match result {
                MatchResult::EnlWin => "enl_win",
                MatchResult::ResWin => "res_win",
                MatchResult::Draw => "draw",
            },
            TournamentOutcome::TechnicalWin { .. } => "technical_win",
            TournamentOutcome::Unscored { .. } => "unscored",
        }
    }

    /// The technical stage, if any.
    pub fn stage(&self) -> Option<Stage> {
        match self {
            TournamentOutcome::Normal { .. } => None,
            TournamentOutcome::TechnicalWin { stage, .. }
            | TournamentOutcome::Unscored { stage, .. } => Some(*stage),
        }
    }

    /// Technical reasons are archived privately in `meta.json`.
    pub fn reason(&self) -> Option<&str> {
        match self {
            TournamentOutcome::Normal { .. } => None,
            TournamentOutcome::TechnicalWin { reason, .. }
            | TournamentOutcome::Unscored { reason, .. } => Some(reason),
        }
    }

    /// Only `Normal` outcomes move ratings and W/D/L counters (D82).
    pub fn elo_relevant(&self) -> bool {
        matches!(self, TournamentOutcome::Normal { .. })
    }

    /// The winning ENTRANT of a `Normal` outcome (None on a draw).
    pub fn normal_winner(&self, a: EntrantId, b: EntrantId, a_is_enl: bool) -> Option<EntrantId> {
        match self {
            TournamentOutcome::Normal { result, .. } => winner_entrant(*result, a, b, a_is_enl),
            _ => None,
        }
    }

    /// Attaches the replayed final state hash to a `Normal` outcome (the
    /// server report itself carries no hash; the executor decodes the
    /// archived IARP to fill this in).
    pub fn with_final_hash(self, hash: [u8; 32]) -> Self {
        match self {
            TournamentOutcome::Normal {
                result,
                score_enl,
                score_res,
                rounds_completed,
                ..
            } => TournamentOutcome::Normal {
                result,
                score_enl,
                score_res,
                rounds_completed,
                final_state_hash: hash,
            },
            other => other,
        }
    }
}

/// The winning entrant of a result under the item's faction assignment.
pub fn winner_entrant(
    result: MatchResult,
    a: EntrantId,
    b: EntrantId,
    a_is_enl: bool,
) -> Option<EntrantId> {
    match result {
        MatchResult::Draw => None,
        MatchResult::EnlWin => Some(if a_is_enl { a } else { b }),
        MatchResult::ResWin => Some(if a_is_enl { b } else { a }),
    }
}

/// Normalizes a served match's outcome. `a`/`b`/`a_is_enl` are the item's
/// entrants and faction assignment; the winning FACTION maps back to the
/// winning ENTRANT. The `Normal` variant comes back with a zero
/// `final_state_hash` — the executor fills it from the archived replay
/// (`with_final_hash`).
pub fn from_server(
    outcome: &MatchOutcome,
    a: EntrantId,
    b: EntrantId,
    a_is_enl: bool,
) -> TournamentOutcome {
    let entrant_of = |faction: Faction| {
        if (faction == Faction::Enl) == a_is_enl {
            a
        } else {
            b
        }
    };
    match outcome {
        MatchOutcome::Normal {
            result,
            score_enl,
            score_res,
            rounds_completed,
        } => TournamentOutcome::Normal {
            result: *result,
            score_enl: *score_enl,
            score_res: *score_res,
            rounds_completed: *rounds_completed,
            final_state_hash: [0u8; 32],
        },
        MatchOutcome::SpawnForfeit {
            winner,
            reason_losing,
        } => match winner {
            Some(faction) => TournamentOutcome::TechnicalWin {
                winner: entrant_of(*faction),
                stage: Stage::SpawnForfeit,
                reason: reason_losing.clone(),
            },
            None => TournamentOutcome::Unscored {
                stage: Stage::SpawnForfeit,
                reason: reason_losing.clone(),
            },
        },
        MatchOutcome::ClientForfeit {
            winner,
            reason_losing,
            ..
        } => match winner {
            Some(faction) => TournamentOutcome::TechnicalWin {
                winner: entrant_of(*faction),
                stage: Stage::ClientForfeit,
                reason: reason_losing.clone(),
            },
            None => TournamentOutcome::Unscored {
                stage: Stage::ClientForfeit,
                reason: reason_losing.clone(),
            },
        },
        MatchOutcome::SetupError { reason } => TournamentOutcome::Unscored {
            stage: Stage::SetupError,
            reason: reason.clone(),
        },
    }
}

/// The in-process path's `Normal` mapping (M6 engine + M5 recorder).
pub fn normal_inprocess(
    result: MatchResult,
    score_enl: Score2,
    score_res: Score2,
    rounds_completed: u32,
    final_state_hash: [u8; 32],
) -> TournamentOutcome {
    TournamentOutcome::Normal {
        result,
        score_enl,
        score_res,
        rounds_completed,
        final_state_hash,
    }
}

/// The in-process crash mapping (a panicking bot/harness is a RUNNER-side
/// fault in tournament mode — never a score result).
pub fn inprocess_crash(reason: &str) -> TournamentOutcome {
    TournamentOutcome::Unscored {
        stage: Stage::RunnerFault,
        reason: reason.to_string(),
    }
}

//! Deterministic replay engine (P1-REPLAY, P5-REPLAY, D46/D50).
//!
//! [`replay`] rebuilds the match from the IARP header alone — the map is
//! LOADED from the file and never regenerated (R-MAP-02, R-RNG-01) — and
//! re-executes every round by feeding the recorded submission slice (first
//! submissions AND ignored duplicates AND unlocatable envelopes, exactly as
//! they arrived) back through [`arena_core::resolve::Resolver::run_round`].
//! Recorded outcomes are never trusted as inputs; each closed round is
//! verified against the record instead:
//!
//! * recomputed D45 state hash vs recorded hash (first divergence names
//!   the round);
//! * recomputed `score2_delta` vs recorded delta;
//! * recomputed new broadcast events vs recorded events (events are
//!   hash-EXCLUDED, so this comparison is their only determinism check);
//! * re-derived intake facts (unlocatable, ignored duplicates, execution
//!   order, slot statuses) vs the recorded ones.
//!
//! At the end the final hash must equal the trailer's and the replayed
//! `final_result` must equal the recorded result tag (P1-REPLAY: the
//! result is only ever ENL_WIN / RES_WIN / DRAW).

use arena_core::config::GameConfig;
use arena_core::resolve::Resolver;
use arena_core::spawn::{InitialSpawns, validate_initial_spawn_set};
use arena_core::state::GameState;
use arena_core::types::{Faction, MatchResult, Score2};

use crate::canon;
use crate::format::{EndOfMatch, ReplayError, ReplayFile, decode};

/// What a successful replay established.
#[derive(Debug)]
pub struct ReplayOutcome {
    /// Rounds replayed and verified.
    pub rounds_completed: u32,
    /// Final D45 state hash (equals the trailer's recorded hash).
    pub final_state_hash: [u8; 32],
    /// `(enl, res)` final scores of the replayed state.
    pub final_scores: (Score2, Score2),
    /// The replayed state's score comparison — for a normally-ended log
    /// this equals the recorded result tag.
    pub result: MatchResult,
    /// Set when the log ended abnormally: the recorded outcome tag
    /// (technical ending; the replay verifies rounds and the last valid
    /// hash but cannot re-derive the technical cause).
    pub abnormal_outcome: Option<u8>,
    /// The fully replayed final state (non-vacuity checks, viewer, tests).
    pub state: GameState,
}

/// Decodes IARP bytes (structural validation, version checks) — the entry
/// point for anything holding a file.
pub fn decode_log(bytes: &[u8]) -> Result<ReplayFile, ReplayError> {
    decode(bytes)
}

/// Replays a decoded log end to end, verifying every round. See the module
/// docs for the exact verification set.
pub fn replay(log: &ReplayFile) -> Result<ReplayOutcome, ReplayError> {
    let header = &log.header;

    // Version agreement is checked by decode() already; in-memory files
    // get the same gate here so the engine never runs against a foreign
    // RNG or state encoding (R-RNG-01: rng_version is rule version).
    if header.rng_version != arena_core::rng::RNG_VERSION {
        return Err(ReplayError::RngVersion {
            found: header.rng_version,
            expected: arena_core::rng::RNG_VERSION,
        });
    }
    if header.state_encoding_version != canon::STATE_ENCODING_VERSION {
        return Err(ReplayError::StateEncodingVersion {
            found: header.state_encoding_version,
            expected: canon::STATE_ENCODING_VERSION,
        });
    }
    if header.server_version != crate::format::SERVER_VERSION {
        return Err(ReplayError::ServerVersion {
            found: header.server_version,
            expected: crate::format::SERVER_VERSION,
        });
    }

    // --- Rebuild the match from header inputs only ---------------------
    // The map comes from the file; the generator is never invoked.
    let enl_spawns = initial_spawns(&header.config, Faction::Enl, &header.enl_spawns)?;
    let res_spawns = initial_spawns(&header.config, Faction::Res, &header.res_spawns)?;
    let mut state =
        GameState::new(header.config, &header.map, &enl_spawns, &res_spawns).map_err(|_| {
            ReplayError::SetupInvalid {
                what: "state construction",
            }
        })?;
    let resolver = Resolver::with_seed(header.game_seed);

    // --- Re-execute and verify every round ------------------------------
    let mut events_seen = 0usize;
    for record in log.round_records() {
        let report = resolver.run_round(&mut state, &record.submissions);
        state.advance_round();

        // Hash first (the primary divergence locator) ...
        let actual_hash = canon::state_hash(&state);
        if actual_hash != record.state_hash {
            return Err(ReplayError::HashMismatch {
                round: record.round,
                expected_hex: canon::hex(&record.state_hash),
                actual_hex: canon::hex(&actual_hash),
            });
        }

        // ... then the hash-excluded records: events, delta, intake facts.
        let new_events = &state.events()[events_seen..];
        if new_events != record.events.as_slice() {
            return Err(ReplayError::EventMismatch {
                round: record.round,
            });
        }
        events_seen = state.events().len();

        let delta = crate::recorder::delta_pair(&report.score2_delta);
        if delta != record.score2_delta {
            return Err(ReplayError::DeltaMismatch {
                round: record.round,
            });
        }
        if report.unlocatable != record.unlocatable {
            return Err(ReplayError::IntakeMismatch {
                round: record.round,
                what: "unlocatable",
            });
        }
        if report.ignored_duplicates != record.ignored_duplicates {
            return Err(ReplayError::IntakeMismatch {
                round: record.round,
                what: "ignored duplicates",
            });
        }
        let replayed_order: Vec<_> = report.execution_order.iter().map(|s| s.agent_id).collect();
        if replayed_order != record.execution_order {
            return Err(ReplayError::IntakeMismatch {
                round: record.round,
                what: "execution order",
            });
        }
        if report.slots != record.slots {
            return Err(ReplayError::IntakeMismatch {
                round: record.round,
                what: "slot statuses",
            });
        }
    }

    // --- Trailer --------------------------------------------------------
    let trailer = log.trailer().ok_or(ReplayError::MissingTrailer)?;
    let final_state_hash = canon::state_hash(&state);
    let result = state.final_result();
    let rounds_completed = log.round_records().count() as u32;
    match trailer {
        EndOfMatch::Normal {
            final_state_hash: recorded_hash,
            result: recorded_result,
            ..
        } => {
            if final_state_hash != *recorded_hash {
                return Err(ReplayError::FinalHashMismatch {
                    expected_hex: canon::hex(recorded_hash),
                    actual_hex: canon::hex(&final_state_hash),
                });
            }
            if result != *recorded_result {
                return Err(ReplayError::ResultMismatch {
                    recorded: *recorded_result,
                    replayed: result,
                });
            }
            Ok(ReplayOutcome {
                rounds_completed,
                final_state_hash,
                final_scores: (state.score(Faction::Enl), state.score(Faction::Res)),
                result,
                abnormal_outcome: None,
                state,
            })
        }
        EndOfMatch::Abnormal {
            last_valid_hash,
            outcome_tag,
            ..
        } => {
            // A technical ending: verify what is verifiable (every round
            // above plus the last valid hash); the outcome tag is archival.
            if final_state_hash != *last_valid_hash {
                return Err(ReplayError::FinalHashMismatch {
                    expected_hex: canon::hex(last_valid_hash),
                    actual_hex: canon::hex(&final_state_hash),
                });
            }
            Ok(ReplayOutcome {
                rounds_completed,
                final_state_hash,
                final_scores: (state.score(Faction::Enl), state.score(Faction::Res)),
                result,
                abnormal_outcome: Some(*outcome_tag),
                state,
            })
        }
    }
}

/// Convenience: decode + replay in one step.
pub fn replay_bytes(bytes: &[u8]) -> Result<ReplayOutcome, ReplayError> {
    replay(&decode(bytes)?)
}

/// Rebuilds a validated [`InitialSpawns`] from the header's spawn list.
/// `pub(crate)`: the offline conflict-analysis pass (crate::analysis)
/// rebuilds the same state without duplicating the validation mapping.
pub(crate) fn initial_spawns(
    cfg: &GameConfig,
    faction: Faction,
    entries: &[(arena_core::types::AgentId, arena_core::types::Coord)],
) -> Result<InitialSpawns, ReplayError> {
    let (enl_roster, res_roster) = GameState::initial_roster(cfg);
    let roster = if faction == Faction::Enl {
        &enl_roster
    } else {
        &res_roster
    };
    validate_initial_spawn_set(roster, entries, cfg)
        .map_err(|_| ReplayError::SetupInvalid { what: "spawn set" })
}

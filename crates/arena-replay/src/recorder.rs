//! Match recorder (D48): a thin, additive wrapper over the public harness
//! API that produces an IARP file while a match runs. The harness is
//! untouched — the caller drives the loop and hands each `RoundReport`
//! plus the post-advance state to the writer:
//!
//! ```ignore
//! let mut writer = ReplayWriter::new(game_seed, &cfg, &map, &enl, &res);
//! for _ in 0..cfg.total_rounds {
//!     let report = harness.run_round();
//!     writer.record_round(&report, harness.state())?;
//! }
//! let bytes = writer.finish(harness.state())?;
//! ```
//!
//! The per-round state hash is taken at the frozen D45 point (after
//! `run_round` — settle + reinforcement — AND `advance_round`, epoch sweep
//! included), events are sliced from the append-only log, and a checkpoint
//! lands after every 50th round (D49).

use arena_core::config::GameConfig;
use arena_core::map::MapDefinition;
use arena_core::resolve::RoundReport;
use arena_core::rng::RNG_VERSION;
use arena_core::spawn::InitialSpawns;
use arena_core::state::GameState;
use arena_core::types::{Faction, Score2};

use crate::canon;
use crate::format::{
    Block, CHECKPOINT_INTERVAL, Checkpoint, EndOfMatch, NotifiedFirst, PROTOCOL_VERSION,
    ReplayError, ReplayFile, ReplayHeader, RoundRecord, SERVER_VERSION,
};

/// Records one in-process match into an IARP file.
#[derive(Debug)]
pub struct ReplayWriter {
    header: ReplayHeader,
    blocks: Vec<Block>,
    rounds_recorded: u32,
    /// How many events of the append-only log already landed in a record.
    events_written: usize,
}

impl ReplayWriter {
    /// Writes the header from the match's deterministic setup inputs (D46):
    /// versions, `game_seed` (D15), config, the FROZEN map, and both spawn
    /// sets — everything a replay needs and nothing it must not choose for
    /// itself.
    pub fn new(
        game_seed: u64,
        cfg: &GameConfig,
        map: &MapDefinition,
        enl: &InitialSpawns,
        res: &InitialSpawns,
    ) -> Self {
        let header = ReplayHeader {
            identity: None,
            server_version: SERVER_VERSION,
            protocol_version: PROTOCOL_VERSION,
            rng_version: RNG_VERSION,
            state_encoding_version: canon::STATE_ENCODING_VERSION,
            game_seed,
            total_rounds: cfg.total_rounds,
            config: *cfg,
            map: map.clone(),
            enl_spawns: enl.iter().collect(),
            res_spawns: res.iter().collect(),
        };
        ReplayWriter {
            header,
            blocks: Vec::new(),
            rounds_recorded: 0,
            events_written: 0,
        }
    }

    pub fn with_identity(mut self, identity: crate::format::ReplayIdentity) -> Self {
        self.header.identity = Some(identity);
        self
    }

    /// Records one closed round. `state` must be the post-`advance_round`
    /// state (`state.round() == r + 1`) — the D45 hash point; the recorded
    /// hash covers exactly what round `r + 1` reads.
    ///
    /// Errors only on misuse (a state that did not advance exactly one
    /// round), which would corrupt the file's round sequence.
    pub fn record_round(
        &mut self,
        report: &RoundReport,
        state: &GameState,
    ) -> Result<(), ReplayError> {
        let round = state
            .round()
            .checked_sub(1)
            .ok_or(ReplayError::RoundSequence {
                expected: self.rounds_recorded + 1,
                found: state.round(),
            })?;
        if round != self.rounds_recorded + 1 {
            return Err(ReplayError::RoundSequence {
                expected: self.rounds_recorded + 1,
                found: round,
            });
        }

        let state_hash = canon::state_hash(state);
        let score2_delta = (
            report.score2_delta.get(&Faction::Enl).copied().unwrap_or(0),
            report.score2_delta.get(&Faction::Res).copied().unwrap_or(0),
        );
        let record = RoundRecord {
            round,
            notified_first: NotifiedFirst::for_round(round),
            submissions: report.submissions.clone(),
            unlocatable: report.unlocatable.clone(),
            ignored_duplicates: report.ignored_duplicates.clone(),
            execution_order: report.execution_order.iter().map(|s| s.agent_id).collect(),
            slots: report.slots.clone(),
            events: state.events()[self.events_written..].to_vec(),
            score2_delta,
            state_hash,
        };
        self.events_written = state.events().len();
        self.blocks.push(Block::Round(record));
        self.rounds_recorded += 1;

        if round % CHECKPOINT_INTERVAL == 0 {
            self.blocks.push(Block::Checkpoint(Checkpoint {
                round,
                state_hash,
                canonical_state: canon::canonical_state_bytes(state),
            }));
        }
        Ok(())
    }

    /// Number of rounds recorded so far.
    pub fn rounds_recorded(&self) -> u32 {
        self.rounds_recorded
    }

    /// Writes the EndOfMatch trailer and serializes the file. The finished
    /// bytes are decoded once as a self-check — a recorder bug surfaces as
    /// an error here instead of producing an unreadable archive.
    pub fn finish(self, state: &GameState) -> Result<Vec<u8>, ReplayError> {
        let settled = state
            .round()
            .checked_sub(1)
            .ok_or(ReplayError::RoundSequence {
                expected: self.rounds_recorded + 1,
                found: state.round(),
            })?;
        if settled != self.rounds_recorded {
            return Err(ReplayError::RoundSequence {
                expected: self.rounds_recorded + 1,
                found: settled,
            });
        }
        let final_state_hash = canon::state_hash(state);
        let mut blocks = self.blocks;
        blocks.push(Block::End(EndOfMatch::Normal {
            rounds_completed: self.rounds_recorded,
            final_state_hash,
            result: state.final_result(),
        }));
        let file = ReplayFile {
            header: self.header,
            blocks,
        };
        let bytes = file.encode();
        crate::format::decode(&bytes)?;
        Ok(bytes)
    }

    /// Writes an ABNORMAL EndOfMatch trailer (D46): the match ended
    /// technically (M7 SPAWN_FORFEIT / CLIENT_FORFEIT) instead of running
    /// to `total_rounds`. `state` is whatever authoritative state exists at
    /// the abort — its hash is the "last valid hash" — and
    /// `last_completed_round` is the number of fully recorded rounds (a
    /// round interrupted before settlement is NOT completed). Added in M7:
    /// in-process M5 always ended normally, so this path was exercised only
    /// by hand-crafted bytes; the external server now ends real matches
    /// this way.
    pub fn finish_abnormal(
        self,
        state: &GameState,
        outcome_tag: u8,
    ) -> Result<Vec<u8>, ReplayError> {
        let mut blocks = self.blocks;
        blocks.push(Block::End(EndOfMatch::Abnormal {
            last_completed_round: self.rounds_recorded,
            last_valid_hash: canon::state_hash(state),
            outcome_tag,
        }));
        let file = ReplayFile {
            header: self.header,
            blocks,
        };
        let bytes = file.encode();
        crate::format::decode(&bytes)?;
        Ok(bytes)
    }
}

/// Reads a `(enl, res)` score pair out of a settled delta map (kept for
/// callers that want the same convention; both factions are always
/// present).
pub fn delta_pair(map: &std::collections::BTreeMap<Faction, Score2>) -> (Score2, Score2) {
    (
        map.get(&Faction::Enl).copied().unwrap_or(0),
        map.get(&Faction::Res).copied().unwrap_or(0),
    )
}

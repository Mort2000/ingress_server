//! IARP ("Ingress Arena Replay") binary file format, version 1 (D46).
//!
//! Layout (every integer little-endian, no padding — D2/D51):
//!
//! ```text
//! Header: magic b"IARP" | format_version u32=1 | server_version u32 |
//!         protocol_version u32 | rng_version u32 | state_encoding_version u32 |
//!         game_seed u64 | total_rounds u32 (echo of cfg.total_rounds) |
//!         config (shared 124-byte codec) |
//!         map: u32 n + per portal (owner u8, hp u32, x u32, y u32) |
//!         spawns ENL then RES: u32 count + (agent u32, x u32, y u32) ascending
//! Blocks: tag u8 — 1 = RoundRecord, 2 = Checkpoint, 3 = EndOfMatch
//! RoundRecord: round u32 | notified_first u8 | submissions (u32 count +
//!         per submission: round u32, faction u8, agent u32, received_at_ms u64,
//!         receive_seq u32, commands u32 count + per command kind_tag u8 +
//!         fixed payload) | unlocatable (u32 count + tagged payloads) |
//!         ignored_duplicates (u32 count + agent u32 each) |
//!         execution_order (u32 count + agent u32 each) |
//!         slots (u32 count + per: agent u32, slot tag u8 + payload) |
//!         events (u32 count + per: seq u64, round u32, kind tag u8 + payload) |
//!         score2_delta (enl u128, res u128) | state_hash 32 B
//! Checkpoint: round u32 | state_hash 32 B | u64 byte length | canonical bytes
//! EndOfMatch: end_kind u8 — 0 normal: rounds_completed u32, final_state_hash
//!         32 B, result_tag u8 (0 EnlWin / 1 ResWin / 2 Draw);
//!         1 abnormal: last_completed_round u32, last_valid_hash 32 B,
//!         outcome_tag u8 (3+ reserved: spawn forfeit etc.)
//! ```
//!
//! Decoding validates structure (magic, versions, round sequencing,
//! notified-first parity against D20, checkpoint placement + hash agreement,
//! trailer counts) and never panics on malformed input. Recorded outcomes
//! are never trusted as replay inputs — the engine re-executes (D46/D50).

use std::collections::BTreeMap;
use std::fmt;

use arena_core::codec;
use arena_core::config::GameConfig;
use arena_core::event::{Event, EventKind};
use arena_core::map::MapDefinition;
use arena_core::resolve::{SlotStatus, StreamRejection, Unlocatable};
use arena_core::rng::RNG_VERSION;
use arena_core::types::{
    AgentId, Command, Coord, Faction, FieldId, LinkId, MatchResult, PortalId, Score2,
};

use crate::canon;

/// File magic.
pub const MAGIC: &[u8; 4] = b"IARP";

/// IARP structural format version.
pub const FORMAT_VERSION: u32 = 1;

/// Server (rules-engine) version that produced the file.
pub const SERVER_VERSION: u32 = 1;

/// Protocol version placeholder (M7 wires real negotiation).
pub const PROTOCOL_VERSION: u32 = 1;

/// Checkpoints are written after every `CHECKPOINT_INTERVAL`-th round
/// (D49; P5-REPLAY recommends every 50 rounds).
pub const CHECKPOINT_INTERVAL: u32 = 50;

/// First reserved abnormal-outcome tag (spawn forfeit etc. — defined now,
/// exercised by hand-crafted tests; in-process M5 matches always end
/// normal).
pub const FIRST_ABNORMAL_OUTCOME_TAG: u8 = 3;

/// Abnormal outcome tag: a faction's Initial Spawn Set was internally
/// invalid or missed the pre-match deadline (M7, R-SPAWN-01).
pub const OUTCOME_SPAWN_FORFEIT: u8 = 3;

/// Abnormal outcome tag: a client process exited, EOF'd, or hit an
/// unrecoverable connection error after the match started (M7, P3-TIMEOUT).
pub const OUTCOME_CLIENT_FORFEIT: u8 = 4;

// ---------------------------------------------------------------------------
// Tags (D51)
// ---------------------------------------------------------------------------

/// Block tags.
pub const TAG_ROUND: u8 = 1;
pub const TAG_CHECKPOINT: u8 = 2;
pub const TAG_END: u8 = 3;

/// Command kind tags, in `Command` declaration order.
const CMD_MOVE: u8 = 1;
const CMD_ATTACK: u8 = 2;
const CMD_CAPTURE: u8 = 3;
const CMD_RECHARGE: u8 = 4;
const CMD_HACK: u8 = 5;
const CMD_TRANSFER: u8 = 6;
const CMD_LINK: u8 = 7;

/// Event kind tags, in `EventKind` declaration order.
const EV_PORTAL_CAPTURED: u8 = 1;
const EV_PORTAL_ATTACKED: u8 = 2;
const EV_PORTAL_NEUTRALIZED: u8 = 3;
const EV_LINK_CREATED: u8 = 4;
const EV_LINK_DESTROYED: u8 = 5;
const EV_FIELD_CREATED: u8 = 6;
const EV_FIELD_DESTROYED: u8 = 7;
const EV_REINFORCEMENT: u8 = 8;

/// `Unlocatable` tags.
const UNLOC_WRONG_ROUND: u8 = 1;
const UNLOC_UNKNOWN_AGENT: u8 = 2;
const UNLOC_FACTION_MISMATCH: u8 = 3;

/// `SlotStatus` tags.
const SLOT_EXECUTED: u8 = 1;
const SLOT_PASS: u8 = 2;
const SLOT_REJECTED: u8 = 3;
const SLOT_NOT_SUBMITTED: u8 = 4;

/// `StreamRejection` tags.
const REJ_TOO_MANY_COMMANDS: u8 = 1;
const REJ_MULTIPLE_MAJORS: u8 = 2;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Every way recording, decoding, or replaying an IARP file can fail.
/// Replay-side variants name the ROUND of the first divergence so a
/// corrupted file is localized immediately.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReplayError {
    BadMagic,
    FormatVersion {
        found: u32,
        expected: u32,
    },
    ServerVersion {
        found: u32,
        expected: u32,
    },
    ProtocolVersion {
        found: u32,
        expected: u32,
    },
    RngVersion {
        found: u32,
        expected: u32,
    },
    StateEncodingVersion {
        found: u32,
        expected: u32,
    },
    /// Ran out of bytes at file offset `at`.
    Truncated {
        at: usize,
    },
    /// Bytes continued after the EndOfMatch trailer.
    TrailingBytes {
        at: usize,
    },
    /// Header `total_rounds` disagrees with the config it echoes.
    TotalRoundsEcho {
        header: u32,
        config: u32,
    },
    /// A spawn list in the header is not strictly ascending by agent id.
    SpawnsOutOfOrder {
        at: usize,
        agent: u32,
    },
    /// A round record broke the 1, 2, 3, ... sequence.
    RoundSequence {
        expected: u32,
        found: u32,
    },
    /// `notified_first` contradicts the round parity rule (D20).
    NotifiedFirstParity {
        round: u32,
        stored: u8,
    },
    /// The multiple-of-50 round record is not followed by its checkpoint.
    CheckpointMissing {
        round: u32,
    },
    /// A checkpoint appeared where no multiple-of-50 round had just closed.
    CheckpointMisplaced {
        round: u32,
    },
    /// The checkpoint hash disagrees with that round record's state hash.
    CheckpointHashDisagreement {
        round: u32,
    },
    /// The checkpoint's canonical bytes do not hash to their own hash.
    CheckpointIntegrity {
        round: u32,
    },
    /// No EndOfMatch trailer before the end of the file.
    MissingTrailer,
    /// Trailer counts disagree with the number of round records.
    RoundCount {
        records: u32,
        trailer: u32,
    },
    /// Unknown tag byte in a tagged position.
    UnknownTag {
        what: &'static str,
        tag: u8,
    },
    /// Replay: the header inputs do not build a valid match (config, map,
    /// or a spawn set failed validation).
    SetupInvalid {
        what: &'static str,
    },
    /// Replay: the recomputed per-round state hash differs (round of first
    /// divergence, recorded vs replayed hash).
    HashMismatch {
        round: u32,
        expected_hex: String,
        actual_hex: String,
    },
    /// Replay: the final hash differs from the trailer's.
    FinalHashMismatch {
        expected_hex: String,
        actual_hex: String,
    },
    /// Replay: the recomputed round delta differs.
    DeltaMismatch {
        round: u32,
    },
    /// Replay: the recomputed event slice differs.
    EventMismatch {
        round: u32,
    },
    /// Replay: an intake fact (unlocatable / duplicates / order / slots)
    /// re-derived differently.
    IntakeMismatch {
        round: u32,
        what: &'static str,
    },
    /// Replay: the final result differs from the recorded tag (P1-REPLAY).
    ResultMismatch {
        recorded: MatchResult,
        replayed: MatchResult,
    },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayError::BadMagic => write!(f, "not an IARP replay file (bad magic)"),
            ReplayError::FormatVersion { found, expected } => {
                write!(f, "format version {found} != supported {expected}")
            }
            ReplayError::ServerVersion { found, expected } => {
                write!(f, "server version {found} != expected {expected}")
            }
            ReplayError::ProtocolVersion { found, expected } => {
                write!(f, "protocol version {found} != expected {expected}")
            }
            ReplayError::RngVersion { found, expected } => {
                write!(f, "rng version {found} != runtime {expected}")
            }
            ReplayError::StateEncodingVersion { found, expected } => {
                write!(f, "state encoding version {found} != supported {expected}")
            }
            ReplayError::Truncated { at } => write!(f, "truncated file at byte {at}"),
            ReplayError::TrailingBytes { at } => {
                write!(f, "bytes after the EndOfMatch trailer at {at}")
            }
            ReplayError::TotalRoundsEcho { header, config } => {
                write!(f, "header total_rounds {header} != config {config}")
            }
            ReplayError::SpawnsOutOfOrder { at, agent } => {
                write!(f, "spawn list not ascending: agent {agent} at byte {at}")
            }
            ReplayError::RoundSequence { expected, found } => {
                write!(
                    f,
                    "round record {found} out of sequence (expected {expected})"
                )
            }
            ReplayError::NotifiedFirstParity { round, stored } => write!(
                f,
                "notified_first {stored} contradicts the D20 parity of round {round}"
            ),
            ReplayError::CheckpointMissing { round } => {
                write!(f, "round {round} lacks its checkpoint block")
            }
            ReplayError::CheckpointMisplaced { round } => {
                write!(f, "checkpoint after non-boundary round {round}")
            }
            ReplayError::CheckpointHashDisagreement { round } => {
                write!(f, "checkpoint hash disagrees with round {round}'s record")
            }
            ReplayError::CheckpointIntegrity { round } => {
                write!(f, "checkpoint {round} canonical bytes fail their own hash")
            }
            ReplayError::MissingTrailer => write!(f, "missing EndOfMatch trailer"),
            ReplayError::RoundCount { records, trailer } => {
                write!(f, "{records} round records but trailer claims {trailer}")
            }
            ReplayError::UnknownTag { what, tag } => write!(f, "unknown {what} tag {tag}"),
            ReplayError::SetupInvalid { what } => {
                write!(f, "replay header fails validation: {what}")
            }
            ReplayError::HashMismatch {
                round,
                expected_hex,
                actual_hex,
            } => write!(
                f,
                "round {round} hash mismatch: recorded {expected_hex}, replayed {actual_hex}"
            ),
            ReplayError::FinalHashMismatch {
                expected_hex,
                actual_hex,
            } => write!(
                f,
                "final hash mismatch: recorded {expected_hex}, replayed {actual_hex}"
            ),
            ReplayError::DeltaMismatch { round } => {
                write!(f, "round {round} score2_delta mismatch")
            }
            ReplayError::EventMismatch { round } => write!(f, "round {round} events mismatch"),
            ReplayError::IntakeMismatch { round, what } => {
                write!(f, "round {round} intake mismatch: {what}")
            }
            ReplayError::ResultMismatch { recorded, replayed } => {
                write!(
                    f,
                    "result mismatch: recorded {recorded:?}, replayed {replayed:?}"
                )
            }
        }
    }
}

impl std::error::Error for ReplayError {}

// ---------------------------------------------------------------------------
// File model
// ---------------------------------------------------------------------------

/// Which faction received the Round Start Signal first (R-RESOLVE-03/D20):
/// odd rounds notify RES first (settlement starts from ENL), even rounds
/// notify ENL first. Stored AND verified in every round record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotifiedFirst {
    /// Tag 0: RES notified first (odd rounds); ENL settles first.
    Res,
    /// Tag 1: ENL notified first (even rounds); RES settles first.
    Enl,
}

impl NotifiedFirst {
    /// The D20 parity rule: odd round ⇒ RES first, even ⇒ ENL first.
    pub fn for_round(round: u32) -> NotifiedFirst {
        if round % 2 == 1 {
            NotifiedFirst::Res
        } else {
            NotifiedFirst::Enl
        }
    }

    fn tag(self) -> u8 {
        match self {
            NotifiedFirst::Res => 0,
            NotifiedFirst::Enl => 1,
        }
    }

    fn from_tag(tag: u8) -> Option<NotifiedFirst> {
        match tag {
            0 => Some(NotifiedFirst::Res),
            1 => Some(NotifiedFirst::Enl),
            _ => None,
        }
    }
}

/// IARP header: every deterministic setup input of the match (P1-REPLAY).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReplayHeader {
    pub server_version: u32,
    pub protocol_version: u32,
    pub rng_version: u32,
    pub state_encoding_version: u32,
    pub game_seed: u64,
    /// Echo of `config.total_rounds` (the config is authoritative).
    pub total_rounds: u32,
    pub config: GameConfig,
    /// The FROZEN complete map definition — replay reloads it and never
    /// re-runs the generator (R-MAP-02, R-RNG-01).
    pub map: MapDefinition,
    /// Ascending `(agent, position)` initial spawn sets (R-SPAWN-01).
    pub enl_spawns: Vec<(AgentId, Coord)>,
    pub res_spawns: Vec<(AgentId, Coord)>,
}

/// One settled round: the echoed submission slice (with unlocatable and
/// ignored-duplicate submissions), the intake facts, the fixed execution
/// order, the broadcast events, the round delta, and the D45 state hash.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RoundRecord {
    pub round: u32,
    pub notified_first: NotifiedFirst,
    pub submissions: Vec<arena_core::resolve::SubmittedCommandStream>,
    pub unlocatable: Vec<Unlocatable>,
    pub ignored_duplicates: Vec<AgentId>,
    /// Final execution order as agent ids (R-RESOLVE-03: fixed before any
    /// command ran; recorded outcomes are never replay inputs).
    pub execution_order: Vec<AgentId>,
    pub slots: BTreeMap<AgentId, SlotStatus>,
    pub events: Vec<Event>,
    /// `(enl, res)` round-end field scoring (R-SCORE-01).
    pub score2_delta: (Score2, Score2),
    pub state_hash: [u8; 32],
}

/// Checkpoint (D49): the canonical state bytes at a boundary round plus the
/// hash the round record already carries. M5 validates checkpoints; seek
/// and resume arrive with the viewer milestone (M10).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Checkpoint {
    pub round: u32,
    pub state_hash: [u8; 32],
    pub canonical_state: Vec<u8>,
}

/// EndOfMatch trailer (D46): a normal finish carries the final hash and the
/// score-comparison result; an abnormal one carries the last completed
/// round, the last valid hash, and a reserved outcome tag.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EndOfMatch {
    Normal {
        rounds_completed: u32,
        final_state_hash: [u8; 32],
        result: MatchResult,
    },
    Abnormal {
        last_completed_round: u32,
        last_valid_hash: [u8; 32],
        /// `FIRST_ABNORMAL_OUTCOME_TAG` or higher (spawn forfeit etc.).
        outcome_tag: u8,
    },
}

/// A decoded IARP file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReplayFile {
    pub header: ReplayHeader,
    pub blocks: Vec<Block>,
}

/// One tagged block of the file body.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Block {
    Round(RoundRecord),
    Checkpoint(Checkpoint),
    End(EndOfMatch),
}

impl ReplayFile {
    /// Serializes the file (D46 layout).
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        encode_header(&mut w, &self.header);
        for block in &self.blocks {
            match block {
                Block::Round(record) => {
                    w.byte(TAG_ROUND);
                    encode_round_record(&mut w, record);
                }
                Block::Checkpoint(checkpoint) => {
                    w.byte(TAG_CHECKPOINT);
                    encode_checkpoint(&mut w, checkpoint);
                }
                Block::End(end) => {
                    w.byte(TAG_END);
                    encode_end(&mut w, end);
                }
            }
        }
        w.buf
    }

    /// Round records in file order.
    pub fn round_records(&self) -> impl Iterator<Item = &RoundRecord> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Round(record) => Some(record),
            _ => None,
        })
    }

    /// The EndOfMatch trailer, if present.
    pub fn trailer(&self) -> Option<&EndOfMatch> {
        self.blocks.iter().find_map(|b| match b {
            Block::End(end) => Some(end),
            _ => None,
        })
    }
}

/// Decodes and structurally validates an IARP file. Never panics on
/// malformed input; every defect is a [`ReplayError`].
pub fn decode(bytes: &[u8]) -> Result<ReplayFile, ReplayError> {
    let mut r = Reader::new(bytes);

    // --- Header -------------------------------------------------------
    if r.take(4)? != MAGIC {
        return Err(ReplayError::BadMagic);
    }
    let format_version = r.u32()?;
    if format_version != FORMAT_VERSION {
        return Err(ReplayError::FormatVersion {
            found: format_version,
            expected: FORMAT_VERSION,
        });
    }
    let server_version = r.u32()?;
    if server_version != SERVER_VERSION {
        return Err(ReplayError::ServerVersion {
            found: server_version,
            expected: SERVER_VERSION,
        });
    }
    let protocol_version = r.u32()?;
    if protocol_version != PROTOCOL_VERSION {
        return Err(ReplayError::ProtocolVersion {
            found: protocol_version,
            expected: PROTOCOL_VERSION,
        });
    }
    let rng_version = r.u32()?;
    if rng_version != RNG_VERSION {
        return Err(ReplayError::RngVersion {
            found: rng_version,
            expected: RNG_VERSION,
        });
    }
    let state_encoding_version = r.u32()?;
    if state_encoding_version != canon::STATE_ENCODING_VERSION {
        return Err(ReplayError::StateEncodingVersion {
            found: state_encoding_version,
            expected: canon::STATE_ENCODING_VERSION,
        });
    }
    let game_seed = r.u64()?;
    let total_rounds = r.u32()?;
    let config = codec::config_from_bytes(r.take(codec::CONFIG_BYTES)?)
        .ok_or(ReplayError::Truncated { at: r.pos })?;
    if total_rounds != config.total_rounds {
        return Err(ReplayError::TotalRoundsEcho {
            header: total_rounds,
            config: config.total_rounds,
        });
    }
    let portal_count = r.u32()? as usize;
    let mut portals = Vec::new();
    for index in 0..portal_count {
        let record = r.take(codec::PORTAL_BYTES)?;
        portals.push(
            codec::portal_from_bytes(PortalId(index as u32), record).ok_or(
                ReplayError::UnknownTag {
                    what: "portal owner",
                    tag: record[0],
                },
            )?,
        );
    }
    let map = MapDefinition::new(portals);
    let enl_spawns = decode_spawns(&mut r)?;
    let res_spawns = decode_spawns(&mut r)?;
    let header = ReplayHeader {
        server_version,
        protocol_version,
        rng_version,
        state_encoding_version,
        game_seed,
        total_rounds,
        config,
        map,
        enl_spawns,
        res_spawns,
    };

    // --- Blocks -------------------------------------------------------
    let mut blocks = Vec::new();
    let mut expected_round: u32 = 1;
    let mut records: u32 = 0;
    let mut last_round_boundary: Option<(u32, [u8; 32])> = None;
    while !r.is_empty() {
        let tag = r.byte()?;
        match tag {
            TAG_ROUND => {
                let record = decode_round_record(&mut r, expected_round)?;
                // Checkpoints close every CHECKPOINT_INTERVAL-th round.
                if last_round_boundary.is_some() {
                    // The previous boundary round's checkpoint is missing.
                    let (round, _) = last_round_boundary.expect("checked is_some");
                    return Err(ReplayError::CheckpointMissing { round });
                }
                if record.round % CHECKPOINT_INTERVAL == 0 {
                    last_round_boundary = Some((record.round, record.state_hash));
                }
                records += 1;
                expected_round += 1;
                blocks.push(Block::Round(record));
            }
            TAG_CHECKPOINT => {
                let checkpoint = decode_checkpoint(&mut r)?;
                let Some((round, hash)) = last_round_boundary else {
                    return Err(ReplayError::CheckpointMisplaced {
                        round: checkpoint.round,
                    });
                };
                if checkpoint.round != round || checkpoint.state_hash != hash {
                    return Err(ReplayError::CheckpointHashDisagreement { round });
                }
                if canon::hash_canonical_bytes(&checkpoint.canonical_state) != checkpoint.state_hash
                {
                    return Err(ReplayError::CheckpointIntegrity { round });
                }
                last_round_boundary = None;
                blocks.push(Block::Checkpoint(checkpoint));
            }
            TAG_END => {
                if let Some((round, _)) = last_round_boundary {
                    return Err(ReplayError::CheckpointMissing { round });
                }
                let end = decode_end(&mut r)?;
                let claimed = match &end {
                    EndOfMatch::Normal {
                        rounds_completed, ..
                    } => *rounds_completed,
                    EndOfMatch::Abnormal {
                        last_completed_round,
                        ..
                    } => *last_completed_round,
                };
                if claimed != records {
                    return Err(ReplayError::RoundCount {
                        records,
                        trailer: claimed,
                    });
                }
                if !r.is_empty() {
                    return Err(ReplayError::TrailingBytes { at: r.pos });
                }
                blocks.push(Block::End(end));
                return Ok(ReplayFile { header, blocks });
            }
            other => {
                return Err(ReplayError::UnknownTag {
                    what: "block",
                    tag: other,
                });
            }
        }
    }
    if let Some((round, _)) = last_round_boundary {
        return Err(ReplayError::CheckpointMissing { round });
    }
    Err(ReplayError::MissingTrailer)
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn byte(&mut self, b: u8) {
        self.buf.push(b);
    }

    fn bytes(&mut self, slice: &[u8]) {
        self.buf.extend_from_slice(slice);
    }

    fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_header(w: &mut Writer, header: &ReplayHeader) {
    w.bytes(MAGIC);
    w.u32(FORMAT_VERSION);
    w.u32(header.server_version);
    w.u32(header.protocol_version);
    w.u32(header.rng_version);
    w.u32(header.state_encoding_version);
    w.u64(header.game_seed);
    w.u32(header.total_rounds);
    codec::push_config(&header.config, &mut w.buf);
    let portals = header.map.portals();
    w.u32(portals.len() as u32);
    for portal in portals {
        codec::push_portal(portal, &mut w.buf);
    }
    encode_spawns(w, &header.enl_spawns);
    encode_spawns(w, &header.res_spawns);
}

fn encode_spawns(w: &mut Writer, spawns: &[(AgentId, Coord)]) {
    w.u32(spawns.len() as u32);
    for &(agent, position) in spawns {
        w.u32(agent.0);
        w.u32(position.x as u32);
        w.u32(position.y as u32);
    }
}

fn encode_command(w: &mut Writer, command: &Command) {
    match command {
        Command::Move { destination } => {
            w.byte(CMD_MOVE);
            w.u32(destination.x as u32);
            w.u32(destination.y as u32);
        }
        Command::Attack => w.byte(CMD_ATTACK),
        Command::Capture { portal } => {
            w.byte(CMD_CAPTURE);
            w.u32(portal.0);
        }
        Command::Recharge { portal } => {
            w.byte(CMD_RECHARGE);
            w.u32(portal.0);
        }
        Command::Hack { portal } => {
            w.byte(CMD_HACK);
            w.u32(portal.0);
        }
        Command::Transfer {
            target_agent,
            supply,
            keys,
        } => {
            w.byte(CMD_TRANSFER);
            w.u32(target_agent.0);
            w.u32(*supply);
            w.u32(keys.len() as u32);
            for (&portal, &count) in keys {
                w.u32(portal.0);
                w.u32(count);
            }
        }
        Command::Link {
            source_portal,
            target_portal,
        } => {
            w.byte(CMD_LINK);
            w.u32(source_portal.0);
            w.u32(target_portal.0);
        }
    }
}

fn encode_rejection(w: &mut Writer, rejection: &StreamRejection) {
    match rejection {
        StreamRejection::TooManyCommands { count, max } => {
            w.byte(REJ_TOO_MANY_COMMANDS);
            w.u32(*count);
            w.u32(*max);
        }
        StreamRejection::MultipleMajorActions { count } => {
            w.byte(REJ_MULTIPLE_MAJORS);
            w.u32(*count);
        }
    }
}

fn encode_slot(w: &mut Writer, status: &SlotStatus) {
    match status {
        SlotStatus::Executed {
            bucket,
            receive_seq,
        } => {
            w.byte(SLOT_EXECUTED);
            w.u64(*bucket);
            w.u32(*receive_seq);
        }
        SlotStatus::Pass => w.byte(SLOT_PASS),
        SlotStatus::Rejected(rejection) => {
            w.byte(SLOT_REJECTED);
            encode_rejection(w, rejection);
        }
        SlotStatus::NotSubmitted => w.byte(SLOT_NOT_SUBMITTED),
    }
}

fn encode_unlocatable(w: &mut Writer, unlocatable: &Unlocatable) {
    match unlocatable {
        Unlocatable::WrongRound { submitted, current } => {
            w.byte(UNLOC_WRONG_ROUND);
            w.u32(*submitted);
            w.u32(*current);
        }
        Unlocatable::UnknownAgent(agent) => {
            w.byte(UNLOC_UNKNOWN_AGENT);
            w.u32(agent.0);
        }
        Unlocatable::FactionMismatch { claimed, actual } => {
            w.byte(UNLOC_FACTION_MISMATCH);
            w.byte(codec::faction_tag(*claimed));
            w.byte(codec::faction_tag(*actual));
        }
    }
}

fn encode_event(w: &mut Writer, event: &Event) {
    w.u64(event.seq);
    w.u32(event.round);
    match event.kind {
        EventKind::PortalCaptured {
            faction,
            agent_id,
            portal,
        } => {
            w.byte(EV_PORTAL_CAPTURED);
            w.byte(codec::faction_tag(faction));
            w.u32(agent_id.0);
            w.u32(portal.0);
        }
        EventKind::PortalAttacked {
            faction,
            agent_id,
            portal,
            damage,
            hp_after,
        } => {
            w.byte(EV_PORTAL_ATTACKED);
            w.byte(codec::faction_tag(faction));
            w.u32(agent_id.0);
            w.u32(portal.0);
            w.u32(damage);
            w.u32(hp_after);
        }
        EventKind::PortalNeutralized {
            faction,
            agent_id,
            portal,
        } => {
            w.byte(EV_PORTAL_NEUTRALIZED);
            w.byte(codec::faction_tag(faction));
            w.u32(agent_id.0);
            w.u32(portal.0);
        }
        EventKind::LinkCreated {
            faction,
            agent_id,
            link,
        } => {
            w.byte(EV_LINK_CREATED);
            w.byte(codec::faction_tag(faction));
            w.u32(agent_id.0);
            w.u32(link.min.0);
            w.u32(link.max.0);
        }
        EventKind::LinkDestroyed {
            faction,
            agent_id,
            link,
        } => {
            w.byte(EV_LINK_DESTROYED);
            w.byte(codec::faction_tag(faction));
            w.u32(agent_id.0);
            w.u32(link.min.0);
            w.u32(link.max.0);
        }
        EventKind::FieldCreated {
            faction,
            agent_id,
            field,
        } => {
            w.byte(EV_FIELD_CREATED);
            w.byte(codec::faction_tag(faction));
            w.u32(agent_id.0);
            w.u32(field.a.0);
            w.u32(field.b.0);
            w.u32(field.c.0);
        }
        EventKind::FieldDestroyed {
            faction,
            agent_id,
            field,
        } => {
            w.byte(EV_FIELD_DESTROYED);
            w.byte(codec::faction_tag(faction));
            w.u32(agent_id.0);
            w.u32(field.a.0);
            w.u32(field.b.0);
            w.u32(field.c.0);
        }
        EventKind::Reinforcement {
            index,
            position,
            count_per_faction,
            first_agent_id,
        } => {
            w.byte(EV_REINFORCEMENT);
            w.u32(index);
            w.u32(position.x as u32);
            w.u32(position.y as u32);
            w.u32(count_per_faction);
            w.u32(first_agent_id.0);
        }
    }
}

fn encode_round_record(w: &mut Writer, record: &RoundRecord) {
    w.u32(record.round);
    w.byte(record.notified_first.tag());
    w.u32(record.submissions.len() as u32);
    for submission in &record.submissions {
        w.u32(submission.round);
        w.byte(codec::faction_tag(submission.faction));
        w.u32(submission.agent_id.0);
        w.u64(submission.received_at_ms);
        w.u32(submission.receive_seq);
        w.u32(submission.commands.len() as u32);
        for command in &submission.commands {
            encode_command(w, command);
        }
    }
    w.u32(record.unlocatable.len() as u32);
    for unlocatable in &record.unlocatable {
        encode_unlocatable(w, unlocatable);
    }
    w.u32(record.ignored_duplicates.len() as u32);
    for agent in &record.ignored_duplicates {
        w.u32(agent.0);
    }
    w.u32(record.execution_order.len() as u32);
    for agent in &record.execution_order {
        w.u32(agent.0);
    }
    w.u32(record.slots.len() as u32);
    for (&agent, status) in &record.slots {
        w.u32(agent.0);
        encode_slot(w, status);
    }
    w.u32(record.events.len() as u32);
    for event in &record.events {
        encode_event(w, event);
    }
    w.u128(record.score2_delta.0);
    w.u128(record.score2_delta.1);
    w.bytes(&record.state_hash);
}

fn encode_checkpoint(w: &mut Writer, checkpoint: &Checkpoint) {
    w.u32(checkpoint.round);
    w.bytes(&checkpoint.state_hash);
    w.u64(checkpoint.canonical_state.len() as u64);
    w.bytes(&checkpoint.canonical_state);
}

fn encode_end(w: &mut Writer, end: &EndOfMatch) {
    match end {
        EndOfMatch::Normal {
            rounds_completed,
            final_state_hash,
            result,
        } => {
            w.byte(0);
            w.u32(*rounds_completed);
            w.bytes(final_state_hash);
            w.byte(match result {
                MatchResult::EnlWin => 0,
                MatchResult::ResWin => 1,
                MatchResult::Draw => 2,
            });
        }
        EndOfMatch::Abnormal {
            last_completed_round,
            last_valid_hash,
            outcome_tag,
        } => {
            w.byte(1);
            w.u32(*last_completed_round);
            w.bytes(last_valid_hash);
            w.byte(*outcome_tag);
        }
    }
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ReplayError> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or(ReplayError::Truncated { at: self.pos })?;
        if end > self.bytes.len() {
            return Err(ReplayError::Truncated { at: self.pos });
        }
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn byte(&mut self) -> Result<u8, ReplayError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, ReplayError> {
        let chunk = self.take(4)?;
        Ok(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
    }

    fn u64(&mut self) -> Result<u64, ReplayError> {
        let chunk = self.take(8)?;
        let mut words = [0u8; 8];
        words.copy_from_slice(chunk);
        Ok(u64::from_le_bytes(words))
    }

    fn u128(&mut self) -> Result<u128, ReplayError> {
        let chunk = self.take(16)?;
        let mut words = [0u8; 16];
        words.copy_from_slice(chunk);
        Ok(u128::from_le_bytes(words))
    }

    fn hash(&mut self) -> Result<[u8; 32], ReplayError> {
        let chunk = self.take(32)?;
        let mut hash = [0u8; 32];
        hash.copy_from_slice(chunk);
        Ok(hash)
    }

    /// A length prefix that must leave room for its elements: guards
    /// against absurd counts allocating before the truncation error.
    fn count(&mut self, element_bytes: usize) -> Result<usize, ReplayError> {
        let count = self.u32()? as usize;
        let end = self
            .pos
            .checked_add(count.saturating_mul(element_bytes))
            .ok_or(ReplayError::Truncated { at: self.pos })?;
        if end > self.bytes.len() {
            return Err(ReplayError::Truncated { at: self.pos });
        }
        Ok(count)
    }
}

fn decode_spawns(r: &mut Reader) -> Result<Vec<(AgentId, Coord)>, ReplayError> {
    let count = r.count(12)?;
    let mut spawns = Vec::new();
    let mut previous: Option<u32> = None;
    for _ in 0..count {
        let at = r.pos;
        let agent = r.u32()?;
        let x = r.u32()? as i32;
        let y = r.u32()? as i32;
        if previous.is_some_and(|previous| previous >= agent) {
            return Err(ReplayError::SpawnsOutOfOrder { at, agent });
        }
        previous = Some(agent);
        spawns.push((AgentId(agent), Coord::new(x, y)));
    }
    Ok(spawns)
}

fn decode_command(r: &mut Reader) -> Result<Command, ReplayError> {
    let tag = r.byte()?;
    match tag {
        CMD_MOVE => {
            let x = r.u32()? as i32;
            let y = r.u32()? as i32;
            Ok(Command::Move {
                destination: Coord::new(x, y),
            })
        }
        CMD_ATTACK => Ok(Command::Attack),
        CMD_CAPTURE => Ok(Command::Capture {
            portal: PortalId(r.u32()?),
        }),
        CMD_RECHARGE => Ok(Command::Recharge {
            portal: PortalId(r.u32()?),
        }),
        CMD_HACK => Ok(Command::Hack {
            portal: PortalId(r.u32()?),
        }),
        CMD_TRANSFER => {
            let target_agent = AgentId(r.u32()?);
            let supply = r.u32()?;
            let count = r.count(8)?;
            let mut keys = BTreeMap::new();
            for _ in 0..count {
                let portal = PortalId(r.u32()?);
                let amount = r.u32()?;
                keys.insert(portal, amount);
            }
            Ok(Command::Transfer {
                target_agent,
                supply,
                keys,
            })
        }
        CMD_LINK => {
            let source_portal = PortalId(r.u32()?);
            let target_portal = PortalId(r.u32()?);
            Ok(Command::Link {
                source_portal,
                target_portal,
            })
        }
        other => Err(ReplayError::UnknownTag {
            what: "command",
            tag: other,
        }),
    }
}

fn decode_rejection(r: &mut Reader) -> Result<StreamRejection, ReplayError> {
    let tag = r.byte()?;
    match tag {
        REJ_TOO_MANY_COMMANDS => Ok(StreamRejection::TooManyCommands {
            count: r.u32()?,
            max: r.u32()?,
        }),
        REJ_MULTIPLE_MAJORS => Ok(StreamRejection::MultipleMajorActions { count: r.u32()? }),
        other => Err(ReplayError::UnknownTag {
            what: "stream rejection",
            tag: other,
        }),
    }
}

fn decode_slot(r: &mut Reader) -> Result<SlotStatus, ReplayError> {
    let tag = r.byte()?;
    match tag {
        SLOT_EXECUTED => Ok(SlotStatus::Executed {
            bucket: r.u64()?,
            receive_seq: r.u32()?,
        }),
        SLOT_PASS => Ok(SlotStatus::Pass),
        SLOT_REJECTED => Ok(SlotStatus::Rejected(decode_rejection(r)?)),
        SLOT_NOT_SUBMITTED => Ok(SlotStatus::NotSubmitted),
        other => Err(ReplayError::UnknownTag {
            what: "slot status",
            tag: other,
        }),
    }
}

fn decode_unlocatable(r: &mut Reader) -> Result<Unlocatable, ReplayError> {
    let tag = r.byte()?;
    match tag {
        UNLOC_WRONG_ROUND => Ok(Unlocatable::WrongRound {
            submitted: r.u32()?,
            current: r.u32()?,
        }),
        UNLOC_UNKNOWN_AGENT => Ok(Unlocatable::UnknownAgent(AgentId(r.u32()?))),
        UNLOC_FACTION_MISMATCH => {
            let claimed_tag = r.byte()?;
            let claimed = codec::faction_from_tag(claimed_tag).ok_or(ReplayError::UnknownTag {
                what: "faction",
                tag: claimed_tag,
            })?;
            let actual_tag = r.byte()?;
            let actual = codec::faction_from_tag(actual_tag).ok_or(ReplayError::UnknownTag {
                what: "faction",
                tag: actual_tag,
            })?;
            Ok(Unlocatable::FactionMismatch { claimed, actual })
        }
        other => Err(ReplayError::UnknownTag {
            what: "unlocatable",
            tag: other,
        }),
    }
}

fn decode_event(r: &mut Reader) -> Result<Event, ReplayError> {
    let seq = r.u64()?;
    let round = r.u32()?;
    let tag = r.byte()?;
    let unknown = |tag: u8| ReplayError::UnknownTag { what: "event", tag };
    let faction = |r: &mut Reader| -> Result<Faction, ReplayError> {
        let faction_tag = r.byte()?;
        codec::faction_from_tag(faction_tag).ok_or_else(|| unknown(faction_tag))
    };
    let kind = match tag {
        EV_PORTAL_CAPTURED => EventKind::PortalCaptured {
            faction: faction(r)?,
            agent_id: AgentId(r.u32()?),
            portal: PortalId(r.u32()?),
        },
        EV_PORTAL_ATTACKED => EventKind::PortalAttacked {
            faction: faction(r)?,
            agent_id: AgentId(r.u32()?),
            portal: PortalId(r.u32()?),
            damage: r.u32()?,
            hp_after: r.u32()?,
        },
        EV_PORTAL_NEUTRALIZED => EventKind::PortalNeutralized {
            faction: faction(r)?,
            agent_id: AgentId(r.u32()?),
            portal: PortalId(r.u32()?),
        },
        EV_LINK_CREATED => EventKind::LinkCreated {
            faction: faction(r)?,
            agent_id: AgentId(r.u32()?),
            link: LinkId {
                min: PortalId(r.u32()?),
                max: PortalId(r.u32()?),
            },
        },
        EV_LINK_DESTROYED => EventKind::LinkDestroyed {
            faction: faction(r)?,
            agent_id: AgentId(r.u32()?),
            link: LinkId {
                min: PortalId(r.u32()?),
                max: PortalId(r.u32()?),
            },
        },
        EV_FIELD_CREATED => EventKind::FieldCreated {
            faction: faction(r)?,
            agent_id: AgentId(r.u32()?),
            field: FieldId {
                a: PortalId(r.u32()?),
                b: PortalId(r.u32()?),
                c: PortalId(r.u32()?),
            },
        },
        EV_FIELD_DESTROYED => EventKind::FieldDestroyed {
            faction: faction(r)?,
            agent_id: AgentId(r.u32()?),
            field: FieldId {
                a: PortalId(r.u32()?),
                b: PortalId(r.u32()?),
                c: PortalId(r.u32()?),
            },
        },
        EV_REINFORCEMENT => EventKind::Reinforcement {
            index: r.u32()?,
            position: Coord::new(r.u32()? as i32, r.u32()? as i32),
            count_per_faction: r.u32()?,
            first_agent_id: AgentId(r.u32()?),
        },
        other => return Err(unknown(other)),
    };
    Ok(Event { seq, round, kind })
}

fn decode_round_record(r: &mut Reader, expected_round: u32) -> Result<RoundRecord, ReplayError> {
    let round = r.u32()?;
    if round != expected_round {
        return Err(ReplayError::RoundSequence {
            expected: expected_round,
            found: round,
        });
    }
    let notified_tag = r.byte()?;
    let notified_first = NotifiedFirst::from_tag(notified_tag).ok_or(ReplayError::UnknownTag {
        what: "notified_first",
        tag: notified_tag,
    })?;
    if notified_first != NotifiedFirst::for_round(round) {
        return Err(ReplayError::NotifiedFirstParity {
            round,
            stored: notified_tag,
        });
    }

    let submission_count = r.count(25)?;
    let mut submissions = Vec::new();
    for _ in 0..submission_count {
        let submission_round = r.u32()?;
        let faction_tag = r.byte()?;
        let faction = codec::faction_from_tag(faction_tag).ok_or(ReplayError::UnknownTag {
            what: "faction",
            tag: faction_tag,
        })?;
        let agent_id = AgentId(r.u32()?);
        let received_at_ms = r.u64()?;
        let receive_seq = r.u32()?;
        let command_count = r.count(1)?;
        let mut commands = Vec::new();
        for _ in 0..command_count {
            commands.push(decode_command(r)?);
        }
        submissions.push(arena_core::resolve::SubmittedCommandStream {
            round: submission_round,
            faction,
            agent_id,
            received_at_ms,
            receive_seq,
            commands,
        });
    }

    let unlocatable_count = r.count(3)?;
    let mut unlocatable = Vec::new();
    for _ in 0..unlocatable_count {
        unlocatable.push(decode_unlocatable(r)?);
    }

    let duplicate_count = r.count(4)?;
    let mut ignored_duplicates = Vec::new();
    for _ in 0..duplicate_count {
        ignored_duplicates.push(AgentId(r.u32()?));
    }

    let order_count = r.count(4)?;
    let mut execution_order = Vec::new();
    for _ in 0..order_count {
        execution_order.push(AgentId(r.u32()?));
    }

    let slot_count = r.count(5)?;
    let mut slots = BTreeMap::new();
    for _ in 0..slot_count {
        let agent = AgentId(r.u32()?);
        let status = decode_slot(r)?;
        slots.insert(agent, status);
    }

    let event_count = r.count(14)?;
    let mut events = Vec::new();
    for _ in 0..event_count {
        events.push(decode_event(r)?);
    }

    let enl_delta = r.u128()?;
    let res_delta = r.u128()?;
    let state_hash = r.hash()?;
    Ok(RoundRecord {
        round,
        notified_first,
        submissions,
        unlocatable,
        ignored_duplicates,
        execution_order,
        slots,
        events,
        score2_delta: (enl_delta, res_delta),
        state_hash,
    })
}

fn decode_checkpoint(r: &mut Reader) -> Result<Checkpoint, ReplayError> {
    let round = r.u32()?;
    let state_hash = r.hash()?;
    let length = r.u64()? as usize;
    let canonical_state = r.take(length)?.to_vec();
    Ok(Checkpoint {
        round,
        state_hash,
        canonical_state,
    })
}

fn decode_end(r: &mut Reader) -> Result<EndOfMatch, ReplayError> {
    let kind = r.byte()?;
    match kind {
        0 => {
            let rounds_completed = r.u32()?;
            let final_state_hash = r.hash()?;
            let result_tag = r.byte()?;
            let result = match result_tag {
                0 => MatchResult::EnlWin,
                1 => MatchResult::ResWin,
                2 => MatchResult::Draw,
                other => {
                    return Err(ReplayError::UnknownTag {
                        what: "result",
                        tag: other,
                    });
                }
            };
            Ok(EndOfMatch::Normal {
                rounds_completed,
                final_state_hash,
                result,
            })
        }
        1 => {
            let last_completed_round = r.u32()?;
            let last_valid_hash = r.hash()?;
            let outcome_tag = r.byte()?;
            if outcome_tag < FIRST_ABNORMAL_OUTCOME_TAG {
                return Err(ReplayError::UnknownTag {
                    what: "outcome",
                    tag: outcome_tag,
                });
            }
            Ok(EndOfMatch::Abnormal {
                last_completed_round,
                last_valid_hash,
                outcome_tag,
            })
        }
        other => Err(ReplayError::UnknownTag {
            what: "end kind",
            tag: other,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::resolve::SubmittedCommandStream;
    use arena_core::types::Owner;
    use arena_core::types::Portal;

    // ------------------------------------------------------------------
    // Fixtures: synthetic files exercising the full tag surface without
    // running a match (the recorder/replay integration tests use real
    // matches).
    // ------------------------------------------------------------------

    fn fixture_header(total_rounds: u32) -> ReplayHeader {
        let mut config = GameConfig::default();
        config.map_width_cm = 40_000;
        config.map_height_cm = 40_000;
        config.portal_count = 3;
        config.portal_cluster_count = 3;
        config.portal_min_distance_cm = 4_000;
        config.initial_agents_per_faction = 1;
        config.total_rounds = total_rounds;
        let map = MapDefinition::new(vec![
            Portal {
                id: PortalId(0),
                position: Coord::new(4_000, 4_000),
                owner: Owner::Neutral,
                hp: 0,
            },
            Portal {
                id: PortalId(1),
                position: Coord::new(20_000, 4_000),
                owner: Owner::Neutral,
                hp: 0,
            },
            Portal {
                id: PortalId(2),
                position: Coord::new(4_000, 20_000),
                owner: Owner::Neutral,
                hp: 0,
            },
        ]);
        ReplayHeader {
            server_version: SERVER_VERSION,
            protocol_version: PROTOCOL_VERSION,
            rng_version: RNG_VERSION,
            state_encoding_version: canon::STATE_ENCODING_VERSION,
            game_seed: 0xABCD_1234,
            total_rounds,
            config,
            map,
            enl_spawns: vec![(AgentId(0), Coord::new(4_000, 4_000))],
            res_spawns: vec![(AgentId(1), Coord::new(36_000, 36_000))],
        }
    }

    fn fixture_record(round: u32, hash: [u8; 32]) -> RoundRecord {
        let mut keys = BTreeMap::new();
        keys.insert(PortalId(1), 2u32);
        let submissions = vec![
            SubmittedCommandStream {
                round,
                faction: Faction::Enl,
                agent_id: AgentId(0),
                received_at_ms: 5,
                receive_seq: 0,
                commands: vec![
                    Command::Move {
                        destination: Coord::new(6_000, 4_000),
                    },
                    Command::Capture {
                        portal: PortalId(0),
                    },
                    Command::Attack,
                    Command::Recharge {
                        portal: PortalId(0),
                    },
                    Command::Hack {
                        portal: PortalId(0),
                    },
                    Command::Transfer {
                        target_agent: AgentId(0),
                        supply: 1,
                        keys: keys.clone(),
                    },
                    Command::Link {
                        source_portal: PortalId(0),
                        target_portal: PortalId(1),
                    },
                ],
            },
            SubmittedCommandStream {
                round,
                faction: Faction::Res,
                agent_id: AgentId(1),
                received_at_ms: 5,
                receive_seq: 0,
                commands: vec![],
            },
        ];
        let mut slots = BTreeMap::new();
        slots.insert(
            AgentId(0),
            SlotStatus::Executed {
                bucket: 1,
                receive_seq: 0,
            },
        );
        slots.insert(
            AgentId(1),
            SlotStatus::Rejected(StreamRejection::TooManyCommands { count: 11, max: 10 }),
        );
        RoundRecord {
            round,
            notified_first: NotifiedFirst::for_round(round),
            submissions,
            unlocatable: vec![
                Unlocatable::WrongRound {
                    submitted: 9,
                    current: round,
                },
                Unlocatable::UnknownAgent(AgentId(77)),
                Unlocatable::FactionMismatch {
                    claimed: Faction::Res,
                    actual: Faction::Enl,
                },
            ],
            ignored_duplicates: vec![AgentId(0), AgentId(0)],
            execution_order: vec![AgentId(0)],
            slots,
            events: vec![
                Event {
                    seq: 0,
                    round,
                    kind: EventKind::PortalCaptured {
                        faction: Faction::Enl,
                        agent_id: AgentId(0),
                        portal: PortalId(0),
                    },
                },
                Event {
                    seq: 1,
                    round,
                    kind: EventKind::Reinforcement {
                        index: 1,
                        position: Coord::new(123_456, 654_321),
                        count_per_faction: 1,
                        first_agent_id: AgentId(2),
                    },
                },
            ],
            score2_delta: (256_000_000u128, 0),
            state_hash: hash,
        }
    }

    fn fixture_file() -> ReplayFile {
        let hash = [7u8; 32];
        ReplayFile {
            header: fixture_header(2),
            blocks: vec![
                Block::Round(fixture_record(1, hash)),
                Block::Round(fixture_record(2, hash)),
                Block::End(EndOfMatch::Normal {
                    rounds_completed: 2,
                    final_state_hash: hash,
                    result: MatchResult::EnlWin,
                }),
            ],
        }
    }

    // Round-trip: encode -> decode == structural equality, and
    // decode -> re-encode is byte-exact.
    #[test]
    fn round_trip_is_structural_and_byte_exact() {
        let file = fixture_file();
        let bytes = file.encode();
        let decoded = decode(&bytes).expect("decodes");
        assert_eq!(decoded, file);
        assert_eq!(decoded.encode(), bytes, "re-encode is byte-exact");
    }

    // Header validation: each defect yields its own distinct error.
    #[test]
    fn header_defects_yield_distinct_errors() {
        let bytes = fixture_file().encode();

        let mut bad_magic = bytes.clone();
        bad_magic[0] = b'X';
        assert_eq!(decode(&bad_magic), Err(ReplayError::BadMagic));

        let mut bad_format = bytes.clone();
        bad_format[4..8].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(
            decode(&bad_format),
            Err(ReplayError::FormatVersion {
                found: 2,
                expected: 1
            })
        );

        let mut bad_server = bytes.clone();
        bad_server[8..12].copy_from_slice(&9u32.to_le_bytes());
        assert_eq!(
            decode(&bad_server),
            Err(ReplayError::ServerVersion {
                found: 9,
                expected: SERVER_VERSION
            })
        );

        let mut bad_protocol = bytes.clone();
        bad_protocol[12..16].copy_from_slice(&8u32.to_le_bytes());
        assert_eq!(
            decode(&bad_protocol),
            Err(ReplayError::ProtocolVersion {
                found: 8,
                expected: PROTOCOL_VERSION
            })
        );

        let mut bad_rng = bytes.clone();
        bad_rng[16..20].copy_from_slice(&(RNG_VERSION + 1).to_le_bytes());
        assert_eq!(
            decode(&bad_rng),
            Err(ReplayError::RngVersion {
                found: RNG_VERSION + 1,
                expected: RNG_VERSION
            })
        );

        let mut bad_encoding = bytes.clone();
        bad_encoding[20..24].copy_from_slice(&99u32.to_le_bytes());
        assert_eq!(
            decode(&bad_encoding),
            Err(ReplayError::StateEncodingVersion {
                found: 99,
                expected: canon::STATE_ENCODING_VERSION
            })
        );

        // total_rounds echo broken against the config word.
        let mut bad_echo = bytes.clone();
        // Header layout: magic4 + 5*u32 + u64 + u32 = 4+20+8+4 = 36; the
        // total_rounds echo sits at offset 32.
        bad_echo[32..36].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(
            decode(&bad_echo),
            Err(ReplayError::TotalRoundsEcho {
                header: 3,
                config: 2
            })
        );

        // Truncation at several offsets never panics.
        for cut in [0usize, 3, 10, 36, 40, bytes.len() / 2, bytes.len() - 1] {
            let result = decode(&bytes[..cut]);
            assert!(result.is_err(), "cut at {cut} must fail");
            assert!(
                !matches!(result, Err(ReplayError::Truncated { at }) if at > cut),
                "truncation offset must not exceed the cut"
            );
        }

        // Trailing garbage after the trailer.
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            decode(&trailing),
            Err(ReplayError::TrailingBytes { at: bytes.len() })
        );

        // Missing trailer.
        let no_trailer = ReplayFile {
            header: fixture_header(1),
            blocks: vec![Block::Round(fixture_record(1, [7; 32]))],
        }
        .encode();
        assert_eq!(decode(&no_trailer), Err(ReplayError::MissingTrailer));
    }

    // Round sequencing: a record claiming the wrong position fails.
    #[test]
    fn round_records_must_follow_the_sequence() {
        let mut file = fixture_file();
        if let Block::Round(record) = &mut file.blocks[1] {
            record.round = 5;
        }
        let bytes = file.encode();
        let err = decode(&bytes).unwrap_err();
        assert_eq!(
            err,
            ReplayError::RoundSequence {
                expected: 2,
                found: 5
            }
        );

        // Trailer count must match the records.
        let mut file = fixture_file();
        if let Block::End(EndOfMatch::Normal {
            rounds_completed, ..
        }) = &mut file.blocks[2]
        {
            *rounds_completed = 1;
        }
        let err = decode(&file.encode()).unwrap_err();
        assert_eq!(
            err,
            ReplayError::RoundCount {
                records: 2,
                trailer: 1
            }
        );
    }

    // notified_first is stored AND verified against the D20 parity.
    #[test]
    fn notified_first_parity_is_verified() {
        let mut file = fixture_file();
        if let Block::Round(record) = &mut file.blocks[0] {
            record.notified_first = NotifiedFirst::Enl; // round 1 must be Res
        }
        let err = decode(&file.encode()).unwrap_err();
        assert_eq!(
            err,
            ReplayError::NotifiedFirstParity {
                round: 1,
                stored: 1
            }
        );
        // The parity rule itself: odd -> Res, even -> Enl.
        assert_eq!(NotifiedFirst::for_round(1), NotifiedFirst::Res);
        assert_eq!(NotifiedFirst::for_round(2), NotifiedFirst::Enl);
        assert_eq!(NotifiedFirst::for_round(499), NotifiedFirst::Res);
        assert_eq!(NotifiedFirst::for_round(500), NotifiedFirst::Enl);
    }

    // Checkpoints land exactly at 50-multiples and agree with the round
    // record hash; tampering with any of the three facts fails decode.
    #[test]
    fn checkpoints_landing_and_agreement() {
        let hash = |seed: u8| {
            let canonical = vec![seed; 64];
            (crate::canon::hash_canonical_bytes(&canonical), canonical)
        };
        let (h50, canonical50) = hash(50);
        let (h100, canonical100) = hash(100);

        let mut blocks = Vec::new();
        for round in 1..=100u32 {
            let record_hash = if round == 50 {
                h50
            } else if round == 100 {
                h100
            } else {
                [round as u8; 32]
            };
            blocks.push(Block::Round(fixture_record(round, record_hash)));
            if round % CHECKPOINT_INTERVAL == 0 {
                blocks.push(Block::Checkpoint(Checkpoint {
                    round,
                    state_hash: record_hash,
                    canonical_state: if round == 50 {
                        canonical50.clone()
                    } else {
                        canonical100.clone()
                    },
                }));
            }
        }
        blocks.push(Block::End(EndOfMatch::Normal {
            rounds_completed: 100,
            final_state_hash: h100,
            result: MatchResult::Draw,
        }));
        let file = ReplayFile {
            header: fixture_header(100),
            blocks,
        };

        // Full round trip with checkpoints present and byte-exact.
        let bytes = file.encode();
        let decoded = decode(&bytes).expect("decodes");
        assert_eq!(decoded, file);
        assert_eq!(decoded.encode(), bytes);

        // The checkpoints decoded back re-encode byte-exact (folded into
        // the assertion above) and sit exactly after rounds 50 and 100.
        let checkpoint_rounds: Vec<u32> = decoded
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Checkpoint(c) => Some(c.round),
                _ => None,
            })
            .collect();
        assert_eq!(checkpoint_rounds, vec![50, 100]);

        // Missing the round-50 checkpoint (block removed).
        let mut tampered = file.clone();
        tampered
            .blocks
            .retain(|b| !matches!(b, Block::Checkpoint(c) if c.round == 50));
        assert_eq!(
            decode(&tampered.encode()),
            Err(ReplayError::CheckpointMissing { round: 50 })
        );

        // Checkpoint hash disagrees with the record's hash.
        let mut tampered = file.clone();
        for block in &mut tampered.blocks {
            if let Block::Checkpoint(c) = block {
                if c.round == 50 {
                    c.state_hash = [9; 32];
                }
            }
        }
        assert_eq!(
            decode(&tampered.encode()),
            Err(ReplayError::CheckpointHashDisagreement { round: 50 })
        );

        // Canonical bytes do not hash to the checkpoint hash.
        let mut tampered = file.clone();
        for block in &mut tampered.blocks {
            if let Block::Checkpoint(c) = block {
                if c.round == 50 {
                    c.canonical_state[0] ^= 1;
                }
            }
        }
        assert_eq!(
            decode(&tampered.encode()),
            Err(ReplayError::CheckpointIntegrity { round: 50 })
        );

        // A checkpoint after a non-boundary round is misplaced.
        let stray = ReplayFile {
            header: fixture_header(2),
            blocks: vec![
                Block::Round(fixture_record(1, [1; 32])),
                Block::Checkpoint(Checkpoint {
                    round: 1,
                    state_hash: [1; 32],
                    canonical_state: vec![0; 8],
                }),
                Block::End(EndOfMatch::Normal {
                    rounds_completed: 1,
                    final_state_hash: [1; 32],
                    result: MatchResult::Draw,
                }),
            ],
        };
        assert_eq!(
            decode(&stray.encode()),
            Err(ReplayError::CheckpointMisplaced { round: 1 })
        );
    }

    // Abnormal-end trailer round trip (hand-crafted; in-process M5 always
    // ends normal, but the format carries technical endings).
    #[test]
    fn abnormal_end_trailer_round_trips() {
        let file = ReplayFile {
            header: fixture_header(10),
            blocks: vec![
                Block::Round(fixture_record(1, [3; 32])),
                Block::End(EndOfMatch::Abnormal {
                    last_completed_round: 1,
                    last_valid_hash: [3; 32],
                    outcome_tag: FIRST_ABNORMAL_OUTCOME_TAG,
                }),
            ],
        };
        let bytes = file.encode();
        let decoded = decode(&bytes).expect("decodes");
        assert_eq!(decoded, file);
        assert_eq!(decoded.encode(), bytes);

        // Reserved outcome tags only.
        let mut bad = file.clone();
        if let Block::End(EndOfMatch::Abnormal { outcome_tag, .. }) = &mut bad.blocks[1] {
            *outcome_tag = 0;
        }
        assert_eq!(
            decode(&bad.encode()),
            Err(ReplayError::UnknownTag {
                what: "outcome",
                tag: 0
            })
        );
    }

    // Unknown tags in every tagged position fail with UnknownTag.
    #[test]
    fn unknown_tags_rejected() {
        let file = fixture_file();
        let bytes = file.encode();
        // The first block tag sits right after the header: magic 4 +
        // 5 u32 (20) + u64 (8) + u32 (4) + config 124 + map (4 + 3*13) +
        // spawns (4 + 12) * 2 = 4+20+8+4+124+43+32 = 235.
        let header_len = 4 + 20 + 8 + 4 + 124 + 4 + 3 * 13 + (4 + 12) * 2;
        assert_eq!(bytes[header_len], TAG_ROUND);
        let mut tampered = bytes.clone();
        tampered[header_len] = 9;
        assert_eq!(
            decode(&tampered),
            Err(ReplayError::UnknownTag {
                what: "block",
                tag: 9
            })
        );

        // Result tag inside a normal trailer: the result byte is the final
        // byte of the file.
        let bytes = file.encode();
        let mut tampered = bytes.clone();
        let result_byte = tampered.len() - 1;
        tampered[result_byte] = 7;
        assert_eq!(
            decode(&tampered),
            Err(ReplayError::UnknownTag {
                what: "result",
                tag: 7
            })
        );
    }

    // Header spawn lists must be ascending.
    #[test]
    fn spawn_lists_must_ascending() {
        let mut file = fixture_file();
        file.header.res_spawns = vec![(AgentId(5), Coord::new(1, 1))];
        file.header.enl_spawns = vec![
            (AgentId(3), Coord::new(1, 1)),
            (AgentId(2), Coord::new(2, 2)),
        ];
        assert!(matches!(
            decode(&file.encode()),
            Err(ReplayError::SpawnsOutOfOrder { .. })
        ));
    }
}

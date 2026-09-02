//! Owned protocol model + eager full-field decoding (the M7 Verifier gate).
//!
//! planus validates LAZILY: `read_as_root` is shallow and a corrupt buffer
//! can decode without error until a field is touched. Our gate therefore
//! converts the root reference into the OWNED generated type immediately —
//! `TryFrom<ProtocolEnvelopeRef> for ProtocolEnvelope` walks every field of
//! the matched union variant, including every vector element and every
//! string — so [`decode_envelope`] returns `Err` for any corrupt,
//! truncated, empty, or junk buffer and never panics. Nothing downstream
//! reads planus `Result` accessors on untrusted bytes.
//!
//! `to_core`-style conversions to arena-core types live here as well: the
//! server and SDK work with arena-core `Command`/`Coord`/`Faction` after
//! decoding, never with generated types.

use std::collections::BTreeMap;

use arena_core::types::{AgentId, Command, Coord, Faction};

use planus::ReadAsRoot;

use crate::generated::proto;

/// FlatBuffers file identifiers: the three roots are otherwise
/// structurally confusable (a short envelope vtable can decode as a
/// snapshot of defaults). Builders stamp the tag and every decoder
/// verifies it, so a buffer of one root kind can never pass as another
/// ("不能串区", P3-SHM).
pub const ENVELOPE_FILE_IDENTIFIER: &[u8; 4] = b"IENV";
pub const PUBLIC_FILE_IDENTIFIER: &[u8; 4] = b"IPUB";
pub const PRIVATE_FILE_IDENTIFIER: &[u8; 4] = b"IPRV";

/// Verifies the 4-byte file identifier and returns the buffer slice the
/// root offset lives in. planus writes `[identifier][root_offset u32]...`
/// and its `read_as_root` expects to start at the root offset, so the
/// decoder slices off the identifier first. Buffers without a matching
/// identifier are rejected for that root.
pub fn strip_file_identifier<'a>(
    bytes: &'a [u8],
    expected: &[u8; 4],
) -> Result<&'a [u8], ProtocolError> {
    if bytes.len() >= 8 && &bytes[0..4] == expected {
        Ok(&bytes[4..])
    } else {
        Err(ProtocolError::Invalid)
    }
}

/// Every envelope decode failure collapses to one recoverable error
/// (P3-RPC: an in-limit undecodable buffer is an envelope reject, never a
/// crash).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProtocolError {
    /// The buffer is not a decodable `ProtocolEnvelope` root (empty,
    /// truncated, corrupt, wrong root type, unknown union tag, ...).
    Invalid,
}

impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProtocolError::Invalid => write!(f, "invalid protocol envelope buffer"),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// Faction byte tags on the wire (D51).
pub fn faction_to_byte(faction: Faction) -> u8 {
    match faction {
        Faction::Enl => 0,
        Faction::Res => 1,
    }
}

/// Inverse of [`faction_to_byte`]; `None` on an unknown tag.
pub fn faction_from_byte(tag: u8) -> Option<Faction> {
    match tag {
        0 => Some(Faction::Enl),
        1 => Some(Faction::Res),
        _ => None,
    }
}

/// Owner byte tags on the wire (D51: 0 Neutral, 1 Enl, 2 Res).
pub fn owner_to_byte(owner: arena_core::types::Owner) -> u8 {
    use arena_core::types::Owner;
    match owner {
        Owner::Neutral => 0,
        Owner::Enl => 1,
        Owner::Res => 2,
    }
}

/// Inverse of [`owner_to_byte`]; `None` on an unknown tag.
pub fn owner_from_byte(tag: u8) -> Option<arena_core::types::Owner> {
    use arena_core::types::Owner;
    match tag {
        0 => Some(Owner::Neutral),
        1 => Some(Owner::Enl),
        2 => Some(Owner::Res),
        _ => None,
    }
}

/// Owned echo of the full rule parameter set (every `GameConfig` field).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MatchInfo {
    pub total_rounds: u32,
    pub initial_spawn_window_ms: u32,
    pub decision_window_ms: u32,
    pub priority_bucket_ms: u32,
    pub epoch_length: u32,
    pub map_width_cm: i32,
    pub map_height_cm: i32,
    pub portal_count: u32,
    pub portal_min_distance_cm: u32,
    pub clustered_portal_ratio_permille: u32,
    pub portal_cluster_count: u32,
    pub initial_agents_per_faction: u32,
    pub reinforcement_interval_rounds: u32,
    pub reinforcement_agents_per_faction: u32,
    pub move_budget_cm: u32,
    pub portal_operation_range_cm: u32,
    pub transfer_range_cm: u32,
    pub initial_supply: u32,
    pub max_supply: u32,
    pub portal_max_hp: u32,
    pub attack_damage: u32,
    pub recharge_amount: u32,
    pub recharge_cost_supply: u32,
    pub hack_supply_yield: u32,
    pub hack_key_yield_min: u32,
    pub hack_key_yield_max: u32,
    pub hack_cooldown_rounds: u32,
    pub hack_limit_per_epoch: u32,
    pub max_link_distance_cm: u32,
    pub max_commands_per_agent_round: u32,
    pub max_frame_payload_bytes: u32,
}

impl MatchInfo {
    /// Pure field copy from the authoritative config (no rule math).
    pub fn from_core(cfg: &arena_core::config::GameConfig) -> Self {
        MatchInfo {
            total_rounds: cfg.total_rounds,
            initial_spawn_window_ms: cfg.initial_spawn_window_ms,
            decision_window_ms: cfg.decision_window_ms,
            priority_bucket_ms: cfg.priority_bucket_ms,
            epoch_length: cfg.epoch_length,
            map_width_cm: cfg.map_width_cm,
            map_height_cm: cfg.map_height_cm,
            portal_count: cfg.portal_count,
            portal_min_distance_cm: cfg.portal_min_distance_cm,
            clustered_portal_ratio_permille: cfg.clustered_portal_ratio_permille,
            portal_cluster_count: cfg.portal_cluster_count,
            initial_agents_per_faction: cfg.initial_agents_per_faction,
            reinforcement_interval_rounds: cfg.reinforcement_interval_rounds,
            reinforcement_agents_per_faction: cfg.reinforcement_agents_per_faction,
            move_budget_cm: cfg.move_budget_cm,
            portal_operation_range_cm: cfg.portal_operation_range_cm,
            transfer_range_cm: cfg.transfer_range_cm,
            initial_supply: cfg.initial_supply,
            max_supply: cfg.max_supply,
            portal_max_hp: cfg.portal_max_hp,
            attack_damage: cfg.attack_damage,
            recharge_amount: cfg.recharge_amount,
            recharge_cost_supply: cfg.recharge_cost_supply,
            hack_supply_yield: cfg.hack_supply_yield,
            hack_key_yield_min: cfg.hack_key_yield_min,
            hack_key_yield_max: cfg.hack_key_yield_max,
            hack_cooldown_rounds: cfg.hack_cooldown_rounds,
            hack_limit_per_epoch: cfg.hack_limit_per_epoch,
            max_link_distance_cm: cfg.max_link_distance_cm,
            max_commands_per_agent_round: cfg.max_commands_per_agent_round,
            // Publish the ENFORCED legal maximum (D62), not the config's
            // exclusive ceiling: this field is the largest payload a
            // client may actually send, so it must equal what the
            // transport accepts — a client sizing its largest frame to
            // exactly the published value must be legal. (GameConfig
            // stores the ceiling, 65,536 per ADR-0008; the legal maximum
            // is one below it. `serve_match` rejects configs that
            // disagree with the compiled-in constant.)
            max_frame_payload_bytes: crate::frame::MAX_FRAME_PAYLOAD_BYTES as u32,
        }
    }

    /// Pure field copy back to a `GameConfig` (the caller validates it).
    pub fn to_core(&self) -> arena_core::config::GameConfig {
        arena_core::config::GameConfig {
            total_rounds: self.total_rounds,
            initial_spawn_window_ms: self.initial_spawn_window_ms,
            decision_window_ms: self.decision_window_ms,
            priority_bucket_ms: self.priority_bucket_ms,
            epoch_length: self.epoch_length,
            map_width_cm: self.map_width_cm,
            map_height_cm: self.map_height_cm,
            portal_count: self.portal_count,
            portal_min_distance_cm: self.portal_min_distance_cm,
            clustered_portal_ratio_permille: self.clustered_portal_ratio_permille,
            portal_cluster_count: self.portal_cluster_count,
            initial_agents_per_faction: self.initial_agents_per_faction,
            reinforcement_interval_rounds: self.reinforcement_interval_rounds,
            reinforcement_agents_per_faction: self.reinforcement_agents_per_faction,
            move_budget_cm: self.move_budget_cm,
            portal_operation_range_cm: self.portal_operation_range_cm,
            transfer_range_cm: self.transfer_range_cm,
            initial_supply: self.initial_supply,
            max_supply: self.max_supply,
            portal_max_hp: self.portal_max_hp,
            attack_damage: self.attack_damage,
            recharge_amount: self.recharge_amount,
            recharge_cost_supply: self.recharge_cost_supply,
            hack_supply_yield: self.hack_supply_yield,
            hack_key_yield_min: self.hack_key_yield_min,
            hack_key_yield_max: self.hack_key_yield_max,
            hack_cooldown_rounds: self.hack_cooldown_rounds,
            hack_limit_per_epoch: self.hack_limit_per_epoch,
            max_link_distance_cm: self.max_link_distance_cm,
            max_commands_per_agent_round: self.max_commands_per_agent_round,
            max_frame_payload_bytes: self.max_frame_payload_bytes,
        }
    }
}

/// One `(agent, coordinate)` spawn entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpawnEntry {
    pub agent_id: AgentId,
    pub position: Coord,
}

/// One command converted to the arena-core representation.
pub type CommandOut = Command;

/// The eleven P3-RPC messages as one owned enum. `SubmitCommandStream`
/// already carries arena-core `Command`s (the `to_core` conversion ran
/// during decode).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Envelope {
    ClientHello {
        client_name: String,
        client_version: String,
        protocol_version: u32,
        public_snapshot_version: u32,
        private_snapshot_version: u32,
    },
    ServerHello {
        /// 0 ok, 1 version mismatch, 2 duplicate connection.
        status: u8,
        assigned_faction: Faction,
        match_id: String,
    },
    InitialSpawnStart {
        spawn_deadline_remaining_ms: u32,
        info: MatchInfo,
    },
    SubmitInitialSpawns {
        positions: Vec<SpawnEntry>,
    },
    InitialSpawnResult {
        /// 0 accepted, 1 invalid set, 2 missed deadline.
        status: u8,
        detail: String,
    },
    RoundStartSignal,
    SubmitCommandStream {
        round: u32,
        agent_id: AgentId,
        commands: Vec<CommandOut>,
    },
    SubmissionResult {
        round: u32,
        agent_id: AgentId,
        /// 0 accepted-first, 1 rejected-structural, 2 ignored-duplicate,
        /// 3 unlocatable, 4 late-drop.
        status: u8,
        detail: String,
    },
    CommitRound {
        round: u32,
    },
    RoundClosed {
        round: u32,
    },
    GameEnd {
        /// 0 EnlWin, 1 ResWin, 2 Draw, 3 SpawnForfeit, 4 ClientForfeit,
        /// 5 SetupError.
        outcome: u8,
        detail: String,
        score_enl: u128,
        score_res: u128,
        rounds_completed: u32,
    },
}

/// Status codes of [`Envelope::SubmissionResult`].
pub const SUBMIT_ACCEPTED: u8 = 0;
pub const SUBMIT_REJECTED_STRUCTURAL: u8 = 1;
pub const SUBMIT_IGNORED_DUPLICATE: u8 = 2;
pub const SUBMIT_UNLOCATABLE: u8 = 3;
pub const SUBMIT_LATE_DROP: u8 = 4;

/// Status codes of [`Envelope::ServerHello`].
pub const HELLO_OK: u8 = 0;
pub const HELLO_VERSION_MISMATCH: u8 = 1;
pub const HELLO_DUPLICATE_CONNECTION: u8 = 2;

/// Outcome codes of [`Envelope::GameEnd`].
pub const END_ENL_WIN: u8 = 0;
pub const END_RES_WIN: u8 = 1;
pub const END_DRAW: u8 = 2;
pub const END_SPAWN_FORFEIT: u8 = 3;
pub const END_CLIENT_FORFEIT: u8 = 4;
pub const END_SETUP_ERROR: u8 = 5;

/// u128 score split into (hi, lo) u64 halves.
pub fn split_u128(value: u128) -> (u64, u64) {
    ((value >> 64) as u64, value as u64)
}

/// Inverse of [`split_u128`].
pub fn join_u128(hi: u64, lo: u64) -> u128 {
    ((hi as u128) << 64) | lo as u128
}

/// Eagerly full-decodes one envelope buffer (the Verifier gate; see the
/// module docs). Empty, truncated, corrupt, or junk input is `Err`.
pub fn decode_envelope(bytes: &[u8]) -> Result<Envelope, ProtocolError> {
    let body = strip_file_identifier(bytes, ENVELOPE_FILE_IDENTIFIER)?;
    let root =
        proto::ProtocolEnvelopeRef::read_as_root(body).map_err(|_| ProtocolError::Invalid)?;
    let owned: proto::ProtocolEnvelope = root.try_into().map_err(|_| ProtocolError::Invalid)?;
    convert_envelope(owned)
}

fn convert_envelope(value: proto::ProtocolEnvelope) -> Result<Envelope, ProtocolError> {
    let message = value.message.ok_or(ProtocolError::Invalid)?;
    Ok(match message {
        proto::Message::ClientHello(inner) => Envelope::ClientHello {
            client_name: inner.client_name.unwrap_or_default(),
            client_version: inner.client_version.unwrap_or_default(),
            protocol_version: inner.protocol_version,
            public_snapshot_version: inner.public_snapshot_version,
            private_snapshot_version: inner.private_snapshot_version,
        },
        proto::Message::ServerHello(inner) => Envelope::ServerHello {
            status: inner.status,
            assigned_faction: match inner.assigned_faction {
                proto::Faction::Enl => Faction::Enl,
                proto::Faction::Res => Faction::Res,
            },
            match_id: inner.match_id.unwrap_or_default(),
        },
        proto::Message::InitialSpawnStart(inner) => {
            let info: proto::MatchInfo = *inner.info.ok_or(ProtocolError::Invalid)?;
            Envelope::InitialSpawnStart {
                spawn_deadline_remaining_ms: inner.spawn_deadline_remaining_ms,
                info: convert_match_info(info),
            }
        }
        proto::Message::SubmitInitialSpawns(inner) => Envelope::SubmitInitialSpawns {
            positions: inner
                .positions
                .unwrap_or_default()
                .into_iter()
                .map(|entry| SpawnEntry {
                    agent_id: AgentId(entry.agent_id),
                    position: Coord::new(entry.x, entry.y),
                })
                .collect(),
        },
        proto::Message::InitialSpawnResult(inner) => Envelope::InitialSpawnResult {
            status: inner.status,
            detail: inner.detail.unwrap_or_default(),
        },
        proto::Message::RoundStartSignal(_) => Envelope::RoundStartSignal,
        proto::Message::SubmitCommandStream(inner) => Envelope::SubmitCommandStream {
            round: inner.round,
            agent_id: AgentId(inner.agent_id),
            commands: inner
                .commands
                .unwrap_or_default()
                .into_iter()
                .map(convert_command)
                .collect::<Result<Vec<_>, _>>()?,
        },
        proto::Message::SubmissionResult(inner) => Envelope::SubmissionResult {
            round: inner.round,
            agent_id: AgentId(inner.agent_id),
            status: inner.status,
            detail: inner.detail.unwrap_or_default(),
        },
        proto::Message::CommitRound(inner) => Envelope::CommitRound { round: inner.round },
        proto::Message::RoundClosed(inner) => Envelope::RoundClosed { round: inner.round },
        proto::Message::GameEnd(inner) => Envelope::GameEnd {
            outcome: inner.outcome,
            detail: inner.detail.unwrap_or_default(),
            score_enl: join_u128(inner.score_enl_hi, inner.score_enl_lo),
            score_res: join_u128(inner.score_res_hi, inner.score_res_lo),
            rounds_completed: inner.rounds_completed,
        },
    })
}

fn convert_match_info(info: proto::MatchInfo) -> MatchInfo {
    MatchInfo {
        total_rounds: info.total_rounds,
        initial_spawn_window_ms: info.initial_spawn_window_ms,
        decision_window_ms: info.decision_window_ms,
        priority_bucket_ms: info.priority_bucket_ms,
        epoch_length: info.epoch_length,
        map_width_cm: info.map_width_cm,
        map_height_cm: info.map_height_cm,
        portal_count: info.portal_count,
        portal_min_distance_cm: info.portal_min_distance_cm,
        clustered_portal_ratio_permille: info.clustered_portal_ratio_permille,
        portal_cluster_count: info.portal_cluster_count,
        initial_agents_per_faction: info.initial_agents_per_faction,
        reinforcement_interval_rounds: info.reinforcement_interval_rounds,
        reinforcement_agents_per_faction: info.reinforcement_agents_per_faction,
        move_budget_cm: info.move_budget_cm,
        portal_operation_range_cm: info.portal_operation_range_cm,
        transfer_range_cm: info.transfer_range_cm,
        initial_supply: info.initial_supply,
        max_supply: info.max_supply,
        portal_max_hp: info.portal_max_hp,
        attack_damage: info.attack_damage,
        recharge_amount: info.recharge_amount,
        recharge_cost_supply: info.recharge_cost_supply,
        hack_supply_yield: info.hack_supply_yield,
        hack_key_yield_min: info.hack_key_yield_min,
        hack_key_yield_max: info.hack_key_yield_max,
        hack_cooldown_rounds: info.hack_cooldown_rounds,
        hack_limit_per_epoch: info.hack_limit_per_epoch,
        max_link_distance_cm: info.max_link_distance_cm,
        max_commands_per_agent_round: info.max_commands_per_agent_round,
        max_frame_payload_bytes: info.max_frame_payload_bytes,
    }
}

fn convert_command(command: proto::Command) -> Result<Command, ProtocolError> {
    use arena_core::types::PortalId;
    let body = command.body.ok_or(ProtocolError::Invalid)?;
    Ok(match body {
        proto::CommandBody::Move(inner) => Command::Move {
            destination: Coord::new(inner.x, inner.y),
        },
        proto::CommandBody::Attack(_) => Command::Attack,
        proto::CommandBody::Capture(inner) => Command::Capture {
            portal: PortalId(inner.portal),
        },
        proto::CommandBody::Recharge(inner) => Command::Recharge {
            portal: PortalId(inner.portal),
        },
        proto::CommandBody::Hack(inner) => Command::Hack {
            portal: PortalId(inner.portal),
        },
        proto::CommandBody::Link(inner) => Command::Link {
            source_portal: PortalId(inner.a),
            target_portal: PortalId(inner.b),
        },
        proto::CommandBody::Transfer(inner) => {
            let mut keys = BTreeMap::new();
            for entry in inner.keys.unwrap_or_default() {
                keys.insert(arena_core::types::PortalId(entry.portal), entry.count);
            }
            Command::Transfer {
                target_agent: AgentId(inner.target_agent),
                supply: inner.supply,
                keys,
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Owned snapshot mirrors (eager decode for tests, journals, and the SDK's
// non-zero-copy paths; the SDK's live view uses the generated `*Ref` types
// directly).
// ---------------------------------------------------------------------------

use arena_core::event::{Event, EventKind};
use arena_core::observe::{AgentPrivate, FieldPublic, LinkPublic, PortalPublic};
use arena_core::types::{FieldId, LinkId, PortalId};

/// Owned mirror of a public snapshot.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PublicSnap {
    pub schema_version: u32,
    pub current_round: u32,
    pub snapshot_version: u64,
    pub round_start_ms: u64,
    pub deadline_ms: u64,
    pub score_enl: u128,
    pub score_res: u128,
    pub portals: Vec<PortalPublic>,
    pub links: Vec<LinkPublic>,
    pub fields: Vec<FieldPublic>,
    pub events: Vec<Event>,
}

/// Owned mirror of a private snapshot.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrivateSnap {
    pub schema_version: u32,
    pub current_round: u32,
    pub snapshot_version: u64,
    pub agents: Vec<AgentPrivate>,
}

/// Eagerly full-decodes a public snapshot buffer.
pub fn decode_public(bytes: &[u8]) -> Result<PublicSnap, ProtocolError> {
    use crate::generated::snap_public as snap_pub;
    let body = strip_file_identifier(bytes, PUBLIC_FILE_IDENTIFIER)?;
    let root =
        snap_pub::PublicSnapshotRef::read_as_root(body).map_err(|_| ProtocolError::Invalid)?;
    let owned: snap_pub::PublicSnapshot = root.try_into().map_err(|_| ProtocolError::Invalid)?;
    let portals = owned
        .portals
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            Ok(PortalPublic {
                id: PortalId(p.id),
                position: Coord::new(p.x, p.y),
                owner: owner_from_byte(p.owner).ok_or(ProtocolError::Invalid)?,
                hp: p.hp,
            })
        })
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    let links = owned
        .links
        .unwrap_or_default()
        .into_iter()
        .map(|l| {
            Ok(LinkPublic {
                id: LinkId {
                    min: PortalId(l.a.min(l.b)),
                    max: PortalId(l.a.max(l.b)),
                },
                faction: faction_from_byte(l.faction).ok_or(ProtocolError::Invalid)?,
            })
        })
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    let fields = owned
        .fields
        .unwrap_or_default()
        .into_iter()
        .map(|f| {
            Ok(FieldPublic {
                id: FieldId::new(PortalId(f.a), PortalId(f.b), PortalId(f.c))
                    .ok_or(ProtocolError::Invalid)?,
                faction: faction_from_byte(f.faction).ok_or(ProtocolError::Invalid)?,
                area2: f.area2,
            })
        })
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    let events = owned
        .events
        .unwrap_or_default()
        .into_iter()
        .map(convert_event_entry)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PublicSnap {
        schema_version: owned.schema_version,
        current_round: owned.current_round,
        snapshot_version: owned.snapshot_version,
        round_start_ms: owned.round_start_ms,
        deadline_ms: owned.deadline_ms,
        score_enl: join_u128(owned.score_enl_hi, owned.score_enl_lo),
        score_res: join_u128(owned.score_res_hi, owned.score_res_lo),
        portals,
        links,
        fields,
        events,
    })
}

fn convert_event_entry(
    entry: crate::generated::snap_public::EventEntry,
) -> Result<Event, ProtocolError> {
    use crate::generated::snap_public as snap_pub;
    let kind = match entry.body.ok_or(ProtocolError::Invalid)? {
        snap_pub::EventBody::PortalCaptured(inner) => EventKind::PortalCaptured {
            faction: faction_from_byte(inner.faction).ok_or(ProtocolError::Invalid)?,
            agent_id: AgentId(inner.agent_id),
            portal: PortalId(inner.portal),
        },
        snap_pub::EventBody::PortalAttacked(inner) => EventKind::PortalAttacked {
            faction: faction_from_byte(inner.faction).ok_or(ProtocolError::Invalid)?,
            agent_id: AgentId(inner.agent_id),
            portal: PortalId(inner.portal),
            damage: inner.damage,
            hp_after: inner.hp_after,
        },
        snap_pub::EventBody::PortalNeutralized(inner) => EventKind::PortalNeutralized {
            faction: faction_from_byte(inner.faction).ok_or(ProtocolError::Invalid)?,
            agent_id: AgentId(inner.agent_id),
            portal: PortalId(inner.portal),
        },
        snap_pub::EventBody::LinkCreated(inner) => EventKind::LinkCreated {
            faction: faction_from_byte(inner.faction).ok_or(ProtocolError::Invalid)?,
            agent_id: AgentId(inner.agent_id),
            link: LinkId {
                min: PortalId(inner.a.min(inner.b)),
                max: PortalId(inner.a.max(inner.b)),
            },
        },
        snap_pub::EventBody::LinkDestroyed(inner) => EventKind::LinkDestroyed {
            faction: faction_from_byte(inner.faction).ok_or(ProtocolError::Invalid)?,
            agent_id: AgentId(inner.agent_id),
            link: LinkId {
                min: PortalId(inner.a.min(inner.b)),
                max: PortalId(inner.a.max(inner.b)),
            },
        },
        snap_pub::EventBody::FieldCreated(inner) => EventKind::FieldCreated {
            faction: faction_from_byte(inner.faction).ok_or(ProtocolError::Invalid)?,
            agent_id: AgentId(inner.agent_id),
            field: FieldId::new(PortalId(inner.a), PortalId(inner.b), PortalId(inner.c))
                .ok_or(ProtocolError::Invalid)?,
        },
        snap_pub::EventBody::FieldDestroyed(inner) => EventKind::FieldDestroyed {
            faction: faction_from_byte(inner.faction).ok_or(ProtocolError::Invalid)?,
            agent_id: AgentId(inner.agent_id),
            field: FieldId::new(PortalId(inner.a), PortalId(inner.b), PortalId(inner.c))
                .ok_or(ProtocolError::Invalid)?,
        },
        snap_pub::EventBody::Reinforcement(inner) => EventKind::Reinforcement {
            index: inner.index,
            position: Coord::new(inner.x, inner.y),
            count_per_faction: inner.count,
            first_agent_id: AgentId(inner.first_agent_id),
        },
    };
    Ok(Event {
        seq: entry.seq,
        round: entry.round,
        kind,
    })
}

/// Eagerly full-decodes a private snapshot buffer.
pub fn decode_private(bytes: &[u8]) -> Result<PrivateSnap, ProtocolError> {
    use crate::generated::snap_private as snap_priv;
    let body = strip_file_identifier(bytes, PRIVATE_FILE_IDENTIFIER)?;
    let root =
        snap_priv::PrivateSnapshotRef::read_as_root(body).map_err(|_| ProtocolError::Invalid)?;
    let owned: snap_priv::PrivateSnapshot = root.try_into().map_err(|_| ProtocolError::Invalid)?;
    let agents = owned
        .agents
        .unwrap_or_default()
        .into_iter()
        .map(|a| {
            let mut keys = BTreeMap::new();
            for entry in a.keys.unwrap_or_default() {
                keys.insert(PortalId(entry.portal), entry.count);
            }
            AgentPrivate {
                id: AgentId(a.id),
                position: Coord::new(a.x, a.y),
                supply: a.supply,
                keys,
            }
        })
        .collect();
    Ok(PrivateSnap {
        schema_version: owned.schema_version,
        current_round: owned.current_round,
        snapshot_version: owned.snapshot_version,
        agents,
    })
}

/// Decodes a buffer whose root must be the PRIVATE snapshot; used by the
/// partition test to prove public and private roots are not interchangeable.
pub fn decode_public_as_private_fails(bytes: &[u8]) -> bool {
    decode_private(bytes).is_err()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_buf() -> Vec<u8> {
        Vec::new()
    }

    #[test]
    fn empty_and_junk_buffers_rejected_without_panic() {
        assert_eq!(decode_envelope(&empty_buf()), Err(ProtocolError::Invalid));
        assert_eq!(decode_envelope(b"junk"), Err(ProtocolError::Invalid));
        assert_eq!(
            decode_envelope(&[0u8; 64]),
            Err(ProtocolError::Invalid),
            "zeros are not a valid root"
        );
    }

    #[test]
    fn truncated_and_corrupt_buffers_rejected() {
        // Build a real envelope, then damage it.
        let good = crate::build::envelope_bytes(&Envelope::ClientHello {
            client_name: "probe".into(),
            client_version: "1".into(),
            protocol_version: 1,
            public_snapshot_version: 1,
            private_snapshot_version: 1,
        });
        // Truncated tail.
        assert_eq!(
            decode_envelope(&good[..good.len() / 2]),
            Err(ProtocolError::Invalid)
        );
        // Flipped interior bytes: still decodes or rejects, never panics;
        // with the interior flipped the string offsets usually break.
        for index in [8usize, 12, 16, 20] {
            let mut damaged = good.clone();
            damaged[index] ^= 0xFF;
            let _ = decode_envelope(&damaged); // must not panic
        }
        // A byte flip inside the string data region (near the tail) keeps
        // the envelope decodable — same structure, possibly changed
        // characters; the gate must never panic either way.
        for index in [good.len() - 1, good.len() - 2, good.len() - 3] {
            let mut flipped = good.clone();
            flipped[index] ^= 0xFF;
            if let Ok(Envelope::ClientHello { client_name, .. }) = decode_envelope(&flipped) {
                assert_eq!(client_name.len(), 5, "string length is structural");
            }
        }
    }
}

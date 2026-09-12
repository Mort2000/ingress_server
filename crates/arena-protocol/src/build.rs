//! Buffer construction: envelopes and SHM snapshots (pure field
//! marshalling of arena-core accessors — zero rule math lives here).

use std::collections::BTreeMap;

use arena_core::event::{Event, EventKind};
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::{AgentId, Command, Coord, Faction, PortalId};

use planus::WriteAsOffset;

use crate::decode::{
    ENVELOPE_FILE_IDENTIFIER, Envelope, MatchInfo, PRIVATE_FILE_IDENTIFIER, PUBLIC_FILE_IDENTIFIER,
    SpawnEntry, faction_to_byte, owner_to_byte, split_u128,
};
use crate::generated::proto;
use crate::generated::snap_private;
use crate::generated::snap_public;
use crate::versions::{PRIVATE_SNAPSHOT_VERSION, PROTOCOL_VERSION, PUBLIC_SNAPSHOT_VERSION};

// ---------------------------------------------------------------------------
// Envelopes
// ---------------------------------------------------------------------------

/// Serializes one envelope into a fresh `ProtocolEnvelope` buffer (no
/// frame prefix — pair with [`crate::frame::encode_frame`]).
pub fn envelope_bytes(envelope: &Envelope) -> Vec<u8> {
    let message = message_owned(envelope);
    let mut builder = planus::Builder::new();
    let offset = proto::ProtocolEnvelope::builder()
        .protocol_version(PROTOCOL_VERSION)
        .message(&message)
        .finish(&mut builder);
    builder
        .finish(offset, Some(*ENVELOPE_FILE_IDENTIFIER))
        .to_vec()
}

/// Serializes one envelope into a complete frame (prefix + payload).
pub fn envelope_frame(envelope: &Envelope) -> Vec<u8> {
    let payload = envelope_bytes(envelope);
    crate::frame::encode_frame_vec(&payload)
        .expect("protocol messages stay far below the frame limit")
}

fn message_owned(envelope: &Envelope) -> proto::Message {
    match envelope {
        Envelope::ClientHello {
            client_name,
            client_version,
            bot_version,
            sdk_version,
            protocol_version,
            public_snapshot_version,
            private_snapshot_version,
        } => proto::Message::ClientHello(Box::new(proto::ClientHello {
            client_name: Some(client_name.clone()),
            client_version: Some(client_version.clone()),
            bot_version: Some(bot_version.clone()),
            sdk_version: Some(sdk_version.clone()),
            protocol_version: *protocol_version,
            public_snapshot_version: *public_snapshot_version,
            private_snapshot_version: *private_snapshot_version,
        })),
        Envelope::ServerHello {
            status,
            assigned_faction,
            match_id,
        } => proto::Message::ServerHello(Box::new(proto::ServerHello {
            status: *status,
            assigned_faction: match assigned_faction {
                Faction::Enl => proto::Faction::Enl,
                Faction::Res => proto::Faction::Res,
            },
            match_id: Some(match_id.clone()),
        })),
        Envelope::InitialSpawnStart {
            spawn_deadline_remaining_ms,
            info,
        } => proto::Message::InitialSpawnStart(Box::new(proto::InitialSpawnStart {
            spawn_deadline_remaining_ms: *spawn_deadline_remaining_ms,
            info: Some(Box::new(match_info_owned(info))),
        })),
        Envelope::SubmitInitialSpawns { positions } => {
            proto::Message::SubmitInitialSpawns(Box::new(proto::SubmitInitialSpawns {
                positions: Some(
                    positions
                        .iter()
                        .map(|entry| spawn_entry_owned(*entry))
                        .collect(),
                ),
            }))
        }
        Envelope::InitialSpawnResult { status, detail } => {
            proto::Message::InitialSpawnResult(Box::new(proto::InitialSpawnResult {
                status: *status,
                detail: Some(detail.clone()),
            }))
        }
        Envelope::RoundStartSignal => {
            proto::Message::RoundStartSignal(Box::new(proto::RoundStartSignal {}))
        }
        Envelope::SubmitCommandStream {
            round,
            agent_id,
            commands,
        } => proto::Message::SubmitCommandStream(Box::new(proto::SubmitCommandStream {
            round: *round,
            agent_id: agent_id.0,
            commands: Some(
                commands
                    .iter()
                    .map(|command| proto::Command {
                        body: Some(command_body_owned(command)),
                    })
                    .collect(),
            ),
        })),
        Envelope::SubmissionResult {
            round,
            agent_id,
            status,
            detail,
        } => proto::Message::SubmissionResult(Box::new(proto::SubmissionResult {
            round: *round,
            agent_id: agent_id.0,
            status: *status,
            detail: Some(detail.clone()),
        })),
        Envelope::CommitRound { round } => {
            proto::Message::CommitRound(Box::new(proto::CommitRound { round: *round }))
        }
        Envelope::RoundClosed { round } => {
            proto::Message::RoundClosed(Box::new(proto::RoundClosed { round: *round }))
        }
        Envelope::GameEnd {
            outcome,
            detail,
            score_enl,
            score_res,
            rounds_completed,
        } => {
            let (enl_hi, enl_lo) = split_u128(*score_enl);
            let (res_hi, res_lo) = split_u128(*score_res);
            proto::Message::GameEnd(Box::new(proto::GameEnd {
                outcome: *outcome,
                detail: Some(detail.clone()),
                score_enl_hi: enl_hi,
                score_enl_lo: enl_lo,
                score_res_hi: res_hi,
                score_res_lo: res_lo,
                rounds_completed: *rounds_completed,
            }))
        }
    }
}

fn match_info_owned(info: &MatchInfo) -> proto::MatchInfo {
    proto::MatchInfo {
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
        max_reinforcement_agents_per_faction: info.max_reinforcement_agents_per_faction,
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

fn spawn_entry_owned(entry: SpawnEntry) -> proto::SpawnEntry {
    proto::SpawnEntry {
        agent_id: entry.agent_id.0,
        x: entry.position.x,
        y: entry.position.y,
    }
}

fn command_body_owned(command: &Command) -> proto::CommandBody {
    match command {
        Command::Move { destination } => proto::CommandBody::Move(Box::new(proto::CmdMove {
            x: destination.x,
            y: destination.y,
        })),
        Command::Attack => proto::CommandBody::Attack(Box::new(proto::CmdEmpty {})),
        Command::Capture { portal } => {
            proto::CommandBody::Capture(Box::new(proto::CmdPortal { portal: portal.0 }))
        }
        Command::Recharge { portal } => {
            proto::CommandBody::Recharge(Box::new(proto::CmdPortal { portal: portal.0 }))
        }
        Command::Hack { portal } => {
            proto::CommandBody::Hack(Box::new(proto::CmdPortal { portal: portal.0 }))
        }
        Command::Link {
            source_portal,
            target_portal,
        } => proto::CommandBody::Link(Box::new(proto::CmdLink {
            a: source_portal.0,
            b: target_portal.0,
        })),
        Command::Transfer {
            target_agent,
            supply,
            keys,
        } => proto::CommandBody::Transfer(Box::new(proto::CmdTransfer {
            target_agent: target_agent.0,
            supply: *supply,
            keys: Some(
                keys.iter()
                    .map(|(&portal, &count)| proto::KeyCount {
                        portal: portal.0,
                        count,
                    })
                    .collect(),
            ),
        })),
    }
}

// ---------------------------------------------------------------------------
// Public snapshot
// ---------------------------------------------------------------------------

/// Builds the round-start public snapshot from one faction's observation
/// (the public part is identical for both factions by construction).
/// `snapshot_version` is the generation counter; `round_start_ms` /
/// `deadline_ms` are the server-monotonic timing fields carried in the SHM
/// metadata as well (client SDKs measure locally against their own
/// ROUND_START_SIGNAL receipt; these fields are informational).
pub fn public_snapshot_bytes(
    obs: &Observation,
    snapshot_version: u64,
    round_start_ms: u64,
    deadline_ms: u64,
) -> Vec<u8> {
    let score_enl = obs.scores.get(&Faction::Enl).copied().unwrap_or(0);
    let score_res = obs.scores.get(&Faction::Res).copied().unwrap_or(0);
    let (enl_hi, enl_lo) = split_u128(score_enl);
    let (res_hi, res_lo) = split_u128(score_res);
    let snapshot = snap_public::PublicSnapshot {
        schema_version: PUBLIC_SNAPSHOT_VERSION,
        current_round: obs.round,
        snapshot_version,
        round_start_ms,
        deadline_ms,
        score_enl_hi: enl_hi,
        score_enl_lo: enl_lo,
        score_res_hi: res_hi,
        score_res_lo: res_lo,
        portals: Some(
            obs.portals
                .iter()
                .map(|p| snap_public::PortalEntry {
                    id: p.id.0,
                    x: p.position.x,
                    y: p.position.y,
                    owner: owner_to_byte(p.owner),
                    hp: p.hp,
                })
                .collect(),
        ),
        links: Some(
            obs.links
                .iter()
                .map(|l| snap_public::LinkEntry {
                    a: l.id.min.0,
                    b: l.id.max.0,
                    faction: faction_to_byte(l.faction),
                })
                .collect(),
        ),
        fields: Some(
            obs.fields
                .iter()
                .map(|f| snap_public::FieldEntry {
                    a: f.id.a.0,
                    b: f.id.b.0,
                    c: f.id.c.0,
                    faction: faction_to_byte(f.faction),
                    area2: f.area2,
                })
                .collect(),
        ),
        events: Some(obs.events.iter().map(event_entry_owned).collect()),
    };
    finish_root!(&snapshot, PUBLIC_FILE_IDENTIFIER)
}

/// Builds generation 0 of the public snapshot: the pre-match frozen map
/// (round 0, no links/fields/events, zero scores). The spawn window timing
/// fills `round_start_ms`/`deadline_ms`.
pub fn map_generation_bytes(map: &MapDefinition, round_start_ms: u64, deadline_ms: u64) -> Vec<u8> {
    let snapshot = snap_public::PublicSnapshot {
        schema_version: PUBLIC_SNAPSHOT_VERSION,
        current_round: 0,
        snapshot_version: 0,
        round_start_ms,
        deadline_ms,
        score_enl_hi: 0,
        score_enl_lo: 0,
        score_res_hi: 0,
        score_res_lo: 0,
        portals: Some(
            map.portals()
                .iter()
                .map(|p| snap_public::PortalEntry {
                    id: p.id.0,
                    x: p.position.x,
                    y: p.position.y,
                    owner: owner_to_byte(p.owner),
                    hp: p.hp,
                })
                .collect(),
        ),
        links: None,
        fields: None,
        events: Some(Vec::new()),
    };
    finish_root!(&snapshot, PUBLIC_FILE_IDENTIFIER)
}

fn event_entry_owned(event: &Event) -> snap_public::EventEntry {
    use snap_public::EventBody;
    let body = match &event.kind {
        EventKind::PortalCaptured {
            faction,
            agent_id,
            portal,
        } => EventBody::PortalCaptured(Box::new(snap_public::EvPortal {
            faction: faction_to_byte(*faction),
            agent_id: agent_id.0,
            portal: portal.0,
        })),
        EventKind::PortalAttacked {
            faction,
            agent_id,
            portal,
            damage,
            hp_after,
        } => EventBody::PortalAttacked(Box::new(snap_public::EvAttacked {
            faction: faction_to_byte(*faction),
            agent_id: agent_id.0,
            portal: portal.0,
            damage: *damage,
            hp_after: *hp_after,
        })),
        EventKind::PortalNeutralized {
            faction,
            agent_id,
            portal,
        } => EventBody::PortalNeutralized(Box::new(snap_public::EvPortal {
            faction: faction_to_byte(*faction),
            agent_id: agent_id.0,
            portal: portal.0,
        })),
        EventKind::LinkCreated {
            faction,
            agent_id,
            link,
        } => EventBody::LinkCreated(Box::new(snap_public::EvLink {
            faction: faction_to_byte(*faction),
            agent_id: agent_id.0,
            a: link.min.0,
            b: link.max.0,
        })),
        EventKind::LinkDestroyed {
            faction,
            agent_id,
            link,
        } => EventBody::LinkDestroyed(Box::new(snap_public::EvLink {
            faction: faction_to_byte(*faction),
            agent_id: agent_id.0,
            a: link.min.0,
            b: link.max.0,
        })),
        EventKind::FieldCreated {
            faction,
            agent_id,
            field,
        } => EventBody::FieldCreated(Box::new(snap_public::EvField {
            faction: faction_to_byte(*faction),
            agent_id: agent_id.0,
            a: field.a.0,
            b: field.b.0,
            c: field.c.0,
        })),
        EventKind::FieldDestroyed {
            faction,
            agent_id,
            field,
        } => EventBody::FieldDestroyed(Box::new(snap_public::EvField {
            faction: faction_to_byte(*faction),
            agent_id: agent_id.0,
            a: field.a.0,
            b: field.b.0,
            c: field.c.0,
        })),
        EventKind::Reinforcement {
            index,
            position,
            count_per_faction,
            first_agent_id,
        } => EventBody::Reinforcement(Box::new(snap_public::EvReinforce {
            index: *index,
            x: position.x,
            y: position.y,
            count: *count_per_faction,
            first_agent_id: first_agent_id.0,
        })),
    };
    snap_public::EventEntry {
        seq: event.seq,
        round: event.round,
        body: Some(body),
    }
}

// ---------------------------------------------------------------------------
// Private snapshot
// ---------------------------------------------------------------------------

/// Builds the round-start private snapshot for the observation's own
/// faction.
pub fn private_snapshot_bytes(obs: &Observation, snapshot_version: u64) -> Vec<u8> {
    let snapshot = snap_private::PrivateSnapshot {
        schema_version: PRIVATE_SNAPSHOT_VERSION,
        current_round: obs.round,
        snapshot_version,
        agents: Some(
            obs.own_agents
                .iter()
                .map(|agent| snap_private::AgentEntry {
                    id: agent.id.0,
                    x: agent.position.x,
                    y: agent.position.y,
                    supply: agent.supply,
                    keys: Some(
                        agent
                            .keys
                            .iter()
                            .map(|(&portal, &count)| snap_private::KeyCount {
                                portal: portal.0,
                                count,
                            })
                            .collect(),
                    ),
                })
                .collect(),
        ),
    };
    finish_root!(&snapshot, PRIVATE_FILE_IDENTIFIER)
}

/// Builds generation 0 of the private snapshot: the faction's own roster
/// (ids from `initial_agents_per_faction`, D17) with pre-spawn sentinel
/// positions (0,0) — documented in `schemas/private_snapshot.fbs`.
pub fn private_roster_bytes(cfg: &arena_core::config::GameConfig, faction: Faction) -> Vec<u8> {
    let (enl_roster, res_roster) = arena_core::state::GameState::initial_roster(cfg);
    let roster: &[AgentId] = match faction {
        Faction::Enl => &enl_roster,
        Faction::Res => &res_roster,
    };
    let sentinel = Coord::new(0, 0);
    let snapshot = snap_private::PrivateSnapshot {
        schema_version: PRIVATE_SNAPSHOT_VERSION,
        current_round: 0,
        snapshot_version: 0,
        agents: Some(
            roster
                .iter()
                .map(|&id| snap_private::AgentEntry {
                    id: id.0,
                    x: sentinel.x,
                    y: sentinel.y,
                    supply: cfg.initial_supply,
                    keys: Some(Vec::new()),
                })
                .collect(),
        ),
    };
    finish_root!(&snapshot, PRIVATE_FILE_IDENTIFIER)
}

/// Keys helper for callers that build `AgentEntry` values by hand.
pub fn key_counts_owned(keys: &BTreeMap<PortalId, u32>) -> Vec<snap_private::KeyCount> {
    keys.iter()
        .map(|(&portal, &count)| snap_private::KeyCount {
            portal: portal.0,
            count,
        })
        .collect()
}

/// Finishes one owned snapshot root into a FlatBuffers buffer.
macro_rules! finish_root {
    ($value:expr, $identifier:expr) => {{
        let mut builder = planus::Builder::new();
        let offset = $value.prepare(&mut builder);
        builder.finish(offset, Some(*$identifier)).to_vec()
    }};
}
use finish_root;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_root_helper_compiles_for_all_three_roots() {
        // Type-level smoke: each root type is WriteAsOffset of itself.
        fn assert_root<T>()
        where
            for<'a> &'a T: planus::WriteAsOffset<T>,
        {
        }
        assert_root::<proto::ProtocolEnvelope>();
        assert_root::<snap_public::PublicSnapshot>();
        assert_root::<snap_private::PrivateSnapshot>();
    }
}

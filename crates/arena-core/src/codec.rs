//! Shared canonical codec primitives (D44/D51,): the fixed-width
//! little-endian encodings used by BOTH the M5 canonical state hash
//! (`arena-replay` canon) and the IARP replay file header, so the two can
//! never drift apart. Everything here is pure byte mapping — no validation,
//! no game semantics; callers validate decoded values through
//! `GameConfig::validate` / `MapDefinition::validate`.
//!
//! Conventions (D2/D51): integers are fixed-width unsigned little-endian,
//! concatenated without padding; `i32` coordinates encode as their
//! two's-complement `u32` bit pattern; `Faction` tags are 0=Enl/1=Res and
//! `Owner` tags 0=Neutral/1=Enl/2=Res. The portal encoding omits the dense
//! id (the slice position IS the id); the config encoding is the 31 u32
//! words of `GameConfig` in struct-declaration order.

use crate::config::GameConfig;
use crate::types::{Faction, Owner, Portal, PortalId};

/// u32 word count of the canonical `GameConfig` encoding (D44 section 1).
pub const CONFIG_U32_COUNT: usize = 31;

/// Byte length of the canonical `GameConfig` encoding.
pub const CONFIG_BYTES: usize = CONFIG_U32_COUNT * 4;

/// Byte length of the canonical portal encoding (owner, hp, x, y).
pub const PORTAL_BYTES: usize = 1 + 4 + 4 + 4;

/// Faction tag (D51): 0=Enl, 1=Res.
pub fn faction_tag(faction: Faction) -> u8 {
    match faction {
        Faction::Enl => 0,
        Faction::Res => 1,
    }
}

/// Inverse of [`faction_tag`]; every other byte is invalid.
pub fn faction_from_tag(tag: u8) -> Option<Faction> {
    match tag {
        0 => Some(Faction::Enl),
        1 => Some(Faction::Res),
        _ => None,
    }
}

/// Owner tag (D51): 0=Neutral, 1=Enl, 2=Res.
pub fn owner_tag(owner: Owner) -> u8 {
    match owner {
        Owner::Neutral => 0,
        Owner::Enl => 1,
        Owner::Res => 2,
    }
}

/// Inverse of [`owner_tag`]; every other byte is invalid.
pub fn owner_from_tag(tag: u8) -> Option<Owner> {
    match tag {
        0 => Some(Owner::Neutral),
        1 => Some(Owner::Enl),
        2 => Some(Owner::Res),
        _ => None,
    }
}

/// Appends the canonical `GameConfig` encoding: 31 u32 words in
/// struct-declaration order (124 bytes). `map_width_cm`/`map_height_cm`
/// travel as their two's-complement u32 bit patterns.
pub fn push_config(cfg: &GameConfig, out: &mut Vec<u8>) {
    let words: [u32; CONFIG_U32_COUNT] = [
        cfg.total_rounds,
        cfg.initial_spawn_window_ms,
        cfg.decision_window_ms,
        cfg.priority_bucket_ms,
        cfg.epoch_length,
        cfg.map_width_cm as u32,
        cfg.map_height_cm as u32,
        cfg.portal_count,
        cfg.portal_min_distance_cm,
        cfg.clustered_portal_ratio_permille,
        cfg.portal_cluster_count,
        cfg.initial_agents_per_faction,
        cfg.reinforcement_interval_rounds,
        cfg.reinforcement_agents_per_faction,
        cfg.move_budget_cm,
        cfg.portal_operation_range_cm,
        cfg.transfer_range_cm,
        cfg.initial_supply,
        cfg.max_supply,
        cfg.portal_max_hp,
        cfg.attack_damage,
        cfg.recharge_amount,
        cfg.recharge_cost_supply,
        cfg.hack_supply_yield,
        cfg.hack_key_yield_min,
        cfg.hack_key_yield_max,
        cfg.hack_cooldown_rounds,
        cfg.hack_limit_per_epoch,
        cfg.max_link_distance_cm,
        cfg.max_commands_per_agent_round,
        cfg.max_frame_payload_bytes,
    ];
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
}

/// Decodes exactly [`CONFIG_BYTES`] bytes into a `GameConfig`; `None` on a
/// wrong length. Field values are NOT validated here.
pub fn config_from_bytes(bytes: &[u8]) -> Option<GameConfig> {
    if bytes.len() != CONFIG_BYTES {
        return None;
    }
    let mut words = [0u32; CONFIG_U32_COUNT];
    for (i, word) in words.iter_mut().enumerate() {
        let (chunk, rest) = bytes[i * 4..].split_at(4);
        debug_assert_eq!(chunk.len(), 4);
        debug_assert_eq!(rest.len(), CONFIG_BYTES - (i + 1) * 4);
        *word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    let [
        total_rounds,
        initial_spawn_window_ms,
        decision_window_ms,
        priority_bucket_ms,
        epoch_length,
        map_width_cm,
        map_height_cm,
        portal_count,
        portal_min_distance_cm,
        clustered_portal_ratio_permille,
        portal_cluster_count,
        initial_agents_per_faction,
        reinforcement_interval_rounds,
        reinforcement_agents_per_faction,
        move_budget_cm,
        portal_operation_range_cm,
        transfer_range_cm,
        initial_supply,
        max_supply,
        portal_max_hp,
        attack_damage,
        recharge_amount,
        recharge_cost_supply,
        hack_supply_yield,
        hack_key_yield_min,
        hack_key_yield_max,
        hack_cooldown_rounds,
        hack_limit_per_epoch,
        max_link_distance_cm,
        max_commands_per_agent_round,
        max_frame_payload_bytes,
    ] = words;
    Some(GameConfig {
        total_rounds,
        initial_spawn_window_ms,
        decision_window_ms,
        priority_bucket_ms,
        epoch_length,
        map_width_cm: map_width_cm as i32,
        map_height_cm: map_height_cm as i32,
        portal_count,
        portal_min_distance_cm,
        clustered_portal_ratio_permille,
        portal_cluster_count,
        initial_agents_per_faction,
        reinforcement_interval_rounds,
        reinforcement_agents_per_faction,
        move_budget_cm,
        portal_operation_range_cm,
        transfer_range_cm,
        initial_supply,
        max_supply,
        portal_max_hp,
        attack_damage,
        recharge_amount,
        recharge_cost_supply,
        hack_supply_yield,
        hack_key_yield_min,
        hack_key_yield_max,
        hack_cooldown_rounds,
        hack_limit_per_epoch,
        max_link_distance_cm,
        max_commands_per_agent_round,
        max_frame_payload_bytes,
    })
}

/// Appends the canonical portal encoding (13 bytes): owner tag u8, hp u32,
/// x u32, y u32. The dense id is the slice position and is not stored.
pub fn push_portal(portal: &Portal, out: &mut Vec<u8>) {
    out.push(owner_tag(portal.owner));
    out.extend_from_slice(&portal.hp.to_le_bytes());
    out.extend_from_slice(&(portal.position.x as u32).to_le_bytes());
    out.extend_from_slice(&(portal.position.y as u32).to_le_bytes());
}

/// Decodes exactly [`PORTAL_BYTES`] bytes into the portal with dense `id`;
/// `None` on a wrong length or an invalid owner tag.
pub fn portal_from_bytes(id: PortalId, bytes: &[u8]) -> Option<Portal> {
    if bytes.len() != PORTAL_BYTES {
        return None;
    }
    Some(Portal {
        id,
        owner: owner_from_tag(bytes[0])?,
        hp: u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]),
        position: crate::types::Coord::new(
            u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as i32,
            u32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]) as i32,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Checklist-level pins for the shared codec: round-trips, frozen widths,
    // declaration order, two's-complement coordinates, and tag tables.

    #[test]
    fn config_round_trips_default_and_modified() {
        let cfg = GameConfig::default();
        let mut bytes = Vec::new();
        push_config(&cfg, &mut bytes);
        assert_eq!(bytes.len(), 124, "31 u32 words, no padding (D2)");
        assert_eq!(config_from_bytes(&bytes), Some(cfg));

        let mut modified = GameConfig::default();
        modified.total_rounds = 17;
        modified.map_width_cm = -5;
        modified.epoch_length = 7;
        modified.hack_key_yield_max = 9;
        let mut bytes = Vec::new();
        push_config(&modified, &mut bytes);
        assert_eq!(config_from_bytes(&bytes), Some(modified));
    }

    #[test]
    fn config_rejects_wrong_lengths() {
        let mut bytes = Vec::new();
        push_config(&GameConfig::default(), &mut bytes);
        assert_eq!(config_from_bytes(&bytes[..123]), None);
        assert_eq!(config_from_bytes(&[]), None);
        let mut long = bytes.clone();
        long.push(0);
        assert_eq!(config_from_bytes(&long), None);
    }

    #[test]
    fn config_words_follow_declaration_order() {
        let mut bytes = Vec::new();
        push_config(&GameConfig::default(), &mut bytes);
        let word = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(word(0), 500, "total_rounds is word 0");
        assert_eq!(word(4), 100, "epoch_length is word 4");
        assert_eq!(word(5), 600_000, "map_width_cm is word 5");
        assert_eq!(word(30), 65_536, "max_frame_payload_bytes is word 30");
        // Negative dimension: two's-complement u32 bit pattern (D44).
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = -5;
        let mut bytes = Vec::new();
        push_config(&cfg, &mut bytes);
        assert_eq!(
            u32::from_le_bytes(bytes[5 * 4..5 * 4 + 4].try_into().unwrap()),
            0xFFFF_FFFB_u32
        );
    }

    #[test]
    fn portal_round_trips_all_owner_tags() {
        for (index, owner) in [Owner::Neutral, Owner::Enl, Owner::Res].iter().enumerate() {
            let portal = Portal {
                id: PortalId(index as u32),
                position: crate::types::Coord::new(-3, 456_789),
                owner: *owner,
                hp: 100,
            };
            let mut bytes = Vec::new();
            push_portal(&portal, &mut bytes);
            assert_eq!(bytes.len(), 13);
            assert_eq!(bytes[0], owner_tag(*owner));
            assert_eq!(portal_from_bytes(portal.id, &bytes), Some(portal));
        }
        assert_eq!(portal_from_bytes(PortalId(0), &[0; 12]), None);
    }

    #[test]
    fn tag_tables_are_frozen() {
        assert_eq!(faction_tag(Faction::Enl), 0);
        assert_eq!(faction_tag(Faction::Res), 1);
        assert_eq!(faction_from_tag(0), Some(Faction::Enl));
        assert_eq!(faction_from_tag(1), Some(Faction::Res));
        assert_eq!(faction_from_tag(2), None);
        assert_eq!(owner_tag(Owner::Neutral), 0);
        assert_eq!(owner_tag(Owner::Enl), 1);
        assert_eq!(owner_tag(Owner::Res), 2);
        assert_eq!(owner_from_tag(0), Some(Owner::Neutral));
        assert_eq!(owner_from_tag(3), None);
        assert_eq!(owner_from_tag(255), None);
    }
}

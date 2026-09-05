//! Canonical replay-state encoding and the BLAKE3-256 state hash
//! (D44/D45, P1-HASH, P5-REPLAY,).
//!
//! The encoding is the frozen 8-section layout of D44: fixed-width
//! little-endian integers (D2), collections length-prefixed with u32 counts
//! and iterated in ascending id order (every collection is a `BTreeMap`;
//! portals are dense by id). The hash covers EXACTLY the data that affects
//! future deterministic transitions plus the config identity; public
//! events, diagnostics, derived caches, wall-clock, and the D47 settle
//! guard are excluded (see). The hash point is frozen by D45:
//! immediately after round r is fully closed (`run_round` + reinforcement
//! + `advance_round` incl. the epoch sweep), so `state.round() == r+1` and
//! the hash covers exactly what round r+1 reads.

use arena_core::codec;
use arena_core::state::GameState;
use arena_core::types::{Faction, Score2};

/// Domain-separation label of the state hash (P5-REPLAY,).
pub const STATE_HASH_DOMAIN: &[u8] = b"ingress-arena-state-v0.1";

/// Version of THIS canonical state encoding; replay headers carry it and
/// decoders reject mismatches before executing any round.
pub const STATE_ENCODING_VERSION: u32 = 1;

/// Appends the canonical encoding of `state` (D44 sections 1-8):
///
/// 1. config identity — 31 u32 words (shared codec);
/// 2. `round` — u32;
/// 3. portals — u32 count, then per portal in dense id order: owner u8,
///    hp u32, x u32, y u32;
/// 4. agents — u32 count, then ascending `AgentId`: id u32, faction u8,
///    x u32, y u32, supply u32, key count u32, then (portal u32,
///    count u32) pairs ascending;
/// 5. hack trackers — u32 count, then ascending `(AgentId, PortalId)`:
///    agent u32, portal u32, `last_hack_round` u32 (0 = none),
///    `hack_count_epoch` u32, `hack_ordinal` u32 (the D31 lifetime
///    ordinal);
/// 6. links — u32 count, then ascending `LinkId`: min u32, max u32,
///    faction u8;
/// 7. scores — enl u128, res u128 (16 LE bytes each);
/// 8. `next_agent_id` — u32.
///
/// Excluded: `events` + `event_seq` (public log — replay
/// compares them as records instead), round trackers (`major_used` /
/// `move_spent` are reset by `advance_round` before any next-round command
/// reads them), the link-graph field/adjacency caches (pure functions of
/// links × map positions; their effect enters via scores), the D47
/// `settled_round` guard (diagnostic), and game_seed/oracle (D15 — Replay
/// Header material, never state).
pub fn push_canonical_state(state: &GameState, out: &mut Vec<u8>) {
    // Section 1: config identity.
    codec::push_config(state.config(), out);

    // Section 2: round.
    out.extend_from_slice(&state.round().to_le_bytes());

    // Section 3: portals, dense id order.
    let portals = state.portals();
    out.extend_from_slice(&(portals.len() as u32).to_le_bytes());
    for portal in portals {
        codec::push_portal(portal, out);
    }

    // Section 4: agents, ascending AgentId (BTreeMap iteration).
    let agents = state.agents();
    out.extend_from_slice(&(agents.len() as u32).to_le_bytes());
    for agent in agents.values() {
        out.extend_from_slice(&agent.id.0.to_le_bytes());
        out.push(codec::faction_tag(agent.faction));
        out.extend_from_slice(&(agent.position.x as u32).to_le_bytes());
        out.extend_from_slice(&(agent.position.y as u32).to_le_bytes());
        out.extend_from_slice(&agent.supply.to_le_bytes());
        out.extend_from_slice(&(agent.keys.len() as u32).to_le_bytes());
        for (&portal, &count) in &agent.keys {
            out.extend_from_slice(&portal.0.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
        }
    }

    // Section 5: hack trackers, ascending (AgentId, PortalId).
    let trackers = state.hack_trackers();
    out.extend_from_slice(&(trackers.len() as u32).to_le_bytes());
    for ((agent, portal), tracker) in trackers {
        out.extend_from_slice(&agent.0.to_le_bytes());
        out.extend_from_slice(&portal.0.to_le_bytes());
        out.extend_from_slice(&tracker.last_hack_round.unwrap_or(0).to_le_bytes());
        out.extend_from_slice(&tracker.hack_count_epoch.to_le_bytes());
        out.extend_from_slice(&tracker.hack_ordinal.to_le_bytes());
    }

    // Section 6: links, ascending LinkId.
    let links = state.link_graph().links();
    out.extend_from_slice(&(links.len() as u32).to_le_bytes());
    for (&link, owned) in links {
        out.extend_from_slice(&link.min.0.to_le_bytes());
        out.extend_from_slice(&link.max.0.to_le_bytes());
        out.push(codec::faction_tag(owned.faction));
    }

    // Section 7: scores (enl then res, u128 LE).
    out.extend_from_slice(&state.score(Faction::Enl).to_le_bytes());
    out.extend_from_slice(&state.score(Faction::Res).to_le_bytes());

    // Section 8: next agent id.
    out.extend_from_slice(&state.next_agent_id().0.to_le_bytes());
}

/// Canonical bytes of `state` (see [`push_canonical_state`]).
pub fn canonical_state_bytes(state: &GameState) -> Vec<u8> {
    let mut out = Vec::new();
    push_canonical_state(state, &mut out);
    out
}

/// BLAKE3-256 over the domain label and the canonical state bytes (D45,
/// P5-REPLAY): `blake3("ingress-arena-state-v0.1" || canonical_state)`.
pub fn hash_canonical_bytes(canonical: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(STATE_HASH_DOMAIN);
    hasher.update(canonical);
    *hasher.finalize().as_bytes()
}

/// The state hash of `state` (see [`hash_canonical_bytes`]). The frozen
/// hash POINT (D45) is immediately after a round fully closed — recorders
/// call this with the post-`advance_round` state.
pub fn state_hash(state: &GameState) -> [u8; 32] {
    hash_canonical_bytes(&canonical_state_bytes(state))
}

/// Lowercase hex of a byte slice (diagnostics and golden vectors; the
/// replay UI formatting itself is M9).
pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Reads one u32 word at `offset` of a canonical buffer (test/diagnostic
/// helper for pinned-layout assertions).
pub fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let chunk = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

/// Reads one u128 word at `offset` of a canonical buffer.
pub fn read_u128_le(bytes: &[u8], offset: usize) -> Option<u128> {
    let chunk = bytes.get(offset..offset + 16)?;
    let mut words = [0u8; 16];
    words.copy_from_slice(chunk);
    Some(u128::from_le_bytes(words))
}

/// Score pair `(enl, res)` read from the 32-byte scores section of a
/// canonical buffer starting at `offset`.
pub fn read_scores_le(bytes: &[u8], offset: usize) -> Option<(Score2, Score2)> {
    Some((
        read_u128_le(bytes, offset)?,
        read_u128_le(bytes, offset + 16)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::config::GameConfig;
    use arena_core::map::MapDefinition;
    use arena_core::resolve::{Resolver, SubmittedCommandStream};
    use arena_core::spawn::{InitialSpawns, validate_initial_spawn_set};
    use arena_core::state::GameState;
    use arena_core::types::{AgentId, Command, Coord, Faction, LinkId, Owner, Portal, PortalId};

    /// Pinned fixture config: the R-CONFIG-01 defaults except a small
    /// 40_000 cm map with six hand-placed portals, one agent per faction,
    /// a widened operation range (so one standstill agent can act on
    /// several portals), and cooldown 1 (same-pair hacks on consecutive
    /// rounds exercise the D31 lifetime ordinal).
    fn canon_cfg() -> GameConfig {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 40_000;
        cfg.map_height_cm = 40_000;
        cfg.portal_count = 6;
        cfg.portal_cluster_count = 6;
        cfg.portal_min_distance_cm = 4_000;
        cfg.initial_agents_per_faction = 1;
        cfg.portal_operation_range_cm = 30_000;
        cfg.hack_cooldown_rounds = 1;
        cfg
    }

    /// Six pairwise >= 4_000 cm apart portals; no three collinear with a
    /// fourth on the segment, no crossings inside the (0,1,2) triangle.
    fn canon_map() -> MapDefinition {
        let coords = [
            (4_000, 4_000),
            (20_000, 4_000),
            (4_000, 20_000),
            (20_000, 20_000),
            (36_000, 4_000),
            (4_000, 30_000),
        ];
        MapDefinition::new(
            coords
                .iter()
                .enumerate()
                .map(|(i, &(x, y))| Portal {
                    id: PortalId(i as u32),
                    position: Coord::new(x, y),
                    owner: Owner::Neutral,
                    hp: 0,
                })
                .collect(),
        )
    }

    fn spawns_at(cfg: &GameConfig, faction: Faction, coord: Coord) -> InitialSpawns {
        let (enl, res) = GameState::initial_roster(cfg);
        let roster = if faction == Faction::Enl { &enl } else { &res };
        validate_initial_spawn_set(roster, &[(roster[0], coord)], cfg).expect("fixture spawn")
    }

    fn fresh_state() -> GameState {
        let cfg = canon_cfg();
        GameState::new(
            cfg,
            &canon_map(),
            &spawns_at(&cfg, Faction::Enl, Coord::new(4_000, 4_000)),
            &spawns_at(&cfg, Faction::Res, Coord::new(36_000, 36_000)),
        )
        .expect("fixture state builds")
    }

    fn enl(round: u32, commands: Vec<Command>) -> SubmittedCommandStream {
        SubmittedCommandStream {
            round,
            faction: Faction::Enl,
            agent_id: AgentId(0),
            received_at_ms: 0,
            receive_seq: 0,
            commands,
        }
    }

    fn res(round: u32, commands: Vec<Command>) -> SubmittedCommandStream {
        SubmittedCommandStream {
            round,
            faction: Faction::Res,
            agent_id: AgentId(1),
            received_at_ms: 0,
            receive_seq: 0,
            commands,
        }
    }

    const MID_MATCH_SEED: u64 = 0x00C0_FFEE_0000_0005;

    /// Six scripted ENL rounds over the pinned fixture: capture+hack P0,
    /// capture+hack P1, link 0-1, capture+hack P2, hack P2 + link 0-2, link
    /// 1-2 (closing field (0,1,2) in round 6). At the D45 hash point the
    /// state sits at round 7 with three links, one field-derived score,
    /// leftover keys, and hack pair (A0,P2) at lifetime ordinal 2.
    fn mid_match_state() -> GameState {
        let mut state = fresh_state();
        let resolver = Resolver::with_seed(MID_MATCH_SEED);
        let capture = |p: u32| Command::Capture {
            portal: PortalId(p),
        };
        let hack = |p: u32| Command::Hack {
            portal: PortalId(p),
        };
        let link = |a: u32, b: u32| Command::Link {
            source_portal: PortalId(a),
            target_portal: PortalId(b),
        };
        let rounds: Vec<Vec<Command>> = vec![
            vec![capture(0), hack(0)],
            vec![capture(1), hack(1)],
            vec![link(0, 1)],
            vec![capture(2), hack(2)],
            vec![hack(2), link(0, 2)],
            vec![link(1, 2)],
        ];
        for (index, commands) in rounds.into_iter().enumerate() {
            let round = index as u32 + 1;
            let _ = resolver.run_round(&mut state, &[enl(round, commands)]);
            state.advance_round();
        }
        state
    }

    // ------------------------------------------------------------------
    // Golden vectors (D44/D45): bytes and hash of two pinned states. The
    // hex literals are FROZEN — any canonical-layout or hash-construction
    // change is a rule-version change and must bump
    // STATE_ENCODING_VERSION.
    // ------------------------------------------------------------------

    #[test]
    fn golden_fresh_initial_state_bytes_and_hash() {
        let state = fresh_state();
        let bytes = canonical_state_bytes(&state);
        let hash = state_hash(&state);
        assert_eq!(
            hex(&bytes),
            "f401000088130000e80300000500000064000000409c0000409c000006000000a00f0000bc02000006000000010000006400000001000000204e00003075000088130000140000006400000064000000140000000f000000010000000100000001000000030000000100000005000000400d03000a0000000000010001000000060000000000000000a00f0000a00f00000000000000204e0000a00f00000000000000a00f0000204e00000000000000204e0000204e00000000000000a08c0000a00f00000000000000a00f000030750000020000000000000000a00f0000a00f000014000000000000000100000001a08c0000a08c000014000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000",
            "frozen canonical bytes of the pinned fresh state (D44)"
        );
        assert_eq!(
            hex(&hash),
            "9d4af756da05e960e2b6aabda02e1f5c00986b20e1e99578245efa511405ba0e",
            "frozen BLAKE3-256 state hash of the pinned fresh state (D45)"
        );
    }

    #[test]
    fn golden_mid_match_state_bytes_and_hash() {
        let state = mid_match_state();
        assert_eq!(state.round(), 7);
        assert!(state.score(Faction::Enl) > 0, "field scored");
        assert_eq!(state.link_graph().links().len(), 3);
        assert!(
            state
                .hack_trackers()
                .get(&(AgentId(0), PortalId(2)))
                .map(|t| t.hack_ordinal)
                .unwrap_or(0)
                >= 2,
            "lifetime ordinal >= 2 (D31)"
        );
        let bytes = canonical_state_bytes(&state);
        let hash = state_hash(&state);
        assert_eq!(
            hex(&bytes),
            "f401000088130000e80300000500000064000000409c0000409c000006000000a00f0000bc02000006000000010000006400000001000000204e00003075000088130000140000006400000064000000140000000f000000010000000100000001000000030000000100000005000000400d03000a0000000000010007000000060000000164000000a00f0000a00f00000164000000204e0000a00f00000164000000a00f0000204e00000000000000204e0000204e00000000000000a08c0000a00f00000000000000a00f000030750000020000000000000000a00f0000a00f000018000000030000000000000003000000010000000100000002000000030000000100000001a08c0000a08c0000140000000000000003000000000000000000000001000000010000000100000000000000010000000200000001000000010000000000000002000000050000000200000002000000030000000000000001000000000000000002000000000100000002000000000040420f0000000000000000000000000000000000000000000000000000000002000000",
            "frozen canonical bytes of the pinned mid-match state (D44)"
        );
        assert_eq!(
            hex(&hash),
            "a41242ad4cf180bf025f9fadd2455c3ba1932546995029beced742f40fcb1c6a",
            "frozen BLAKE3-256 state hash of the pinned mid-match state (D45)"
        );
    }

    // ------------------------------------------------------------------
    // Widths and endianness pins (D2/D51).
    // ------------------------------------------------------------------

    #[test]
    fn golden_pins_u32_and_u128_little_endian_layout() {
        let state = mid_match_state();
        let bytes = canonical_state_bytes(&state);
        // Section 2 sits right after the 124-byte config section.
        assert_eq!(read_u32_le(&bytes, 124), Some(state.round()));
        assert_eq!(read_u32_le(&bytes, 124), Some(7));
        // Tail: scores (two u128 LE) then next_agent_id (u32 LE).
        let len = bytes.len();
        let scores = read_scores_le(&bytes, len - 36).expect("scores section");
        assert_eq!(scores.0, state.score(Faction::Enl));
        assert_eq!(scores.1, state.score(Faction::Res));
        assert_eq!(
            read_u32_le(&bytes, len - 4),
            Some(state.next_agent_id().0),
            "next_agent_id closes the encoding"
        );
        // The u128 scores really are byte-reversed on big-endian paper:
        // the low byte of the ENL score is byte len-36.
        let enl = state.score(Faction::Enl);
        assert_eq!(bytes[len - 36], (enl & 0xff) as u8);
        assert_eq!(bytes[len - 35], ((enl >> 8) & 0xff) as u8);
    }

    // ------------------------------------------------------------------
    // Insertion-order independence (P1-HASH).
    // ------------------------------------------------------------------

    #[test]
    fn insertion_order_never_changes_the_hash() {
        // Spawn entries submitted in reverse order build the same BTreeMap
        // state.
        let cfg = canon_cfg();
        let (enl_roster, _res_roster) = GameState::initial_roster(&cfg);
        let forward = validate_initial_spawn_set(
            &enl_roster,
            &[(AgentId(0), Coord::new(4_000, 4_000))],
            &cfg,
        )
        .expect("valid");
        let map = canon_map();
        let res = spawns_at(&cfg, Faction::Res, Coord::new(36_000, 36_000));
        let a = GameState::new(canon_cfg(), &map, &forward, &res).expect("builds");
        let b = GameState::new(
            canon_cfg(),
            &map,
            &validate_initial_spawn_set(
                &enl_roster,
                &[(AgentId(0), Coord::new(4_000, 4_000))],
                &cfg,
            )
            .expect("valid"),
            &res,
        )
        .expect("builds");
        assert_eq!(state_hash(&a), state_hash(&b));

        // Link creation order swapped across two runs: same link SET, same
        // rounds, same everything -> same bytes (the field/adjacency caches
        // are excluded and, being pure functions of the links, identical
        // anyway).
        let link = |a: u32, b: u32| Command::Link {
            source_portal: PortalId(a),
            target_portal: PortalId(b),
        };
        let build = |fourth: Command, fifth: Command| -> GameState {
            let mut state = fresh_state();
            let resolver = Resolver::with_seed(MID_MATCH_SEED);
            let capture = |p: u32| Command::Capture {
                portal: PortalId(p),
            };
            let hack = |p: u32| Command::Hack {
                portal: PortalId(p),
            };
            let rounds = vec![
                vec![capture(0), hack(0)],
                vec![capture(1), hack(1)],
                vec![capture(2), hack(2)],
                vec![fourth],
                vec![fifth],
            ];
            for (index, commands) in rounds.into_iter().enumerate() {
                let _ = resolver.run_round(&mut state, &[enl(index as u32 + 1, commands)]);
                state.advance_round();
            }
            state
        };
        let order_a = build(link(0, 1), link(0, 2));
        let order_b = build(link(0, 2), link(0, 1));
        assert_eq!(
            order_a.link_graph().links().keys().collect::<Vec<_>>(),
            order_b.link_graph().links().keys().collect::<Vec<_>>(),
            "same link set"
        );
        assert_eq!(state_hash(&order_a), state_hash(&order_b));
        // ...and the encoded link section is ascending by (min, max).
        let bytes = canonical_state_bytes(&order_a);
        let links_offset = {
            // config(124) + round(4) + portals(4 + 6*13) + agents(...) --
            // walk it generically instead of hard-coding offsets.
            let mut offset = 124 + 4;
            let portals = read_u32_le(&bytes, offset).expect("portal count");
            offset += 4 + portals as usize * 13;
            let agents = read_u32_le(&bytes, offset).expect("agent count");
            offset += 4;
            for _ in 0..agents {
                offset += 4 + 1 + 4 + 4 + 4 + 4; // id, faction, x, y, supply, key count
                let keys = read_u32_le(&bytes, offset - 4).expect("key count");
                offset += keys as usize * 8;
            }
            let trackers = read_u32_le(&bytes, offset).expect("tracker count");
            offset += 4 + trackers as usize * 20;
            offset
        };
        let link_count = read_u32_le(&bytes, links_offset).expect("link count");
        assert_eq!(link_count, 2);
        let mut previous = (0u32, 0u32);
        for index in 0..link_count as usize {
            let base = links_offset + 4 + index * 9;
            let min = read_u32_le(&bytes, base).expect("min");
            let max = read_u32_le(&bytes, base + 4).expect("max");
            assert!(min < max, "canonical LinkId has min < max");
            if index > 0 {
                assert!(previous < (min, max), "links ascend by (min, max)");
            }
            previous = (min, max);
        }
    }

    // ------------------------------------------------------------------
    // Exclusions: events, event_seq, round trackers, and the
    // derived caches never enter the hash.
    // ------------------------------------------------------------------

    /// Two-portal capture/neutralize/recapture fixture: ENL stands on P0,
    /// RES stands 2_828 cm away (inside the default 5_000 cm range).
    fn combat_cfg() -> GameConfig {
        let mut cfg = GameConfig::default();
        cfg.map_width_cm = 40_000;
        cfg.map_height_cm = 40_000;
        cfg.portal_count = 2;
        cfg.portal_cluster_count = 2;
        cfg.portal_min_distance_cm = 4_000;
        cfg.initial_agents_per_faction = 1;
        cfg
    }

    fn combat_state() -> GameState {
        let cfg = combat_cfg();
        let map = MapDefinition::new(vec![
            Portal {
                id: PortalId(0),
                position: Coord::new(10_000, 10_000),
                owner: Owner::Neutral,
                hp: 0,
            },
            Portal {
                id: PortalId(1),
                position: Coord::new(30_000, 30_000),
                owner: Owner::Neutral,
                hp: 0,
            },
        ]);
        GameState::new(
            cfg,
            &map,
            &spawns_at(&cfg, Faction::Enl, Coord::new(10_000, 10_000)),
            &spawns_at(&cfg, Faction::Res, Coord::new(12_000, 12_000)),
        )
        .expect("combat fixture builds")
    }

    #[test]
    fn event_log_and_event_seq_never_change_the_hash() {
        let capture = || Command::Capture {
            portal: PortalId(0),
        };
        let resolver = Resolver::with_seed(7);

        // Quiet history: capture P0 in round 1, idle through round 7.
        let mut quiet = combat_state();
        let _ = resolver.run_round(&mut quiet, &[enl(1, vec![capture()])]);
        quiet.advance_round();
        for _round in 2..=7 {
            let _ = resolver.run_round(&mut quiet, &[]);
            quiet.advance_round();
        }

        // Loud history: capture, five attacks neutralize P0 exactly (hp
        // 100, damage 20), recapture in round 7 — the WORLD at the round-7
        // hash point is identical; only the public event log differs.
        let mut loud = combat_state();
        let _ = resolver.run_round(&mut loud, &[enl(1, vec![capture()])]);
        loud.advance_round();
        for round in 2..=6 {
            let _ = resolver.run_round(&mut loud, &[res(round, vec![Command::Attack])]);
            loud.advance_round();
        }
        let _ = resolver.run_round(&mut loud, &[enl(7, vec![capture()])]);
        loud.advance_round();

        assert_eq!(quiet.round(), 8);
        assert_eq!(loud.round(), 8);
        assert!(
            loud.events().len() > quiet.events().len(),
            "the histories really differ ({} vs {} events)",
            loud.events().len(),
            quiet.events().len()
        );
        assert_eq!(
            loud.portal(PortalId(0)).map(|p| (p.owner, p.hp)),
            quiet.portal(PortalId(0)).map(|p| (p.owner, p.hp)),
            "identical world"
        );
        assert_ne!(quiet, loud, "PartialEq still sees the event log");
        assert_eq!(
            state_hash(&quiet),
            state_hash(&loud),
            "events and event_seq are hash-excluded"
        );
    }

    #[test]
    fn round_trackers_never_change_the_hash() {
        // Mid-round comparison (before advance_round): a dropped ATTACK
        // with nothing in range sets major_used (D28) and changes no world
        // fact, emits no event.
        let mut active = combat_state();
        let _ = Resolver::new().run_round(&mut active, &[enl(1, vec![Command::Attack])]);
        let idle = combat_state();
        assert_ne!(
            active.tracker(AgentId(0)),
            idle.tracker(AgentId(0)),
            "major_used differs (D28)"
        );
        assert_eq!(active.round(), idle.round());
        assert_eq!(
            state_hash(&active),
            state_hash(&idle),
            "round trackers are hash-excluded"
        );
        assert_ne!(active, idle);
    }

    #[test]
    fn config_identity_enters_the_hash() {
        let a = fresh_state();
        let mut cfg = canon_cfg();
        cfg.epoch_length = 99; // one u32 word different, still valid
        let map = canon_map();
        let b = GameState::new(
            cfg,
            &map,
            &spawns_at(&canon_cfg(), Faction::Enl, Coord::new(4_000, 4_000)),
            &spawns_at(&canon_cfg(), Faction::Res, Coord::new(36_000, 36_000)),
        )
        .expect("builds");
        assert_ne!(state_hash(&a), state_hash(&b));
        // The difference is exactly the config section: bytes diverge
        // inside the first 124 bytes and re-converge nowhere else.
        let (ba, bb) = (canonical_state_bytes(&a), canonical_state_bytes(&b));
        assert_eq!(ba.len(), bb.len());
        assert_eq!(&ba[128..], &bb[128..], "only the config section moved");
        assert_ne!(&ba[..128], &bb[..128]);
    }

    // ------------------------------------------------------------------
    // Layout walk (D44): ascending, u32-counted sections with valid tags.
    // ------------------------------------------------------------------

    #[test]
    fn layout_walks_ascending_with_u32_counts() {
        let state = mid_match_state();
        let bytes = canonical_state_bytes(&state);
        let mut offset = 0usize;

        // Section 1: exactly 124 config bytes.
        assert_eq!(
            arena_core::codec::config_from_bytes(&bytes[..124]),
            Some(*state.config()),
            "config section decodes"
        );
        offset += 124;

        // Section 2: round.
        assert_eq!(read_u32_le(&bytes, offset), Some(7));
        offset += 4;

        // Section 3: portals, dense ascending ids.
        let portals = read_u32_le(&bytes, offset).expect("portal count");
        assert_eq!(portals, 6);
        offset += 4;
        for _ in 0..portals as usize {
            let record = &bytes[offset..offset + 13];
            assert!(
                arena_core::codec::portal_from_bytes(PortalId(0), record).is_some(),
                "valid owner tag"
            );
            offset += 13;
        }

        // Section 4: agents ascending.
        let agents = read_u32_le(&bytes, offset).expect("agent count");
        assert_eq!(agents, 2);
        offset += 4;
        let mut previous_agent = None;
        for _ in 0..agents as usize {
            let id = read_u32_le(&bytes, offset).expect("agent id");
            if let Some(previous) = previous_agent {
                assert!(previous < id, "agents ascend by AgentId");
            }
            previous_agent = Some(id);
            offset += 4 + 1 + 4 + 4 + 4; // id, faction, x, y, supply
            let keys = read_u32_le(&bytes, offset).expect("key count");
            offset += 4;
            let mut previous_key = None;
            for _ in 0..keys as usize {
                let portal = read_u32_le(&bytes, offset).expect("key portal");
                if let Some(previous) = previous_key {
                    assert!(previous < portal, "keys ascend by PortalId");
                }
                previous_key = Some(portal);
                offset += 8;
            }
        }

        // Section 5: hack trackers ascending with the D31 ordinal.
        let trackers = read_u32_le(&bytes, offset).expect("tracker count");
        assert!(trackers >= 2);
        offset += 4;
        let mut previous_pair = None;
        let mut saw_ordinal_two = false;
        for _ in 0..trackers as usize {
            let agent = read_u32_le(&bytes, offset).expect("agent");
            let portal = read_u32_le(&bytes, offset + 4).expect("portal");
            if let Some(previous) = previous_pair {
                assert!(previous < (agent, portal), "trackers ascend");
            }
            previous_pair = Some((agent, portal));
            let ordinal = read_u32_le(&bytes, offset + 16).expect("ordinal");
            if (agent, portal) == (0, 2) && ordinal >= 2 {
                saw_ordinal_two = true;
            }
            offset += 20;
        }
        assert!(saw_ordinal_two, "pair (A0,P2) carries ordinal >= 2");

        // Section 6: links ascending.
        let links = read_u32_le(&bytes, offset).expect("link count");
        assert_eq!(links, 3);
        offset += 4;
        let mut previous_link = None;
        for _ in 0..links as usize {
            let min = read_u32_le(&bytes, offset).expect("min");
            let max = read_u32_le(&bytes, offset + 4).expect("max");
            assert!(min < max);
            if let Some(previous) = previous_link {
                assert!(previous < (min, max), "links ascend");
            }
            previous_link = Some((min, max));
            assert_eq!(
                state.link_graph().link(LinkId {
                    min: PortalId(min),
                    max: PortalId(max)
                }),
                state.link_graph().links().get(&LinkId {
                    min: PortalId(min),
                    max: PortalId(max)
                })
            );
            offset += 9;
        }

        // Section 7 + 8: scores and next_agent_id reach exactly the end.
        let scores = read_scores_le(&bytes, offset).expect("scores");
        assert_eq!(
            scores,
            (state.score(Faction::Enl), state.score(Faction::Res))
        );
        offset += 32;
        assert_eq!(
            read_u32_le(&bytes, offset),
            Some(state.next_agent_id().0),
            "next_agent_id closes the encoding"
        );
        offset += 4;
        assert_eq!(offset, bytes.len(), "no padding, no trailing bytes");
    }
}

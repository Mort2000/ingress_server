//! arena-protocol integration tests (M7 checklist 16 + 18): every P3-RPC
//! message encodes, full-decodes, and converts to core types; snapshot
//! marshalling round-trips against arena-core observation fixtures; u128
//! scores are exact; generation-0 public snapshot equals the frozen map;
//! public and private roots are not interchangeable.

use std::collections::BTreeMap;

use arena_core::config::GameConfig;
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::{
    AgentId, Command, Coord, Faction, FieldId, LinkId, Owner, Portal, PortalId,
};

use arena_protocol::decode::{
    END_CLIENT_FORFEIT, END_DRAW, END_ENL_WIN, END_RES_WIN, END_SETUP_ERROR, END_SPAWN_FORFEIT,
    Envelope, HELLO_DUPLICATE_CONNECTION, HELLO_OK, HELLO_VERSION_MISMATCH, MatchInfo,
    SUBMIT_ACCEPTED, SUBMIT_IGNORED_DUPLICATE, SUBMIT_LATE_DROP, SUBMIT_REJECTED_STRUCTURAL,
    SUBMIT_UNLOCATABLE, SpawnEntry, decode_envelope, decode_private, decode_public,
};
use arena_protocol::frame::{FrameReader, MAX_FRAME_PAYLOAD_BYTES, encode_frame};
use arena_protocol::{PRIVATE_SNAPSHOT_VERSION, PROTOCOL_VERSION, PUBLIC_SNAPSHOT_VERSION, build};

fn round_trip(envelope: &Envelope) -> Envelope {
    let bytes = build::envelope_bytes(envelope);
    decode_envelope(&bytes).expect("round trip decodes")
}

fn all_commands() -> Vec<Command> {
    let mut keys = BTreeMap::new();
    keys.insert(PortalId(2), 3u32);
    keys.insert(PortalId(7), 1u32);
    vec![
        Command::Move {
            destination: Coord::new(-0, -0),
        },
        Command::Attack,
        Command::Capture {
            portal: PortalId(4),
        },
        Command::Recharge {
            portal: PortalId(5),
        },
        Command::Hack {
            portal: PortalId(6),
        },
        Command::Link {
            source_portal: PortalId(1),
            target_portal: PortalId(9),
        },
        Command::Transfer {
            target_agent: AgentId(11),
            supply: 12,
            keys,
        },
    ]
}

// Checklist 18: all eleven messages round-trip and convert to core types.
#[test]
fn every_message_round_trips() {
    // CLIENT_HELLO
    let hello = Envelope::ClientHello {
        client_name: "probe-client".into(),
        client_version: "m7.0".into(),
        protocol_version: PROTOCOL_VERSION,
        public_snapshot_version: PUBLIC_SNAPSHOT_VERSION,
        private_snapshot_version: PRIVATE_SNAPSHOT_VERSION,
    };
    assert_eq!(round_trip(&hello), hello);

    // SERVER_HELLO (both statuses and factions)
    for (status, faction) in [
        (HELLO_OK, Faction::Enl),
        (HELLO_OK, Faction::Res),
        (HELLO_VERSION_MISMATCH, Faction::Enl),
        (HELLO_DUPLICATE_CONNECTION, Faction::Res),
    ] {
        let msg = Envelope::ServerHello {
            status,
            assigned_faction: faction,
            match_id: "m-1234".into(),
        };
        assert_eq!(round_trip(&msg), msg);
    }

    // INITIAL_SPAWN_START carries the full MatchInfo.
    let cfg = GameConfig::default();
    let info = MatchInfo::from_core(&cfg);
    let start = Envelope::InitialSpawnStart {
        spawn_deadline_remaining_ms: 4_321,
        info,
    };
    let decoded = round_trip(&start);
    match &decoded {
        Envelope::InitialSpawnStart {
            spawn_deadline_remaining_ms,
            info: got,
        } => {
            assert_eq!(*spawn_deadline_remaining_ms, 4_321);
            assert_eq!(*got, info);
            // to_core inverts every field except the frame ceiling:
            // from_core publishes the ENFORCED legal maximum (D62), one
            // below the config's exclusive ceiling.
            let mut expected = cfg;
            expected.max_frame_payload_bytes = MAX_FRAME_PAYLOAD_BYTES as u32;
            assert_eq!(got.to_core(), expected);
        }
        other => panic!("unexpected {other:?}"),
    }

    // SUBMIT_INITIAL_SPAWNS
    let spawns = Envelope::SubmitInitialSpawns {
        positions: vec![
            SpawnEntry {
                agent_id: AgentId(0),
                position: Coord::new(1, 2),
            },
            SpawnEntry {
                agent_id: AgentId(1),
                position: Coord::new(600_000, 600_000),
            },
        ],
    };
    assert_eq!(round_trip(&spawns), spawns);

    // INITIAL_SPAWN_RESULT
    for status in [0u8, 1, 2] {
        let msg = Envelope::InitialSpawnResult {
            status,
            detail: format!("detail-{status}"),
        };
        assert_eq!(round_trip(&msg), msg);
    }

    // ROUND_START_SIGNAL (empty table)
    assert_eq!(
        round_trip(&Envelope::RoundStartSignal),
        Envelope::RoundStartSignal
    );

    // SUBMIT_COMMAND_STREAM (to_core conversion: arena-core Commands)
    let stream = Envelope::SubmitCommandStream {
        round: 42,
        agent_id: AgentId(7),
        commands: all_commands(),
    };
    let decoded = round_trip(&stream);
    match decoded {
        Envelope::SubmitCommandStream {
            round,
            agent_id,
            commands,
        } => {
            assert_eq!((round, agent_id), (42, AgentId(7)));
            assert_eq!(commands, all_commands(), "commands survive as core types");
        }
        other => panic!("unexpected {other:?}"),
    }

    // SUBMISSION_RESULT (every status code)
    for status in [
        SUBMIT_ACCEPTED,
        SUBMIT_REJECTED_STRUCTURAL,
        SUBMIT_IGNORED_DUPLICATE,
        SUBMIT_UNLOCATABLE,
        SUBMIT_LATE_DROP,
    ] {
        let msg = Envelope::SubmissionResult {
            round: 3,
            agent_id: AgentId(2),
            status,
            detail: "why".into(),
        };
        assert_eq!(round_trip(&msg), msg);
    }

    // COMMIT_ROUND / ROUND_CLOSED
    assert_eq!(
        round_trip(&Envelope::CommitRound { round: 500 }),
        Envelope::CommitRound { round: 500 }
    );
    assert_eq!(
        round_trip(&Envelope::RoundClosed { round: 499 }),
        Envelope::RoundClosed { round: 499 }
    );

    // GAME_END with u128 scores above u64 (hi halves must be exact).
    let game_end = Envelope::GameEnd {
        outcome: END_DRAW,
        detail: String::new(),
        score_enl: (u128::from(u64::MAX) << 64) | 0x0123_4567_89ab_cdef,
        score_res: 42,
        rounds_completed: 500,
    };
    assert_eq!(round_trip(&game_end), game_end);
    // Technical outcome codes survive untouched.
    for outcome in [
        END_ENL_WIN,
        END_RES_WIN,
        END_DRAW,
        END_SPAWN_FORFEIT,
        END_CLIENT_FORFEIT,
        END_SETUP_ERROR,
    ] {
        let msg = Envelope::GameEnd {
            outcome,
            detail: "x".into(),
            score_enl: 0,
            score_res: 0,
            rounds_completed: 0,
        };
        match round_trip(&msg) {
            Envelope::GameEnd { outcome: got, .. } => assert_eq!(got, outcome),
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn unknown_union_tag_and_wrong_root_rejected() {
    // A PublicSnapshot buffer is not a ProtocolEnvelope root.
    let snap = build::map_generation_bytes(&MapDefinition::new(vec![]), 0, 0);
    assert!(decode_envelope(&snap).is_err());
    // Neither is a PrivateSnapshot.
    let privy = build::private_roster_bytes(&GameConfig::default(), Faction::Enl);
    assert!(decode_envelope(&privy).is_err());
    // And an envelope is not a snapshot root.
    let hello = Envelope::ClientHello {
        client_name: "x".into(),
        client_version: "y".into(),
        protocol_version: 1,
        public_snapshot_version: 1,
        private_snapshot_version: 1,
    };
    let bytes = build::envelope_bytes(&hello);
    assert!(decode_public(&bytes).is_err());
    assert!(decode_private(&bytes).is_err());
}

// ---------------------------------------------------------------------------
// Snapshot marshalling (checklist 16)
// ---------------------------------------------------------------------------

fn observed_state() -> GameState {
    // Small triangle world driven through the real Resolver (state
    // mutators are pub(crate); the protocol crate must not need them).
    let mut cfg = GameConfig::default();
    cfg.map_width_cm = 40_000;
    cfg.map_height_cm = 40_000;
    cfg.portal_min_distance_cm = 4_000;
    cfg.portal_count = 3;
    cfg.portal_cluster_count = 3;
    cfg.initial_agents_per_faction = 2;
    let map = MapDefinition::new(vec![
        Portal {
            id: PortalId(0),
            position: Coord::new(4_000, 4_000),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(1),
            position: Coord::new(12_000, 4_000),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(2),
            position: Coord::new(8_000, 10_000),
            owner: Owner::Neutral,
            hp: 0,
        },
    ]);
    // ENL agent 0 stands within operation range of all three portals;
    // everyone else idles.
    let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
    let enl_entries = vec![
        (AgentId(0), Coord::new(8_000, 6_000)),
        (AgentId(1), Coord::new(0, 0)),
    ];
    let res_entries = vec![
        (AgentId(2), Coord::new(30_000, 30_000)),
        (AgentId(3), Coord::new(30_000, 30_000)),
    ];
    let enl = validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg).expect("spawns");
    let res = validate_initial_spawn_set(&res_roster, &res_entries, &cfg).expect("spawns");
    let mut state = GameState::new(cfg, &map, &enl, &res).expect("state");
    let resolver = arena_core::resolve::Resolver::with_seed(0);
    use arena_core::resolve::SubmittedCommandStream;
    let submit = |state: &mut GameState, round: u32, commands: Vec<Command>| {
        let _ = resolver.run_round(
            state,
            &[SubmittedCommandStream {
                round,
                faction: Faction::Enl,
                agent_id: AgentId(0),
                received_at_ms: 0,
                receive_seq: 0,
                commands,
            }],
        );
        state.advance_round();
    };
    let p = |id: u32| PortalId(id);
    // Rounds 1-3: capture the triangle; round 4: hack one key of each
    // portal; round 5: close the triangle and score the field.
    submit(&mut state, 1, vec![Command::Capture { portal: p(0) }]);
    submit(&mut state, 2, vec![Command::Capture { portal: p(1) }]);
    submit(&mut state, 3, vec![Command::Capture { portal: p(2) }]);
    submit(
        &mut state,
        4,
        vec![
            Command::Hack { portal: p(0) },
            Command::Hack { portal: p(1) },
            Command::Hack { portal: p(2) },
        ],
    );
    submit(
        &mut state,
        5,
        vec![
            Command::Link {
                source_portal: p(0),
                target_portal: p(1),
            },
            Command::Link {
                source_portal: p(1),
                target_portal: p(2),
            },
            Command::Link {
                source_portal: p(2),
                target_portal: p(0),
            },
        ],
    );
    state
}

#[test]
fn public_snapshot_round_trips_against_observation() {
    let state = observed_state();
    let obs = Observation::for_faction(&state, Faction::Enl);
    let bytes = build::public_snapshot_bytes(&obs, 7, 1_000, 2_000);
    let snap = decode_public(&bytes).expect("decodes");
    assert_eq!(snap.schema_version, PUBLIC_SNAPSHOT_VERSION);
    assert_eq!(snap.current_round, obs.round);
    assert_eq!(snap.snapshot_version, 7);
    assert_eq!(snap.round_start_ms, 1_000);
    assert_eq!(snap.deadline_ms, 2_000);
    assert_eq!(snap.portals, obs.portals);
    assert_eq!(snap.links, obs.links);
    assert_eq!(snap.fields, obs.fields);
    assert_eq!(snap.events, obs.events);
    assert_eq!(
        snap.score_enl,
        *obs.scores.get(&Faction::Enl).unwrap(),
        "u128 score exact"
    );
    assert!(snap.score_enl > 0);
    assert_eq!(snap.score_res, *obs.scores.get(&Faction::Res).unwrap());
    // Identical public bytes for the other faction's observation.
    let res_obs = Observation::for_faction(&state, Faction::Res);
    assert_eq!(snap.portals, res_obs.portals);
    assert_eq!(snap.links, res_obs.links);
    assert_eq!(snap.fields, res_obs.fields);
    assert_eq!(snap.events, res_obs.events);
}

#[test]
fn private_snapshot_round_trips_own_agents_only() {
    let state = observed_state();
    let obs = Observation::for_faction(&state, Faction::Res);
    let bytes = build::private_snapshot_bytes(&obs, 9);
    let snap = decode_private(&bytes).expect("decodes");
    assert_eq!(snap.schema_version, PRIVATE_SNAPSHOT_VERSION);
    assert_eq!(snap.snapshot_version, 9);
    assert_eq!(snap.current_round, obs.round);
    assert_eq!(snap.agents, obs.own_agents);
    // No ENL agent (roster 0..2) appears in the RES private snapshot.
    assert!(snap.agents.iter().all(|a| a.id.0 >= 2));

    // Cross-partition: a public buffer is not a private root and vice versa.
    let public_bytes = build::public_snapshot_bytes(&obs, 7, 0, 0);
    assert!(decode_private(&public_bytes).is_err());
    assert!(decode_public(&bytes).is_err());
}

#[test]
fn generation_zero_public_snapshot_equals_frozen_map() {
    let state = observed_state();
    // The map is recoverable from the state's portals; use them as the
    // frozen definition.
    let map = MapDefinition::new(state.portals().to_vec());
    let bytes = build::map_generation_bytes(&map, 111, 222);
    let snap = decode_public(&bytes).expect("decodes");
    assert_eq!(snap.current_round, 0);
    assert_eq!(snap.snapshot_version, 0, "generation 0");
    assert_eq!(snap.round_start_ms, 111);
    assert_eq!(snap.deadline_ms, 222);
    assert_eq!(snap.score_enl, 0);
    assert_eq!(snap.score_res, 0);
    assert!(snap.links.is_empty());
    assert!(snap.fields.is_empty());
    assert!(snap.events.is_empty());
    assert_eq!(snap.portals.len(), map.len());
    for (entry, portal) in snap.portals.iter().zip(map.portals()) {
        assert_eq!(entry.id, portal.id);
        assert_eq!(entry.position, portal.position);
        assert_eq!(entry.owner, portal.owner);
        assert_eq!(entry.hp, portal.hp);
    }
}

#[test]
fn generation_zero_private_snapshot_is_own_roster() {
    let cfg = GameConfig::default();
    let bytes = build::private_roster_bytes(&cfg, Faction::Res);
    let snap = decode_private(&bytes).expect("decodes");
    assert_eq!(snap.snapshot_version, 0);
    assert_eq!(snap.current_round, 0);
    // D17 roster: RES occupies n..2n-1.
    let ids: Vec<u32> = snap.agents.iter().map(|a| a.id.0).collect();
    assert_eq!(
        ids,
        (cfg.initial_agents_per_faction..2 * cfg.initial_agents_per_faction).collect::<Vec<_>>()
    );
    for agent in &snap.agents {
        assert_eq!(agent.supply, cfg.initial_supply);
        assert_eq!(agent.position, Coord::new(0, 0), "documented sentinel");
        assert!(agent.keys.is_empty());
    }
    // ENL roster is the low block.
    let enl = decode_private(&build::private_roster_bytes(&cfg, Faction::Enl)).unwrap();
    let enl_ids: Vec<u32> = enl.agents.iter().map(|a| a.id.0).collect();
    assert_eq!(
        enl_ids,
        (0..cfg.initial_agents_per_faction).collect::<Vec<_>>()
    );
}

#[test]
fn events_of_every_kind_round_trip() {
    use arena_core::event::{Event, EventKind};
    let events = vec![
        Event {
            seq: 0,
            round: 1,
            kind: EventKind::PortalCaptured {
                faction: Faction::Enl,
                agent_id: AgentId(0),
                portal: PortalId(3),
            },
        },
        Event {
            seq: 1,
            round: 1,
            kind: EventKind::PortalAttacked {
                faction: Faction::Res,
                agent_id: AgentId(2),
                portal: PortalId(0),
                damage: 20,
                hp_after: 35,
            },
        },
        Event {
            seq: 2,
            round: 2,
            kind: EventKind::PortalNeutralized {
                faction: Faction::Res,
                agent_id: AgentId(2),
                portal: PortalId(0),
            },
        },
        Event {
            seq: 3,
            round: 2,
            kind: EventKind::LinkCreated {
                faction: Faction::Enl,
                agent_id: AgentId(1),
                link: LinkId::new(PortalId(0), PortalId(1)).unwrap(),
            },
        },
        Event {
            seq: 4,
            round: 3,
            kind: EventKind::LinkDestroyed {
                faction: Faction::Res,
                agent_id: AgentId(3),
                link: LinkId::new(PortalId(2), PortalId(3)).unwrap(),
            },
        },
        Event {
            seq: 5,
            round: 3,
            kind: EventKind::FieldCreated {
                faction: Faction::Enl,
                agent_id: AgentId(1),
                field: FieldId::new(PortalId(0), PortalId(1), PortalId(2)).unwrap(),
            },
        },
        Event {
            seq: 6,
            round: 4,
            kind: EventKind::FieldDestroyed {
                faction: Faction::Res,
                agent_id: AgentId(3),
                field: FieldId::new(PortalId(2), PortalId(3), PortalId(4)).unwrap(),
            },
        },
        Event {
            seq: 7,
            round: 100,
            kind: EventKind::Reinforcement {
                index: 1,
                position: Coord::new(123_456, 654_321),
                count_per_faction: 2,
                first_agent_id: AgentId(6),
            },
        },
    ];
    let state = observed_state();
    let mut obs = Observation::for_faction(&state, Faction::Enl);
    obs.events = events.clone();
    let bytes = build::public_snapshot_bytes(&obs, 1, 0, 0);
    let snap = decode_public(&bytes).expect("decodes");
    assert_eq!(snap.events, events);
}

// A ClientHello whose padded name grows the frame to the exact legal
// maximum (65,535 payload bytes) — binary-searched here, reused by the
// server boundary test as the construction recipe (checklist 3).
#[test]
fn exact_legal_maximum_client_hello_frame() {
    let mut lo = 1usize;
    let mut hi = MAX_FRAME_PAYLOAD_BYTES;
    let hello_for = |name_len: usize| -> Vec<u8> {
        let payload = build::envelope_bytes(&Envelope::ClientHello {
            client_name: "P".repeat(name_len),
            client_version: "1".into(),
            protocol_version: PROTOCOL_VERSION,
            public_snapshot_version: PUBLIC_SNAPSHOT_VERSION,
            private_snapshot_version: PRIVATE_SNAPSHOT_VERSION,
        });
        let mut frame = Vec::new();
        encode_frame(&payload, &mut frame).expect("in limit during search");
        frame
    };
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let payload = build::envelope_bytes(&Envelope::ClientHello {
            client_name: "P".repeat(mid),
            client_version: "1".into(),
            protocol_version: PROTOCOL_VERSION,
            public_snapshot_version: PUBLIC_SNAPSHOT_VERSION,
            private_snapshot_version: PRIVATE_SNAPSHOT_VERSION,
        });
        if payload.len() <= MAX_FRAME_PAYLOAD_BYTES {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let frame = hello_for(lo);
    // The frame total (prefix + payload) sits at the exact boundary.
    assert_eq!(frame.len(), 4 + MAX_FRAME_PAYLOAD_BYTES);

    // The reader accepts it and the envelope decodes.
    let mut reader = FrameReader::new();
    reader.push_bytes(&frame);
    let payload = match reader.next_frame() {
        arena_protocol::FramePoll::Frame(payload) => payload,
        other => panic!("exact legal maximum must pass, got {other:?}"),
    };
    assert_eq!(payload.len(), MAX_FRAME_PAYLOAD_BYTES);
    let decoded = decode_envelope(&payload).expect("decodes");
    match decoded {
        Envelope::ClientHello { client_name, .. } => assert_eq!(client_name.len(), lo),
        other => panic!("unexpected {other:?}"),
    }
}

// D62 / ADR-0008: the advertised frame limit is the ENFORCED legal
// maximum. MatchInfo echoes the transport constant, never the config's
// exclusive ceiling — a client sizing its largest frame to exactly the
// published value must be legal (a 65,536-byte payload would already be
// over-limit and hard-close the connection).
#[test]
fn match_info_publishes_the_enforced_legal_maximum() {
    let cfg = GameConfig::default();
    assert_eq!(cfg.max_frame_payload_bytes, 65_536, "the config ceiling");
    assert_eq!(MAX_FRAME_PAYLOAD_BYTES, 65_535, "the enforced legal max");
    let info = MatchInfo::from_core(&cfg);
    assert_eq!(info.max_frame_payload_bytes, MAX_FRAME_PAYLOAD_BYTES as u32);
    // ...and the value survives the wire exactly.
    let decoded = round_trip(&Envelope::InitialSpawnStart {
        spawn_deadline_remaining_ms: 5_000,
        info,
    });
    match decoded {
        Envelope::InitialSpawnStart { info, .. } => {
            assert_eq!(info.max_frame_payload_bytes, 65_535);
        }
        other => panic!("unexpected {other:?}"),
    }
}

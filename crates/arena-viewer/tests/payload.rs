//! Tactical Replay data consistency and browser fixtures.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use arena_core::config::GameConfig;
use arena_core::harness::{AgentSubmission, MatchHarness, ScriptedStrategy};
use arena_core::map::MapDefinition;
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::{Coord, Faction, Owner, Portal, PortalId, Score2};
use arena_replay::canon;
use arena_replay::recorder::ReplayWriter;
use arena_replay::replay::decode_log;
use arena_viewer::data;
use arena_viewer::json::{self, Json};

const GAME_SEED: u64 = 0x006d_3a00_0000_0033;

fn fixture_config() -> GameConfig {
    GameConfig {
        total_rounds: 150,
        reinforcement_interval_rounds: 75,
        // Every scripted destination is reachable, including reinforcement spawns.
        move_budget_cm: 60_000,
        map_width_cm: 40_000,
        map_height_cm: 40_000,
        portal_count: 6,
        portal_cluster_count: 6,
        portal_min_distance_cm: 4_000,
        ..GameConfig::default()
    }
}

fn fixture_map() -> MapDefinition {
    let positions = [
        (4_000, 4_000),
        (16_000, 4_000),
        (4_000, 16_000),
        (36_000, 4_000),
        (36_000, 16_000),
        (24_000, 16_000),
    ];
    MapDefinition::new(
        positions
            .into_iter()
            .enumerate()
            .map(|(id, (x, y))| Portal {
                id: PortalId(id as u32),
                position: Coord::new(x, y),
                owner: Owner::Neutral,
                hp: 0,
            })
            .collect(),
    )
}

fn fixture_spawns(faction: Faction) -> arena_core::spawn::InitialSpawns {
    let cfg = fixture_config();
    let (enl, res) = GameState::initial_roster(&cfg);
    let roster = if faction == Faction::Enl { enl } else { res };
    let offset = if faction == Faction::Enl { 0 } else { 3 };
    let map = fixture_map();
    let entries: Vec<_> = roster
        .iter()
        .enumerate()
        .map(|(i, &agent)| {
            (
                agent,
                map.portal(PortalId(offset + i as u32)).unwrap().position,
            )
        })
        .collect();
    validate_initial_spawn_set(&roster, &entries, &cfg).unwrap()
}

fn fixture_script(faction: Faction) -> ScriptedStrategy {
    use arena_core::types::{AgentId, Command};
    let (enl, res) = GameState::initial_roster(&fixture_config());
    let roster = if faction == Faction::Enl { enl } else { res };
    let offset = if faction == Faction::Enl { 0 } else { 3 };
    let map = fixture_map();
    let mut rounds = vec![vec![]; 150];
    for (i, agent) in roster.into_iter().enumerate() {
        let target = PortalId(offset + i as u32);
        let source = PortalId(offset + ((i + 2) % 3) as u32);
        rounds[0].push(AgentSubmission::new(
            agent,
            vec![Command::Capture { portal: target }],
        ));
        rounds[1].push(AgentSubmission::new(
            agent,
            vec![Command::Hack { portal: target }],
        ));
        rounds[2].push(AgentSubmission::new(
            agent,
            vec![
                Command::Move {
                    destination: map.portal(source).unwrap().position,
                },
                Command::Link {
                    source_portal: source,
                    target_portal: target,
                },
            ],
        ));
    }
    // Keep telemetry non-vacuous with deterministic patrols and inventory changes.
    // Both factions stay on their disjoint triangles, including reinforcements,
    // so a privacy test never mistakes identical friendly/enemy row values for a leak.
    for round in 4..=150 {
        for i in 0..3 {
            let agent = AgentId(offset + i);
            let portal = PortalId(offset + (i + round) % 3);
            let mut commands = vec![Command::Move {
                destination: map.portal(portal).unwrap().position,
            }];
            if round == 4 || round == 101 {
                commands.push(Command::Hack { portal });
            }
            rounds[round as usize - 1].push(AgentSubmission::new(agent, commands));
        }
        for epoch in 1..=round / 75 {
            let agent = AgentId(6 + (epoch - 1) * 2 + if faction == Faction::Enl { 0 } else { 1 });
            let portal = PortalId(offset + round % 3);
            rounds[round as usize - 1].push(AgentSubmission::new(
                agent,
                vec![Command::Move {
                    destination: map.portal(portal).unwrap().position,
                }],
            ));
        }
    }
    ScriptedStrategy::new(rounds)
}

fn play_rounds(rounds: u32) -> (Vec<u8>, Vec<(Score2, Score2)>) {
    let cfg = fixture_config();
    let map = fixture_map();
    let enl_spawns = fixture_spawns(Faction::Enl);
    let res_spawns = fixture_spawns(Faction::Res);
    let mut writer = ReplayWriter::new(GAME_SEED, &cfg, &map, &enl_spawns, &res_spawns);
    let mut harness = MatchHarness::new(
        GAME_SEED,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(fixture_script(Faction::Enl)),
        Box::new(fixture_script(Faction::Res)),
    )
    .expect("harness");
    let mut deltas = Vec::new();
    for _ in 0..rounds {
        let report = harness.run_round();
        writer
            .record_round(&report, harness.state())
            .expect("record");
        deltas.push((
            report.score2_delta.get(&Faction::Enl).copied().unwrap_or(0),
            report.score2_delta.get(&Faction::Res).copied().unwrap_or(0),
        ));
    }
    let state = harness.state();
    // Non-vacuity pins for the fixture itself.
    assert!(
        state.link_graph().fields().len() >= 1,
        "fixture must produce fields"
    );
    assert!(deltas.iter().any(|(enl, _)| *enl > 0), "fixture must score");
    (writer.finish(state).expect("finish"), deltas)
}

fn normal_iarp() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| play_rounds(fixture_config().total_rounds).0)
}

fn abnormal_iarp() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let cfg = fixture_config();
        let map = fixture_map();
        let enl_spawns = fixture_spawns(Faction::Enl);
        let res_spawns = fixture_spawns(Faction::Res);
        let mut writer = ReplayWriter::new(GAME_SEED, &cfg, &map, &enl_spawns, &res_spawns);
        let mut harness = MatchHarness::new(
            GAME_SEED,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(fixture_script(Faction::Enl)),
            Box::new(fixture_script(Faction::Res)),
        )
        .expect("harness");
        for _ in 0..37 {
            let report = harness.run_round();
            writer
                .record_round(&report, harness.state())
                .expect("record");
        }
        writer
            .finish_abnormal(
                harness.state(),
                arena_replay::format::OUTCOME_CLIENT_FORFEIT,
            )
            .expect("abnormal finish")
    })
}

fn assert_score_accumulation_exact(payload: &Json) {
    let rounds = payload
        .get("rounds")
        .and_then(Json::as_array)
        .expect("rounds");
    let mut fold = (0u128, 0u128);
    for (index, round) in rounds.iter().enumerate() {
        let delta = round
            .get("delta2")
            .and_then(Json::as_array)
            .expect("delta2");
        let score = round
            .get("score2")
            .and_then(Json::as_array)
            .expect("score2");
        fold.0 += delta[0]
            .as_str()
            .expect("delta is a decimal string")
            .parse::<u128>()
            .expect("u128");
        fold.1 += delta[1]
            .as_str()
            .expect("delta is a decimal string")
            .parse::<u128>()
            .expect("u128");
        assert_eq!(
            score[0].as_str(),
            Some(fold.0.to_string().as_str()),
            "round {} enl cumulative",
            index + 1
        );
        assert_eq!(
            score[1].as_str(),
            Some(fold.1.to_string().as_str()),
            "round {} res cumulative",
            index + 1
        );
        // D91: scores are decimal STRINGS, never JSON numbers.
        assert!(score[0].as_str().is_some() && score[1].as_str().is_some());
        assert!(delta[0].as_str().is_some() && delta[1].as_str().is_some());
    }
    let outcome = payload.get("header").unwrap().get("outcome").unwrap();
    let final_score = outcome
        .get("final_score2")
        .and_then(Json::as_array)
        .unwrap();
    assert_eq!(final_score[0].as_str(), Some(fold.0.to_string().as_str()));
    assert_eq!(final_score[1].as_str(), Some(fold.1.to_string().as_str()));
}

#[test]
fn embedded_score2_accumulation_exact() {
    let normal = data::build_payload(normal_iarp()).unwrap();
    assert_score_accumulation_exact(&normal.clone());

    let abnormal = data::build_payload(abnormal_iarp()).unwrap();
    assert_score_accumulation_exact(&abnormal.clone());

    // Cross-check against the IARP's own RoundRecord deltas: the embedded
    // cumulative values are exactly the record accumulation, and every
    // per-round hash is the record's verified D45 hash.
    for (payload, source) in [(normal, normal_iarp()), (abnormal, abnormal_iarp())] {
        let log = decode_log(source).expect("decode");
        let rounds = payload.get("rounds").unwrap().as_array().unwrap();
        let mut fold = (0u128, 0u128);
        for (record, round) in log.round_records().zip(rounds) {
            fold.0 += record.score2_delta.0;
            fold.1 += record.score2_delta.1;
            let score = round.get("score2").unwrap().as_array().unwrap();
            assert_eq!(score[0].as_str(), Some(fold.0.to_string().as_str()));
            assert_eq!(score[1].as_str(), Some(fold.1.to_string().as_str()));
            let delta = round.get("delta2").unwrap().as_array().unwrap();
            assert_eq!(
                delta[0].as_str(),
                Some(record.score2_delta.0.to_string().as_str())
            );
            assert_eq!(
                delta[1].as_str(),
                Some(record.score2_delta.1.to_string().as_str())
            );
            assert_eq!(
                round.get("hash").unwrap().as_str(),
                Some(canon::hex(&record.state_hash).as_str())
            );
        }
    }
    // No floats anywhere in the payloads: the Json type cannot represent
    // them and the parser rejects float syntax — every payload above
    // parsed through that gate.
}

#[test]
fn round_count_matches() {
    // Normal: rounds.len == rounds_completed == record count; total_rounds
    // echoes the config.
    let payload = data::build_payload(normal_iarp()).unwrap();
    let header = payload.get("header").unwrap();
    let rounds = payload.get("rounds").unwrap().as_array().unwrap();
    let log = decode_log(normal_iarp()).unwrap();
    assert_eq!(
        rounds.len() as i64,
        header.get("rounds_completed").unwrap().as_int().unwrap()
    );
    assert_eq!(rounds.len(), log.round_records().count());
    assert_eq!(
        header.get("total_rounds").unwrap().as_int().unwrap(),
        log.header.total_rounds as i64
    );
    assert_eq!(
        header.get("total_rounds").unwrap().as_int().unwrap(),
        log.header.config.total_rounds as i64,
        "total_rounds echoes the config"
    );
    // rounds[i].r == i+1 (viewer jump indexing).
    for (index, round) in rounds.iter().enumerate() {
        assert_eq!(round.get("r").unwrap().as_int().unwrap(), index as i64 + 1);
    }
    // Checkpoints are display ticks at the IARP boundary rounds.
    let checkpoints: Vec<i64> = header
        .get("checkpoints")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Json::as_int)
        .collect();
    assert_eq!(checkpoints, vec![50, 100, 150]);

    // Abnormal: last_round + outcome_tag preserved, rounds truncated at
    // the last COMPLETED round.
    let payload = data::build_payload(abnormal_iarp()).unwrap();
    let header = payload.get("header").unwrap();
    let outcome = header.get("outcome").unwrap();
    assert_eq!(outcome.get("kind").and_then(Json::as_str), Some("abnormal"));
    assert_eq!(
        outcome.get("outcome_tag").and_then(Json::as_int),
        Some(arena_replay::format::OUTCOME_CLIENT_FORFEIT as i64)
    );
    assert!(
        arena_replay::format::OUTCOME_CLIENT_FORFEIT >= 3,
        "tag >= 3 (D46)"
    );
    assert_eq!(outcome.get("last_round").and_then(Json::as_int), Some(37));
    let rounds = payload.get("rounds").unwrap().as_array().unwrap();
    assert_eq!(rounds.len(), 37);
    assert_eq!(
        header.get("rounds_completed").unwrap().as_int(),
        Some(37),
        "rounds_completed == last completed round"
    );
    assert_eq!(
        header.get("total_rounds").unwrap().as_int(),
        Some(150),
        "total_rounds echoes the config"
    );
}

#[test]
fn unverified_input_is_rejected() {
    let log = decode_log(normal_iarp()).expect("decode");
    let mut bytes = log.encode();
    // Flip one byte of round 61's recorded state hash (61 is NOT a
    // checkpoint round, so decode's structural checks pass and it is the
    // replay verification that catches the corruption).
    let record = log
        .round_records()
        .nth(60)
        .expect("round 61 record")
        .clone();
    let offset = bytes
        .windows(32)
        .position(|window| window == record.state_hash)
        .expect("hash bytes located");
    bytes[offset] ^= 1;
    assert_eq!(
        decode_log(&bytes).err(),
        None,
        "the tampered file is still structurally decodable"
    );

    let error = data::build_payload(&bytes).expect_err("must refuse");
    assert!(error.contains("verification failed"), "got: {error}");

    // A structurally broken file also refuses.
    let error = data::build_payload(b"not an iarp").expect_err("refuse");
    assert!(error.contains("decode failed"));

    // The healthy archive passes the same pipeline (the materialization
    // tripwires — per-round hash, public equality, fold, final hash —
    // run inside every successful materialization).
    assert!(data::build_payload(normal_iarp()).is_ok());
}

#[test]
fn abnormal_fixture_payload() {
    let payload = data::build_payload(abnormal_iarp()).unwrap();
    assert_score_accumulation_exact(&payload);
    let outcome = payload.get("header").unwrap().get("outcome").unwrap();
    assert_eq!(outcome.get("kind").and_then(Json::as_str), Some("abnormal"));
    // Viewer banner data: tag + last round + last hash present; the last
    // completed round's hash equals the outcome hash.
    let last_hash = outcome.get("last_hash").and_then(Json::as_str).unwrap();
    let rounds = payload.get("rounds").unwrap().as_array().unwrap();
    let last_round_hash = rounds[36].get("hash").and_then(Json::as_str).unwrap();
    assert_eq!(last_hash, last_round_hash);
}

#[test]
fn reinforcement_rounds_present_in_embedded_data() {
    let payload = data::build_payload(normal_iarp()).unwrap();

    // The IARP carried the fixture's single reinforcement boundary (round
    // 75; round 150 is NOT < total, so it does not fire).
    let reinforcement_rounds: Vec<i64> = payload
        .get("rounds")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|round| {
            round
                .get("events")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event.get("k").and_then(Json::as_int) == Some(8))
        })
        .map(|round| round.get("r").unwrap().as_int().unwrap())
        .collect();
    assert_eq!(reinforcement_rounds, vec![75]);

    // Both factions' NEW agents exist in the embedded per-round private
    // data from the boundary round on: the fixture starts with 3 agents
    // per faction (ENL 0-2, RES 3-5), so the reinforcement adds one id
    // >= 6 per faction. Round 75 close (rounds[74]) shows 4 agents each.
    for key in ["enl", "res"] {
        let agents = payload
            .get("agents")
            .unwrap()
            .get(key)
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(agents.len(), 150, "one agent row per round for {key}");
        let at_boundary: Vec<i64> = agents[74]
            .as_array()
            .unwrap()
            .iter()
            .map(|agent| agent.as_array().unwrap()[0].as_int().unwrap())
            .collect();
        assert_eq!(at_boundary.len(), 4, "{key} roster grew");
        assert!(
            at_boundary.iter().any(|id| *id >= 6),
            "{key} reinforcement agent present: {at_boundary:?}"
        );
        // And it stays for the rest of the match.
        let last: Vec<i64> = agents[149]
            .as_array()
            .unwrap()
            .iter()
            .map(|agent| agent.as_array().unwrap()[0].as_int().unwrap())
            .collect();
        assert!(last.iter().any(|id| *id >= 6));
        // Before the boundary the roster was exactly the initial three.
        let before: Vec<i64> = agents[73]
            .as_array()
            .unwrap()
            .iter()
            .map(|agent| agent.as_array().unwrap()[0].as_int().unwrap())
            .collect();
        assert_eq!(before.len(), 3);
    }

    // The reinforcement event itself carries the D51 shape (both-faction:
    // index, position, count, first agent — no f/a fields).
    let event = payload.get("rounds").unwrap().as_array().unwrap()[74]
        .get("events")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event.get("k").and_then(Json::as_int) == Some(8))
        .expect("round 75 carries the reinforcement event");
    assert_eq!(event.get("i").and_then(Json::as_int), Some(1));
    assert_eq!(event.get("n").and_then(Json::as_int), Some(1));
    assert_eq!(event.get("fa").and_then(Json::as_int), Some(6));
    assert!(event.get("f").is_none() && event.get("a").is_none());
}

#[test]
fn hud_telemetry_and_browser_fixtures() {
    use arena_core::observe::Observation;
    use arena_core::resolve::{CommandEffect, CommandOutcome, Resolver};
    let bytes = normal_iarp();
    let log = decode_log(bytes).unwrap();
    let h = &log.header;
    let (enl, res) = GameState::initial_roster(&h.config);
    let enl = validate_initial_spawn_set(&enl, &h.enl_spawns, &h.config).unwrap();
    let res = validate_initial_spawn_set(&res, &h.res_spawns, &h.config).unwrap();
    let mut state = GameState::new(h.config, &h.map, &enl, &res).unwrap();
    let resolver = Resolver::with_seed(h.game_seed);
    let payload = data::build_payload(bytes).unwrap();
    let mut key_entries = 0;
    let mut moves = 0;
    for (i, record) in log.round_records().enumerate() {
        let report = resolver.run_round(&mut state, &record.submissions);
        state.advance_round();
        for (faction, name) in [(Faction::Enl, "enl"), (Faction::Res, "res")] {
            let observation = Observation::for_faction(&state, faction);
            for agent in &observation.own_agents {
                let key = agent.id.0.to_string();
                let keys = payload
                    .get("keys")
                    .unwrap()
                    .get(name)
                    .unwrap()
                    .as_array()
                    .unwrap()[i]
                    .get(&key)
                    .unwrap();
                let expected = Json::Object(
                    agent
                        .keys
                        .iter()
                        .map(|(p, n)| (p.0.to_string(), Json::uint(*n as u64)))
                        .collect(),
                );
                assert_eq!(keys, &expected);
                key_entries += agent.keys.len();
                let stream = payload
                    .get("commands")
                    .unwrap()
                    .get(name)
                    .unwrap()
                    .as_array()
                    .unwrap()[i]
                    .get(&key)
                    .unwrap();
                if let Some(executed) = report
                    .execution_order
                    .iter()
                    .find(|s| s.agent_id == agent.id)
                {
                    let cmds = stream.get("cmds").unwrap().as_array().unwrap();
                    assert_eq!(cmds.len(), executed.outcomes.len());
                    for (cmd, outcome) in cmds.iter().zip(&executed.outcomes) {
                        if let CommandOutcome::Ok(CommandEffect::Moved { from, to, .. }) = outcome {
                            moves += 1;
                            assert_eq!(
                                cmd.get("from"),
                                Some(&Json::arr(vec![
                                    Json::int(from.x as i64),
                                    Json::int(from.y as i64)
                                ]))
                            );
                            assert_eq!(
                                cmd.get("to"),
                                Some(&Json::arr(vec![
                                    Json::int(to.x as i64),
                                    Json::int(to.y as i64)
                                ]))
                            );
                        } else {
                            assert!(cmd.get("from").is_none());
                            assert!(cmd.get("to").is_none());
                        }
                    }
                }
            }
        }
    }
    assert!(
        key_entries > 100 && moves > 100,
        "non-vacuous inventory/trajectory checks"
    );
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../viewer/app/tests/fixtures");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("normal.iarp"), bytes).unwrap();
    std::fs::write(dir.join("normal.json"), json::write_json(&payload).unwrap()).unwrap();
    std::fs::write(dir.join("abnormal.iarp"), abnormal_iarp()).unwrap();
    let mut named = decode_log(bytes).unwrap();
    let mut identity = arena_replay::format::ReplayIdentity::default();
    identity.execution_id = "viewer-identity-fixture".into();
    for (index, p) in identity.participants.iter_mut().enumerate() {
        p.display_name = "星际 <astra-bot> & 长名称".repeat(5);
        p.registered_id = index.to_string();
        p.registered_version = format!("v{}", index + 1);
        p.client_name = "different-self-report".into();
        p.client_version = "sdk-0.1.0".into();
        p.sdk_version = "0.1.0".into();
        p.binary_hash = "abcdef0123456789".repeat(4);
        p.binding_verified = true;
    }
    named.header.identity = Some(identity);
    let bytes = named.encode();
    std::fs::write(dir.join("identity.iarp"), &bytes).unwrap();
    std::fs::write(
        dir.join("identity.json"),
        json::write_json(&data::build_payload(&bytes).unwrap()).unwrap(),
    )
    .unwrap();
    write_edge_browser_fixtures(&dir);
}

fn write_edge_browser_fixtures(dir: &Path) {
    use arena_core::resolve::{CommandEffect, CommandOutcome, Resolver, SubmittedCommandStream};
    use arena_core::types::{Command, Coord};
    let log = decode_log(normal_iarp()).unwrap();
    let h = &log.header;
    let (enl, res) = GameState::initial_roster(&h.config);
    let enl = validate_initial_spawn_set(&enl, &h.enl_spawns, &h.config).unwrap();
    let res = validate_initial_spawn_set(&res, &h.res_spawns, &h.config).unwrap();
    let mut state = GameState::new(h.config, &h.map, &enl, &res).unwrap();
    let writer = || ReplayWriter::new(h.game_seed, &h.config, &h.map, &enl, &res);
    let tag = arena_replay::format::OUTCOME_CLIENT_FORFEIT;
    std::fs::write(
        dir.join("zero.iarp"),
        writer().finish_abnormal(&state, tag).unwrap(),
    )
    .unwrap();
    let mut writer = writer();
    let resolver = Resolver::with_seed(h.game_seed);
    let (id, from) = h.enl_spawns[0];
    let first = Coord::new(
        from.x
            + if from.x < h.config.map_width_cm / 2 {
                100
            } else {
                -100
            },
        from.y,
    );
    let second = Coord::new(
        first.x,
        from.y
            + if from.y < h.config.map_height_cm / 2 {
                100
            } else {
                -100
            },
    );
    let portal = h
        .map
        .portals()
        .iter()
        .min_by_key(|p| arena_core::geometry::dist2(p.position, from))
        .unwrap()
        .id;
    for round in 1..=2 {
        let destinations = if round == 1 {
            [first, second]
        } else {
            [first, from]
        };
        let submitted = SubmittedCommandStream {
            round,
            faction: Faction::Enl,
            agent_id: id,
            received_at_ms: 0,
            receive_seq: round,
            commands: vec![
                Command::Move {
                    destination: destinations[0],
                },
                Command::Move {
                    destination: Coord::new(-1, -1),
                },
                Command::Move {
                    destination: destinations[1],
                },
                Command::Hack { portal },
            ],
        };
        // Wrong-round input and duplicate are archived but must not replace
        // the first locatable stream in the HUD or fabricate movement.
        let mut wrong = submitted.clone();
        wrong.round = 99;
        let report = resolver.run_round(&mut state, &[wrong, submitted.clone(), submitted]);
        assert_eq!(
            report.execution_order[0]
                .outcomes
                .iter()
                .filter(|o| matches!(o, CommandOutcome::Ok(CommandEffect::Moved { .. })))
                .count(),
            2
        );
        assert!(matches!(
            report.execution_order[0].outcomes[1],
            CommandOutcome::Drop(_)
        ));
        state.advance_round();
        writer.record_round(&report, &state).unwrap();
    }
    let bytes = writer.finish_abnormal(&state, tag).unwrap();
    let payload = data::build_payload(&bytes).unwrap();
    let stream = &payload
        .get("commands")
        .unwrap()
        .get("enl")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .get(&id.0.to_string())
        .unwrap();
    assert_eq!(stream.get("cmds").unwrap().as_array().unwrap().len(), 4);
    std::fs::write(dir.join("movement.iarp"), bytes).unwrap();
    std::fs::write(
        dir.join("movement.json"),
        json::write_json(&payload).unwrap(),
    )
    .unwrap();
}

#[test]
fn final_outcome_matches_verified_archive() {
    for bytes in [normal_iarp(), abnormal_iarp()] {
        let verified = arena_replay::replay::replay_bytes(bytes).unwrap();
        let payload = data::build_payload(bytes).unwrap();
        let outcome = payload.get("header").unwrap().get("outcome").unwrap();
        let hash_key = if verified.abnormal_outcome.is_some() {
            "last_hash"
        } else {
            "final_hash"
        };
        assert_eq!(
            outcome.get(hash_key).unwrap().as_str(),
            Some(canon::hex(&verified.final_state_hash).as_str())
        );
        let scores = outcome.get("final_score2").unwrap().as_array().unwrap();
        assert_eq!(
            scores[0].as_str(),
            Some(verified.final_scores.0.to_string().as_str())
        );
        assert_eq!(
            scores[1].as_str(),
            Some(verified.final_scores.1.to_string().as_str())
        );
        let encoded = json::write_json(&payload).unwrap();
        assert_eq!(json::parse_json(&encoded).unwrap(), payload);
    }
}

//! M10 export-pipeline consistency suite — the machine-tested half of the
//! D93 split (the human-verified half — canvas rendering, keyboard feel,
//! animation — is documented in docs/viewer.md). Every test reads the
//! payload back OUT of the emitted HTML with the strict mini-JSON parser,
//! so the assertions cover exactly what a browser would execute.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use arena_bots::bot::{BotKind, BotStrategy, make_bot};
use arena_core::config::GameConfig;
use arena_core::harness::MatchHarness;
use arena_core::map::{MapDefinition, generate_map};
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::{Faction, Score2};
use arena_replay::canon;
use arena_replay::format::EndOfMatch;
use arena_replay::recorder::ReplayWriter;
use arena_replay::replay::{decode_log, replay_bytes};
use replay_export::data::{self, ExportOptions, Exported, VIEWER_FORMAT, extract_embedded_json};
use replay_export::json::{self, Json};
use replay_export::views::{View, event_visible, visible_agents, visible_factions};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const GAME_SEED: u64 = 0x006d_3a00_0000_0033;
const BOT_SEED: u64 = 0x5eed_006d_3a00_0001;
const MAP_SEED: u64 = 0x006d_3a00_0000_0034;

fn fixture_config() -> GameConfig {
    let mut cfg = GameConfig::default();
    cfg.total_rounds = 100;
    cfg.map_width_cm = 300_000;
    cfg.map_height_cm = 300_000;
    cfg.portal_count = 40;
    cfg.portal_min_distance_cm = 8_000;
    cfg.initial_agents_per_faction = 3;
    cfg
}

fn fixture_map() -> MapDefinition {
    let cfg = fixture_config();
    for seed in MAP_SEED..MAP_SEED + 64 {
        if let Ok(map) = generate_map(seed, &cfg) {
            return map;
        }
    }
    panic!("no feasible fixture map in 64 seeds");
}

/// Plays `rounds` rounds of GreedyField vs Hybrid and returns the finished
/// NORMAL IARP plus the per-round deltas.
fn play_rounds(rounds: u32) -> (Vec<u8>, Vec<(Score2, Score2)>) {
    let cfg = fixture_config();
    let map = fixture_map();
    let mut enl_bot = make_bot(BotKind::GreedyField, Faction::Enl, BOT_SEED, &cfg, &map);
    let mut res_bot = make_bot(BotKind::Hybrid, Faction::Res, BOT_SEED, &cfg, &map);
    let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
    let enl_spawns = validate_initial_spawn_set(
        &enl_roster,
        &enl_bot.choose_spawns(&cfg, &map, &enl_roster),
        &cfg,
    )
    .expect("ENL spawns");
    let res_spawns = validate_initial_spawn_set(
        &res_roster,
        &res_bot.choose_spawns(&cfg, &map, &res_roster),
        &cfg,
    )
    .expect("RES spawns");
    let mut writer = ReplayWriter::new(GAME_SEED, &cfg, &map, &enl_spawns, &res_spawns);
    let mut harness = MatchHarness::new(
        GAME_SEED,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(BotStrategy(enl_bot)),
        Box::new(BotStrategy(res_bot)),
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

/// A verified NORMAL 100-round IARP (fields, captures, links; checkpoints
/// at 50 and 100).
fn normal_iarp() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| play_rounds(100).0)
}

/// An ABNORMAL IARP: the same match cut at round 37 with the D68
/// `OUTCOME_CLIENT_FORFEIT` trailer (the first 37 rounds are identical to
/// the normal fixture by determinism).
fn abnormal_iarp() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let cfg = fixture_config();
        let map = fixture_map();
        let mut enl_bot = make_bot(BotKind::GreedyField, Faction::Enl, BOT_SEED, &cfg, &map);
        let mut res_bot = make_bot(BotKind::Hybrid, Faction::Res, BOT_SEED, &cfg, &map);
        let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
        let enl_spawns = validate_initial_spawn_set(
            &enl_roster,
            &enl_bot.choose_spawns(&cfg, &map, &enl_roster),
            &cfg,
        )
        .expect("ENL spawns");
        let res_spawns = validate_initial_spawn_set(
            &res_roster,
            &res_bot.choose_spawns(&cfg, &map, &res_roster),
            &cfg,
        )
        .expect("RES spawns");
        let mut writer = ReplayWriter::new(GAME_SEED, &cfg, &map, &enl_spawns, &res_spawns);
        let mut harness = MatchHarness::new(
            GAME_SEED,
            cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(BotStrategy(enl_bot)),
            Box::new(BotStrategy(res_bot)),
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

fn dev_options(meta: Option<Json>) -> ExportOptions {
    ExportOptions {
        dev: true,
        faction: None,
        meta,
    }
}

fn export_normal() -> Exported {
    static EXPORT: OnceLock<Exported> = OnceLock::new();
    EXPORT
        .get_or_init(|| data::export(normal_iarp(), &dev_options(None)).expect("normal export"))
        .clone()
}

fn export_abnormal() -> Exported {
    static EXPORT: OnceLock<Exported> = OnceLock::new();
    EXPORT
        .get_or_init(|| data::export(abnormal_iarp(), &dev_options(None)).expect("abnormal export"))
        .clone()
}

/// Parses the embedded payload out of an emitted page.
fn payload_of(html: &str) -> Json {
    let text = extract_embedded_json(html).expect("embedded payload present");
    json::parse_json(text).expect("embedded payload parses")
}

/// The shortest real r1 tournament replay, `matches/m11` at 32,714 B
/// (present wherever the runs/ artifacts exist; skip on a fresh clone —
/// the M9 convention).
fn real_iarp() -> Option<&'static [u8]> {
    static BYTES: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    BYTES
        .get_or_init(|| std::fs::read("runs/tournaments/r1/matches/m11/server/replay.iarp").ok())
        .as_deref()
}

/// A meta.json echo parsed from the real r1 archive when present.
fn real_meta() -> Option<Json> {
    let text = std::fs::read_to_string("runs/tournaments/r1/matches/m11/meta.json").ok()?;
    Some(json::parse_json(&text).expect("meta.json parses"))
}

fn repo_relative(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn committed_sample_html() -> String {
    std::fs::read_to_string(repo_relative("viewer/sample.html"))
        .expect("viewer/sample.html is committed with the milestone")
}

// ---------------------------------------------------------------------------
// Shared invariant helpers (also used against the committed sample)
// ---------------------------------------------------------------------------

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

fn assert_self_contained(html: &str) {
    let lower = html.to_ascii_lowercase();
    for forbidden in [
        "http://",
        "https://",
        "src=",
        "href=",
        "fetch(",
        "xmlhttprequest",
        "websocket",
    ] {
        assert!(
            !lower.contains(forbidden),
            "self-contained page must not contain {forbidden:?}"
        );
    }
}

fn assert_three_views(payload: &Json) {
    let embedded_keys: Vec<&str> = payload
        .get("header")
        .unwrap()
        .get("factions_embedded")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Json::as_str)
        .collect();
    let rounds = payload.get("rounds").unwrap().as_array().unwrap().len();
    // Public View: no agents at any round (and no spawn dots pre-match).
    for i in 1..=rounds {
        assert!(
            visible_agents(payload, View::Public, i).is_empty(),
            "public view must draw no agents (round {i})"
        );
    }
    assert!(visible_agents(payload, View::Public, 0).is_empty());
    // Faction views: exactly the own faction's agents, never the enemy's;
    // God: a superset of every faction view.
    for view in [View::Enl, View::Res] {
        let key = match view {
            View::Enl => "enl",
            _ => "res",
        };
        let enemy = if key == "enl" { "res" } else { "enl" };
        for i in 1..=rounds {
            let visible = visible_agents(payload, view, i);
            assert!(visible.iter().all(|a| a.faction == key));
            let god = visible_agents(payload, View::God, i);
            assert!(!god.is_empty() || visible.is_empty());
            for agent in &visible {
                assert!(
                    god.iter()
                        .any(|g| g.faction == agent.faction && g.id == agent.id),
                    "god view must contain {key} agent {}",
                    agent.id
                );
            }
            // The enemy's private agents never surface through this view
            // even when embedded (dev export) — R-VIS is enforced by the
            // predicate, not just by the embedding inventory.
            assert!(visible.iter().all(|a| a.faction != enemy));
        }
    }
    // The predicate agrees with the embedding inventory for every view.
    assert!(visible_factions(View::God, &embedded_keys).len() <= embedded_keys.len());
    assert!(visible_factions(View::Public, &embedded_keys).is_empty());
}

fn assert_html_shape(html: &str) {
    for id in [
        "map",
        "hdr",
        "events",
        "curve",
        "controls",
        "views",
        "selftest-result",
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "DOM id #{id} missing"
        );
    }
    assert!(html.contains("<style>"), "inline stylesheet");
    assert!(html.contains("<script>"), "inline script");
    assert!(html.contains("const VIEWER_DATA = "), "one data block");
    assert!(!html.contains("/*__VIEWER"), "no unfilled template markers");
}

// ---------------------------------------------------------------------------
// 1. P6-GATE(a) — the real archived replay exports to a valid page
// ---------------------------------------------------------------------------

#[test]
fn export_real_iarp_produces_valid_html() {
    let Some(iarp) = real_iarp() else {
        eprintln!("SKIP: runs/tournaments/r1 artifacts absent (M9 convention)");
        return;
    };
    let exported = data::export(
        iarp,
        &ExportOptions {
            dev: true,
            faction: None,
            meta: real_meta(),
        },
    )
    .expect("real r1 replay exports");
    assert_html_shape(&exported.html);
    assert_self_contained(&exported.html);
    // The meta echo landed.
    let payload = payload_of(&exported.html);
    let meta = payload
        .get("header")
        .unwrap()
        .get("meta")
        .and_then(Json::as_object)
        .expect("meta echoed");
    assert!(meta.iter().any(|(k, _)| k == "entrant_a"));
}

// ---------------------------------------------------------------------------
// 2. P6-GATE(b1) — score2 accumulation is exact everywhere
// ---------------------------------------------------------------------------

#[test]
fn embedded_score2_accumulation_exact() {
    let normal = export_normal();
    assert_score_accumulation_exact(&payload_of(&normal.html));

    let abnormal = export_abnormal();
    assert_score_accumulation_exact(&payload_of(&abnormal.html));

    // The committed sample carries the same invariant (no runs/ needed).
    let sample = committed_sample_html();
    assert_score_accumulation_exact(&payload_of(&sample));

    // Cross-check against the IARP's own RoundRecord deltas: the embedded
    // cumulative values are exactly the record accumulation, and every
    // per-round hash is the record's verified D45 hash.
    for (exported, source) in [(normal, normal_iarp()), (abnormal, abnormal_iarp())] {
        let log = decode_log(source).expect("decode");
        let payload = payload_of(&exported.html);
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

// ---------------------------------------------------------------------------
// 3. P6-GATE(b2) — result/final hash/final score equal the replay truth
// ---------------------------------------------------------------------------

#[test]
fn final_result_and_hash_equality() {
    for (exported, source) in [
        (export_normal(), normal_iarp().to_vec()),
        (export_abnormal(), abnormal_iarp().to_vec()),
    ] {
        let outcome = replay_bytes(&source).expect("replay verifies");
        let payload = payload_of(&exported.html);
        let header = payload.get("header").unwrap();
        let embedded = header.get("outcome").unwrap();
        let embedded_hash = match embedded.get("kind").and_then(Json::as_str) {
            Some("normal") => embedded.get("final_hash"),
            _ => embedded.get("last_hash"),
        }
        .and_then(Json::as_str)
        .unwrap();
        assert_eq!(embedded_hash, canon::hex(&outcome.final_state_hash));
        let final_score = embedded.get("final_score2").unwrap().as_array().unwrap();
        assert_eq!(
            final_score[0].as_str(),
            Some(outcome.final_scores.0.to_string().as_str())
        );
        assert_eq!(
            final_score[1].as_str(),
            Some(outcome.final_scores.1.to_string().as_str())
        );
        // The summary line carries the same values the page embeds.
        let summary = &exported.summary;
        assert!(summary.contains(&format!("rounds={}", outcome.rounds_completed)));
        assert!(summary.contains(&format!("score2_enl={}", outcome.final_scores.0)));
        assert!(summary.contains(&format!("score2_res={}", outcome.final_scores.1)));
        match outcome.abnormal_outcome {
            None => {
                assert!(summary.contains(&format!("final_hash={embedded_hash}")));
                let result = embedded.get("result").and_then(Json::as_str).unwrap();
                let expected = match outcome.result {
                    arena_core::types::MatchResult::EnlWin => "enl_win",
                    arena_core::types::MatchResult::ResWin => "res_win",
                    arena_core::types::MatchResult::Draw => "draw",
                };
                assert_eq!(result, expected);
                assert!(summary.contains(&format!("result={expected}")));
            }
            Some(tag) => {
                assert!(summary.contains(&format!("last_hash={embedded_hash}")));
                assert!(summary.contains(&format!("outcome_tag={tag}")));
            }
        }
        // The trailer agrees with both (decode enforces the counts; the
        // values come from the same source of truth).
        let log = decode_log(&source).unwrap();
        match log.trailer().unwrap() {
            EndOfMatch::Normal {
                final_state_hash, ..
            } => assert_eq!(canon::hex(final_state_hash), embedded_hash),
            EndOfMatch::Abnormal {
                last_valid_hash, ..
            } => assert_eq!(canon::hex(last_valid_hash), embedded_hash),
        }
    }

    // The real r1 archive, wherever the artifacts exist: the embedded hash
    // equals both the trailer and the tournament archive's meta.json.
    if let (Some(iarp), Some(meta)) = (real_iarp(), real_meta()) {
        let exported = data::export(iarp, &dev_options(Some(meta.clone()))).expect("export");
        let payload = payload_of(&exported.html);
        let embedded_hash = payload
            .get("header")
            .unwrap()
            .get("outcome")
            .unwrap()
            .get("final_hash")
            .and_then(Json::as_str)
            .unwrap();
        let recorded = meta.get("final_state_hash").and_then(Json::as_str).unwrap();
        assert_eq!(embedded_hash, recorded);
    }
}

// ---------------------------------------------------------------------------
// 4. P6-GATE(b3) — round counts and the abnormal shape
// ---------------------------------------------------------------------------

#[test]
fn round_count_matches() {
    // Normal: rounds.len == rounds_completed == record count; total_rounds
    // echoes the config.
    let exported = export_normal();
    let payload = payload_of(&exported.html);
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
    assert_eq!(checkpoints, vec![50, 100]);

    // Abnormal: last_round + outcome_tag preserved, rounds truncated at
    // the last COMPLETED round.
    let exported = export_abnormal();
    let payload = payload_of(&exported.html);
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
    assert_eq!(header.get("total_rounds").unwrap().as_int(), Some(100));
}

// ---------------------------------------------------------------------------
// 5. R-VIS(d) — a faction export contains no enemy private data
// ---------------------------------------------------------------------------

#[test]
fn faction_export_contains_no_enemy_agent_data() {
    let enl = data::export(
        normal_iarp(),
        &ExportOptions {
            dev: false,
            faction: Some(Faction::Enl),
            meta: None,
        },
    )
    .expect("enl export");
    let payload = payload_of(&enl.html);
    let header = payload.get("header").unwrap();

    // agents keys are EXACTLY the embedded faction.
    let agents = payload.get("agents").and_then(Json::as_object).unwrap();
    let keys: Vec<&str> = agents.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["enl"]);
    assert_eq!(
        header
            .get("factions_embedded")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(header.get("dev").and_then(Json::as_bool), Some(false));

    // The RES roster exists in the source match — collect its private
    // (position, supply) rows from the IARP-verified final state and
    // prove none of them exist anywhere in the payload text (enemy
    // private data does not exist in the file, D90).
    let outcome = replay_bytes(normal_iarp()).unwrap();
    let enemy_agents: Vec<(i64, i64, i64)> = outcome
        .state
        .agents()
        .values()
        .filter(|agent| agent.faction == Faction::Res)
        .map(|agent| {
            (
                agent.position.x as i64,
                agent.position.y as i64,
                agent.supply as i64,
            )
        })
        .collect();
    assert!(!enemy_agents.is_empty(), "fixture has RES agents");
    for (x, y, supply) in enemy_agents {
        let tail = format!(",{x},{y},{supply}]");
        assert!(
            !enl.payload_text.contains(&tail),
            "enemy private row tail {tail} must not exist in a faction export"
        );
    }
    // Enemy initial spawn coordinates are likewise absent (positions are
    // private, R-VIS-02): the spawns.res list is present but empty.
    let spawns_res = header
        .get("spawns")
        .unwrap()
        .get("res")
        .and_then(Json::as_array)
        .unwrap();
    assert!(spawns_res.is_empty());
    let log = decode_log(normal_iarp()).unwrap();
    assert!(!log.header.res_spawns.is_empty(), "source had RES spawns");

    // The symmetric RES export behaves identically.
    let res = data::export(
        normal_iarp(),
        &ExportOptions {
            dev: false,
            faction: Some(Faction::Res),
            meta: None,
        },
    )
    .expect("res export");
    let payload = payload_of(&res.html);
    let keys: Vec<&str> = payload
        .get("agents")
        .and_then(Json::as_object)
        .unwrap()
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(keys, vec!["res"]);

    // Dev mode embeds both and flags itself (D90).
    let dev = export_normal();
    let payload = payload_of(&dev.html);
    let keys: Vec<&str> = payload
        .get("agents")
        .and_then(Json::as_object)
        .unwrap()
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(keys, vec!["enl", "res"]);
    assert_eq!(
        payload
            .get("header")
            .unwrap()
            .get("dev")
            .and_then(Json::as_bool),
        Some(true)
    );

    // Default (no faction, no dev) is a usage error, not a silent leak.
    let error = data::export(normal_iarp(), &ExportOptions::default()).unwrap_err();
    assert!(error.contains("--faction"), "got: {error}");
}

// ---------------------------------------------------------------------------
// 6. Hard rule (e) — the page is fully self-contained
// ---------------------------------------------------------------------------

#[test]
fn self_contained_scan() {
    assert_self_contained(&export_normal().html);
    assert_self_contained(&export_abnormal().html);
    assert_self_contained(&committed_sample_html());
}

// ---------------------------------------------------------------------------
// 7. P6-GATE(c) — three views, one base state
// ---------------------------------------------------------------------------

#[test]
fn three_views_derive_from_one_base() {
    let exported = export_normal();
    let payload = payload_of(&exported.html);
    assert_three_views(&payload);

    // Per-round public materialization exists for every round (one base
    // state shared by all views; the twin only selects agents).
    let rounds = payload.get("rounds").unwrap().as_array().unwrap();
    assert_eq!(rounds.len(), 100);
    for round in rounds {
        assert!(!round.get("portals").unwrap().as_array().unwrap().is_empty());
    }

    // And on the committed sample.
    assert_three_views(&payload_of(&committed_sample_html()));

    // The event predicate twin: reinforcement is both-faction (no f), a
    // tagged kind hides without its faction, factions cannot replace kinds.
    assert!(event_visible(8, None, &[8], &[]));
    assert!(!event_visible(1, Some(0), &[1], &[1]));
    assert!(event_visible(1, Some(0), &[1], &[0, 1]));
    assert!(!event_visible(1, Some(0), &[], &[0, 1]));
}

// ---------------------------------------------------------------------------
// 8. D92 — mini-JSON round trip over the full payload
// ---------------------------------------------------------------------------

#[test]
fn json_mini_roundtrip_on_full_payload() {
    let exported = export_normal();
    let parsed = payload_of(&exported.html);
    // writer -> parser identity (structural equality) ...
    assert_eq!(&parsed, &exported.payload);
    // ... and the re-write is byte-identical (canonical compact form).
    assert_eq!(json::write_json(&parsed), exported.payload_text);
    // The data block can never close the script element and never carries
    // a raw angle bracket or ampersand.
    assert!(!exported.payload_text.contains("</script>"));
    assert!(!exported.payload_text.contains('<'));
    assert!(!exported.payload_text.contains('&'));

    // A hostile meta string stays inert through the whole pipeline while
    // parsing back to the exact original value.
    let hostile = Json::obj(vec![(
        "entrant_a",
        Json::str("</script><script>steal()</script>&amp;"),
    )]);
    let exported = data::export(
        normal_iarp(),
        &ExportOptions {
            dev: true,
            faction: None,
            meta: Some(hostile),
        },
    )
    .expect("export with hostile meta");
    // Exactly the two structural script closes (template's own), none
    // contributed by the data.
    assert_eq!(exported.html.matches("</script>").count(), 2);
    let payload = payload_of(&exported.html);
    assert_eq!(
        payload
            .get("header")
            .unwrap()
            .get("meta")
            .unwrap()
            .get("entrant_a")
            .and_then(Json::as_str),
        Some("</script><script>steal()</script>&amp;")
    );
}

// ---------------------------------------------------------------------------
// 9. D88 — unverified input refuses export (no HTML written)
// ---------------------------------------------------------------------------

#[test]
fn unverified_input_refuses_export() {
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

    let error = data::export(&bytes, &dev_options(None)).expect_err("must refuse");
    assert!(error.contains("verification failed"), "got: {error}");

    // A structurally broken file also refuses.
    let error = data::export(b"not an iarp", &dev_options(None)).expect_err("refuse");
    assert!(error.contains("decode failed"));

    // The healthy export passes the same pipeline (the materialization
    // tripwires — per-round hash, public equality, fold, final hash —
    // run inside every successful export).
    let exported = export_normal();
    assert!(exported.html.contains("const VIEWER_DATA"));
}

// ---------------------------------------------------------------------------
// 10. Abnormal fixture — full consistency over the completed prefix
// ---------------------------------------------------------------------------

#[test]
fn abnormal_fixture_exports() {
    let exported = export_abnormal();
    assert_html_shape(&exported.html);
    let payload = payload_of(&exported.html);
    assert_score_accumulation_exact(&payload);
    let outcome = payload.get("header").unwrap().get("outcome").unwrap();
    assert_eq!(outcome.get("kind").and_then(Json::as_str), Some("abnormal"));
    // Viewer banner data: tag + last round + last hash present; the last
    // completed round's hash equals the outcome hash.
    let last_hash = outcome.get("last_hash").and_then(Json::as_str).unwrap();
    let rounds = payload.get("rounds").unwrap().as_array().unwrap();
    let last_round_hash = rounds[36].get("hash").and_then(Json::as_str).unwrap();
    assert_eq!(last_hash, last_round_hash);
    // The in-page banner marker exists in the emitted HTML.
    assert!(exported.html.contains("ABNORMAL TERMINATION"));
}

// ---------------------------------------------------------------------------
// 11. Committed sample invariants (fresh clone, no runs/ needed)
// ---------------------------------------------------------------------------

#[test]
fn sample_committed_invariants() {
    let html = committed_sample_html();
    assert_html_shape(&html);
    assert_self_contained(&html);
    let payload = payload_of(&html);
    assert_eq!(
        payload.get("format").and_then(Json::as_str),
        Some(VIEWER_FORMAT)
    );
    assert_score_accumulation_exact(&payload);
    assert_three_views(&payload);
    // It is the dev sample (both factions embedded, header.dev true).
    let header = payload.get("header").unwrap();
    assert_eq!(header.get("dev").and_then(Json::as_bool), Some(true));
    let keys: Vec<&str> = payload
        .get("agents")
        .and_then(Json::as_object)
        .unwrap()
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(keys, vec!["enl", "res"]);
    // Pinned to the real r1 m11 archive data (committed bytes).
    let outcome = header.get("outcome").unwrap();
    assert_eq!(
        outcome.get("final_hash").and_then(Json::as_str),
        Some("9b3777f68a567e4729c43c3d50b9437cbf238ad24e496b49b862aa6ceb607079")
    );
    assert_eq!(
        outcome.get("final_score2").unwrap().as_array().unwrap()[0].as_str(),
        Some("1859239392301")
    );
    // Round count and checkpoints of the archived 100-round match.
    assert_eq!(
        payload.get("rounds").unwrap().as_array().unwrap().len(),
        100
    );
    assert_eq!(
        header.get("checkpoints").unwrap().as_array().unwrap().len(),
        2
    );
}

// ---------------------------------------------------------------------------
// 12. Selftest mode present (human-triggered; no browser automation)
// ---------------------------------------------------------------------------

#[test]
fn selftest_mode_present() {
    let html = export_normal().html;
    // The guard, the render target, and each frozen invariant.
    assert!(html.contains("selftest=1"));
    assert!(html.contains("selftest-result"));
    assert!(html.contains("delta2 fold == score2"));
    assert!(html.contains("round indexing"));
    assert!(html.contains("view filters"));
    assert!(html.contains("event filter predicate"));
    assert!(html.contains("jump determinism"));
    assert!(html.contains("score2 display conversion"));
}

// ---------------------------------------------------------------------------
// 14. Docs gate
// ---------------------------------------------------------------------------

#[test]
fn docs_gate() {
    let viewer_doc =
        std::fs::read_to_string(repo_relative("docs/viewer.md")).expect("docs/viewer.md");
    for section in [
        "View semantics",
        "ingress-viewer-v1",
        "Machine-tested",
        "regenerat",
        "selftest",
    ] {
        assert!(
            viewer_doc.contains(section),
            "docs/viewer.md lacks {section:?}"
        );
    }
    let progress =
        std::fs::read_to_string(repo_relative("docs/progress.md")).expect("docs/progress.md");
    for marker in ["D87", "D88", "D89", "D90", "D91", "D92", "D93"] {
        assert!(progress.contains(marker), "progress.md lacks {marker}");
    }
}

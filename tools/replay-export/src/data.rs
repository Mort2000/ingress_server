//! The `ingress-viewer-v1` payload builder + HTML assembly (D87–D91).
//!
//! Pipeline (D88): decode → `arena_replay::replay` (full verification —
//! unverified input is REFUSED, no HTML is produced) → a second
//! materialization pass over the same recorded submissions snapshotting
//! `Observation::for_faction` per faction at every D45 round-close point →
//! tripwires (per-round hash, public-section equality between the two
//! faction observations, delta fold == observed cumulative score, final
//! hash == the VERIFIED outcome's hash). The embedded `agents` object
//! carries exactly the factions chosen by D90.

use arena_core::config::GameConfig;
use arena_core::event::{Event, EventKind};
use arena_core::geometry;
use arena_core::observe::Observation;
use arena_core::resolve::Resolver;
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::{Faction, Owner, PortalId, Score2};
use arena_replay::canon;
use arena_replay::format::Block;
use arena_replay::replay::{decode_log, replay};

use crate::json::Json;

/// The embedded data schema identifier.
pub const VIEWER_FORMAT: &str = "ingress-viewer-v1";

/// Viewer source assets (P-ARCH-03: repository root `viewer/`, outside the
/// Rust crates; `include_str!`-ed here, concatenated into ONE self-contained
/// HTML page).
const TEMPLATE: &str = include_str!("../../../viewer/template.html");
const VIEWER_CSS: &str = include_str!("../../../viewer/viewer.css");
const JS_DATA: &str = include_str!("../../../viewer/data.js");
const JS_FILTER: &str = include_str!("../../../viewer/filter.js");
const JS_RENDER: &str = include_str!("../../../viewer/render.js");
const JS_PANELS: &str = include_str!("../../../viewer/panels.js");
const JS_CONTROLS: &str = include_str!("../../../viewer/controls.js");
const JS_SELFTEST: &str = include_str!("../../../viewer/selftest.js");

/// What to embed and how (D90).
#[derive(Clone, Debug)]
pub struct ExportOptions {
    /// `--dev 1`: embed BOTH factions' private agents, activate all four
    /// view labels, set `header.dev = true` (debug/balance tool).
    pub dev: bool,
    /// `--faction enl|res`: required when `dev == false`; the single
    /// faction whose private agents are embedded (R-VIS by construction —
    /// enemy private data does not exist in the file).
    pub faction: Option<Faction>,
    /// Parsed `meta.json` echo for `header.meta` (None embeds null).
    pub meta: Option<Json>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            dev: false,
            faction: None,
            meta: None,
        }
    }
}

/// A finished export: the payload value, its compact JSON text, the
/// assembled self-contained HTML, and the summary line (built from the
/// SAME verified `ReplayOutcome` whose values are embedded, so the CLI
/// text and the page can never disagree).
#[derive(Clone, Debug)]
pub struct Exported {
    pub payload: Json,
    pub payload_text: String,
    pub html: String,
    pub summary: String,
}

/// Decode → verify → materialize → build payload + summary. This is the
/// whole export except writing the file.
pub fn build_payload(iarp: &[u8], options: &ExportOptions) -> Result<(Json, String), String> {
    let log = decode_log(iarp).map_err(|error| format!("replay decode failed: {error}"))?;
    // The P6-GATE reference: every per-round hash/delta/event/intake fact
    // and the trailer are verified BEFORE anything is embedded. An
    // unverified IARP produces an error and no HTML (D88).
    let outcome = replay(&log).map_err(|error| format!("replay verification failed: {error}"))?;

    let embedded_factions: Vec<Faction> = if options.dev {
        vec![Faction::Enl, Faction::Res]
    } else {
        vec![
            options
                .faction
                .ok_or("production exports must name --faction enl|res (or pass --dev 1)")?,
        ]
    };

    let materialized = materialize(&log, &outcome, &embedded_factions)?;

    // --- header ------------------------------------------------------
    let header = &log.header;
    let mut header_pairs: Vec<(String, Json)> = vec![
        (
            "game_seed".into(),
            Json::str(format!("{:#018x}", header.game_seed)),
        ),
        (
            "total_rounds".into(),
            Json::uint(header.total_rounds as u64),
        ),
        (
            "rounds_completed".into(),
            Json::uint(outcome.rounds_completed as u64),
        ),
        (
            "versions".into(),
            Json::obj(vec![
                ("server", Json::uint(header.server_version as u64)),
                ("protocol", Json::uint(header.protocol_version as u64)),
                ("rng", Json::uint(header.rng_version as u64)),
                (
                    "state_encoding",
                    Json::uint(header.state_encoding_version as u64),
                ),
            ]),
        ),
        ("config".into(), config_json(&header.config)),
        (
            "map".into(),
            Json::obj(vec![(
                "portals",
                Json::arr(
                    header
                        .map
                        .portals()
                        .iter()
                        .map(|portal| {
                            Json::arr(vec![
                                Json::uint(portal.id.0 as u64),
                                Json::int(portal.position.x as i64),
                                Json::int(portal.position.y as i64),
                            ])
                        })
                        .collect(),
                ),
            )]),
        ),
        // R-VIS note: spawn coordinates are agent positions (private,
        // R-VIS-02) — a faction export embeds ONLY its own faction's
        // spawn list; the other key stays present-but-empty so the viewer
        // reads a uniform shape. The IARP itself keeps both (replay needs
        // them); the EXPORT does not.
        (
            "spawns".into(),
            Json::obj(vec![
                (
                    "enl",
                    if embedded_factions.contains(&Faction::Enl) {
                        spawns_json(&header.enl_spawns)
                    } else {
                        Json::arr(vec![])
                    },
                ),
                (
                    "res",
                    if embedded_factions.contains(&Faction::Res) {
                        spawns_json(&header.res_spawns)
                    } else {
                        Json::arr(vec![])
                    },
                ),
            ]),
        ),
    ];
    header_pairs.push(("outcome".into(), outcome_json(&outcome)));
    header_pairs.push((
        "checkpoints".into(),
        Json::arr(
            log.blocks
                .iter()
                .filter_map(|block| match block {
                    Block::Checkpoint(checkpoint) => Some(Json::uint(checkpoint.round as u64)),
                    _ => None,
                })
                .collect(),
        ),
    ));
    header_pairs.push(("dev".into(), Json::Bool(options.dev)));
    header_pairs.push((
        "factions_embedded".into(),
        Json::arr(
            embedded_factions
                .iter()
                .map(|faction| Json::str(faction_key(*faction)))
                .collect(),
        ),
    ));
    header_pairs.push(("meta".into(), options.meta.clone().unwrap_or(Json::Null)));

    let payload = Json::obj(vec![
        ("format", Json::str(VIEWER_FORMAT)),
        (
            "generated",
            Json::obj(vec![
                ("tool", Json::str("replay-export")),
                ("version", Json::str(env!("CARGO_PKG_VERSION"))),
                (
                    "iarp_format",
                    Json::uint(arena_replay::format::FORMAT_VERSION as u64),
                ),
            ]),
        ),
        ("header", Json::Object(header_pairs)),
        ("rounds", Json::Array(materialized.rounds)),
        ("agents", Json::Object(materialized.agents)),
    ]);
    Ok((payload, summary_line(&outcome)))
}

/// Decode → verify → materialize → payload → ONE self-contained HTML page.
pub fn export(iarp: &[u8], options: &ExportOptions) -> Result<Exported, String> {
    let (payload, summary) = build_payload(iarp, options)?;
    let payload_text = crate::json::write_json(&payload);
    let html = assemble_html(&payload_text);
    Ok(Exported {
        payload,
        payload_text,
        html,
        summary,
    })
}

/// Assembles the final page: the template with the stylesheet, the
/// `VIEWER_DATA` constant, and the six viewer scripts inlined.
pub fn assemble_html(payload_text: &str) -> String {
    let data_block = format!("const VIEWER_DATA = {payload_text};");
    let scripts = [
        JS_DATA,
        JS_FILTER,
        JS_RENDER,
        JS_PANELS,
        JS_CONTROLS,
        JS_SELFTEST,
    ]
    .join("\n");
    TEMPLATE
        .replace("/*__VIEWER_CSS__*/", VIEWER_CSS)
        .replace("/*__VIEWER_DATA__*/", &data_block)
        .replace("/*__VIEWER_JS__*/", &scripts)
}

/// Extracts the embedded payload text back out of an emitted page (the
/// inverse of [`assemble_html`]'s data block; used by every consistency
/// test so the tests read exactly what a browser would execute).
pub fn extract_embedded_json(html: &str) -> Option<&str> {
    // Newline-anchored so the marker inside data.js's explanatory comment
    // can never shadow the real data block.
    const MARKER: &str = "\nconst VIEWER_DATA = ";
    let start = html.find(MARKER)? + MARKER.len();
    let end = html[start..].find("</script>")? + start;
    let text = html[start..end].trim();
    text.strip_suffix(';').map(str::trim)
}

// ---------------------------------------------------------------------------
// Materialization (D88 second pass)
// ---------------------------------------------------------------------------

struct Materialized {
    rounds: Vec<Json>,
    /// `(faction key, per-round agent rows)` in embedding order.
    agents: Vec<(String, Json)>,
}

fn materialize(
    log: &arena_replay::format::ReplayFile,
    outcome: &arena_replay::replay::ReplayOutcome,
    embedded: &[Faction],
) -> Result<Materialized, String> {
    let header = &log.header;
    // Rebuild from header inputs only — the map is loaded from the file,
    // never regenerated (same construction as the verified replay pass).
    let (enl_roster, res_roster) = GameState::initial_roster(&header.config);
    let enl_spawns = validate_initial_spawn_set(&enl_roster, &header.enl_spawns, &header.config)
        .map_err(|error| format!("ENL spawn set invalid: {error:?}"))?;
    let res_spawns = validate_initial_spawn_set(&res_roster, &header.res_spawns, &header.config)
        .map_err(|error| format!("RES spawn set invalid: {error:?}"))?;
    let mut state = GameState::new(header.config, &header.map, &enl_spawns, &res_spawns)
        .map_err(|error| format!("state construction failed: {error:?}"))?;
    let resolver = Resolver::with_seed(header.game_seed);

    let positions: Vec<(i64, i64)> = header
        .map
        .portals()
        .iter()
        .map(|portal| (portal.position.x as i64, portal.position.y as i64))
        .collect();
    let area_of = |a: u32, b: u32, c: u32| -> Option<u64> {
        let (ax, ay) = *positions.get(a as usize)?;
        let (bx, by) = *positions.get(b as usize)?;
        let (cx, cy) = *positions.get(c as usize)?;
        Some(geometry::area2(
            arena_core::types::Coord::new(ax as i32, ay as i32),
            arena_core::types::Coord::new(bx as i32, by as i32),
            arena_core::types::Coord::new(cx as i32, cy as i32),
        ))
    };

    let mut rounds = Vec::new();
    let mut agent_rounds: Vec<Vec<Json>> = vec![Vec::new(); embedded.len()];
    let mut cumulative: (Score2, Score2) = (0, 0);

    for record in log.round_records() {
        let _report = resolver.run_round(&mut state, &record.submissions);
        state.advance_round();

        // Tripwire 1: this pass hashes identically to the verified replay.
        let actual_hash = canon::state_hash(&state);
        if actual_hash != record.state_hash {
            return Err(format!(
                "materialization diverged at round {}: hash {} != recorded {}",
                record.round,
                canon::hex(&actual_hash),
                canon::hex(&record.state_hash)
            ));
        }

        // The one R-VIS construction path: snapshot per faction (D88/D90).
        let observations: Vec<Observation> = embedded
            .iter()
            .map(|faction| Observation::for_faction(&state, *faction))
            .collect();
        // Tripwire 2: the public sections are identical for both factions
        // (R-VIS-01) — the embedded public state comes from one of them.
        for observation in &observations[1..] {
            if observation.portals != observations[0].portals
                || observation.links != observations[0].links
                || observation.fields != observations[0].fields
                || observation.scores != observations[0].scores
            {
                return Err(format!(
                    "R-VIS public sections differ between factions at round {}",
                    record.round
                ));
            }
        }

        // Tripwire 3: integer fold of the recorded deltas == the observed
        // cumulative scores at this round-close point.
        cumulative.0 = cumulative
            .0
            .checked_add(record.score2_delta.0)
            .ok_or("score2 fold overflow")?;
        cumulative.1 = cumulative
            .1
            .checked_add(record.score2_delta.1)
            .ok_or("score2 fold overflow")?;
        let observed_enl = observations[0]
            .scores
            .get(&Faction::Enl)
            .copied()
            .unwrap_or(0);
        let observed_res = observations[0]
            .scores
            .get(&Faction::Res)
            .copied()
            .unwrap_or(0);
        if (observed_enl, observed_res) != cumulative {
            return Err(format!(
                "score fold diverged at round {}: deltas fold to ({}, {}) but the state says ({}, {})",
                record.round, cumulative.0, cumulative.1, observed_enl, observed_res
            ));
        }

        // Round payload: public state from the first observation, events
        // from the replay-verified record (the round's NEW events).
        let public = &observations[0];
        let mut events = Vec::with_capacity(record.events.len());
        for event in &record.events {
            events.push(event_json(event, &area_of));
        }
        rounds.push(Json::obj(vec![
            ("r", Json::uint(record.round as u64)),
            (
                "score2",
                Json::arr(vec![
                    Json::str(cumulative.0.to_string()),
                    Json::str(cumulative.1.to_string()),
                ]),
            ),
            (
                "delta2",
                Json::arr(vec![
                    Json::str(record.score2_delta.0.to_string()),
                    Json::str(record.score2_delta.1.to_string()),
                ]),
            ),
            (
                "portals",
                Json::arr(
                    public
                        .portals
                        .iter()
                        .map(|portal| {
                            Json::arr(vec![
                                Json::uint(portal.id.0 as u64),
                                Json::uint(owner_tag(portal.owner) as u64),
                                Json::uint(portal.hp as u64),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "links",
                Json::arr(
                    public
                        .links
                        .iter()
                        .map(|link| {
                            Json::arr(vec![
                                Json::uint(link.id.min.0 as u64),
                                Json::uint(link.id.max.0 as u64),
                                Json::uint(faction_tag(link.faction) as u64),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "fields",
                Json::arr(
                    public
                        .fields
                        .iter()
                        .map(|field| {
                            Json::arr(vec![
                                Json::uint(field.id.a.0 as u64),
                                Json::uint(field.id.b.0 as u64),
                                Json::uint(field.id.c.0 as u64),
                                Json::uint(faction_tag(field.faction) as u64),
                                Json::uint(field.area2),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("events", Json::arr(events)),
            ("hash", Json::str(canon::hex(&record.state_hash))),
        ]));

        // Per-faction private agents at this round-close point.
        for (slot, observation) in observations.iter().enumerate() {
            agent_rounds[slot].push(Json::arr(
                observation
                    .own_agents
                    .iter()
                    .map(|agent| {
                        Json::arr(vec![
                            Json::uint(agent.id.0 as u64),
                            Json::int(agent.position.x as i64),
                            Json::int(agent.position.y as i64),
                            Json::uint(agent.supply as u64),
                        ])
                    })
                    .collect(),
            ));
        }
    }

    // Tripwire 4 (D88): the materialized end state hashes to the VERIFIED
    // outcome's final hash.
    let final_hash = canon::state_hash(&state);
    if final_hash != outcome.final_state_hash {
        return Err(format!(
            "materialization final hash {} != verified outcome {}",
            canon::hex(&final_hash),
            canon::hex(&outcome.final_state_hash)
        ));
    }

    let agents = embedded
        .iter()
        .zip(agent_rounds)
        .map(|(faction, rounds)| (faction_key(*faction).to_string(), Json::Array(rounds)))
        .collect();
    Ok(Materialized { rounds, agents })
}

// ---------------------------------------------------------------------------
// Payload pieces
// ---------------------------------------------------------------------------

fn faction_key(faction: Faction) -> &'static str {
    match faction {
        Faction::Enl => "enl",
        Faction::Res => "res",
    }
}

/// Faction tag (D51): 0=ENL, 1=RES.
fn faction_tag(faction: Faction) -> u8 {
    arena_core::codec::faction_tag(faction)
}

/// Owner tag (D51): 0=Neutral, 1=ENL, 2=RES.
fn owner_tag(owner: Owner) -> u8 {
    arena_core::codec::owner_tag(owner)
}

fn spawns_json(spawns: &[(arena_core::types::AgentId, arena_core::types::Coord)]) -> Json {
    Json::arr(
        spawns
            .iter()
            .map(|(agent, position)| {
                Json::arr(vec![
                    Json::uint(agent.0 as u64),
                    Json::int(position.x as i64),
                    Json::int(position.y as i64),
                ])
            })
            .collect(),
    )
}

/// The 31 config identity fields as name → int, in D44 codec order
/// (exactly the `GameConfig` declaration order the canonical hash uses).
fn config_json(cfg: &GameConfig) -> Json {
    let words: [(&str, i64); 31] = [
        ("total_rounds", cfg.total_rounds as i64),
        (
            "initial_spawn_window_ms",
            cfg.initial_spawn_window_ms as i64,
        ),
        ("decision_window_ms", cfg.decision_window_ms as i64),
        ("priority_bucket_ms", cfg.priority_bucket_ms as i64),
        ("epoch_length", cfg.epoch_length as i64),
        ("map_width_cm", cfg.map_width_cm as i64),
        ("map_height_cm", cfg.map_height_cm as i64),
        ("portal_count", cfg.portal_count as i64),
        ("portal_min_distance_cm", cfg.portal_min_distance_cm as i64),
        (
            "clustered_portal_ratio_permille",
            cfg.clustered_portal_ratio_permille as i64,
        ),
        ("portal_cluster_count", cfg.portal_cluster_count as i64),
        (
            "initial_agents_per_faction",
            cfg.initial_agents_per_faction as i64,
        ),
        (
            "reinforcement_interval_rounds",
            cfg.reinforcement_interval_rounds as i64,
        ),
        (
            "reinforcement_agents_per_faction",
            cfg.reinforcement_agents_per_faction as i64,
        ),
        ("move_budget_cm", cfg.move_budget_cm as i64),
        (
            "portal_operation_range_cm",
            cfg.portal_operation_range_cm as i64,
        ),
        ("transfer_range_cm", cfg.transfer_range_cm as i64),
        ("initial_supply", cfg.initial_supply as i64),
        ("max_supply", cfg.max_supply as i64),
        ("portal_max_hp", cfg.portal_max_hp as i64),
        ("attack_damage", cfg.attack_damage as i64),
        ("recharge_amount", cfg.recharge_amount as i64),
        ("recharge_cost_supply", cfg.recharge_cost_supply as i64),
        ("hack_supply_yield", cfg.hack_supply_yield as i64),
        ("hack_key_yield_min", cfg.hack_key_yield_min as i64),
        ("hack_key_yield_max", cfg.hack_key_yield_max as i64),
        ("hack_cooldown_rounds", cfg.hack_cooldown_rounds as i64),
        ("hack_limit_per_epoch", cfg.hack_limit_per_epoch as i64),
        ("max_link_distance_cm", cfg.max_link_distance_cm as i64),
        (
            "max_commands_per_agent_round",
            cfg.max_commands_per_agent_round as i64,
        ),
        (
            "max_frame_payload_bytes",
            cfg.max_frame_payload_bytes as i64,
        ),
    ];
    Json::obj(words.into_iter().map(|(k, v)| (k, Json::int(v))).collect())
}

/// Trailer echo. Normal: kind/result/final_hash/final_score2. Abnormal:
/// kind/outcome_tag/last_round/last_hash/final_score2 (the cumulative
/// scores at the last completed round).
fn outcome_json(outcome: &arena_replay::replay::ReplayOutcome) -> Json {
    let scores = Json::arr(vec![
        Json::str(outcome.final_scores.0.to_string()),
        Json::str(outcome.final_scores.1.to_string()),
    ]);
    match outcome.abnormal_outcome {
        None => Json::obj(vec![
            ("kind", Json::str("normal")),
            (
                "result",
                Json::str(match outcome.result {
                    arena_core::types::MatchResult::EnlWin => "enl_win",
                    arena_core::types::MatchResult::ResWin => "res_win",
                    arena_core::types::MatchResult::Draw => "draw",
                }),
            ),
            (
                "final_hash",
                Json::str(canon::hex(&outcome.final_state_hash)),
            ),
            ("final_score2", scores),
        ]),
        Some(tag) => Json::obj(vec![
            ("kind", Json::str("abnormal")),
            ("outcome_tag", Json::uint(tag as u64)),
            ("last_round", Json::uint(outcome.rounds_completed as u64)),
            (
                "last_hash",
                Json::str(canon::hex(&outcome.final_state_hash)),
            ),
            ("final_score2", scores),
        ]),
    }
}

/// The CLI summary line, built from the SAME verified outcome whose values
/// are embedded (rounds, result/outcome tag, final hash hex, final score2).
fn summary_line(outcome: &arena_replay::replay::ReplayOutcome) -> String {
    match outcome.abnormal_outcome {
        None => format!(
            "verified rounds={} result={} final_hash={} score2_enl={} score2_res={}",
            outcome.rounds_completed,
            match outcome.result {
                arena_core::types::MatchResult::EnlWin => "enl_win",
                arena_core::types::MatchResult::ResWin => "res_win",
                arena_core::types::MatchResult::Draw => "draw",
            },
            canon::hex(&outcome.final_state_hash),
            outcome.final_scores.0,
            outcome.final_scores.1
        ),
        Some(tag) => format!(
            "verified rounds={} outcome_tag={} last_hash={} score2_enl={} score2_res={}",
            outcome.rounds_completed,
            tag,
            canon::hex(&outcome.final_state_hash),
            outcome.final_scores.0,
            outcome.final_scores.1
        ),
    }
}

/// One broadcast event, D51 kind tags 1..=8 in `EventKind` declaration
/// order (identical to the IARP event tags). Extras per kind: Attacked
/// `d`/`h`; Link* `l`; Field* `t` + `ar` (the exact integer area2 recomputed
/// from the frozen map with the same `geometry::area2` the link graph used
/// at derivation); Reinforcement `i`/`pos`/`n`/`fa` (both-faction: no
/// `f`/`a`).
fn event_json<F>(event: &Event, area_of: &F) -> Json
where
    F: Fn(u32, u32, u32) -> Option<u64>,
{
    use arena_core::codec::faction_tag;
    let faction_value = |faction: Faction| Json::uint(faction_tag(faction) as u64);
    let agent_value = |agent: arena_core::types::AgentId| Json::uint(agent.0 as u64);
    let portal_value = |portal: PortalId| Json::uint(portal.0 as u64);
    let field_triple = |field: &arena_core::types::FieldId| {
        Json::arr(vec![
            Json::uint(field.a.0 as u64),
            Json::uint(field.b.0 as u64),
            Json::uint(field.c.0 as u64),
        ])
    };
    let mut pairs: Vec<(String, Json)> = vec![
        ("s".into(), Json::uint(event.seq)),
        ("r".into(), Json::uint(event.round as u64)),
    ];
    let kind: u8 = match &event.kind {
        EventKind::PortalCaptured {
            faction,
            agent_id,
            portal,
        } => {
            pairs.push(("f".into(), faction_value(*faction)));
            pairs.push(("a".into(), agent_value(*agent_id)));
            pairs.push(("p".into(), portal_value(*portal)));
            1
        }
        EventKind::PortalAttacked {
            faction,
            agent_id,
            portal,
            damage,
            hp_after,
        } => {
            pairs.push(("f".into(), faction_value(*faction)));
            pairs.push(("a".into(), agent_value(*agent_id)));
            pairs.push(("p".into(), portal_value(*portal)));
            pairs.push(("d".into(), Json::uint(*damage as u64)));
            pairs.push(("h".into(), Json::uint(*hp_after as u64)));
            2
        }
        EventKind::PortalNeutralized {
            faction,
            agent_id,
            portal,
        } => {
            pairs.push(("f".into(), faction_value(*faction)));
            pairs.push(("a".into(), agent_value(*agent_id)));
            pairs.push(("p".into(), portal_value(*portal)));
            3
        }
        EventKind::LinkCreated {
            faction,
            agent_id,
            link,
        } => {
            pairs.push(("f".into(), faction_value(*faction)));
            pairs.push(("a".into(), agent_value(*agent_id)));
            pairs.push((
                "l".into(),
                Json::arr(vec![
                    Json::uint(link.min.0 as u64),
                    Json::uint(link.max.0 as u64),
                ]),
            ));
            4
        }
        EventKind::LinkDestroyed {
            faction,
            agent_id,
            link,
        } => {
            pairs.push(("f".into(), faction_value(*faction)));
            pairs.push(("a".into(), agent_value(*agent_id)));
            pairs.push((
                "l".into(),
                Json::arr(vec![
                    Json::uint(link.min.0 as u64),
                    Json::uint(link.max.0 as u64),
                ]),
            ));
            5
        }
        EventKind::FieldCreated {
            faction,
            agent_id,
            field,
        } => {
            pairs.push(("f".into(), faction_value(*faction)));
            pairs.push(("a".into(), agent_value(*agent_id)));
            pairs.push(("t".into(), field_triple(field)));
            pairs.push((
                "ar".into(),
                Json::uint(area_of(field.a.0, field.b.0, field.c.0).unwrap_or(0)),
            ));
            6
        }
        EventKind::FieldDestroyed {
            faction,
            agent_id,
            field,
        } => {
            pairs.push(("f".into(), faction_value(*faction)));
            pairs.push(("a".into(), agent_value(*agent_id)));
            pairs.push(("t".into(), field_triple(field)));
            pairs.push((
                "ar".into(),
                Json::uint(area_of(field.a.0, field.b.0, field.c.0).unwrap_or(0)),
            ));
            7
        }
        EventKind::Reinforcement {
            index,
            position,
            count_per_faction,
            first_agent_id,
        } => {
            pairs.push(("i".into(), Json::uint(*index as u64)));
            pairs.push((
                "pos".into(),
                Json::arr(vec![
                    Json::int(position.x as i64),
                    Json::int(position.y as i64),
                ]),
            ));
            pairs.push(("n".into(), Json::uint(*count_per_faction as u64)));
            pairs.push(("fa".into(), Json::uint(first_agent_id.0 as u64)));
            8
        }
    };
    pairs.insert(2, ("k".into(), Json::uint(kind as u64)));
    Json::Object(pairs)
}

//! Tournament archive (D83): the frozen `tournament.json` config echo,
//! the lossless `index.tsv` (one row per match; parsing reproduces the
//! records exactly — the M6 `matches.tsv` convention), `ratings.tsv`
//! (per scored match pre/post), `standings.txt` (see `report.rs`), and
//! per-match `meta.json` under `matches/m<index>/`. Spawn forfeits carry
//! meta + journal only (no IARP is possible, D67); every match that
//! entered Round 1 has its `replay.iarp` archived.

use std::path::Path;

use crate::tournament::config::{ClientSandbox, TournamentConfig};
use crate::tournament::elo::AppliedRating;
use crate::tournament::outcome::TournamentOutcome;
use crate::tournament::schedule::TournamentItem;

/// Minimal JSON: enough to emit and re-read the frozen echo (objects keep
/// insertion order; numbers stay integers or strings — seeds print as
/// hex strings, so no float machinery at all).
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Json::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(pairs) => Some(pairs),
            _ => None,
        }
    }
}

/// JSON string escaping (control characters as `\u00XX`).
pub fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// `0x%016x` seed formatting (exact u64 echo).
pub fn hex64(value: u64) -> String {
    format!("{value:#018x}")
}

// ---------------------------------------------------------------------------
// tournament.json — the frozen config echo
// ---------------------------------------------------------------------------

/// The `tournament.json` body. Every value here is frozen BEFORE the
/// matches run; the file is what makes the run reproducible and
/// traceable (P5: Map Definition / build version / config traceability).
pub fn tournament_json_text(cfg: &TournamentConfig, items: &[TournamentItem]) -> String {
    let mut maps: Vec<String> = Vec::new();
    let mut seen = 0u32;
    for item in items {
        if item.map_index >= seen {
            seen = item.map_index + 1;
        }
    }
    for k in 0..seen {
        // The map hash of the first item using map k (all items sharing a
        // map index share the hash by construction; the set is also fully
        // listed even when only part of it is scheduled).
        let hash = items
            .iter()
            .find(|item| item.map_index == k)
            .map(|item| item.map_hash.clone())
            .unwrap_or_default();
        maps.push(format!(
            "    {{ \"index\": {}, \"seed\": \"{}\", \"hash\": \"{}\" }}",
            k,
            hex64(cfg.map_seed(k)),
            json_escape(&hash)
        ));
    }
    let entrants: Vec<String> = cfg
        .entrants
        .iter()
        .map(|entrant| {
            let kind = match &entrant.kind {
                crate::tournament::entrant::EntrantKind::External { binary, env } => {
                    let env: Vec<String> = env
                        .iter()
                        .map(|(key, value)| {
                            format!("[\"{}\", \"{}\"]", json_escape(key), json_escape(value))
                        })
                        .collect();
                    format!(
                        "{{ \"mode\": \"external\", \"binary\": \"{}\", \"env\": [{}] }}",
                        json_escape(&binary.to_string_lossy()),
                        env.join(", ")
                    )
                }
                crate::tournament::entrant::EntrantKind::InProcess { bot } => format!(
                    "{{ \"mode\": \"in-process\", \"bot\": \"{}\" }}",
                    bot.tag()
                ),
            };
            let provenance = &entrant.provenance;
            let optional = |value: &Option<String>| match value {
                Some(text) => format!("\"{}\"", json_escape(text)),
                None => "null".to_string(),
            };
            format!(
                "    {{ \"name\": \"{}\", \"kind\": {}, \"provenance\": {{ \"binary_blake3\": {}, \"tree_hash\": {}, \"lock_hash\": {}, \"toolchain_hash\": {}, \"manifest_id\": {}, \"bot_version\": {} }} }}",
                json_escape(&entrant.name),
                kind,
                optional(&provenance.binary_blake3),
                optional(&provenance.tree_hash),
                optional(&provenance.lock_hash),
                optional(&provenance.toolchain_hash),
                optional(&provenance.manifest_id),
                optional(&provenance.bot_version),
            )
        })
        .collect();
    let matchups: Vec<String> = cfg
        .matchups
        .iter()
        .map(|matchup| {
            format!(
                "    {{ \"tag\": \"{}\", \"a\": {}, \"b\": {}, \"count\": {} }}",
                json_escape(&matchup.tag),
                matchup.a,
                matchup.b,
                matchup.count
            )
        })
        .collect();
    let sandbox = match &cfg.sandbox {
        ClientSandbox::Off => "  { \"mode\": \"off\" }".to_string(),
        ClientSandbox::SandboxRunner {
            runner,
            wall_ms,
            address_space_bytes,
        } => format!(
            "  {{ \"mode\": \"sandbox-runner\", \"runner\": \"{}\", \"wall_ms\": {wall_ms}, \"as_bytes\": {address_space_bytes} }}",
            json_escape(&runner.to_string_lossy())
        ),
    };
    format!(
        "{{\n  \"schema\": \"ingress-tournament-v1\",\n  \"name\": \"{}\",\n  \"master_seed\": \"{}\",\n  \"schedule_seed\": \"{}\",\n  \"config_hash\": \"{}\",\n  \"rounds_per_match\": {},\n  \"map_count\": {},\n  \"maps\": [\n{}\n  ],\n  \"elo\": {{ \"k\": {}, \"initial\": {} }},\n  \"entrants\": [\n{}\n  ],\n  \"matchups\": [\n{}\n  ],\n  \"sandbox\": {},\n  \"per_match_budget_ms\": {},\n  \"concurrency\": {}\n}}\n",
        json_escape(&cfg.name),
        hex64(cfg.master_seed),
        hex64(cfg.schedule_seed()),
        format!("{:#018x}", cfg.config_identity()),
        cfg.rounds_per_match,
        cfg.map_count,
        maps.join(",\n"),
        cfg.elo.k,
        cfg.elo.initial,
        entrants.join(",\n"),
        matchups.join(",\n"),
        sandbox,
        cfg.per_match_budget_ms,
        cfg.concurrency,
    )
}

/// The parsed echo (everything the file carries).
#[derive(Clone, Debug, PartialEq)]
pub struct TournamentEcho {
    pub name: String,
    pub master_seed: u64,
    pub schedule_seed: u64,
    pub config_hash: u64,
    pub rounds_per_match: u32,
    pub map_count: u32,
    pub maps: Vec<(u32, u64, String)>,
    pub elo: (u32, i64),
    pub entrants: Vec<crate::tournament::entrant::Entrant>,
    pub matchups: Vec<crate::tournament::config::TMatchup>,
    pub sandbox_mode: String,
    pub per_match_budget_ms: u64,
    pub concurrency: usize,
}

/// Parses `tournament.json` back (the exact grammar `tournament_json_text`
/// emits; a generic-enough mini JSON reader).
pub fn parse_tournament_json(text: &str) -> Result<TournamentEcho, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        position: 0,
    };
    let value = parser.value()?;
    let hex_parse = |value: &Json, what: &str| -> Result<u64, String> {
        value
            .as_str()
            .and_then(|text| u64::from_str_radix(text.trim_start_matches("0x"), 16).ok())
            .ok_or_else(|| format!("{what}: expected 0x-hex string"))
    };
    let name = value
        .get("name")
        .and_then(Json::as_str)
        .ok_or("name")?
        .to_string();
    let master_seed = hex_parse(
        value.get("master_seed").ok_or("master_seed")?,
        "master_seed",
    )?;
    let schedule_seed = hex_parse(
        value.get("schedule_seed").ok_or("schedule_seed")?,
        "schedule_seed",
    )?;
    let config_hash = hex_parse(
        value.get("config_hash").ok_or("config_hash")?,
        "config_hash",
    )?;
    let rounds_per_match = value
        .get("rounds_per_match")
        .and_then(Json::as_int)
        .ok_or("rounds_per_match")? as u32;
    let map_count = value
        .get("map_count")
        .and_then(Json::as_int)
        .ok_or("map_count")? as u32;
    let mut maps = Vec::new();
    for entry in value.get("maps").and_then(Json::as_array).ok_or("maps")? {
        let index = entry
            .get("index")
            .and_then(Json::as_int)
            .ok_or("maps.index")? as u32;
        let seed = hex_parse(entry.get("seed").ok_or("maps.seed")?, "maps.seed")?;
        let hash = entry
            .get("hash")
            .and_then(Json::as_str)
            .ok_or("maps.hash")?
            .to_string();
        maps.push((index, seed, hash));
    }
    let elo_value = value.get("elo").ok_or("elo")?;
    let elo = (
        elo_value.get("k").and_then(Json::as_int).ok_or("elo.k")? as u32,
        elo_value
            .get("initial")
            .and_then(Json::as_int)
            .ok_or("elo.initial")?,
    );
    let mut entrants = Vec::new();
    for entry in value
        .get("entrants")
        .and_then(Json::as_array)
        .ok_or("entrants")?
    {
        let name = entry
            .get("name")
            .and_then(Json::as_str)
            .ok_or("entrants.name")?
            .to_string();
        let kind_value = entry.get("kind").ok_or("entrants.kind")?;
        let mode = kind_value
            .get("mode")
            .and_then(Json::as_str)
            .ok_or("kind.mode")?;
        let kind = if mode == "external" {
            let binary = std::path::PathBuf::from(
                kind_value
                    .get("binary")
                    .and_then(Json::as_str)
                    .ok_or("kind.binary")?,
            );
            let mut env = Vec::new();
            for pair in kind_value
                .get("env")
                .and_then(Json::as_array)
                .ok_or("kind.env")?
            {
                let array = pair.as_array().ok_or("kind.env pair")?;
                if array.len() == 2 {
                    env.push((
                        array[0].as_str().ok_or("env key")?.to_string(),
                        array[1].as_str().ok_or("env value")?.to_string(),
                    ));
                }
            }
            crate::tournament::entrant::EntrantKind::External { binary, env }
        } else {
            let bot = kind_value
                .get("bot")
                .and_then(Json::as_str)
                .ok_or("kind.bot")?;
            crate::tournament::entrant::EntrantKind::InProcess {
                bot: arena_bots::bot::BotKind::from_tag(bot).ok_or("unknown bot tag")?,
            }
        };
        let provenance_value = entry.get("provenance").ok_or("entrants.provenance")?;
        let optional = |key: &str| -> Result<Option<String>, String> {
            match provenance_value.get(key) {
                Some(Json::Str(text)) => Ok(Some(text.clone())),
                _ => Ok(None),
            }
        };
        let provenance = crate::tournament::entrant::EntrantProvenance {
            binary_blake3: optional("binary_blake3")?,
            tree_hash: optional("tree_hash")?,
            lock_hash: optional("lock_hash")?,
            toolchain_hash: optional("toolchain_hash")?,
            manifest_id: optional("manifest_id")?,
            bot_version: optional("bot_version")?,
        };
        entrants.push(crate::tournament::entrant::Entrant {
            name,
            kind,
            provenance,
        });
    }
    let mut matchups = Vec::new();
    for entry in value
        .get("matchups")
        .and_then(Json::as_array)
        .ok_or("matchups")?
    {
        matchups.push(crate::tournament::config::TMatchup {
            tag: entry
                .get("tag")
                .and_then(Json::as_str)
                .ok_or("matchups.tag")?
                .to_string(),
            a: entry.get("a").and_then(Json::as_int).ok_or("matchups.a")? as u32,
            b: entry.get("b").and_then(Json::as_int).ok_or("matchups.b")? as u32,
            count: entry
                .get("count")
                .and_then(Json::as_int)
                .ok_or("matchups.count")? as u32,
        });
    }
    let sandbox_mode = value
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("mode"))
        .and_then(Json::as_str)
        .ok_or("sandbox.mode")?
        .to_string();
    let per_match_budget_ms = value
        .get("per_match_budget_ms")
        .and_then(Json::as_int)
        .ok_or("per_match_budget_ms")? as u64;
    let concurrency = value
        .get("concurrency")
        .and_then(Json::as_int)
        .ok_or("concurrency")? as usize;
    Ok(TournamentEcho {
        name,
        master_seed,
        schedule_seed,
        config_hash,
        rounds_per_match,
        map_count,
        maps,
        elo,
        entrants,
        matchups,
        sandbox_mode,
        per_match_budget_ms,
        concurrency,
    })
}

/// Parses any JSON document (the exact grammar the emitter uses, plus
/// generic objects/arrays/integers/strings/bools/null).
pub fn parse_json(text: &str) -> Result<Json, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        position: 0,
    };
    let value = parser.value()?;
    parser.skip_ws();
    if parser.position != parser.bytes.len() {
        return Err("trailing bytes after the JSON document".to_string());
    }
    Ok(value)
}

/// The mini JSON reader (whitespace-tolerant; strings support the escapes
/// `json_escape` emits).
struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(byte) = self.bytes.get(self.position) {
            if byte.is_ascii_whitespace() {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.bytes.get(self.position) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(byte) if byte.is_ascii_digit() || *byte == b'-' => self.number(),
            other => Err(format!("unexpected byte {other:?} at {}", self.position)),
        }
    }

    fn literal(&mut self, text: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.position..].starts_with(text.as_bytes()) {
            self.position += text.len();
            Ok(value)
        } else {
            Err(format!("bad literal at {}", self.position))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.position;
        if self.bytes.get(self.position) == Some(&b'-') {
            self.position += 1;
        }
        while let Some(byte) = self.bytes.get(self.position) {
            if byte.is_ascii_digit() {
                self.position += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.position]).map_err(|_| "utf8")?;
        text.parse::<i64>()
            .map(Json::Int)
            .map_err(|error| error.to_string())
    }

    fn string(&mut self) -> Result<String, String> {
        self.position += 1; // opening quote
        let mut out = String::new();
        loop {
            match self.bytes.get(self.position) {
                Some(b'"') => {
                    self.position += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.position += 1;
                    match self.bytes.get(self.position) {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => {
                            let hex = std::str::from_utf8(
                                self.bytes
                                    .get(self.position + 1..self.position + 5)
                                    .ok_or("truncated \\u escape")?,
                            )
                            .map_err(|_| "utf8")?;
                            let code = u32::from_str_radix(hex, 16).map_err(|_| "bad \\u")?;
                            out.push(char::from_u32(code).ok_or("bad \\u code")?);
                            self.position += 4;
                        }
                        other => return Err(format!("bad escape {other:?}")),
                    }
                    self.position += 1;
                }
                Some(_byte) => {
                    let text =
                        std::str::from_utf8(&self.bytes[self.position..]).map_err(|_| "utf8")?;
                    let character = text.chars().next().ok_or("eof in string")?;
                    out.push(character);
                    self.position += character.len_utf8();
                }
                None => return Err("unterminated string".to_string()),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.position += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.position) == Some(&b']') {
            self.position += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.bytes.get(self.position) {
                Some(b',') => self.position += 1,
                Some(b']') => {
                    self.position += 1;
                    return Ok(Json::Array(items));
                }
                other => return Err(format!("expected , or ], got {other:?}")),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.position += 1;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.position) == Some(&b'}') {
            self.position += 1;
            return Ok(Json::Object(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            if self.bytes.get(self.position) != Some(&b':') {
                return Err(format!("expected : at {}", self.position));
            }
            self.position += 1;
            let value = self.value()?;
            pairs.push((key, value));
            self.skip_ws();
            match self.bytes.get(self.position) {
                Some(b',') => self.position += 1,
                Some(b'}') => {
                    self.position += 1;
                    return Ok(Json::Object(pairs));
                }
                other => return Err(format!("expected , or }}, got {other:?}")),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// index.tsv — the lossless per-match table
// ---------------------------------------------------------------------------

/// One `index.tsv` row (round-trips exactly with `index_record_to_row`).
#[derive(Clone, Debug, PartialEq)]
pub struct IndexRecord {
    pub index: u64,
    pub tag: String,
    pub j: u32,
    pub entrant_a: String,
    pub entrant_b: String,
    pub a_is_enl: bool,
    pub map_index: u32,
    pub map_seed: u64,
    pub map_hash: String,
    pub game_seed: u64,
    pub client_seed: u64,
    pub schedule_seed: u64,
    pub config_hash: u64,
    pub outcome: String,
    pub stage: String,
    pub winner: String,
    pub score_enl: u128,
    pub score_res: u128,
    pub final_state_hash: String,
    pub rounds_completed: u32,
    pub replay: String,
    pub prov_a: String,
    pub prov_b: String,
    pub elapsed_ms: u64,
}

/// The fixed `index.tsv` column order (frozen with r1).
pub fn index_columns() -> Vec<String> {
    [
        "index",
        "tag",
        "j",
        "entrant_a",
        "entrant_b",
        "a_is_enl",
        "map_index",
        "map_seed",
        "map_hash",
        "game_seed",
        "client_seed",
        "schedule_seed",
        "config_hash",
        "outcome",
        "stage",
        "winner",
        "score_enl",
        "score_res",
        "final_state_hash",
        "rounds_completed",
        "replay",
        "prov_a",
        "prov_b",
        "elapsed_ms",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Builds one row from the item, outcome, and entrant names/provenance.
#[allow(clippy::too_many_arguments)]
pub fn index_record(
    item: &TournamentItem,
    outcome: &TournamentOutcome,
    cfg: &TournamentConfig,
    replay: Option<&str>,
    elapsed_ms: u64,
) -> IndexRecord {
    let name = |id: u32| cfg.entrant(id).name.clone();
    let winner = match outcome {
        TournamentOutcome::Normal { .. } => outcome
            .normal_winner(item.a, item.b, item.a_is_enl)
            .map(name)
            .unwrap_or_else(|| "-".to_string()),
        TournamentOutcome::TechnicalWin { winner, .. } => name(*winner),
        TournamentOutcome::Unscored { .. } => "-".to_string(),
    };
    let (score_enl, score_res, final_hash, rounds) = match outcome {
        TournamentOutcome::Normal {
            score_enl,
            score_res,
            rounds_completed,
            final_state_hash,
            ..
        } => (
            *score_enl,
            *score_res,
            arena_replay::canon::hex(final_state_hash),
            *rounds_completed,
        ),
        _ => (0, 0, "-".to_string(), 0),
    };
    IndexRecord {
        index: item.index,
        tag: item.tag.clone(),
        j: item.j,
        entrant_a: name(item.a),
        entrant_b: name(item.b),
        a_is_enl: item.a_is_enl,
        map_index: item.map_index,
        map_seed: item.map_seed,
        map_hash: item.map_hash.clone(),
        game_seed: item.game_seed,
        client_seed: item.client_seed,
        schedule_seed: item.schedule_seed,
        config_hash: item.config_hash,
        outcome: outcome.outcome_tag().to_string(),
        stage: outcome
            .stage()
            .map(|s| s.tag().to_string())
            .unwrap_or("-".into()),
        winner,
        score_enl,
        score_res,
        final_state_hash: final_hash,
        rounds_completed: rounds,
        replay: replay.unwrap_or("-").to_string(),
        prov_a: cfg
            .entrant(item.a)
            .provenance
            .id_tag(&cfg.entrant(item.a).kind),
        prov_b: cfg
            .entrant(item.b)
            .provenance
            .id_tag(&cfg.entrant(item.b).kind),
        elapsed_ms,
    }
}

fn index_row(record: &IndexRecord) -> Vec<String> {
    vec![
        record.index.to_string(),
        record.tag.clone(),
        record.j.to_string(),
        record.entrant_a.clone(),
        record.entrant_b.clone(),
        u8::from(record.a_is_enl).to_string(),
        record.map_index.to_string(),
        record.map_seed.to_string(),
        record.map_hash.clone(),
        record.game_seed.to_string(),
        record.client_seed.to_string(),
        record.schedule_seed.to_string(),
        format!("{:016x}", record.config_hash),
        record.outcome.clone(),
        record.stage.clone(),
        record.winner.clone(),
        record.score_enl.to_string(),
        record.score_res.to_string(),
        record.final_state_hash.clone(),
        record.rounds_completed.to_string(),
        record.replay.clone(),
        record.prov_a.clone(),
        record.prov_b.clone(),
        record.elapsed_ms.to_string(),
    ]
}

/// The whole `index.tsv` document.
pub fn index_records_to_tsv(records: &[IndexRecord]) -> String {
    let mut text = String::new();
    text.push_str(&index_columns().join("\t"));
    text.push('\n');
    for record in records {
        text.push_str(&index_row(record).join("\t"));
        text.push('\n');
    }
    text
}

/// Parses `index.tsv` back into records (exact round-trip).
pub fn parse_index_tsv(text: &str) -> Result<Vec<IndexRecord>, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty document")?.split('\t').collect();
    let expected = index_columns();
    if header != expected.iter().map(|s| s.as_str()).collect::<Vec<_>>() {
        return Err("header mismatch".to_string());
    }
    let mut records = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let values: Vec<&str> = line.split('\t').collect();
        if values.len() != expected.len() {
            return Err(format!("row width {} != {}", values.len(), expected.len()));
        }
        let map: std::collections::BTreeMap<&str, &str> = header
            .iter()
            .zip(values.iter())
            .map(|(k, v)| (*k, *v))
            .collect();
        let str_field = |name: &str| map.get(name).copied().unwrap_or("").to_string();
        let u_of = |name: &str| -> Result<u64, String> {
            map.get(name)
                .copied()
                .unwrap_or("0")
                .parse()
                .map_err(|error| format!("{name}: {error}"))
        };
        records.push(IndexRecord {
            index: u_of("index")?,
            tag: str_field("tag"),
            j: u_of("j")? as u32,
            entrant_a: str_field("entrant_a"),
            entrant_b: str_field("entrant_b"),
            a_is_enl: map.get("a_is_enl").copied() == Some("1"),
            map_index: u_of("map_index")? as u32,
            map_seed: u_of("map_seed")?,
            map_hash: str_field("map_hash"),
            game_seed: u_of("game_seed")?,
            client_seed: u_of("client_seed")?,
            schedule_seed: u_of("schedule_seed")?,
            config_hash: u64::from_str_radix(map.get("config_hash").copied().unwrap_or("0"), 16)
                .map_err(|error| format!("config_hash: {error}"))?,
            outcome: str_field("outcome"),
            stage: str_field("stage"),
            winner: str_field("winner"),
            score_enl: u_of("score_enl")? as u128,
            score_res: u_of("score_res")? as u128,
            final_state_hash: str_field("final_state_hash"),
            rounds_completed: u_of("rounds_completed")? as u32,
            replay: str_field("replay"),
            prov_a: str_field("prov_a"),
            prov_b: str_field("prov_b"),
            elapsed_ms: u_of("elapsed_ms")?,
        });
    }
    Ok(records)
}

// ---------------------------------------------------------------------------
// ratings.tsv
// ---------------------------------------------------------------------------

/// The `ratings.tsv` body: one row per SCORED match in schedule order.
pub fn ratings_tsv(cfg: &TournamentConfig, ratings: &[AppliedRating]) -> String {
    let mut text = String::new();
    text.push_str("index\tentrant_a\tentrant_b\tscore_a\tpre_a\tpost_a\tpre_b\tpost_b\n");
    for applied in ratings {
        let score = match applied.score_a_milli {
            1000 => "1",
            500 => "0.5",
            _ => "0",
        };
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            applied.index,
            cfg.entrant(applied.entrant_a).name,
            cfg.entrant(applied.entrant_b).name,
            score,
            applied.pre_a,
            applied.post_a,
            applied.pre_b,
            applied.post_b,
        ));
    }
    text
}

/// Parses `ratings.tsv` back into `(index, score_a_milli, pre_a, post_a,
/// pre_b, post_b)` rows.
pub fn parse_ratings_tsv(text: &str) -> Result<Vec<(u64, i64, i64, i64, i64, i64)>, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty")?.split('\t').collect();
    if header
        != [
            "index",
            "entrant_a",
            "entrant_b",
            "score_a",
            "pre_a",
            "post_a",
            "pre_b",
            "post_b",
        ]
    {
        return Err("header mismatch".to_string());
    }
    let mut rows = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let values: Vec<&str> = line.split('\t').collect();
        if values.len() != 8 {
            return Err(format!("row width {} != 8", values.len()));
        }
        let index: u64 = values[0].parse::<u64>().map_err(|e| e.to_string())?;
        let score = match values[3] {
            "1" => 1000i64,
            "0.5" => 500,
            "0" => 0,
            other => return Err(format!("bad score {other:?}")),
        };
        let number = |slot: usize| -> Result<i64, String> {
            values[slot].parse::<i64>().map_err(|e| e.to_string())
        };
        rows.push((index, score, number(4)?, number(5)?, number(6)?, number(7)?));
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// meta.json — per-match outcome metadata
// ---------------------------------------------------------------------------

/// The per-match `meta.json` body (outcome, stage/private reason, seeds,
/// factions, provenance ids, elapsed metadata).
pub fn meta_json_text(
    item: &TournamentItem,
    outcome: &TournamentOutcome,
    cfg: &TournamentConfig,
    replay: Option<&str>,
    elapsed_ms: u64,
    budget_exceeded: bool,
    connect_order_verified: bool,
) -> String {
    let winner = match outcome {
        TournamentOutcome::Normal { .. } => outcome
            .normal_winner(item.a, item.b, item.a_is_enl)
            .map(|id| cfg.entrant(id).name.clone())
            .unwrap_or_default(),
        TournamentOutcome::TechnicalWin { winner, .. } => cfg.entrant(*winner).name.clone(),
        TournamentOutcome::Unscored { .. } => String::new(),
    };
    let (score_enl, score_res, final_hash, rounds) = match outcome {
        TournamentOutcome::Normal {
            score_enl,
            score_res,
            rounds_completed,
            final_state_hash,
            ..
        } => (
            (*score_enl).to_string(),
            (*score_res).to_string(),
            arena_replay::canon::hex(final_state_hash),
            rounds_completed.to_string(),
        ),
        _ => (
            '0'.to_string(),
            '0'.to_string(),
            "-".to_string(),
            "0".to_string(),
        ),
    };
    format!(
        "{{\n  \"schema\": \"ingress-tournament-match-v1\",\n  \"index\": {},\n  \"tag\": \"{}\",\n  \"j\": {},\n  \"entrant_a\": \"{}\",\n  \"entrant_b\": \"{}\",\n  \"a_is_enl\": {},\n  \"outcome\": \"{}\",\n  \"stage\": \"{}\",\n  \"winner\": \"{}\",\n  \"reason\": \"{}\",\n  \"score_enl\": {},\n  \"score_res\": {},\n  \"final_state_hash\": \"{}\",\n  \"rounds_completed\": {},\n  \"map_index\": {},\n  \"map_seed\": \"{}\",\n  \"map_hash\": \"{}\",\n  \"game_seed\": \"{}\",\n  \"client_seed\": \"{}\",\n  \"schedule_seed\": \"{}\",\n  \"config_hash\": \"{}\",\n  \"replay\": \"{}\",\n  \"prov_a\": \"{}\",\n  \"prov_b\": \"{}\",\n  \"budget_exceeded\": {},\n  \"connect_order_verified\": {},\n  \"elapsed_ms\": {}\n}}\n",
        item.index,
        json_escape(&item.tag),
        item.j,
        json_escape(&cfg.entrant(item.a).name),
        json_escape(&cfg.entrant(item.b).name),
        item.a_is_enl,
        outcome.outcome_tag(),
        outcome.stage().map(|s| s.tag()).unwrap_or("-"),
        json_escape(&winner),
        json_escape(outcome.reason().unwrap_or("")),
        score_enl,
        score_res,
        final_hash,
        rounds,
        item.map_index,
        hex64(item.map_seed),
        json_escape(&item.map_hash),
        hex64(item.game_seed),
        hex64(item.client_seed),
        hex64(item.schedule_seed),
        format!("{:#018x}", item.config_hash),
        json_escape(replay.unwrap_or("-")),
        json_escape(
            &cfg.entrant(item.a)
                .provenance
                .id_tag(&cfg.entrant(item.a).kind)
        ),
        json_escape(
            &cfg.entrant(item.b)
                .provenance
                .id_tag(&cfg.entrant(item.b).kind)
        ),
        budget_exceeded,
        connect_order_verified,
        elapsed_ms,
    )
}

/// Writes the whole archive except `standings.txt` (report.rs) — the
/// config echo FIRST (a crashed run still leaves the frozen inputs),
/// then per-match `meta.json`, `index.tsv`, and `ratings.tsv`.
pub fn write_archive(
    root: &Path,
    cfg: &TournamentConfig,
    run: &crate::tournament::run::TournamentRun,
) -> std::io::Result<Vec<IndexRecord>> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("tournament.json"),
        tournament_json_text(cfg, &run.items),
    )?;
    let mut records = Vec::with_capacity(run.items.len());
    for (((item, outcome), (replay, elapsed)), (budget, verified)) in run
        .items
        .iter()
        .zip(run.outcomes.iter())
        .zip(run.replay_rel.iter().zip(run.elapsed_ms.iter()))
        .zip(
            run.budget_exceeded
                .iter()
                .zip(run.connect_order_verified.iter()),
        )
    {
        let match_dir = root.join("matches").join(format!("m{}", item.index));
        std::fs::create_dir_all(&match_dir)?;
        std::fs::write(
            match_dir.join("meta.json"),
            meta_json_text(
                item,
                outcome,
                cfg,
                replay.as_deref(),
                *elapsed,
                *budget,
                *verified,
            ),
        )?;
        records.push(index_record(
            item,
            outcome,
            cfg,
            replay.as_deref(),
            *elapsed,
        ));
    }
    std::fs::write(root.join("index.tsv"), index_records_to_tsv(&records))?;
    std::fs::write(root.join("ratings.tsv"), ratings_tsv(cfg, &run.ratings))?;
    Ok(records)
}

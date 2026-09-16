//! Incremental local discovery. Network writes never hold a verifier lock.
use arena_runner::tournament::{
    archive::Json,
    monitor::{ArchiveMonitor, MatchView, Snapshot, guarded_path},
    outcome::TournamentOutcome,
};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn object(fields: Vec<(&str, Json)>) -> Json {
    Json::Object(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
pub fn text(s: impl ToString) -> Json {
    Json::Str(s.to_string())
}
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Clone)]
pub struct Event {
    pub id: String,
    pub label: String,
    pub error: Option<String>,
    pub snapshot: Option<Arc<Snapshot>>,
    pub reader: Option<Arc<Mutex<ArchiveMonitor>>>,
}
#[derive(Clone, Default)]
pub struct Published {
    pub scanned_unix_ms: u64,
    pub events: Vec<Event>,
    pub errors: Vec<String>,
}
impl Published {
    pub fn list(&self) -> Json {
        object(vec![
            ("schema", text("arena-catalog-v1")),
            ("scanned_unix_ms", Json::Int(self.scanned_unix_ms as i64)),
            (
                "errors",
                Json::Array(self.errors.iter().map(text).collect()),
            ),
            (
                "events",
                Json::Array(
                    self.events
                        .iter()
                        .map(|e| {
                            let mut summary = e.base();
                            if let Some(s) = &e.snapshot {
                                summary.extend([
                                    ("name", text(&s.name)),
                                    ("identity", text(&s.identity)),
                                    ("total", Json::Int(s.matches.len() as i64)),
                                    (
                                        "completed",
                                        Json::Int(
                                            s.matches
                                                .iter()
                                                .filter(|m| m.state() == "completed")
                                                .count()
                                                as i64,
                                        ),
                                    ),
                                    ("scanned_unix_ms", Json::Int(s.scanned_unix_ms as i64)),
                                ]);
                            }
                            object(summary)
                        })
                        .collect(),
                ),
            ),
        ])
    }
}
impl Event {
    fn base(&self) -> Vec<(&str, Json)> {
        vec![
            ("id", text(&self.id)),
            ("label", text(&self.label)),
            ("error", self.error.as_ref().map(text).unwrap_or(Json::Null)),
            ("stale", Json::Bool(self.error.is_some())),
        ]
    }
    pub fn detail(&self, query: &Query) -> Json {
        let Some(snapshot) = &self.snapshot else {
            return object(self.base());
        };
        let names: Vec<_> = snapshot
            .entrants
            .iter()
            .map(|e| {
                format!(
                    "{} {} {}",
                    e.name,
                    e.registered_id,
                    e.version.as_deref().unwrap_or("")
                )
                .to_lowercase()
            })
            .collect();
        let mut counts = BTreeMap::new();
        for m in &snapshot.matches {
            *counts.entry(m.state()).or_insert(0i64) += 1;
        }
        let mut rows: Vec<_> = snapshot
            .matches
            .iter()
            .filter(|m| {
                let a = m.item.enl_entrant() as usize;
                let b = m.item.res_entrant() as usize;
                (query.q.is_empty() || names[a].contains(&query.q) || names[b].contains(&query.q))
                    && query
                        .entrant
                        .is_none_or(|id| m.item.a == id || m.item.b == id)
                    && query
                        .opponent
                        .is_none_or(|id| m.item.a == id || m.item.b == id)
                    && query.map.is_none_or(|i| m.item.map_index as i64 == i)
                    && (query.state.is_empty() || m.state() == query.state)
                    && query.matches_outcome(m)
                    && query.matches_attention(m)
            })
            .collect();
        if query.order == "latest" {
            rows.sort_by_key(|m| {
                std::cmp::Reverse((
                    m.attempts
                        .last()
                        .and_then(|a| a.finished_unix_ms)
                        .unwrap_or(0),
                    m.item.index,
                ))
            });
        } else if query.order != "schedule" {
            rows.sort_by_key(|m| {
                let players = query.players(m);
                let metric = match query.order.as_str() {
                    "score" => players.iter().map(|s| s.score2).max(),
                    "idle" => players
                        .iter()
                        .filter(|s| s.agent_rounds > 0)
                        .map(|s| {
                            u128::from(
                                (s.pass_slots + s.not_submitted_slots) * 10000 / s.agent_rounds,
                            )
                        })
                        .max(),
                    "destruction" => players
                        .iter()
                        .map(|s| {
                            u128::from(s.portals_destroyed + s.links_destroyed + s.fields_destroyed)
                        })
                        .max(),
                    _ => None,
                };
                std::cmp::Reverse((metric, m.item.index))
            });
        }
        let matched = rows.len();
        let mut response =
            snapshot.json_matches(rows.into_iter().skip(query.offset).take(query.limit));
        // JSON is assembled only at the HTTP boundary; filtering and caching
        // use the official typed snapshot rather than navigating JSON fields.
        if let Json::Object(fields) = &mut response {
            fields.extend(self.base().into_iter().map(|(k, v)| (k.into(), v)));
            fields.extend([
                ("matched".into(), Json::Int(matched as i64)),
                ("offset".into(), Json::Int(query.offset as i64)),
                ("limit".into(), Json::Int(query.limit as i64)),
                ("matchups".into(), matchups(snapshot)),
                (
                    "counts".into(),
                    object(counts.into_iter().map(|(s, n)| (s, Json::Int(n))).collect()),
                ),
            ]);
        }
        response
    }
    pub fn replay(&self, item: u64, attempt: &str) -> io::Result<Vec<u8>> {
        if self.error.is_some() {
            return Err(io::Error::other("archive is unavailable or changed"));
        }
        self.reader
            .as_ref()
            .ok_or_else(|| io::Error::other("archive is not ready"))?
            .lock()
            .map_err(|_| io::Error::other("reader failed"))?
            .replay(item, attempt)
    }
}

pub struct Query {
    pub offset: usize,
    pub limit: usize,
    pub q: String,
    pub map: Option<i64>,
    pub entrant: Option<u32>,
    pub opponent: Option<u32>,
    pub state: String,
    pub outcome: String,
    pub order: String,
    pub attention: String,
}
impl Query {
    fn players<'a>(
        &self,
        m: &'a MatchView,
    ) -> Vec<&'a arena_runner::tournament::monitor_stats::PlayerStatistics> {
        let Some(stats) = m.attempts.last().and_then(|a| a.statistics.as_ref()) else {
            return vec![];
        };
        match self.entrant {
            Some(id) if id == m.item.enl_entrant() => vec![&stats.enl],
            Some(_) => vec![&stats.res],
            None => vec![&stats.enl, &stats.res],
        }
    }
    fn matches_attention(&self, m: &MatchView) -> bool {
        if self.attention.is_empty() {
            return true;
        }
        self.players(m)
            .iter()
            .any(|s| match self.attention.as_str() {
                "high_idle" => {
                    s.agent_rounds > 0
                        && (s.pass_slots + s.not_submitted_slots) * 2 >= s.agent_rounds
                }
                "zero_score" => {
                    s.score2 == 0
                        && m.attempts
                            .last()
                            .and_then(|a| a.statistics.as_ref())
                            .is_some_and(|s| s.rounds > 0)
                }
                "submission_issue" => s.not_submitted_slots > 0 || s.rejected_slots > 0,
                _ => false,
            })
    }
    fn matches_outcome(&self, m: &MatchView) -> bool {
        if self.outcome.is_empty() {
            return true;
        }
        let Some(result) = m.result() else {
            return false;
        };
        match self.outcome.as_str() {
            "win" | "loss" | "technical_win" | "technical_loss" if self.entrant.is_some() => {
                let winner = match &result.outcome {
                    TournamentOutcome::TechnicalWin { winner, .. }
                        if self.outcome.starts_with("technical_") =>
                    {
                        Some(*winner)
                    }
                    outcome @ TournamentOutcome::Normal { .. }
                        if !self.outcome.starts_with("technical_") =>
                    {
                        outcome.normal_winner(m.item.a, m.item.b, m.item.a_is_enl)
                    }
                    _ => None,
                };
                winner.is_some_and(|winner| {
                    (Some(winner) == self.entrant) == self.outcome.ends_with("win")
                })
            }
            _ => result.outcome.outcome_tag() == self.outcome,
        }
    }
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut q = Self {
            offset: 0,
            limit: 50,
            q: String::new(),
            map: None,
            entrant: None,
            opponent: None,
            state: String::new(),
            outcome: String::new(),
            order: "latest".into(),
            attention: String::new(),
        };
        let mut seen = std::collections::BTreeSet::new();
        for pair in raw.split('&').filter(|s| !s.is_empty()) {
            let (key, v) = pair.split_once('=').ok_or("invalid query")?;
            if !seen.insert(key) {
                return Err("duplicate query option".into());
            }
            let value = decode(v)?;
            match key {
                "offset" => q.offset = value.parse().map_err(|_| "invalid offset")?,
                "limit" => {
                    q.limit = value.parse().map_err(|_| "invalid limit")?;
                    if !(1..=200).contains(&q.limit) {
                        return Err("limit must be 1..200".into());
                    }
                }
                "q" => q.q = value.to_lowercase(),
                "map" if !value.is_empty() => {
                    q.map = Some(value.parse().map_err(|_| "invalid map")?)
                }
                "map" => {}
                "entrant" if !value.is_empty() => {
                    q.entrant = Some(value.parse().map_err(|_| "invalid entrant")?)
                }
                "opponent" if !value.is_empty() => {
                    q.opponent = Some(value.parse().map_err(|_| "invalid opponent")?)
                }
                "entrant" | "opponent" => {}
                "state" => q.state = value,
                "outcome" => q.outcome = value,
                "attention"
                    if matches!(
                        value.as_str(),
                        "" | "high_idle" | "zero_score" | "submission_issue"
                    ) =>
                {
                    q.attention = value
                }
                "order" => {
                    q.order = match value.as_str() {
                        "latest" | "schedule" | "score" | "idle" | "destruction" => value,
                        _ => return Err("invalid order".into()),
                    }
                }
                _ => return Err("unknown query option".into()),
            }
        }
        if q.entrant.is_some() && q.entrant == q.opponent {
            return Err("entrant and opponent must differ".into());
        }
        if matches!(q.outcome.as_str(), "win" | "loss" | "technical_loss") && q.entrant.is_none() {
            return Err("entrant is required for player-relative outcome".into());
        }
        Ok(q)
    }
}

/// One logical match contributes once, using the same latest attempt as rankings.
/// Pair order is canonical; faction swaps do not create another matrix cell.
#[derive(Default)]
struct Matchup {
    total: u64,
    a_wins: u64,
    b_wins: u64,
    draws: u64,
    a_technical_wins: u64,
    b_technical_wins: u64,
    pending: u64,
    faults: u64,
    unscored: u64,
}
impl Matchup {
    fn apply(&mut self, m: &MatchView, a: u32) {
        self.total += 1;
        match m.state() {
            "corrupt" | "interrupted" | "infrastructure_fault" => self.faults += 1,
            "completed" => match m.result().map(|r| &r.outcome) {
                Some(outcome @ TournamentOutcome::Normal { .. }) => {
                    match outcome.normal_winner(m.item.a, m.item.b, m.item.a_is_enl) {
                        Some(winner) if winner == a => self.a_wins += 1,
                        Some(_) => self.b_wins += 1,
                        None => self.draws += 1,
                    }
                }
                Some(TournamentOutcome::TechnicalWin { winner, .. }) if *winner == a => {
                    self.a_technical_wins += 1
                }
                Some(TournamentOutcome::TechnicalWin { .. }) => self.b_technical_wins += 1,
                Some(TournamentOutcome::Unscored { .. }) => self.unscored += 1,
                None => self.faults += 1,
            },
            _ => self.pending += 1,
        }
    }
    fn json(&self, a: u32, b: u32) -> Json {
        object(
            [
                ("a", u64::from(a)),
                ("b", u64::from(b)),
                ("total", self.total),
                ("a_wins", self.a_wins),
                ("b_wins", self.b_wins),
                ("draws", self.draws),
                ("a_technical_wins", self.a_technical_wins),
                ("b_technical_wins", self.b_technical_wins),
                ("pending", self.pending),
                ("faults", self.faults),
                ("unscored", self.unscored),
            ]
            .into_iter()
            .map(|(key, value)| (key, Json::Int(value as i64)))
            .collect(),
        )
    }
}
fn matchups(snapshot: &Snapshot) -> Json {
    let mut pairs = BTreeMap::<(u32, u32), Matchup>::new();
    for m in &snapshot.matches {
        let a = m.item.a.min(m.item.b);
        let b = m.item.a.max(m.item.b);
        pairs.entry((a, b)).or_default().apply(m, a);
    }
    Json::Array(
        pairs
            .into_iter()
            .map(|((a, b), pair)| pair.json(a, b))
            .collect(),
    )
}
fn decode(s: &str) -> Result<String, String> {
    if s.len() > 2048 {
        return Err("query value too long".into());
    }
    let mut bytes = Vec::new();
    let mut it = s.bytes();
    while let Some(c) = it.next() {
        bytes.push(match c {
            b'+' => b' ',
            b'%' => {
                let a = it
                    .next()
                    .and_then(|b| (b as char).to_digit(16))
                    .ok_or("invalid escape")?;
                let b = it
                    .next()
                    .and_then(|b| (b as char).to_digit(16))
                    .ok_or("invalid escape")?;
                (a * 16 + b) as u8
            }
            _ => c,
        });
    }
    String::from_utf8(bytes).map_err(|_| "invalid UTF-8 query".into())
}

pub struct Catalog {
    roots: Vec<PathBuf>,
    events: BTreeMap<PathBuf, Event>,
}
impl Catalog {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            events: BTreeMap::new(),
        }
    }
    pub fn scan(&mut self) -> Published {
        let mut paths = Vec::new();
        let mut errors = Vec::new();
        let mut unavailable = BTreeMap::new();
        for root in &self.roots {
            let discover = || -> io::Result<Vec<PathBuf>> {
                // One event or immediate child events. Never recursive discovery.
                if root.join("plan").try_exists()? || self.events.contains_key(root) {
                    return Ok(vec![root.clone()]);
                }
                let mut found = Vec::new();
                for entry in fs::read_dir(root)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir()
                        && (entry.path().join("plan").try_exists()?
                            || self.events.contains_key(&entry.path()))
                    {
                        guarded_path(root, Path::new(&entry.file_name()))?;
                        found.push(entry.path());
                    }
                }
                Ok(found)
            };
            match discover() {
                Ok(p) => paths.extend(p),
                Err(e) => {
                    let message = format!("{}: {e}", root.display());
                    errors.push(message.clone());
                    unavailable.insert(root.clone(), message);
                    paths.extend(self.events.keys().filter(|p| p.starts_with(root)).cloned());
                }
            }
        }
        paths.sort();
        paths.dedup();
        self.events.retain(|p, _| paths.contains(p));
        for path in paths {
            let entry = self.events.entry(path.clone()).or_insert_with(|| Event {
                id: blake3::hash(path.as_os_str().as_encoded_bytes()).to_hex()[..24].into(),
                label: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                error: None,
                snapshot: None,
                reader: None,
            });
            if let Some((_, error)) = unavailable.iter().find(|(root, _)| path.starts_with(root)) {
                entry.error = Some(error.clone());
                continue;
            }
            let scan = || -> io::Result<(Arc<Mutex<ArchiveMonitor>>, Snapshot)> {
                let reader = match &entry.reader {
                    Some(r) => r.clone(),
                    None => Arc::new(Mutex::new(ArchiveMonitor::open(&path)?)),
                };
                let snapshot = reader
                    .lock()
                    .map_err(|_| io::Error::other("reader failed"))?
                    .snapshot()?;
                Ok((reader, snapshot))
            };
            match scan() {
                Ok((reader, snapshot)) => {
                    entry.reader = Some(reader);
                    entry.snapshot = Some(Arc::new(snapshot));
                    entry.error = None;
                }
                Err(e) => entry.error = Some(e.to_string()),
            }
        }
        Published {
            scanned_unix_ms: now_ms(),
            events: self.events.values().cloned().collect(),
            errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::types::MatchResult;
    use arena_runner::tournament::{
        exec_external::ExternalResult,
        monitor::{AttemptState, AttemptView, Participant},
        outcome::Stage,
        schedule::TournamentItem,
    };

    fn attempt(state: AttemptState, outcome: Option<TournamentOutcome>) -> AttemptView {
        AttemptView {
            number: "0001".into(),
            attempt_reason: None,
            state,
            error: None,
            result: outcome.map(|outcome| ExternalResult {
                outcome,
                replay_rel: None,
                sandbox_reports: vec![],
                budget_exceeded: false,
                connect_order_verified: true,
                elapsed_ms: 0,
            }),
            finished_unix_ms: None,
            statistics: None,
        }
    }
    fn normal(result: MatchResult) -> TournamentOutcome {
        TournamentOutcome::Normal {
            result,
            score_enl: 0,
            score_res: 0,
            rounds_completed: 0,
            final_state_hash: [0; 32],
        }
    }
    fn item(index: u64, a: u32, b: u32, a_is_enl: bool, attempts: Vec<AttemptView>) -> MatchView {
        MatchView {
            item: TournamentItem {
                index,
                a,
                b,
                a_is_enl,
                tag: String::new(),
                j: 0,
                map_index: 0,
                map_seed: 0,
                map_hash: String::new(),
                game_seed: 0,
                client_seed: 0,
                schedule_seed: 0,
                config_hash: 0,
            },
            attempts,
        }
    }
    fn value(json: &Json, key: &str) -> i64 {
        json.get(key).unwrap().as_int().unwrap()
    }

    #[test]
    fn matrix_covers_all_pages_and_maps_faction_swaps_without_counting_old_attempts() {
        let mut matches: Vec<_> = (0..55)
            .map(|i| {
                item(
                    i,
                    0,
                    1,
                    i % 2 == 0,
                    vec![attempt(
                        AttemptState::Completed,
                        Some(normal(MatchResult::EnlWin)),
                    )],
                )
            })
            .collect();
        // Latest attempt supersedes an earlier completed result, even while pending.
        matches.push(item(
            55,
            0,
            1,
            true,
            vec![
                attempt(AttemptState::Completed, Some(normal(MatchResult::EnlWin))),
                attempt(AttemptState::RecordedRunning, None),
            ],
        ));
        matches.push(item(
            56,
            1,
            0,
            false,
            vec![attempt(
                AttemptState::Completed,
                Some(normal(MatchResult::Draw)),
            )],
        ));
        matches.push(item(
            57,
            0,
            1,
            true,
            vec![attempt(
                AttemptState::Completed,
                Some(TournamentOutcome::TechnicalWin {
                    winner: 1,
                    stage: Stage::ClientForfeit,
                    reason: String::new(),
                }),
            )],
        ));
        matches.push(item(
            58,
            0,
            1,
            true,
            vec![attempt(
                AttemptState::Completed,
                Some(TournamentOutcome::Unscored {
                    stage: Stage::SpawnForfeit,
                    reason: String::new(),
                }),
            )],
        ));
        for (i, state) in [
            AttemptState::Corrupt,
            AttemptState::InfrastructureFault,
            AttemptState::Interrupted,
        ]
        .into_iter()
        .enumerate()
        {
            matches.push(item(59 + i as u64, 0, 1, true, vec![attempt(state, None)]));
        }
        matches.push(item(62, 0, 2, true, vec![]));
        matches.push(item(63, 1, 2, false, vec![]));
        let event = Event {
            id: "test".into(),
            label: "test".into(),
            error: None,
            reader: None,
            snapshot: Some(Arc::new(Snapshot {
                name: "test".into(),
                identity: "test".into(),
                scanned_unix_ms: 0,
                entrants: (0..3)
                    .map(|id| Participant {
                        id,
                        registered_id: id.to_string(),
                        name: format!("Bot {id}"),
                        version: None,
                    })
                    .collect(),
                matches,
                standings: vec![],
                provisional: true,
                rating_method: "online_elo_v1",
            })),
        };
        let all = event.detail(&Query::parse("limit=50").unwrap());
        assert_eq!(all.get("matches").unwrap().as_array().unwrap().len(), 50);
        let pairs = all.get("matchups").unwrap().as_array().unwrap();
        assert_eq!(pairs.len(), 3);
        for (key, expected) in [
            ("total", 62),
            ("a_wins", 28),
            ("b_wins", 27),
            ("draws", 1),
            ("b_technical_wins", 1),
            ("a_technical_wins", 0),
            ("pending", 1),
            ("faults", 3),
            ("unscored", 1),
        ] {
            assert_eq!(value(&pairs[0], key), expected, "{key}");
        }
        let page = event.detail(
            &Query::parse("entrant=1&opponent=0&offset=50&limit=50&order=schedule").unwrap(),
        );
        assert_eq!(value(&page, "matched"), 62);
        assert_eq!(page.get("matches").unwrap().as_array().unwrap().len(), 12);
        assert_eq!(page.get("matchups"), all.get("matchups"));
        let empty = event.detail(&Query::parse("entrant=2&opponent=0&state=completed").unwrap());
        assert_eq!(value(&empty, "matched"), 0);
        assert_eq!(empty.get("matchups"), all.get("matchups"));
        for (query, count) in [
            ("entrant=0&outcome=win", 28),
            ("entrant=0&outcome=loss", 27),
            ("entrant=1&outcome=win", 27),
            ("entrant=1&outcome=loss", 28),
            ("entrant=0&outcome=technical_loss", 1),
            ("entrant=1&outcome=technical_win", 1),
            ("entrant=0&outcome=technical_win", 0),
            ("outcome=technical_win", 1),
        ] {
            assert_eq!(
                value(&event.detail(&Query::parse(query).unwrap()), "matched"),
                count,
                "{query}"
            );
        }
    }

    #[test]
    fn telemetry_filters_and_sorting_use_selected_player_before_pagination() {
        use arena_runner::tournament::monitor_stats::{MatchStatistics, PlayerStatistics};
        let matches = (0..60)
            .map(|index| {
                let mut a = attempt(AttemptState::Completed, Some(normal(MatchResult::Draw)));
                a.statistics = Some(MatchStatistics {
                    rounds: 10,
                    enl: PlayerStatistics {
                        score2: index as u128,
                        agent_rounds: 100,
                        pass_slots: index,
                        ..PlayerStatistics::default()
                    },
                    res: PlayerStatistics {
                        score2: 1000,
                        agent_rounds: 100,
                        not_submitted_slots: 1,
                        ..PlayerStatistics::default()
                    },
                });
                item(index, 0, 1, true, vec![a])
            })
            .collect();
        let event = Event {
            id: "stats".into(),
            label: "stats".into(),
            error: None,
            reader: None,
            snapshot: Some(Arc::new(Snapshot {
                name: "stats".into(),
                identity: "stats".into(),
                entrants: (0..2)
                    .map(|id| Participant {
                        id,
                        registered_id: id.to_string(),
                        name: id.to_string(),
                        version: None,
                    })
                    .collect(),
                matches,
                standings: vec![],
                provisional: false,
                rating_method: "batch_elo_v1",
                scanned_unix_ms: 0,
            })),
        };
        for (query, expected) in [
            ("entrant=0&attention=high_idle", 10),
            ("entrant=1&attention=high_idle", 0),
            ("attention=high_idle", 10),
            ("entrant=0&attention=zero_score", 1),
            ("entrant=0&attention=submission_issue", 0),
            ("entrant=1&attention=submission_issue", 60),
        ] {
            assert_eq!(
                value(&event.detail(&Query::parse(query).unwrap()), "matched"),
                expected,
                "{query}"
            );
        }
        for order in ["score", "idle"] {
            let page = event.detail(
                &Query::parse(&format!("entrant=0&order={order}&offset=50&limit=1")).unwrap(),
            );
            assert_eq!(
                value(
                    &page.get("matches").unwrap().as_array().unwrap()[0],
                    "index"
                ),
                9
            );
        }
    }

    #[test]
    fn pair_query_rejects_invalid_or_identical_entrants() {
        for query in [
            "entrant=-1",
            "opponent=bad",
            "entrant=1&opponent=1",
            "entrant=0&entrant=1",
            "outcome=win",
            "opponent=0&outcome=loss",
            "outcome=technical_loss",
            "attention=unknown",
        ] {
            assert!(Query::parse(query).is_err(), "{query}");
        }
        assert!(Query::parse("entrant=0&opponent=1").is_ok());
        assert!(Query::parse("entrant=&opponent=").is_ok());
    }
}

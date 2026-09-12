//! Incremental local discovery. Network writes never hold a verifier lock.
use arena_runner::tournament::{
    archive::Json,
    monitor::{ArchiveMonitor, Snapshot, guarded_path},
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
                    && query.map.is_none_or(|i| m.item.map_index as i64 == i)
                    && (query.state.is_empty() || m.state() == query.state)
                    && (query.outcome.is_empty()
                        || m.result()
                            .is_some_and(|r| r.outcome.outcome_tag() == query.outcome))
            })
            .collect();
        if query.latest {
            rows.sort_by_key(|m| {
                std::cmp::Reverse((
                    m.attempts
                        .last()
                        .and_then(|a| a.finished_unix_ms)
                        .unwrap_or(0),
                    m.item.index,
                ))
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
    pub state: String,
    pub outcome: String,
    pub latest: bool,
}
impl Query {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let mut q = Self {
            offset: 0,
            limit: 50,
            q: String::new(),
            map: None,
            state: String::new(),
            outcome: String::new(),
            latest: true,
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
                "state" => q.state = value,
                "outcome" => q.outcome = value,
                "order" => {
                    q.latest = match value.as_str() {
                        "latest" => true,
                        "schedule" => false,
                        _ => return Err("invalid order".into()),
                    }
                }
                _ => return Err("unknown query option".into()),
            }
        }
        Ok(q)
    }
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

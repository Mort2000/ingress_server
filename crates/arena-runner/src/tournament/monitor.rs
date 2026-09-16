//! Read-only tournament view. No execution lock, repair, process launch or writes.
//! Shares the executor's committed-result verifier and official Elo rules.
use super::{
    archive::Json,
    document::{encode, invalid, object, string},
    durable::{self, Plan},
    elo::EloTable,
    exec_external::ExternalResult,
    outcome::TournamentOutcome,
    schedule::TournamentItem,
};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Defensive bound for individual artifacts (including frozen builds and IARP).
pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ATTEMPT_TEXT_BYTES: u64 = 64 * 1024;

fn attempt_text(dir: &Path, name: &str) -> io::Result<String> {
    use std::io::Read;
    let file = fs::File::open(guarded_path(dir, Path::new(name))?)?;
    if file.metadata()?.len() > MAX_ATTEMPT_TEXT_BYTES {
        return Err(invalid("attempt text exceeds 64 KiB"));
    }
    let mut text = String::new();
    file.take(MAX_ATTEMPT_TEXT_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_ATTEMPT_TEXT_BYTES {
        return Err(invalid("attempt text exceeds 64 KiB"));
    }
    Ok(text)
}

/// Resolve only normal relative components under an operator-owned root.
/// Reject links and special files. This is not a sandbox against a concurrent
/// malicious writer replacing paths between checking and opening them.
pub fn guarded_path(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("root must be a real directory"));
    }
    let mut path = root.to_path_buf();
    for part in relative.components() {
        let Component::Normal(name) = part else {
            return Err(invalid("invalid relative path"));
        };
        path.push(name);
        let m = fs::symlink_metadata(&path)?;
        if m.file_type().is_symlink() || !(m.is_file() || m.is_dir()) {
            return Err(invalid("links and special files are not allowed"));
        }
    }
    Ok(path)
}

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    len: u64,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}
fn stamps(dir: &Path) -> io::Result<Vec<Stamp>> {
    fn visit(
        path: &Path,
        out: &mut Vec<Stamp>,
        depth: usize,
        entries: &mut usize,
    ) -> io::Result<()> {
        *entries += 1;
        if depth > 12 || *entries > 100_000 {
            return Err(invalid("archive tree limit exceeded"));
        }
        let m = fs::symlink_metadata(path)?;
        if m.file_type().is_symlink() {
            return Err(invalid("archive contains a symbolic link"));
        }
        if m.is_dir() {
            for e in fs::read_dir(path)? {
                visit(&e?.path(), out, depth + 1, entries)?;
            }
        } else if m.is_file() && m.len() <= MAX_FILE_BYTES {
            // Match the official verifier: recurse into directories even when
            // their names end in .tmp; ignore only ordinary temporary files.
            if path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".tmp")
            {
                return Ok(());
            }
            out.push(Stamp {
                path: path.to_path_buf(),
                len: m.len(),
                modified: m.modified()?,
                #[cfg(unix)]
                identity: {
                    use std::os::unix::fs::MetadataExt;
                    (m.dev(), m.ino(), m.ctime(), m.ctime_nsec())
                },
            });
        } else {
            return Err(invalid("special or oversized archive file"));
        }
        Ok(())
    }
    let mut out = Vec::new();
    visit(dir, &mut out, 0, &mut 0)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptState {
    RecordedRunning,
    Publishing,
    Completed,
    InfrastructureFault,
    Interrupted,
    Corrupt,
}
impl AttemptState {
    pub fn tag(self) -> &'static str {
        match self {
            Self::RecordedRunning => "recorded_running",
            Self::Publishing => "publishing",
            Self::Completed => "completed",
            Self::InfrastructureFault => "infrastructure_fault",
            Self::Interrupted => "interrupted",
            Self::Corrupt => "corrupt",
        }
    }
}

#[derive(Clone)]
pub struct AttemptView {
    pub number: String,
    pub attempt_reason: Option<String>,
    /// recorded_running is not a process-liveness claim.
    pub state: AttemptState,
    pub error: Option<String>,
    pub result: Option<ExternalResult>,
    pub finished_unix_ms: Option<u64>,
    pub statistics: Option<super::monitor_stats::MatchStatistics>,
}
#[derive(Clone)]
pub struct MatchView {
    pub item: TournamentItem,
    pub attempts: Vec<AttemptView>,
}
impl MatchView {
    pub fn state(&self) -> &str {
        self.attempts
            .last()
            .map(|a| a.state.tag())
            .unwrap_or("pending")
    }
    pub fn result(&self) -> Option<&ExternalResult> {
        self.attempts.last().and_then(|a| a.result.as_ref())
    }
}
#[derive(Clone)]
pub struct Participant {
    pub id: u32,
    pub registered_id: String,
    pub name: String,
    pub version: Option<String>,
}

/// A point-in-time, eventually consistent projection, never an archive rewrite.
pub struct Snapshot {
    pub name: String,
    pub identity: String,
    pub entrants: Vec<Participant>,
    pub matches: Vec<MatchView>,
    pub standings: Vec<super::report::StandingRow>,
    pub provisional: bool,
    pub rating_method: &'static str,
    pub scanned_unix_ms: u64,
}
impl Snapshot {
    pub fn to_json(&self) -> String {
        encode(&self.json())
    }
    pub fn json(&self) -> Json {
        self.json_matches(self.matches.iter())
    }
    /// Serialize selected matches while retaining the full snapshot's totals
    /// and standings. Selection never changes official scoring.
    pub fn json_matches<'a>(&self, matches: impl Iterator<Item = &'a MatchView>) -> Json {
        let n = |n: u64| Json::Int(n as i64);
        let entries = self
            .entrants
            .iter()
            .map(|p| {
                object(vec![
                    ("id", n(p.id as u64)),
                    ("registered_id", string(&p.registered_id)),
                    ("name", string(&p.name)),
                    (
                        "version",
                        p.version.as_ref().map(string).unwrap_or(Json::Null),
                    ),
                ])
            })
            .collect();
        let rows = matches
            .map(|m| {
                object(vec![
                    ("index", n(m.item.index)),
                    ("map", n(m.item.map_index as u64)),
                    ("map_hash", string(&m.item.map_hash)),
                    ("enl", n(m.item.enl_entrant() as u64)),
                    ("res", n(m.item.res_entrant() as u64)),
                    ("state", string(m.state())),
                    (
                        "attempts",
                        Json::Array(m.attempts.iter().map(attempt_json).collect()),
                    ),
                ])
            })
            .collect();
        let standings = self
            .standings
            .iter()
            .map(|s| {
                let id = self
                    .entrants
                    .iter()
                    .find(|p| p.registered_id == s.name)
                    .unwrap()
                    .id;
                object(vec![
                    ("entrant", n(id as u64)),
                    ("rank", n(s.rank as u64)),
                    ("rating", Json::Int(s.rating)),
                    ("wins", n(s.wins)),
                    ("draws", n(s.draws)),
                    ("losses", n(s.losses)),
                    ("forfeit_wins", n(s.forfeit_wins)),
                    ("forfeit_losses", n(s.forfeit_losses)),
                    ("unscored", n(s.unscored)),
                ])
            })
            .collect();
        object(vec![
            ("schema", string("arena-monitor-v1")),
            ("name", string(&self.name)),
            ("identity", string(&self.identity)),
            ("scanned_unix_ms", n(self.scanned_unix_ms)),
            ("liveness", string("unknown")),
            ("provisional", Json::Bool(self.provisional)),
            ("rating_method", string(self.rating_method)),
            ("total", n(self.matches.len() as u64)),
            (
                "completed",
                n(self
                    .matches
                    .iter()
                    .filter(|m| m.state() == "completed")
                    .count() as u64),
            ),
            ("entrants", Json::Array(entries)),
            ("matches", Json::Array(rows)),
            ("standings", Json::Array(standings)),
        ])
    }
}
fn attempt_json(a: &AttemptView) -> Json {
    let mut fields = vec![
        (
            "statistics",
            a.statistics
                .as_ref()
                .map(|s| s.json())
                .unwrap_or(Json::Null),
        ),
        ("number", string(&a.number)),
        ("state", string(a.state.tag())),
        (
            "attempt_reason",
            a.attempt_reason.as_ref().map(string).unwrap_or(Json::Null),
        ),
        ("error", a.error.as_ref().map(string).unwrap_or(Json::Null)),
        (
            "finished_unix_ms",
            a.finished_unix_ms
                .map(|n| Json::Int(n as i64))
                .unwrap_or(Json::Null),
        ),
        (
            "replay",
            Json::Bool(a.result.as_ref().is_some_and(|r| r.replay_rel.is_some())),
        ),
    ];
    if let Some(r) = &a.result {
        fields.extend([
            ("outcome", string(r.outcome.outcome_tag())),
            (
                "stage",
                r.outcome
                    .stage()
                    .map(|s| string(s.tag()))
                    .unwrap_or(Json::Null),
            ),
            ("elapsed_ms", Json::Int(r.elapsed_ms as i64)),
            ("factions_verified", Json::Bool(r.connect_order_verified)),
        ]);
        match &r.outcome {
            TournamentOutcome::Normal {
                score_enl,
                score_res,
                rounds_completed,
                ..
            } => fields.extend([
                ("score_enl", string(score_enl)),
                ("score_res", string(score_res)),
                ("rounds", Json::Int(*rounds_completed as i64)),
            ]),
            TournamentOutcome::TechnicalWin { winner, reason, .. } => fields.extend([
                ("winner", Json::Int(*winner as i64)),
                ("reason", string(reason)),
            ]),
            TournamentOutcome::Unscored { reason, .. } => fields.push(("reason", string(reason))),
        }
    }
    object(fields)
}

struct CachedAttempt {
    stamps: Vec<Stamp>,
    view: AttemptView,
}
pub struct ArchiveMonitor {
    root: PathBuf,
    plan: Plan,
    plan_stamps: Vec<Stamp>,
    identity: String,
    cache: BTreeMap<PathBuf, CachedAttempt>,
}
impl ArchiveMonitor {
    pub fn open(root: &Path) -> io::Result<Self> {
        let root = root.canonicalize()?;
        let plan_dir = guarded_path(&root, Path::new("plan"))?;
        let before = stamps(&plan_dir)?;
        let plan = durable::load(&root)?;
        if before != stamps(&plan_dir)? {
            return Err(invalid("plan changed while being read; retry"));
        }
        let identity = blake3::hash(&fs::read(plan_dir.join("manifest"))?)
            .to_hex()
            .to_string();
        Ok(Self {
            root,
            plan,
            plan_stamps: before,
            identity,
            cache: BTreeMap::new(),
        })
    }
    pub fn snapshot(&mut self) -> io::Result<Snapshot> {
        let plan_dir = guarded_path(&self.root, Path::new("plan"))?;
        if stamps(&plan_dir)? != self.plan_stamps {
            let refreshed = Self::open(&self.root)?;
            if refreshed.identity != self.identity {
                return Err(invalid(
                    "frozen plan identity changed; restart monitor to explicitly accept it",
                ));
            }
            *self = refreshed;
        }
        let mut matches = Vec::new();
        for item in &self.plan.items {
            let relative = PathBuf::from(format!("matches/m{}/attempts", item.index));
            let dirs = match guarded_path(&self.root, &relative) {
                Ok(_) => durable::attempt_dirs(&self.root, item.index)?,
                Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
                Err(e) => return Err(e),
            };
            let mut attempts = Vec::new();
            for dir in dirs {
                let number = dir.file_name().unwrap().to_string_lossy().into_owned();
                if number.len() != 4 || !number.bytes().all(|c| c.is_ascii_digit()) {
                    return Err(invalid("invalid attempt number"));
                }
                attempts.push(read_attempt(&dir, &self.plan, item, &mut self.cache));
            }
            let view = MatchView {
                item: item.clone(),
                attempts,
            };
            matches.push(view);
        }
        let table = EloTable::for_standings(
            &self.plan.config.elo,
            self.plan.config.entrants.len(),
            self.plan.items.len(),
            matches
                .iter()
                .filter_map(|view| view.result().map(|result| (&view.item, &result.outcome))),
        )?;
        let entrants = self
            .plan
            .config
            .entrants
            .iter()
            .enumerate()
            .map(|(id, p)| {
                let display = self
                    .plan
                    .extension
                    .as_ref()
                    .and_then(|e| e.get("entrants"))
                    .and_then(Json::as_array)
                    .and_then(|e| e.get(id))
                    .and_then(|e| e.get("name"))
                    .and_then(Json::as_str);
                Participant {
                    id: id as u32,
                    registered_id: p.name.clone(),
                    name: display.unwrap_or(&p.name).into(),
                    version: p.provenance.bot_version.clone(),
                }
            })
            .collect();
        Ok(Snapshot {
            name: self.plan.config.name.clone(),
            identity: self.identity.clone(),
            entrants,
            matches,
            standings: super::report::standings(&self.plan.config, &table),
            provisional: !table.is_final(),
            rating_method: table.method(),
            scanned_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        })
    }
    /// Only committed, verified attempts may be downloaded. The hash check is
    /// over these exact returned bytes, not a previous pathname observation.
    pub fn replay(&mut self, index: u64, attempt: &str) -> io::Result<Vec<u8>> {
        if attempt.len() != 4 || !attempt.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("invalid attempt"));
        }
        let snapshot = self.snapshot()?;
        let a = snapshot
            .matches
            .iter()
            .find(|m| m.item.index == index)
            .and_then(|m| m.attempts.iter().find(|a| a.number == attempt))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unknown match or attempt"))?;
        let r = a
            .result
            .as_ref()
            .ok_or_else(|| invalid("attempt is not verified"))?;
        let relative = r
            .replay_rel
            .as_ref()
            .ok_or_else(|| invalid("no replay for this attempt"))?;
        let dir = PathBuf::from(format!("matches/m{index}/attempts/{attempt}"));
        let path = guarded_path(&self.root, &dir.join(relative))?;
        use std::io::Read;
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(invalid("replay too large"));
        }
        let manifest = fs::read_to_string(guarded_path(&self.root, &dir.join("artifacts"))?)?;
        let hash = manifest
            .lines()
            .filter_map(|l| l.split_once(' '))
            .find(|(_, p)| *p == relative)
            .map(|(h, _)| h);
        if hash != Some(blake3::hash(&bytes).to_hex().as_str()) {
            return Err(invalid("replay changed"));
        }
        Ok(bytes)
    }
}

fn read_attempt(
    dir: &Path,
    plan: &Plan,
    item: &TournamentItem,
    cache: &mut BTreeMap<PathBuf, CachedAttempt>,
) -> AttemptView {
    let mut view = AttemptView {
        number: dir.file_name().unwrap().to_string_lossy().into_owned(),
        attempt_reason: None,
        state: AttemptState::Publishing,
        error: None,
        result: None,
        finished_unix_ms: None,
        statistics: None,
    };
    let inspect = || -> io::Result<Option<Vec<Stamp>>> {
        if fs::symlink_metadata(dir)?.file_type().is_symlink() {
            return Err(invalid("attempt is a symbolic link"));
        }
        match guarded_path(dir, Path::new("artifacts")) {
            Ok(_) => Ok(Some(stamps(dir)?)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    };
    match inspect() {
        Ok(Some(before)) => {
            if let Some(c) = cache.get(dir).filter(|c| c.stamps == before) {
                return c.view.clone();
            }
            let mut statistics = None;
            match durable::read_result_inspect(dir, plan, item, |log, verified| {
                statistics = Some(super::monitor_stats::MatchStatistics::from_verified(
                    log, verified,
                ));
            }) {
                Ok(result) => {
                    view.statistics = statistics;
                    view.state = if durable::infrastructure(&result) {
                        AttemptState::InfrastructureFault
                    } else {
                        AttemptState::Completed
                    };
                    view.result = Some(result);
                    view.finished_unix_ms = super::document::read(&dir.join("resources.json"))
                        .ok()
                        .and_then(|j| j.get("finished_unix_ms").and_then(Json::as_int))
                        .and_then(|n| n.try_into().ok());
                }
                Err(e) => {
                    view.state = AttemptState::Corrupt;
                    view.error = Some(e.to_string());
                }
            }
            if stamps(dir).ok().as_ref() != Some(&before) {
                view.state = AttemptState::Publishing;
                view.result = None;
                view.statistics = None;
                view.error = None;
            } else {
                cache.insert(
                    dir.to_path_buf(),
                    CachedAttempt {
                        stamps: before,
                        view: view.clone(),
                    },
                );
            }
        }
        Ok(None) => {
            let state = attempt_text(dir, "state");
            match state.as_deref() {
                Ok("running") => view.state = AttemptState::RecordedRunning,
                Ok("interrupted") => view.state = AttemptState::Interrupted,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                _ => {
                    view.state = AttemptState::Corrupt;
                    view.error =
                        Some("invalid state or committed attempt missing artifacts".into());
                }
            }
        }
        Err(e) => {
            view.state = AttemptState::Corrupt;
            view.error = Some(e.to_string());
        }
    }
    match attempt_text(dir, "reason.txt") {
        Ok(reason) => view.attempt_reason = Some(reason),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            view.state = AttemptState::Corrupt;
            view.result = None;
            view.error = Some(e.to_string());
        }
    }
    if let Some(c) = cache.get_mut(dir) {
        c.view = view.clone();
    }
    view
}

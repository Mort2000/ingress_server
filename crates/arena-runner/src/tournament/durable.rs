//! Durable single-machine tournament workflow. Frozen inputs, append-only
//! attempts and adopted results are authoritative; summaries are disposable.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{
    archive::{self, Json},
    config::*,
    entrant::compute_binary_hash,
    exec_external::{ExternalResult, execution_id},
    outcome::{Stage, TournamentOutcome},
    run::TournamentRun,
    schedule::{self, TournamentItem},
};
use arena_core::{
    codec,
    map::MapDefinition,
    types::{MatchResult, PortalId},
};

fn invalid(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

/// Write and fsync before atomic replacement; directory sync makes the rename
/// durable. A failure is propagated, never converted to a successful result.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| invalid("missing parent"))?;
    let temp = parent.join(format!(".{}.tmp", execution_id()?));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    File::open(parent)?.sync_all()
}

pub struct Plan {
    pub config: TournamentConfig,
    pub items: Vec<TournamentItem>,
    pub maps: Vec<MapDefinition>,
    /// Parsed at the archive boundary; every execution/validation uses these.
    pub map_configs: Vec<arena_core::config::GameConfig>,
    pub attempt_limit: u32,
    /// Versioned batch-two input metadata. Absent in first-batch archives.
    pub extension: Option<Json>,
}

impl Plan {
    pub fn match_config(&self, item: &TournamentItem) -> io::Result<TournamentConfig> {
        let mut cfg = self.config.clone();
        cfg.config = *self
            .map_configs
            .get(item.map_index as usize)
            .ok_or_else(|| invalid("missing map rules"))?;
        cfg.rounds_per_match = cfg.config.total_rounds;
        Ok(cfg)
    }
}

fn map_bytes(map: &MapDefinition) -> Vec<u8> {
    let mut bytes = Vec::new();
    for portal in map.portals() {
        codec::push_portal(portal, &mut bytes);
    }
    bytes
}

/// Publish a complete plan before any client can be launched.
pub fn prepare(cfg: &TournamentConfig, root: &Path, attempt_limit: u32) -> io::Result<Plan> {
    if !(1..=100).contains(&attempt_limit) {
        return Err(invalid("attempt limit must be 1..100"));
    }
    let items = schedule::build_items(cfg).map_err(invalid)?;
    let maps = schedule::map_pool(cfg).map_err(invalid)?;
    let mut cfg = cfg.clone();
    for entrant in &mut cfg.entrants {
        let super::EntrantKind::External { binary, .. } = &mut entrant.kind;
        *binary = binary.canonicalize()?;
        entrant.compute_binary_hash()?;
    }
    publish(
        Plan {
            map_configs: vec![cfg.config; maps.len()],
            config: cfg,
            items,
            maps,
            attempt_limit,
            extension: None,
        },
        root,
    )
}

/// Publish supplied frozen inputs. Batch-two callers resolve/validate the
/// declaration before creating the archive; no generator is called here.
pub fn publish(mut plan: Plan, root: &Path) -> io::Result<Plan> {
    let dir = root.join("plan");
    fs::create_dir(&dir)?;
    fs::create_dir(dir.join("maps"))?;
    let mut extra = Vec::new();
    if let Some(ext) = &plan.extension {
        atomic_write(
            &dir.join("batch2.json"),
            super::document::encode(ext).as_bytes(),
        )?;
        extra.push("batch2.json".to_string());
        fs::create_dir(dir.join("builds"))?;
        for (i, entry) in super::document::array(ext, "entrants")?.iter().enumerate() {
            super::builds::copy_into(
                Path::new(super::document::text(entry, "build")?),
                &dir.join(format!("builds/{i}")),
            )?;
            let super::EntrantKind::External { binary, .. } = &mut plan.config.entrants[i].kind;
            *binary = PathBuf::from(format!("builds/{i}/binary"));
            extra
                .extend(["binary", "manifest.json", "identity"].map(|f| format!("builds/{i}/{f}")));
        }
    }
    let Plan {
        config: cfg,
        items,
        maps,
        map_configs,
        attempt_limit,
        extension,
    } = plan;
    let mut bytes = Vec::new();
    codec::push_config(&cfg.config, &mut bytes);
    atomic_write(&dir.join("config.bin"), &bytes)?;
    atomic_write(
        &dir.join("tournament.json"),
        archive::tournament_json_text(&cfg, &items).as_bytes(),
    )?;
    atomic_write(
        &dir.join("attempt-limit"),
        attempt_limit.to_string().as_bytes(),
    )?;
    for (i, map) in maps.iter().enumerate() {
        atomic_write(&dir.join(format!("maps/{i}.bin")), &map_bytes(map))?;
    }
    let rows: Vec<_> = items
        .iter()
        .map(|item| archive::index_record(item, &pending(), &cfg, None, 0))
        .collect();
    atomic_write(
        &dir.join("schedule.tsv"),
        archive::index_records_to_tsv(&rows).as_bytes(),
    )?;
    // Manifest includes all plan bytes, not paths outside this directory.
    let mut manifest = String::new();
    let mut names = vec![
        "config.bin".to_string(),
        "tournament.json".into(),
        "attempt-limit".into(),
        "schedule.tsv".into(),
    ];
    names.extend((0..maps.len()).map(|i| format!("maps/{i}.bin")));
    names.extend(extra);
    for name in names {
        manifest.push_str(&format!(
            "{} {name}\n",
            blake3::hash(&fs::read(dir.join(&name))?).to_hex()
        ));
    }
    atomic_write(&dir.join("manifest"), manifest.as_bytes())?;
    Ok(Plan {
        config: cfg,
        items,
        maps,
        map_configs,
        attempt_limit,
        extension,
    })
}

fn string<'a>(json: &'a Json, key: &str) -> io::Result<&'a str> {
    json.get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| invalid(format!("missing {key}")))
}
fn number(json: &Json, key: &str) -> io::Result<u64> {
    json.get(key)
        .and_then(Json::as_int)
        .and_then(|v| u64::try_from(v).ok())
        .ok_or_else(|| invalid(format!("invalid {key}")))
}

pub fn load(root: &Path) -> io::Result<Plan> {
    let dir = root.join("plan");
    let mut covered = std::collections::BTreeSet::new();
    for line in fs::read_to_string(dir.join("manifest"))?.lines() {
        let (hash, name) = line
            .split_once(' ')
            .ok_or_else(|| invalid("invalid plan manifest"))?;
        if name.contains("..") || Path::new(name).is_absolute() {
            return Err(invalid("invalid manifest path"));
        }
        if !covered.insert(name.to_owned()) {
            return Err(invalid("duplicate plan manifest entry"));
        }
        if blake3::hash(&fs::read(dir.join(name))?).to_hex().as_str() != hash {
            return Err(invalid(format!("plan changed: {name}")));
        }
    }
    let text = fs::read_to_string(dir.join("tournament.json"))?;
    let echo = archive::parse_tournament_json(&text).map_err(invalid)?;
    let json = archive::parse_json(&text).map_err(invalid)?;
    let sandbox = json.get("sandbox").ok_or_else(|| invalid("sandbox"))?;
    let sandbox = match string(sandbox, "mode")? {
        "off" => ClientSandbox::Off,
        "sandbox-runner" => ClientSandbox::SandboxRunner {
            runner: string(sandbox, "runner")?.into(),
            wall_ms: number(sandbox, "wall_ms")?,
            address_space_bytes: number(sandbox, "as_bytes")?,
        },
        _ => return Err(invalid("unknown sandbox")),
    };
    let config = codec::config_from_bytes(&fs::read(dir.join("config.bin"))?)
        .ok_or_else(|| invalid("invalid frozen rules"))?;
    config.validate().map_err(|e| invalid(format!("{e:?}")))?;
    let mut cfg = TournamentConfig {
        name: echo.name,
        master_seed: echo.master_seed,
        entrants: echo.entrants,
        matchups: echo.matchups,
        map_count: echo.map_count,
        rounds_per_match: echo.rounds_per_match,
        config,
        elo: EloConfig {
            k: echo.elo.0,
            initial: echo.elo.1,
        },
        concurrency: echo.concurrency,
        sandbox,
        per_match_budget_ms: echo.per_match_budget_ms,
    };
    let extension = if dir.join("batch2.json").is_file() {
        Some(super::document::read(&dir.join("batch2.json"))?)
    } else {
        None
    };
    let mut required = vec![
        "tournament.json".to_owned(),
        "config.bin".into(),
        "schedule.tsv".into(),
        "attempt-limit".into(),
    ];
    required.extend((0..cfg.map_count).map(|i| format!("maps/{i}.bin")));
    if extension.is_some() {
        required.push("batch2.json".into());
        for i in 0..cfg.entrants.len() {
            required
                .extend(["binary", "manifest.json", "identity"].map(|f| format!("builds/{i}/{f}")));
        }
    }
    if required
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        != covered
    {
        return Err(invalid("incomplete or unexpected plan manifest coverage"));
    }
    if let Some(ext) = &extension {
        if super::document::text(ext, "schema")? != "arena-frozen-event-v1" {
            return Err(invalid("unsupported frozen event schema"));
        }
        if super::document::array(ext, "entrants")?.len() != cfg.entrants.len() {
            return Err(invalid("entrant metadata length mismatch"));
        }
        for (i, entrant) in cfg.entrants.iter_mut().enumerate() {
            let super::EntrantKind::External { binary, .. } = &mut entrant.kind;
            *binary = dir.join(format!("builds/{i}/binary")).canonicalize()?;
            let manifest = super::builds::inspect(&dir.join(format!("builds/{i}")))?;
            let entry = &super::document::array(ext, "entrants")?[i];
            if entry.get("manifest") != Some(&manifest)
                || super::document::text(entry, "id")? != entrant.name
                || entrant.provenance.binary_blake3.as_deref()
                    != Some(super::document::text(&manifest, "binary_hash")?)
            {
                return Err(invalid("frozen build identity disagrees with entrant"));
            }
        }
    }
    let mut maps = Vec::new();
    let mut map_configs = Vec::new();
    for i in 0..cfg.map_count {
        let bytes = fs::read(dir.join(format!("maps/{i}.bin")))?;
        if bytes.len() % codec::PORTAL_BYTES != 0 {
            return Err(invalid("truncated frozen map"));
        }
        let portals = bytes
            .chunks_exact(codec::PORTAL_BYTES)
            .enumerate()
            .map(|(id, bytes)| {
                codec::portal_from_bytes(PortalId(id as u32), bytes)
                    .ok_or_else(|| invalid("invalid portal"))
            })
            .collect::<io::Result<Vec<_>>>()?;
        let map = MapDefinition::new(portals);
        let map_config = if let Some(ext) = &extension {
            super::document::config(
                super::document::array(ext, "maps")?
                    .get(i as usize)
                    .and_then(|m| m.get("config"))
                    .ok_or_else(|| invalid("map rules missing"))?,
            )?
        } else {
            cfg.config
        };
        map.validate(&map_config)
            .map_err(|e| invalid(format!("map: {e:?}")))?;
        if let Some(ext) = &extension {
            let asset = super::maps::MapAsset::from_json(&super::document::encode(
                &super::document::array(ext, "maps")?[i as usize],
            ))?;
            if asset.map != map {
                return Err(invalid("frozen map metadata disagrees with map bytes"));
            }
        }
        maps.push(map);
        map_configs.push(map_config);
    }
    let rows = archive::parse_index_tsv(&fs::read_to_string(dir.join("schedule.tsv"))?)
        .map_err(invalid)?;
    let mut items = Vec::new();
    for row in rows {
        let id = |name: &str| {
            cfg.entrants
                .iter()
                .position(|e| e.name == name)
                .map(|i| i as u32)
                .ok_or_else(|| invalid("unknown entrant"))
        };
        if row.index != items.len() as u64
            || Some(row.config_hash)
                != map_configs
                    .get(row.map_index as usize)
                    .map(super::config::config_identity)
            || maps
                .get(row.map_index as usize)
                .map(schedule::map_codec_hash)
                .as_deref()
                != Some(&row.map_hash)
        {
            return Err(invalid("frozen schedule inputs disagree"));
        }
        items.push(TournamentItem {
            index: row.index,
            tag: row.tag,
            j: row.j,
            a: id(&row.entrant_a)?,
            b: id(&row.entrant_b)?,
            a_is_enl: row.a_is_enl,
            map_index: row.map_index,
            map_seed: row.map_seed,
            map_hash: row.map_hash,
            game_seed: row.game_seed,
            client_seed: row.client_seed,
            schedule_seed: row.schedule_seed,
            config_hash: row.config_hash,
        });
    }
    let attempt_limit = fs::read_to_string(dir.join("attempt-limit"))?
        .parse()
        .map_err(invalid)?;
    if let Some(ext) = &extension {
        let samples = super::document::array(ext, "samples")?;
        if samples.len() != items.len() || super::document::array(ext, "maps")?.len() != maps.len()
        {
            return Err(invalid("frozen metadata length mismatch"));
        }
        for (sample, item) in samples.iter().zip(&items) {
            let candidate = super::document::text(sample, "candidate")?;
            let baseline = super::document::text(sample, "baseline")?;
            let a = &cfg.entrant(item.a).name;
            let b = &cfg.entrant(item.b).name;
            if a == b || !((candidate == a && baseline == b) || (candidate == b && baseline == a)) {
                return Err(invalid("sample entrants disagree"));
            }
        }
        super::event::launches(Some(ext))?;
    }
    if !(1..=100).contains(&attempt_limit) || cfg.concurrency == 0 {
        return Err(invalid("invalid execution policy"));
    }
    Ok(Plan {
        config: cfg,
        items,
        maps,
        map_configs,
        attempt_limit,
        extension,
    })
}

fn pending() -> TournamentOutcome {
    TournamentOutcome::Unscored {
        stage: Stage::RunnerFault,
        reason: "pending".into(),
    }
}
pub(crate) fn infrastructure(result: &ExternalResult) -> bool {
    matches!(
        result.outcome.stage(),
        Some(Stage::RunnerFault | Stage::SetupError)
    )
}

pub(crate) fn attempt_dirs(root: &Path, index: u64) -> io::Result<Vec<PathBuf>> {
    let dir = root.join(format!("matches/m{index}/attempts"));
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut dirs = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    dirs.retain(|p| p.is_dir());
    dirs.sort();
    Ok(dirs)
}

pub(crate) fn read_result(
    dir: &Path,
    plan: &Plan,
    item: &TournamentItem,
) -> io::Result<ExternalResult> {
    let meta = archive::parse_json(&fs::read_to_string(dir.join("meta.json"))?).map_err(invalid)?;
    let mut rows =
        archive::parse_index_tsv(&fs::read_to_string(dir.join("result.tsv"))?).map_err(invalid)?;
    if rows.len() != 1 {
        return Err(invalid("one attempt must have one result"));
    }
    let row = rows.remove(0);
    if row.index != item.index
        || row.game_seed != item.game_seed
        || row.map_hash != item.map_hash
        || row.config_hash != item.config_hash
        || row.a_is_enl != item.a_is_enl
    {
        return Err(invalid("attempt input mismatch"));
    }
    let reason = string(&meta, "reason")?.to_string();
    let outcome = match row.outcome.as_str() {
        "enl_win" | "res_win" | "draw" => {
            let raw = row.final_state_hash.as_bytes();
            if raw.len() != 64 {
                return Err(invalid("invalid state hash"));
            }
            let mut hash = [0u8; 32];
            for (i, byte) in hash.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&row.final_state_hash[i * 2..i * 2 + 2], 16)
                    .map_err(invalid)?;
            }
            TournamentOutcome::Normal {
                result: match row.outcome.as_str() {
                    "enl_win" => MatchResult::EnlWin,
                    "res_win" => MatchResult::ResWin,
                    _ => MatchResult::Draw,
                },
                score_enl: row.score_enl,
                score_res: row.score_res,
                rounds_completed: row.rounds_completed,
                final_state_hash: hash,
            }
        }
        "technical_win" => TournamentOutcome::TechnicalWin {
            winner: plan
                .config
                .entrants
                .iter()
                .position(|e| e.name == row.winner)
                .ok_or_else(|| invalid("unknown winner"))? as u32,
            stage: Stage::from_tag(&row.stage).ok_or_else(|| invalid("unknown stage"))?,
            reason,
        },
        "unscored" => TournamentOutcome::Unscored {
            stage: Stage::from_tag(&row.stage).ok_or_else(|| invalid("unknown stage"))?,
            reason,
        },
        _ => return Err(invalid("unknown outcome")),
    };
    let result = ExternalResult {
        outcome,
        replay_rel: (row.replay != "-").then_some(row.replay),
        sandbox_reports: Vec::new(),
        budget_exceeded: meta.get("budget_exceeded") == Some(&Json::Bool(true)),
        connect_order_verified: meta.get("connect_order_verified") == Some(&Json::Bool(true)),
        elapsed_ms: row.elapsed_ms,
    };
    verify_artifacts(dir)?;
    if let Some(relative) = &result.replay_rel {
        if relative != "server/replay.iarp" {
            return Err(invalid("invalid replay path"));
        }
        let bytes = fs::read(dir.join(relative))?;
        let log = arena_replay::replay::decode_log(&bytes).map_err(invalid)?;
        let verified = arena_replay::replay::replay(&log).map_err(invalid)?;
        if log.header.game_seed != item.game_seed
            || log.header.config != plan.match_config(item)?.config
            || schedule::map_codec_hash(&log.header.map) != item.map_hash
        {
            return Err(invalid("replay does not match frozen inputs"));
        }
        if let TournamentOutcome::Normal {
            final_state_hash,
            score_enl,
            score_res,
            rounds_completed,
            result: expected,
        } = &result.outcome
        {
            if verified.final_state_hash != *final_state_hash
                || verified.final_scores != (*score_enl, *score_res)
                || verified.rounds_completed != *rounds_completed
                || verified.result != *expected
                || verified.abnormal_outcome.is_some()
            {
                return Err(invalid("replay outcome mismatch"));
            }
        }
    } else if result.outcome.elo_relevant() {
        return Err(invalid("normal result missing replay"));
    }
    if !infrastructure(&result) && !result.connect_order_verified {
        return Err(invalid("missing faction proof"));
    }
    Ok(result)
}

fn artifact_manifest(dir: &Path, current: &Path, out: &mut String, sync: bool) -> io::Result<()> {
    let mut entries = fs::read_dir(current)?
        .map(|e| e.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            artifact_manifest(dir, &path, out, sync)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(dir)
                .map_err(invalid)?
                .to_str()
                .ok_or_else(|| invalid("artifact name"))?;
            // Supervisor diagnostics may be written after the artifact commit
            // point (e.g. state/adoption fsync failed). They are not evidence.
            if matches!(relative, "artifacts" | "state" | "storage-error.txt")
                || relative.ends_with(".tmp")
            {
                continue;
            }
            if sync {
                File::open(&path)?.sync_all()?;
            }
            out.push_str(&format!(
                "{} {relative}\n",
                blake3::hash(&fs::read(&path)?).to_hex()
            ));
        }
    }
    Ok(())
}
fn verify_artifacts(dir: &Path) -> io::Result<()> {
    let recorded = fs::read_to_string(dir.join("artifacts"))?;
    let mut actual = String::new();
    artifact_manifest(dir, dir, &mut actual, false)?;
    if actual != recorded {
        return Err(invalid(format!(
            "artifact validation failed: {}",
            dir.display()
        )));
    }
    Ok(())
}

/// Advisory OS lock automatically released on process death; never delete the
/// lock inode, so simultaneous resumes cannot both own a tournament.
fn lock(root: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("execution.lock"))?;
    file.try_lock()
        .map_err(|e| invalid(format!("tournament already running: {e}")))?;
    Ok(file)
}

pub fn status(root: &Path) -> io::Result<String> {
    let plan = load(root)?;
    let mut pending = 0;
    let mut completed = 0;
    let mut faults = 0;
    let mut interrupted = 0;
    let mut running = 0;
    let active = OpenOptions::new()
        .write(true)
        .open(root.join("execution.lock"))
        .ok()
        .map(|f| f.try_lock().is_err())
        .unwrap_or(false);
    for item in &plan.items {
        match attempt_dirs(root, item.index)?.last() {
            None => pending += 1,
            Some(dir) => match attempt_state(dir, &plan, item, active)? {
                AttemptState::Completed => completed += 1,
                AttemptState::Fault => faults += 1,
                AttemptState::Running => running += 1,
                AttemptState::Interrupted => interrupted += 1,
            },
        }
    }
    Ok(format!(
        "pending={pending} running={running} completed={completed} infrastructure_fault={faults} interrupted={interrupted}\n"
    ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttemptState {
    Running,
    Completed,
    Fault,
    Interrupted,
}
impl AttemptState {
    fn tag(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Fault => "infrastructure_fault",
            Self::Interrupted => "interrupted",
        }
    }
}

/// One crash-recovery rule for status, resume, retry and rebuild. A published,
/// validated artifact manifest is the commit point; the state label is a cache.
fn attempt_state(
    dir: &Path,
    plan: &Plan,
    item: &TournamentItem,
    active: bool,
) -> io::Result<AttemptState> {
    let recorded = match fs::read_to_string(dir.join("state")) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => "interrupted".into(),
        Err(e) => return Err(e),
    };
    let state = match recorded.as_str() {
        "running" => AttemptState::Running,
        "completed" => AttemptState::Completed,
        "infrastructure_fault" => AttemptState::Fault,
        "interrupted" => AttemptState::Interrupted,
        _ => return Err(invalid("unknown attempt state")),
    };
    if state == AttemptState::Running && active {
        return Ok(state);
    }
    if dir.join("artifacts").is_file() {
        let result = read_result(dir, plan, item)?;
        return Ok(if infrastructure(&result) {
            AttemptState::Fault
        } else {
            AttemptState::Completed
        });
    }
    if matches!(state, AttemptState::Completed | AttemptState::Fault) {
        return Err(invalid("committed attempt missing artifacts"));
    }
    Ok(AttemptState::Interrupted)
}

#[derive(Clone, Copy)]
pub enum Action {
    Resume,
    Retry(u64),
    Rebuild,
}

pub fn execute(root: &Path, action: Action) -> io::Result<TournamentRun> {
    let _lock = lock(root)?;
    let plan = load(root)?;
    for item in &plan.items {
        if let Some(dir) = attempt_dirs(root, item.index)?.last() {
            let state = attempt_state(dir, &plan, item, false)?;
            atomic_write(&dir.join("state"), state.tag().as_bytes())?;
        }
    }
    if let Action::Retry(index) = action {
        let item = plan
            .items
            .get(index as usize)
            .ok_or_else(|| invalid("unknown item"))?;
        let dirs = attempt_dirs(root, item.index)?;
        let last = dirs
            .last()
            .ok_or_else(|| invalid("retry requires a previous infrastructure fault"))?;
        if fs::read_to_string(last.join("state"))? != "infrastructure_fault"
            || !infrastructure(&read_result(last, &plan, item)?)
        {
            return Err(invalid(
                "only infrastructure faults may be retried; Bot results are final",
            ));
        }
        if dirs.len() >= plan.attempt_limit as usize {
            return Err(invalid("attempt limit reached"));
        }
    }
    if !matches!(action, Action::Rebuild) {
        let resources = super::resources::Resources::inspect(&plan)?;
        let measurements = root.join("executions");
        fs::create_dir_all(&measurements)?;
        atomic_write(
            &measurements.join(format!("{}.json", super::exec_external::execution_id()?)),
            super::document::encode(&resources.report).as_bytes(),
        )?;
        // Validate the referenced build before launching anything (frozen build
        // registry comes in batch two; changed binaries are refused here).
        for entrant in &plan.config.entrants {
            let super::EntrantKind::External { binary, .. } = &entrant.kind;
            if entrant.provenance.binary_blake3.as_deref() != Some(&compute_binary_hash(binary)?) {
                return Err(invalid("registered binary changed"));
            }
        }
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| -> io::Result<()> {
            let next = &next;
            let mut handles = Vec::new();
            for _ in 0..resources.workers.min(plan.items.len()) {
                let plan = &plan;
                let resources = &resources;
                handles.push(scope.spawn(move || -> io::Result<()> {
                    loop {
                        let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(item) = plan.items.get(index) else {
                            break;
                        };
                        run_pending(root, plan, item, action, resources)?;
                    }
                    Ok(())
                }));
            }
            for handle in handles {
                handle.join().map_err(|_| invalid("worker panicked"))??;
            }
            Ok(())
        })?;
    }
    rebuild(root, &plan)
}

fn run_pending(
    root: &Path,
    plan: &Plan,
    item: &TournamentItem,
    action: Action,
    resources: &super::resources::Resources,
) -> io::Result<()> {
    let dirs = attempt_dirs(root, item.index)?;
    let mut reason = "initial execution";
    if let Some(last) = dirs.last() {
        match attempt_state(last, plan, item, false)? {
            AttemptState::Completed => {
                return Ok(());
            }
            AttemptState::Fault if !matches!(action, Action::Retry(i) if i == item.index) => {
                return Ok(());
            }
            AttemptState::Fault => reason = "explicit bounded infrastructure retry",
            AttemptState::Running | AttemptState::Interrupted => {
                atomic_write(&last.join("state"), b"interrupted")?;
                reason = "resume interrupted attempt";
            }
        }
    }
    if matches!(action, Action::Retry(i) if i != item.index) {
        return Ok(());
    }
    if dirs.len() >= plan.attempt_limit as usize {
        return Err(invalid(format!(
            "item {} attempt limit reached",
            item.index
        )));
    }
    let waiting = std::time::Instant::now();
    let permit = resources.acquire()?;
    let started = super::resources::unix_ms();
    let start_sample = super::resources::host_sample();
    let waited_ms = waiting.elapsed().as_millis();
    let parent = root.join(format!("matches/m{}/attempts", item.index));
    fs::create_dir_all(&parent)?;
    let dir = parent.join(format!("{:04}", dirs.len() + 1));
    fs::create_dir(&dir)?;
    atomic_write(&dir.join("reason.txt"), reason.as_bytes())?;
    atomic_write(&dir.join("state"), b"running")?;
    let cfg = plan.match_config(item)?;
    let launches = super::event::launches(plan.extension.as_ref())?;
    let result = super::exec_external::run_item_external_with_launches(
        item,
        &cfg,
        &dir,
        Some(plan.maps[item.map_index as usize].clone()),
        &launches,
    );
    let save = || -> io::Result<()> {
        let mut measurement = resources.report.clone();
        super::document::set(&mut measurement, "start_sample", start_sample.clone())?;
        super::document::set(
            &mut measurement,
            "finish_sample",
            super::resources::host_sample(),
        )?;
        for (key, value) in [
            ("started_unix_ms", Json::Int(started as i64)),
            (
                "finished_unix_ms",
                Json::Int(super::resources::unix_ms() as i64),
            ),
            (
                "admission_wait_ms",
                Json::Int(waited_ms.min(i64::MAX as u128) as i64),
            ),
            (
                "host_slot",
                permit
                    .slot
                    .map(|n| Json::Int(n as i64))
                    .unwrap_or(Json::Null),
            ),
        ] {
            super::document::set(&mut measurement, key, value)?;
        }
        atomic_write(
            &dir.join("resources.json"),
            super::document::encode(&measurement).as_bytes(),
        )?;
        for (side, report) in &result.sandbox_reports {
            atomic_write(&dir.join(format!("sandbox_{side}.json")), report.as_bytes())?;
        }
        atomic_write(
            &dir.join("meta.json"),
            archive::meta_json_text(
                item,
                &result.outcome,
                &cfg,
                result.replay_rel.as_deref(),
                result.elapsed_ms,
                result.budget_exceeded,
                result.connect_order_verified,
            )
            .as_bytes(),
        )?;
        let record = archive::index_record(
            item,
            &result.outcome,
            &cfg,
            result.replay_rel.as_deref(),
            result.elapsed_ms,
        );
        atomic_write(
            &dir.join("result.tsv"),
            archive::index_records_to_tsv(&[record]).as_bytes(),
        )?;
        let mut manifest = String::new();
        artifact_manifest(&dir, &dir, &mut manifest, true)?;
        atomic_write(&dir.join("artifacts"), manifest.as_bytes())?;
        read_result(&dir, plan, item)?;
        atomic_write(
            &dir.join("state"),
            if infrastructure(&result) {
                b"infrastructure_fault"
            } else {
                b"completed"
            },
        )?;
        if !infrastructure(&result) {
            atomic_write(
                &root.join(format!("matches/m{}/adopted", item.index)),
                format!("{}\n{reason}\n", dir.file_name().unwrap().to_string_lossy()).as_bytes(),
            )?;
        }
        Ok(())
    };
    if let Err(error) = save() {
        // If storage itself is unavailable the process still fails loudly;
        // the previous 'running' marker remains recoverable as interrupted.
        let _ = atomic_write(&dir.join("storage-error.txt"), error.to_string().as_bytes());
        return Err(error);
    }
    Ok(())
}

fn rebuild(root: &Path, plan: &Plan) -> io::Result<TournamentRun> {
    let mut run = TournamentRun {
        items: plan.items.clone(),
        outcomes: Vec::new(),
        elapsed_ms: Vec::new(),
        replay_rel: Vec::new(),
        budget_exceeded: Vec::new(),
        connect_order_verified: Vec::new(),
        ratings: Vec::new(),
        total_elapsed_ms: 0,
        workers: plan.config.concurrency,
    };
    let mut adoption = String::from("item\tattempt\treason\n");
    for item in &plan.items {
        let dirs = attempt_dirs(root, item.index)?;
        let result = if let Some(dir) = dirs.last().filter(|d| d.join("artifacts").is_file()) {
            let mut result = read_result(dir, plan, item)?;
            if !infrastructure(&result) {
                let name = dir.file_name().unwrap().to_string_lossy();
                let reason = fs::read_to_string(dir.join("reason.txt"))?;
                atomic_write(
                    &root.join(format!("matches/m{}/adopted", item.index)),
                    format!("{name}\n{reason}\n").as_bytes(),
                )?;
                adoption.push_str(&format!("{}\t{name}\t{reason}\n", item.index));
            }
            result.replay_rel = result.replay_rel.map(|p| {
                format!(
                    "attempts/{}/{p}",
                    dir.file_name().unwrap().to_string_lossy()
                )
            });
            result
        } else {
            ExternalResult {
                outcome: pending(),
                replay_rel: None,
                sandbox_reports: Vec::new(),
                budget_exceeded: false,
                connect_order_verified: false,
                elapsed_ms: 0,
            }
        };
        run.outcomes.push(result.outcome);
        run.elapsed_ms.push(result.elapsed_ms);
        run.replay_rel.push(result.replay_rel);
        run.budget_exceeded.push(result.budget_exceeded);
        run.connect_order_verified
            .push(result.connect_order_verified);
    }
    let mut elo = super::elo::EloTable::new(&plan.config.elo, plan.config.entrants.len());
    for (item, outcome) in run.items.iter().zip(&run.outcomes) {
        if let Some(rating) = elo.apply(item, outcome) {
            run.ratings.push(rating);
        }
    }
    super::write_tournament_artifacts(&plan.config, &run, root, None)?;
    atomic_write(&root.join("adoptions.tsv"), adoption.as_bytes())?;
    Ok(run)
}

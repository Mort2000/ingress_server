//! Durable single-machine tournament workflow. Frozen inputs, append-only
//! attempts and adopted results are authoritative; summaries are disposable.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{
    archive::{self, Json},
    config::*,
    entrant::compute_binary_hash,
    exec_external::{ExternalResult, execution_id, run_item_external_on_map},
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
    pub attempt_limit: u32,
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
    let dir = root.join("plan");
    fs::create_dir(&dir)?;
    fs::create_dir(dir.join("maps"))?;
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
        attempt_limit,
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
    for line in fs::read_to_string(dir.join("manifest"))?.lines() {
        let (hash, name) = line
            .split_once(' ')
            .ok_or_else(|| invalid("invalid plan manifest"))?;
        if name.contains("..") || Path::new(name).is_absolute() {
            return Err(invalid("invalid manifest path"));
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
    let cfg = TournamentConfig {
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
    let mut maps = Vec::new();
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
        map.validate(&cfg.config)
            .map_err(|e| invalid(format!("map: {e:?}")))?;
        maps.push(map);
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
            || row.config_hash != cfg.config_identity()
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
    if !(1..=100).contains(&attempt_limit) || cfg.concurrency == 0 {
        return Err(invalid("invalid execution policy"));
    }
    Ok(Plan {
        config: cfg,
        items,
        maps,
        attempt_limit,
    })
}

fn pending() -> TournamentOutcome {
    TournamentOutcome::Unscored {
        stage: Stage::RunnerFault,
        reason: "pending".into(),
    }
}
fn infrastructure(result: &ExternalResult) -> bool {
    matches!(
        result.outcome.stage(),
        Some(Stage::RunnerFault | Stage::SetupError)
    )
}

fn attempt_dirs(root: &Path, index: u64) -> io::Result<Vec<PathBuf>> {
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

fn read_result(dir: &Path, plan: &Plan, item: &TournamentItem) -> io::Result<ExternalResult> {
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
            || log.header.config != plan.config.config
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

fn artifact_manifest(dir: &Path, current: &Path, out: &mut String) -> io::Result<()> {
    let mut entries = fs::read_dir(current)?
        .map(|e| e.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            artifact_manifest(dir, &path, out)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(dir)
                .map_err(invalid)?
                .to_str()
                .ok_or_else(|| invalid("artifact name"))?;
            if matches!(relative, "artifacts" | "state") || relative.ends_with(".tmp") {
                continue;
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
    artifact_manifest(dir, dir, &mut actual)?;
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
            Some(dir) => match fs::read_to_string(dir.join("state"))?.as_str() {
                "completed" => {
                    read_result(dir, &plan, item)?;
                    completed += 1;
                }
                "infrastructure_fault" => faults += 1,
                "running" if active => running += 1,
                _ => interrupted += 1,
            },
        }
    }
    Ok(format!(
        "pending={pending} running={running} completed={completed} infrastructure_fault={faults} interrupted={interrupted}\n"
    ))
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
        // Validate the referenced build before launching anything (frozen build
        // registry comes in batch two; changed binaries are refused here).
        for entrant in &plan.config.entrants {
            let super::EntrantKind::External { binary, .. } = &entrant.kind;
            if entrant.provenance.binary_blake3.as_deref() != Some(&compute_binary_hash(binary)?) {
                return Err(invalid("registered binary changed"));
            }
        }
        std::thread::scope(|scope| -> io::Result<()> {
            let mut handles = Vec::new();
            for worker in 0..plan.config.concurrency.min(plan.items.len()) {
                let plan = &plan;
                handles.push(scope.spawn(move || -> io::Result<()> {
                    for item in plan
                        .items
                        .iter()
                        .skip(worker)
                        .step_by(plan.config.concurrency)
                    {
                        run_pending(root, plan, item, action)?;
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

fn run_pending(root: &Path, plan: &Plan, item: &TournamentItem, action: Action) -> io::Result<()> {
    let dirs = attempt_dirs(root, item.index)?;
    let mut reason = "initial execution";
    if let Some(last) = dirs.last() {
        let state = fs::read_to_string(last.join("state"))?;
        match state.as_str() {
            "completed" => {
                read_result(last, plan, item)?;
                return Ok(());
            }
            "infrastructure_fault" if !matches!(action, Action::Retry(i) if i == item.index) => {
                return Ok(());
            }
            "infrastructure_fault" => reason = "explicit bounded infrastructure retry",
            _ => {
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
    let parent = root.join(format!("matches/m{}/attempts", item.index));
    fs::create_dir_all(&parent)?;
    let dir = parent.join(format!("{:04}", dirs.len() + 1));
    fs::create_dir(&dir)?;
    atomic_write(&dir.join("reason.txt"), reason.as_bytes())?;
    atomic_write(&dir.join("state"), b"running")?;
    let result = run_item_external_on_map(
        item,
        &plan.config,
        &dir,
        Some(plan.maps[item.map_index as usize].clone()),
    );
    let save = || -> io::Result<()> {
        for (side, report) in &result.sandbox_reports {
            atomic_write(&dir.join(format!("sandbox_{side}.json")), report.as_bytes())?;
        }
        atomic_write(
            &dir.join("meta.json"),
            archive::meta_json_text(
                item,
                &result.outcome,
                &plan.config,
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
            &plan.config,
            result.replay_rel.as_deref(),
            result.elapsed_ms,
        );
        atomic_write(
            &dir.join("result.tsv"),
            archive::index_records_to_tsv(&[record]).as_bytes(),
        )?;
        let mut manifest = String::new();
        artifact_manifest(&dir, &dir, &mut manifest)?;
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

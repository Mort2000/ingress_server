//! Run a tournament between independently built external clients.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use arena_core::config::GameConfig;
use arena_runner::tournament::{
    ClientSandbox, EloConfig, Entrant, EntrantKind, EntrantProvenance, TMatchup, TournamentConfig,
    durable::{self, Action},
};

const USAGE: &str = "Usage: arena-runner tournament --out <new-directory> --client <binary> --client <binary> [options]\n\
  --client <binary>   Repeat for each entrant (at least two).\n\
  --matches <even>    Matches per distinct entrant pair (default: 2).\n\
  --maps <count>      Generated map pool (default: 1).\n\
  --rounds <count>    Rounds per match (default: 500).\n\
  --seed <decimal>    Master seed (default: 12648430).\n\
  --workers <count>  Concurrent matches (default: 1).\n\
  --sandbox <binary> Run clients through sandbox-runner.\n\
  --fast             Short decision windows for protocol smoke tests.\n\
  --name <name>      Optional registry name, once per client.\n\
  --bot-version <v> Optional Bot version, once per client.\n\
  --attempt-limit N Total attempts per item, including interruptions (default: 3).\n\
  plan [same options] saves inputs and previews without launching clients.\n\
  status --out DIR | resume --out DIR | rebuild --out DIR\n\
  retry --out DIR --item INDEX (infrastructure faults only).";

fn execute(args: &[String]) -> Result<(), String> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    if matches!(verb, "resume" | "retry" | "status" | "rebuild") {
        let mut out = None;
        let mut item = None;
        let mut options = args[1..].chunks_exact(2);
        for pair in &mut options {
            match pair[0].as_str() {
                "--out" => out = Some(PathBuf::from(&pair[1])),
                "--item" if verb == "retry" => {
                    item = Some(pair[1].parse::<u64>().map_err(|e| e.to_string())?)
                }
                _ => return Err(format!("unknown option {}", pair[0])),
            }
        }
        if !options.remainder().is_empty() {
            return Err("option requires a value".into());
        }
        let out = out.ok_or("--out is required")?;
        if verb == "status" {
            print!("{}", durable::status(&out).map_err(|e| e.to_string())?);
            return Ok(());
        }
        let action = match verb {
            "retry" => Action::Retry(item.ok_or("--item is required")?),
            "rebuild" => Action::Rebuild,
            _ => Action::Resume,
        };
        durable::execute(&out, action).map_err(|e| e.to_string())?;
        print!("{}", durable::status(&out).map_err(|e| e.to_string())?);
        return Ok(());
    }
    if !matches!(verb, "tournament" | "plan") {
        return Err(USAGE.into());
    }
    let mut out = None;
    let mut clients = Vec::new();
    let mut matches = 2u32;
    let mut maps = 1u32;
    let mut rounds = 500u32;
    let mut seed = 12_648_430u64;
    let mut workers = 1usize;
    let mut sandbox = None;
    let mut fast = false;
    let mut attempt_limit = 3u32;
    let mut names: Vec<String> = Vec::new();
    let mut versions: Vec<String> = Vec::new();
    let mut options = args[1..].iter();
    while let Some(flag) = options.next() {
        if flag == "--fast" {
            fast = true;
            continue;
        }
        let value = options
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        let bad = || format!("invalid {flag}: {value}");
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(value)),
            "--client" => clients.push(PathBuf::from(value)),
            "--name" => names.push(value.clone()),
            "--bot-version" => versions.push(value.clone()),
            "--attempt-limit" => attempt_limit = value.parse().map_err(|_| bad())?,
            "--matches" => matches = value.parse().map_err(|_| bad())?,
            "--maps" => maps = value.parse().map_err(|_| bad())?,
            "--rounds" => rounds = value.parse().map_err(|_| bad())?,
            "--seed" => seed = value.parse().map_err(|_| bad())?,
            "--workers" => workers = value.parse().map_err(|_| bad())?,
            "--sandbox" => sandbox = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option: {flag}")),
        }
    }
    let out = out.ok_or("--out is required")?;
    if clients.len() < 2 {
        return Err("at least two --client entries are required".into());
    }
    if (!names.is_empty() && names.len() != clients.len())
        || (!versions.is_empty() && versions.len() != clients.len())
    {
        return Err(
            "--name and --bot-version must each be omitted or supplied once per client".into(),
        );
    }
    if !(1..=100).contains(&attempt_limit) {
        return Err("--attempt-limit must be 1..100".into());
    }
    if matches == 0 || matches % 2 != 0 {
        return Err("--matches must be positive and even".into());
    }
    let config = if fast {
        arena_server::fast_config(rounds)
    } else {
        GameConfig {
            total_rounds: rounds,
            ..GameConfig::default()
        }
    };
    config
        .validate()
        .map_err(|error| format!("invalid rules: {error:?}"))?;
    let mut entrants = Vec::new();
    for (index, binary) in clients.into_iter().enumerate() {
        let binary = binary
            .canonicalize()
            .map_err(|error| format!("{}: {error}", binary.display()))?;
        let label = binary
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("client");
        let mut entrant = Entrant {
            name: names
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("{label}-{}", index + 1)),
            kind: EntrantKind::External {
                binary,
                env: vec![],
            },
            provenance: EntrantProvenance {
                bot_version: versions.get(index).cloned(),
                ..Default::default()
            },
        };
        entrant
            .compute_binary_hash()
            .map_err(|error| error.to_string())?;
        entrants.push(entrant);
    }
    let mut matchups = Vec::new();
    for a in 0..entrants.len() as u32 {
        for b in a + 1..entrants.len() as u32 {
            matchups.push(TMatchup::new(&format!("{a}-{b}"), a, b, matches));
        }
    }
    let budget = config.initial_spawn_window_ms as u64
        + rounds as u64 * (config.decision_window_ms as u64 + 100)
        + 20_000;
    let sandbox = match sandbox {
        Some(runner) => ClientSandbox::SandboxRunner {
            runner: runner.canonicalize().map_err(|error| error.to_string())?,
            wall_ms: budget,
            address_space_bytes: 512 * 1024 * 1024,
        },
        None => ClientSandbox::Off,
    };
    let cfg = TournamentConfig {
        name: out
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("tournament")
            .into(),
        master_seed: seed,
        entrants,
        matchups,
        map_count: maps,
        rounds_per_match: rounds,
        config,
        elo: EloConfig::default(),
        concurrency: workers,
        sandbox,
        per_match_budget_ms: budget,
    };
    cfg.validate()?;
    if let Some(parent) = out.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    // A fresh directory is mandatory: never overwrite an archived tournament.
    std::fs::create_dir(&out).map_err(|error| format!("{}: {error}", out.display()))?;
    let plan = durable::prepare(&cfg, &out, attempt_limit).map_err(|error| error.to_string())?;
    println!(
        "{} matches planned in {} ({} maps; per-pair matches cycle maps, not per-map pairs)",
        plan.items.len(),
        out.display(),
        plan.maps.len()
    );
    if verb == "tournament" {
        durable::execute(&out, Action::Resume).map_err(|error| error.to_string())?;
    }
    print!(
        "{}",
        durable::status(&out).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match execute(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

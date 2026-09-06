//! Run a tournament between independently built external clients.
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use arena_core::config::GameConfig;
use arena_runner::tournament::{
    ClientSandbox, EloConfig, Entrant, EntrantKind, EntrantProvenance, TMatchup, TournamentConfig,
    run_external_tournament, write_tournament_artifacts,
};

const USAGE: &str = "Usage: arena-runner tournament --out <new-directory> --client <binary> --client <binary> [options]\n\
  --client <binary>   Repeat for each entrant (at least two).\n\
  --matches <even>    Matches per distinct entrant pair (default: 2).\n\
  --maps <count>      Generated map pool (default: 1).\n\
  --rounds <count>    Rounds per match (default: 500).\n\
  --seed <decimal>    Master seed (default: 12648430).\n\
  --workers <count>  Concurrent matches (default: 1).\n\
  --sandbox <binary> Run clients through sandbox-runner.\n\
  --fast             Short decision windows for protocol smoke tests.";

fn execute(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some("tournament") {
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
            name: format!("{label}-{}", index + 1),
            kind: EntrantKind::External {
                binary,
                env: vec![],
            },
            provenance: EntrantProvenance::default(),
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
    let run = run_external_tournament(&cfg, &out).map_err(|error| format!("{error:?}"))?;
    write_tournament_artifacts(&cfg, &run, &out, None).map_err(|error| error.to_string())?;
    println!("{} matches archived in {}", run.items.len(), out.display());
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

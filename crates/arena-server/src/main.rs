//! arena-server binary: one match per invocation.
//!
//! ```text
//! arena-server --socket /tmp/match.s --seed 0xC0FFEE --map-seed 7 \
//!              --config fast --rounds 10 --match-id m-0001 --out /tmp/out
//! ```
//!
//! Exit codes: 0 normal (enl_win/res_win/draw); 3/4 spawn forfeit with
//! winner ENL/RES; 5 unscored spawn forfeit; 6/7 client forfeit with
//! winner ENL/RES; 8 unscored client forfeit; 9 setup error. 1 = usage/io.

use std::path::PathBuf;

fn main() {
    let mut socket = PathBuf::from("/tmp/ingress-arena.sock");
    let mut seed: u64 = 0;
    let mut map_seed: u64 = 0;
    let mut preset = String::from("default");
    let mut rounds: Option<u32> = None;
    let mut match_id = String::new();
    let mut out_dir = PathBuf::from("/tmp/ingress-arena-out");

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match flag.as_str() {
            "--socket" => socket = PathBuf::from(value()),
            "--seed" => seed = value().parse().unwrap_or(0),
            "--map-seed" => map_seed = value().parse().unwrap_or(0),
            "--config" => preset = value(),
            "--rounds" => rounds = value().parse().ok(),
            "--match-id" => match_id = value(),
            "--out" => out_dir = PathBuf::from(value()),
            other => {
                eprintln!("unknown flag {other:?}");
                std::process::exit(1);
            }
        }
    }
    if match_id.is_empty() {
        match_id = format!("m-{}-{}", std::process::id(), seed);
    }

    let mut config = match preset.as_str() {
        "default" => arena_core::config::GameConfig::default(),
        "fast" => arena_server::fast_config(rounds.unwrap_or(10)),
        other => {
            eprintln!("unknown config preset {other:?} (default|fast)");
            std::process::exit(1);
        }
    };
    if let Some(rounds) = rounds {
        config.total_rounds = rounds;
    }
    if let Err(error) = config.validate() {
        eprintln!("invalid config: {error:?}");
        std::process::exit(1);
    }

    let out_dir = out_dir.join(&match_id);
    let setup = arena_server::MatchSetup {
        game_seed: seed,
        map_seed,
        config,
        socket_path: socket,
        match_id,
        out_dir: out_dir.clone(),
    };
    match arena_server::serve_match(setup) {
        Ok(report) => {
            println!("outcome={}", report.outcome.tag());
            println!("rounds_completed={}", report.rounds_completed);
            println!("out={}", out_dir.display());
            std::process::exit(match &report.outcome {
                arena_server::MatchOutcome::Normal { .. } => 0,
                arena_server::MatchOutcome::SpawnForfeit { winner, .. } => match winner {
                    Some(arena_core::types::Faction::Enl) => 3,
                    Some(arena_core::types::Faction::Res) => 4,
                    None => 5,
                },
                arena_server::MatchOutcome::ClientForfeit { winner, .. } => match winner {
                    Some(arena_core::types::Faction::Enl) => 6,
                    Some(arena_core::types::Faction::Res) => 7,
                    None => 8,
                },
                arena_server::MatchOutcome::SetupError { .. } => 9,
            });
        }
        Err(error) => {
            eprintln!("serve_match failed: {error}");
            std::process::exit(1);
        }
    }
}

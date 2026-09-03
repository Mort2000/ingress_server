//! replay-export CLI (D87):
//!
//! ```text
//! replay-export <replay.iarp> [-o out.html] [--meta <meta.json>]
//!                             [--dev 0|1] [--faction enl|res]
//! ```
//!
//! Verifies the IARP first (`arena_replay::replay`, the P6-GATE reference);
//! an unverified replay is an error and NO HTML is written (D88). On
//! success prints one summary line (rounds, result, final hash, final
//! score2 — taken from the SAME verified outcome embedded into the page)
//! plus the output path. Exit codes: 0 ok, 1 usage/IO, 2 unverified input.

use std::path::PathBuf;

use arena_core::types::Faction;

use replay_export::data::{self, ExportOptions};
use replay_export::json::{self, Json};

fn main() {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut meta_path: Option<PathBuf> = None;
    let mut dev: Option<bool> = None;
    let mut faction: Option<Faction> = None;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage_exit(&flag));
        match flag.as_str() {
            "-o" | "--out" => output = Some(PathBuf::from(value())),
            "--meta" => meta_path = Some(PathBuf::from(value())),
            "--dev" => match value().as_str() {
                "0" => dev = Some(false),
                "1" => dev = Some(true),
                other => {
                    eprintln!("--dev expects 0 or 1, got {other:?}");
                    std::process::exit(1);
                }
            },
            "--faction" => match value().as_str() {
                "enl" => faction = Some(Faction::Enl),
                "res" => faction = Some(Faction::Res),
                other => {
                    eprintln!("--faction expects enl or res, got {other:?}");
                    std::process::exit(1);
                }
            },
            "-h" | "--help" => {
                println!(
                    "replay-export <replay.iarp> [-o out.html] [--meta <meta.json>] [--dev 0|1] [--faction enl|res]"
                );
                std::process::exit(0);
            }
            other if other.starts_with('-') => {
                eprintln!("unknown flag {other:?}");
                eprintln!(
                    "usage: replay-export <replay.iarp> [-o out.html] [--meta <meta.json>] [--dev 0|1] [--faction enl|res]"
                );
                std::process::exit(1);
            }
            other => {
                if input.is_some() {
                    eprintln!("unexpected extra argument {other:?}");
                    std::process::exit(1);
                }
                input = Some(PathBuf::from(other));
            }
        }
    }

    let input = input.unwrap_or_else(|| {
        eprintln!("usage: replay-export <replay.iarp> [-o out.html] [--meta <meta.json>] [--dev 0|1] [--faction enl|res]");
        std::process::exit(1);
    });
    let iarp = std::fs::read(&input).unwrap_or_else(|error| {
        eprintln!("cannot read {}: {error}", input.display());
        std::process::exit(1);
    });

    // Defaults (D90): --dev 0 REQUIRES --faction; --dev 1 embeds both.
    let dev = dev.unwrap_or(false);
    let options = if dev {
        if faction.is_some() {
            eprintln!("--faction applies only to --dev 0 (dev mode embeds both factions)");
            std::process::exit(1);
        }
        ExportOptions {
            dev: true,
            faction: None,
            meta: read_meta(meta_path.as_deref()),
        }
    } else {
        ExportOptions {
            dev: false,
            faction: Some(faction.unwrap_or_else(|| {
                eprintln!("production exports must name the embedded faction: pass --faction enl or --faction res (or use --dev 1)");
                std::process::exit(1);
            })),
            meta: read_meta(meta_path.as_deref()),
        }
    };

    let output = output.unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        input.with_file_name(format!("{stem}.html"))
    });

    // Everything (including full replay verification) happens BEFORE the
    // file is created: a refused export leaves no HTML behind.
    let exported = match data::export(&iarp, &options) {
        Ok(exported) => exported,
        Err(error) => {
            eprintln!("replay-export: {error}");
            eprintln!("no HTML written");
            std::process::exit(2);
        }
    };
    std::fs::write(&output, &exported.html).unwrap_or_else(|error| {
        eprintln!("cannot write {}: {error}", output.display());
        std::process::exit(1);
    });

    let factions = options
        .dev
        .then(|| "enl,res".to_string())
        .unwrap_or_else(|| match options.faction {
            Some(Faction::Enl) => "enl".to_string(),
            _ => "res".to_string(),
        });
    println!("{}", exported.summary);
    println!(
        "exported dev={} factions={} -> {}",
        options.dev as u8,
        factions,
        output.display()
    );
}

fn read_meta(path: Option<&std::path::Path>) -> Option<Json> {
    let path = path?;
    let text = std::fs::read_to_string(path).unwrap_or_else(|error| {
        eprintln!("cannot read {}: {error}", path.display());
        std::process::exit(1);
    });
    Some(json::parse_json(&text).unwrap_or_else(|error| {
        eprintln!(
            "{} is not strict mini-JSON (integers only): {error}",
            path.display()
        );
        std::process::exit(1);
    }))
}

fn usage_exit(flag: &str) -> ! {
    eprintln!("{flag} expects a value");
    eprintln!(
        "usage: replay-export <replay.iarp> [-o out.html] [--meta <meta.json>] [--dev 0|1] [--faction enl|res]"
    );
    std::process::exit(1);
}

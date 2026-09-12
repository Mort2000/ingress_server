//! External-client tournaments with recorded seeds, provenance, replays, and ratings.

pub mod archive;
pub mod assets_cli;
pub mod builds;
pub mod comparison;
pub mod config;
mod document;
pub mod durable;
pub mod elo;
pub mod entrant;
pub mod event;
pub mod exec_external;
mod logs;
pub mod maps;
pub mod outcome;
pub mod report;
pub mod run;
pub mod schedule;
pub mod selection;

pub use config::{ClientSandbox, EloConfig, TMatchup, TournamentConfig};
pub use entrant::{Entrant, EntrantId, EntrantKind, EntrantProvenance};
pub use outcome::{Stage, TournamentOutcome};
pub use run::{TournamentRun, run_external_tournament};
pub use schedule::{TournamentError, TournamentItem, build_items};

/// Seed domain labels (D80) — re-exported for tests and artifact docs.
pub const MAP_SEED_LABEL: &[u8] = schedule::MAP_SEED_LABEL;
pub const GAME_SEED_LABEL: &[u8] = schedule::GAME_SEED_LABEL;
pub const CLIENT_SEED_LABEL: &[u8] = schedule::CLIENT_SEED_LABEL;
pub const SCHEDULE_SEED_LABEL: &[u8] = schedule::SCHEDULE_SEED_LABEL;
/// blake3 domain over external entrant binary file bytes.
pub const BINARY_HASH_DOMAIN: &[u8] = b"ingress-arena-m9-binary";

/// Everything a finished run leaves on disk, in one call:
/// `tournament.json` + per-match `meta.json` + `index.tsv` +
/// `ratings.tsv` + `standings.txt` under `root`, and (optionally) the
/// committed report at `doc_path` generated from the same data.
pub fn write_tournament_artifacts(
    cfg: &TournamentConfig,
    run: &TournamentRun,
    root: &std::path::Path,
    doc_path: Option<&std::path::Path>,
) -> std::io::Result<()> {
    let records = archive::write_archive(root, cfg, run)?;
    let mut table = elo::EloTable::new(&cfg.elo, cfg.entrants.len());
    for (item, outcome) in run.items.iter().zip(run.outcomes.iter()) {
        let _ = table.apply(item, outcome);
    }
    let splits = report::faction_splits(cfg, &run.items, &run.outcomes);
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("standings.txt"),
        report::standings_text(cfg, &table, &splits),
    )?;
    if let Some(doc_path) = doc_path {
        if let Some(dir) = doc_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(
            doc_path,
            report::tournament_doc_markdown(cfg, run, &table, &splits, &records),
        )?;
    }
    Ok(())
}

//! The frozen 10,000-match M6 experiment (P2.5-GATE, D58/D59):
//!
//! ```text
//! Random x Random            1000      Greedy x DestroyAnchor  1750
//! Nearest x Nearest          1500      Greedy x Hybrid         1750
//! Nearest x Greedy           1750      Hybrid x Hybrid         2250
//! ```
//!
//! on 500 distinct generated default-config maps (shared across matchups),
//! `Flat(0)` virtual buckets, 12 threads, all seeds derived from one
//! master seed. Artifacts land in `runs/m6-r1/`. A second, explicit
//! cross-bucket batch (120 Hybrid x Hybrid matches under
//! `Factions { enl: 0, res: 1 }`) lands in `runs/m6-r1-crossbucket/` —
//! end-to-end evidence that bucket order dominates parity (D19/D20/D56).

#![forbid(unsafe_code)]

use std::time::Instant;

use arena_bots::bot::BotKind;
use arena_core::config::GameConfig;
use arena_runner::batch::{BatchConfig, Matchup};
use arena_runner::policy::BucketPolicy;
use arena_runner::report::{BatchSummary, summary_text, write_artifacts};
use arena_runner::run::{MAX_WORKERS, run_batch};
use arena_runner::tournament::config::{ClientSandbox, EloConfig, TMatchup, TournamentConfig};
use arena_runner::tournament::entrant::{Entrant, EntrantKind, EntrantProvenance};
use arena_runner::tournament::run::{run_external_tournament, run_inprocess_tournament};
use arena_runner::tournament::write_tournament_artifacts;

/// The frozen master seed of experiment round 1 (D58).
pub const M6_R1_MASTER_SEED: u64 = 0x006D_3600_0000_0031;

/// The frozen master seed of tournament round 1 (M9, D80): the Benchmark
/// Seed Set (map seeds, schedule seed, game/client seeds) all derive from
/// it and the derivation is disjoint from every M6 domain.
pub const M9_R1_MASTER_SEED: u64 = 0x006D_3900_0000_0032;

/// Recorded build provenance for the r1 clients (D79): the id of the
/// `tools/provenance` manifest recorded for this tree. The tree/lock/
/// toolchain hashes are READ from that manifest at run time (a manifest
/// can never hash the sentence citing it — the M8 convention), and the
/// runner computes the exact binary hashes itself over the files it
/// actually executes.
pub const M9_R1_MANIFEST_ID: &str = "runs/builds/20260903-0221-578243c3d20f";

/// Provenance-as-data for one r1 entrant: the recorded manifest's
/// tree/lock/toolchain literals (when the manifest file exists) plus the
/// computed binary hash.
fn m9_provenance(binary: Option<String>) -> EntrantProvenance {
    let (tree_hash, lock_hash, toolchain_hash) = match std::fs::read_to_string(
        std::path::Path::new(M9_R1_MANIFEST_ID).join("manifest.json"),
    ) {
        Ok(text) => match arena_runner::tournament::archive::parse_json(&text) {
            Ok(json) => {
                let field = |path: [&str; 2]| -> Option<String> {
                    json.get(path[0])
                        .and_then(|value| value.get(path[1]))
                        .and_then(|value| value.as_str())
                        .map(|text| text.to_string())
                };
                (
                    field(["tree", "hash"]),
                    field(["lock", "hash"]),
                    field(["toolchain", "hash"]),
                )
            }
            Err(_) => (None, None, None),
        },
        Err(_) => (None, None, None),
    };
    EntrantProvenance {
        binary_blake3: binary,
        tree_hash,
        lock_hash,
        toolchain_hash,
        manifest_id: Some(M9_R1_MANIFEST_ID.to_string()),
        bot_version: Some(arena_bots::bot::BOT_VERSION.to_string()),
    }
}

fn external_entrant(name: &str, binary_name: &str, env: &[(&str, &str)]) -> Option<Entrant> {
    let binary = arena_runner::tournament::exec_external::find_workspace_binary(binary_name)?;
    let hash = arena_runner::tournament::entrant::compute_binary_hash(&binary).ok()?;
    Some(Entrant {
        name: name.to_string(),
        kind: EntrantKind::External {
            binary,
            env: env
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        },
        provenance: m9_provenance(Some(hash)),
    })
}

fn in_process_entrant(name: &str, bot: BotKind) -> Entrant {
    Entrant {
        name: name.to_string(),
        kind: EntrantKind::InProcess { bot },
        provenance: EntrantProvenance::in_process(arena_bots::bot::BOT_VERSION),
    }
}

/// The frozen external tournament r1 (D80/D85): 14 single matches over a
/// 4-map Benchmark Seed Set, sandboxed clients, 100 rounds each, every
/// count even so the D55 balance holds exactly. (The plan sketch said 24
/// rounds; the smoke run showed 24 ends before ANY faction completes a
/// field — all matches 0-0 draws, no ranking signal — so r1 runs 100
/// rounds. Prompt COMMITs settle rounds early, so wall clock stays ~2-3 s
/// per match.) Entrants:
/// `nearest` / `hybrid` (the frozen M6 algorithms as external SDK
/// clients) and `probe` (the diagnostic fixture in its well-behaved
/// mode — it commits promptly and plays empty rounds).
fn frozen_tournament_r1() -> Option<TournamentConfig> {
    let nearest = external_entrant("nearest", "nearest-client", &[])?;
    let hybrid = external_entrant("hybrid", "hybrid-client", &[])?;
    let probe = external_entrant("probe", "probe-client", &[("PROBE_MODE", "valid")])?;
    let sandbox_runner =
        arena_runner::tournament::exec_external::find_workspace_binary("sandbox-runner")?;
    Some(TournamentConfig {
        name: "r1".to_string(),
        master_seed: M9_R1_MASTER_SEED,
        entrants: vec![nearest, hybrid, probe],
        matchups: vec![
            TMatchup::new("nearest-hybrid", 0, 1, 6),
            TMatchup::new("nearest-probe", 0, 2, 4),
            TMatchup::new("hybrid-probe", 1, 2, 4),
        ],
        map_count: 4,
        rounds_per_match: 100,
        config: arena_server::fast_config(100),
        elo: EloConfig::default(),
        concurrency: 3,
        sandbox: ClientSandbox::SandboxRunner {
            runner: sandbox_runner,
            wall_ms: 20_000,
            address_space_bytes: 2 << 30,
        },
        per_match_budget_ms: 20_000,
    })
}

/// The frozen in-process tournament r1 (D86): the M6 engine + M5 recorder
/// path over the same scheduling machinery.
fn frozen_tournament_inprocess_r1() -> TournamentConfig {
    let entrants = vec![
        in_process_entrant("nearest", BotKind::NearestPortal),
        in_process_entrant("greedy", BotKind::GreedyField),
        in_process_entrant("hybrid", BotKind::Hybrid),
    ];
    let mut config = GameConfig::default();
    config.total_rounds = 60;
    TournamentConfig {
        name: "inproc-r1".to_string(),
        master_seed: M9_R1_MASTER_SEED ^ 0x1D0C_E5E5_0000_0000,
        entrants,
        matchups: vec![
            TMatchup::new("nearest-greedy", 0, 1, 2),
            TMatchup::new("greedy-hybrid", 1, 2, 2),
        ],
        map_count: 2,
        rounds_per_match: 60,
        config,
        elo: EloConfig::default(),
        concurrency: 2,
        sandbox: ClientSandbox::Off,
        per_match_budget_ms: 30_000,
    }
}

fn run_tournament_r1(output_root: &std::path::Path) {
    let cfg = frozen_tournament_r1().expect(
        "client binaries + sandbox-runner must be built (cargo build --release --workspace)",
    );
    println!(
        "arena-runner: tournament r1 — {} matches over {} maps, sandbox on, budget {} ms",
        cfg.total_matches(),
        cfg.map_count,
        cfg.per_match_budget_ms
    );
    let start = Instant::now();
    let root = output_root.join("tournaments").join(&cfg.name);
    let run = run_external_tournament(&cfg, &root).expect("the frozen tournament runs");
    let doc = output_root.join("..").join("docs").join("tournament-r1.md");
    write_tournament_artifacts(&cfg, &run, &root, Some(&doc)).expect("artifacts write");
    let scored = run.outcomes.iter().filter(|o| o.elo_relevant()).count();
    let technical = run
        .outcomes
        .iter()
        .filter(|o| {
            matches!(
                o,
                arena_runner::tournament::TournamentOutcome::TechnicalWin { .. }
            )
        })
        .count();
    let unscored = run.outcomes.len() - scored - technical;
    println!(
        "arena-runner: tournament r1 done in {:?} ({} matches: {} scored, {} technical, {} unscored)",
        start.elapsed(),
        run.outcomes.len(),
        scored,
        technical,
        unscored
    );
    println!("arena-runner: artifacts in {}", root.display());
    println!("arena-runner: committed report at {}", doc.display());
    print!(
        "{}",
        std::fs::read_to_string(root.join("standings.txt")).expect("standings")
    );
}

fn run_tournament_inprocess_r1(output_root: &std::path::Path) {
    let cfg = frozen_tournament_inprocess_r1();
    println!(
        "arena-runner: in-process tournament r1 — {} matches, {} rounds each",
        cfg.total_matches(),
        cfg.rounds_per_match
    );
    let start = Instant::now();
    let root = output_root.join("tournaments").join(&cfg.name);
    let run = run_inprocess_tournament(&cfg, &root).expect("the frozen in-process tournament runs");
    write_tournament_artifacts(&cfg, &run, &root, None).expect("artifacts write");
    println!(
        "arena-runner: in-process tournament r1 done in {:?} ({} matches)",
        start.elapsed(),
        run.outcomes.len()
    );
    println!("arena-runner: artifacts in {}", root.display());
    print!(
        "{}",
        std::fs::read_to_string(root.join("standings.txt")).expect("standings")
    );
}

/// The frozen matchup table (all counts even; each bot gets exactly M/2
/// ENL and M/2 RES slots, D55).
pub fn frozen_matchups() -> Vec<Matchup> {
    vec![
        Matchup::new("Random-Random", BotKind::Random, BotKind::Random, 1000),
        Matchup::new(
            "Nearest-Nearest",
            BotKind::NearestPortal,
            BotKind::NearestPortal,
            1500,
        ),
        Matchup::new(
            "Nearest-Greedy",
            BotKind::NearestPortal,
            BotKind::GreedyField,
            1750,
        ),
        Matchup::new(
            "Greedy-DestroyAnchor",
            BotKind::GreedyField,
            BotKind::DestroyAnchor,
            1750,
        ),
        Matchup::new("Greedy-Hybrid", BotKind::GreedyField, BotKind::Hybrid, 1750),
        Matchup::new("Hybrid-Hybrid", BotKind::Hybrid, BotKind::Hybrid, 2250),
    ]
}

fn main() {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(MAX_WORKERS);
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("tournament-r1") => {
            let output_root =
                std::path::PathBuf::from(args.get(2).map(|s| s.as_str()).unwrap_or("runs"));
            run_tournament_r1(&output_root);
            return;
        }
        Some("tournament-inprocess-r1") => {
            let output_root =
                std::path::PathBuf::from(args.get(2).map(|s| s.as_str()).unwrap_or("runs"));
            run_tournament_inprocess_r1(&output_root);
            return;
        }
        Some(other) if other.starts_with("tournament") => {
            eprintln!(
                "unknown tournament subcommand {other:?} (tournament-r1 | tournament-inprocess-r1)"
            );
            std::process::exit(1);
        }
        _ => {}
    }
    let output_root = std::path::PathBuf::from(args.get(1).map(|s| s.as_str()).unwrap_or("runs"));

    // ---------------------------------------------------------------
    // The frozen 10,000-match balance batch (Flat(0)).
    // ---------------------------------------------------------------
    let balance = BatchConfig {
        batch_id: "m6-r1".to_string(),
        matchups: frozen_matchups(),
        map_count: 500,
        master_seed: M6_R1_MASTER_SEED,
        bucket_policy: BucketPolicy::Flat(0),
        config: GameConfig::default(),
    };
    println!(
        "arena-runner: {} matches on {} maps, {workers} threads, bucket policy {}",
        balance.total_matches(),
        balance.map_count,
        balance.bucket_policy.tag()
    );
    let start = Instant::now();
    let run = run_batch(&balance, workers).expect("the frozen batch runs");
    println!(
        "arena-runner: batch m6-r1 done in {:?} ({} matches, {} crashes)",
        start.elapsed(),
        run.records.len(),
        run.records
            .iter()
            .filter(|r| r.result == arena_runner::metrics::Outcome::Crash)
            .count()
    );
    let dir = output_root.join("m6-r1");
    write_artifacts(&dir, &balance, &run.records, run.workers, run.elapsed_ms)
        .expect("write m6-r1 artifacts");
    let summary = BatchSummary::from_records(&run.records, &balance.matchups);
    println!(
        "arena-runner: artifacts in {} (manifest.txt, matches.tsv, summary.txt)",
        dir.display()
    );
    println!();
    print!(
        "{}",
        summary_text(&summary, balance.config.portal_count as u64)
    );

    // ---------------------------------------------------------------
    // The explicit cross-bucket batch (D56): bucket order dominates
    // parity — ENL always executes first. Still equal ENL/RES exposure
    // per bot through the schedule.
    // ---------------------------------------------------------------
    let cross = BatchConfig {
        batch_id: "m6-r1-crossbucket".to_string(),
        matchups: vec![Matchup::new(
            "Hybrid-Hybrid",
            BotKind::Hybrid,
            BotKind::Hybrid,
            120,
        )],
        map_count: 60,
        master_seed: M6_R1_MASTER_SEED ^ 0xC505_B0C0_0000_0000,
        bucket_policy: BucketPolicy::Factions { enl: 0, res: 1 },
        config: GameConfig::default(),
    };
    let start = Instant::now();
    let run = run_batch(&cross, workers).expect("the cross-bucket batch runs");
    println!(
        "arena-runner: batch m6-r1-crossbucket done in {:?} ({} matches)",
        start.elapsed(),
        run.records.len()
    );
    let dir = output_root.join("m6-r1-crossbucket");
    write_artifacts(&dir, &cross, &run.records, run.workers, run.elapsed_ms)
        .expect("write cross-bucket artifacts");
    let summary = BatchSummary::from_records(&run.records, &cross.matchups);
    print!(
        "{}",
        summary_text(&summary, cross.config.portal_count as u64)
    );
}

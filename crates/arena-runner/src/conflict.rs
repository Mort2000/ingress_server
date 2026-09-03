//! Offline conflict statistics (A-LATENCY, D70 realized): the operator
//! entry point behind the `arena-runner conflict-stats` subcommand.
//!
//! Two evidence sources, both through the same
//! verify-then-analyze pipeline ([`arena_replay::analysis`]):
//!
//! 1. a FROZEN self-play batch — 8 in-process default-config 500-round
//!    matches (Hybrid x Hybrid and GreedyField x DestroyAnchor, the
//!    conflict-richest M6 pairings) recorded through the M5 recorder and
//!    archived under `<root>/conflict-r1/`;
//! 2. every archived tournament IARP under `<root>/tournaments/`
//!    (the r1 external matches and the in-process r1 twins).
//!
//! Everything prints to stdout; nothing feeds back into any match. All
//! aggregation is integer counts and permille.

use std::path::Path;

use arena_bots::bot::{BotKind, BotStrategy, make_bot};
use arena_core::config::GameConfig;
use arena_core::harness::MatchHarness;
use arena_core::map::generate_map;
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::Faction;
use arena_replay::analysis::{self, ConflictReport};
use arena_replay::recorder::ReplayWriter;

/// Frozen master seed of the conflict-analysis batch round 1.
pub const CONFLICT_R1_MASTER_SEED: u64 = 0x006D_3A00_0000_00C1;

/// The frozen pairings (4 matches each; the bots' fixed M6 versions).
pub fn conflict_pairings() -> [(BotKind, BotKind); 2] {
    [
        (BotKind::Hybrid, BotKind::Hybrid),
        (BotKind::GreedyField, BotKind::DestroyAnchor),
    ]
}

/// One recorded self-play match: fresh bots, default config, every round
/// through the recorder, finished normally.
fn play_recorded(
    game_seed: u64,
    map_seed: u64,
    bot_seed: u64,
    enl: BotKind,
    res: BotKind,
) -> Vec<u8> {
    let cfg = GameConfig::default();
    let map = generate_map(map_seed, &cfg).expect("default-config map generates");
    let mut enl_bot = make_bot(enl, Faction::Enl, bot_seed, &cfg, &map);
    let mut res_bot = make_bot(res, Faction::Res, bot_seed, &cfg, &map);
    let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
    let enl_spawns = validate_initial_spawn_set(
        &enl_roster,
        &enl_bot.choose_spawns(&cfg, &map, &enl_roster),
        &cfg,
    )
    .expect("bot spawn set");
    let res_spawns = validate_initial_spawn_set(
        &res_roster,
        &res_bot.choose_spawns(&cfg, &map, &res_roster),
        &cfg,
    )
    .expect("bot spawn set");
    let mut writer = ReplayWriter::new(game_seed, &cfg, &map, &enl_spawns, &res_spawns);
    let mut harness = MatchHarness::new(
        game_seed,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(BotStrategy(enl_bot)),
        Box::new(BotStrategy(res_bot)),
    )
    .expect("harness builds from validated inputs");
    for _ in 0..cfg.total_rounds {
        let report = harness.run_round();
        writer
            .record_round(&report, harness.state())
            .expect("record");
    }
    writer.finish(harness.state()).expect("finish")
}

/// blake3-derived u64 seed (the D58/D80 convention).
fn derive(label: &[u8], payload: &[u8]) -> u64 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(label);
    hasher.update(payload);
    let bytes = hasher.finalize();
    u64::from_le_bytes(bytes.as_bytes()[..8].try_into().expect("8 bytes"))
}

/// Runs the whole command: generates + analyzes the frozen batch, then
/// analyzes every archived tournament replay found under `root`.
/// Returns the process exit code (0 success, 1 nothing was analyzed).
pub fn run_conflict_stats(root: &Path) -> i32 {
    let mut lines: Vec<String> = Vec::new();
    let mut totals = Totals::default();
    let mut analyzed = 0u32;

    // --- Source 1: the frozen self-play batch -------------------------
    let batch_dir = root.join("conflict-r1");
    if std::fs::create_dir_all(&batch_dir).is_err() {
        eprintln!("conflict-stats: cannot create {}", batch_dir.display());
        return 1;
    }
    let pairings = conflict_pairings();
    let mut index = 0u32;
    for (tag, (enl_kind, res_kind)) in [
        ("Hybrid-Hybrid", pairings[0]),
        ("GreedyField-DestroyAnchor", pairings[1]),
    ] {
        for j in 0..4u32 {
            let game_seed = derive(
                b"ingress-arena-conflict-game",
                &format!("{CONFLICT_R1_MASTER_SEED:#x}/{tag}/{j}").as_bytes(),
            );
            let map_seed = derive(
                b"ingress-arena-conflict-map",
                &format!("{CONFLICT_R1_MASTER_SEED:#x}/{tag}/{j}").as_bytes(),
            );
            let bot_seed = derive(
                b"ingress-arena-conflict-bot",
                &format!("{CONFLICT_R1_MASTER_SEED:#x}/{tag}/{j}").as_bytes(),
            );
            let iarp = play_recorded(game_seed, map_seed, bot_seed, enl_kind, res_kind);
            let report = analysis::analyze_conflicts_bytes(&iarp)
                .unwrap_or_else(|error| panic!("self-play batch item {index}: {error}"));
            let path = batch_dir.join(format!("m{index}.iarp"));
            if std::fs::write(&path, &iarp).is_err() {
                eprintln!("conflict-stats: cannot write {}", path.display());
                return 1;
            }
            lines.push(format!(
                "conflict-r1/m{index}.iarp {tag}#j{j}: {}",
                report.summary()
            ));
            totals.add(&report);
            analyzed += 1;
            index += 1;
        }
    }

    // --- Source 2: archived tournament replays ------------------------
    let tournaments = root.join("tournaments");
    let mut archives: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&tournaments) {
        for entry in entries.flatten() {
            let matches = entry.path().join("matches");
            let Ok(match_dirs) = std::fs::read_dir(&matches) else {
                continue;
            };
            for match_dir in match_dirs.flatten() {
                for candidate in ["server/replay.iarp", "replay.iarp"] {
                    let path = match_dir.path().join(candidate);
                    if path.is_file() {
                        archives.push(path);
                    }
                }
            }
        }
    }
    archives.sort();
    for path in &archives {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("conflict-stats: cannot read {}: {error}", path.display());
                return 1;
            }
        };
        let report = match analysis::analyze_conflicts_bytes(&bytes) {
            Ok(report) => report,
            Err(error) => {
                eprintln!("conflict-stats: {} failed: {error}", path.display());
                return 1;
            }
        };
        lines.push(format!("{}: {}", path.display(), report.summary()));
        totals.add(&report);
        analyzed += 1;
    }

    if analyzed == 0 {
        eprintln!("conflict-stats: nothing analyzed under {}", root.display());
        return 1;
    }
    for line in &lines {
        println!("{line}");
    }
    println!("TOTAL over {analyzed} replays: {}", totals.text());
    let summary_path = batch_dir.join("summary.txt");
    let mut summary = lines.join("\n");
    summary.push_str(&format!(
        "\nTOTAL over {analyzed} replays: {}\n",
        totals.text()
    ));
    if let Err(error) = std::fs::write(&summary_path, summary) {
        eprintln!(
            "conflict-stats: cannot write {}: {error}",
            summary_path.display()
        );
        return 1;
    }
    println!("summary written to {}", summary_path.display());
    0
}

/// Cross-replay sums (integer only).
#[derive(Default)]
struct Totals {
    replays: u64,
    rounds: u64,
    decisive: u64,
    indecisive: u64,
    winner_earlier_bucket: u64,
    winner_same_bucket_first_arrival: u64,
    loser_pre_empted: u64,
}

impl Totals {
    fn add(&mut self, report: &ConflictReport) {
        let a = &report.aggregate;
        self.replays += 1;
        self.rounds += report.rounds_analyzed as u64;
        self.decisive += a.decisive;
        self.indecisive += a.indecisive;
        self.winner_earlier_bucket += a.winner_earlier_bucket;
        self.winner_same_bucket_first_arrival += a.winner_same_bucket_first_arrival;
        self.loser_pre_empted += a.loser_pre_empted;
    }

    fn text(&self) -> String {
        let permille = |part: u64| ConflictReport::permille(part, self.decisive);
        format!(
            "replays={} rounds={} decisive={} indecisive={} \
             winner_earlier_bucket={} ({} permille) \
             winner_same_bucket_first_arrival={} ({} permille) \
             loser_pre_empted={} ({} permille)",
            self.replays,
            self.rounds,
            self.decisive,
            self.indecisive,
            self.winner_earlier_bucket,
            permille(self.winner_earlier_bucket),
            self.winner_same_bucket_first_arrival,
            permille(self.winner_same_bucket_first_arrival),
            self.loser_pre_empted,
            permille(self.loser_pre_empted),
        )
    }
}

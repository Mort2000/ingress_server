//! M9 external tournament tests (P5-GATE): real client binaries against
//! the in-process `serve_match` — a smoke tournament, the technical
//! outcomes (SPAWN_FORFEIT / CLIENT_FORFEIT), per-match budget
//! enforcement, the sandboxed path, the archive round-trip, replay
//! verification of every archived IARP, and the r1-shaped gate
//! (>=3 entrants, >=2 balanced matchups, >=12 external matches).
//!
//! Binaries must be built (`cargo build --workspace`; the default
//! workspace test run builds them). `ARENA_SKIP_LIVE=1` skips this file
//! for tight iteration (the M7 convention).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use arena_core::config::GameConfig;
use arena_runner::tournament::archive::{
    index_record, index_records_to_tsv, parse_index_tsv, parse_ratings_tsv, parse_tournament_json,
    tournament_json_text,
};
use arena_runner::tournament::config::{ClientSandbox, EloConfig, TMatchup, TournamentConfig};
use arena_runner::tournament::entrant::{
    Entrant, EntrantKind, EntrantProvenance, compute_binary_hash,
};
use arena_runner::tournament::outcome::{Stage, TournamentOutcome};
use arena_runner::tournament::run::run_external_tournament;
use arena_runner::tournament::write_tournament_artifacts;

/// Serializes the live-process tests within this binary: each spawns a
/// `serve_match` plus 2-3 child processes, and unbounded parallelism
/// inside the binary stacks on top of the OTHER test binaries cargo runs
/// concurrently (the M7 timing caveat — the semaphore is that suite's
/// convention).
static LIVE_SEMAPHORE: Mutex<()> = Mutex::new(());

fn live_slot() -> std::sync::MutexGuard<'static, ()> {
    LIVE_SEMAPHORE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn skip_live() -> bool {
    std::env::var("ARENA_SKIP_LIVE")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn target_bin(name: &str) -> PathBuf {
    // Use the same Cargo target directory/profile as this test binary.
    // Looking in debug first can silently execute a stale fixture in release tests.
    let executable = std::env::current_exe().expect("test executable");
    let profile = executable.parent().unwrap().parent().unwrap();
    let candidate = profile.join(name);
    assert!(
        candidate.is_file(),
        "{} not built; run cargo build --workspace with this profile",
        candidate.display()
    );
    candidate
}

fn external_entrant(name: &str, bin: &str, env: &[(&str, &str)]) -> Entrant {
    let mut entrant = Entrant {
        name: name.to_string(),
        kind: EntrantKind::External {
            binary: target_bin(bin),
            env: env
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        },
        provenance: EntrantProvenance::recorded(
            Some("tree-literal".into()),
            Some("lock-literal".into()),
            Some("toolchain-literal".into()),
            Some("runs/builds/test-manifest".into()),
        ),
    };
    entrant.compute_binary_hash().expect("hash the binary");
    entrant
}

fn fast_windows(rounds: u32) -> GameConfig {
    let mut cfg = GameConfig::default();
    cfg.total_rounds = rounds;
    cfg.initial_spawn_window_ms = 400;
    cfg.decision_window_ms = 60;
    cfg
}

fn external_config(
    name: &str,
    entrants: Vec<Entrant>,
    matchups: Vec<TMatchup>,
    rounds: u32,
    sandbox: ClientSandbox,
    budget_ms: u64,
) -> TournamentConfig {
    TournamentConfig {
        name: name.into(),
        master_seed: 0x006D_3600_0000_00F1,
        entrants,
        matchups,
        map_count: 2,
        rounds_per_match: rounds,
        config: fast_windows(rounds),
        elo: EloConfig::default(),
        concurrency: 2,
        sandbox,
        per_match_budget_ms: budget_ms,
    }
}

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m9-tournament")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp root");
    dir
}

/// (9/15-shared) Runs a tournament and writes the full archive; returns
/// the root, run, and parsed index records.
fn run_and_archive(
    cfg: &TournamentConfig,
    tag: &str,
) -> (
    PathBuf,
    arena_runner::tournament::run::TournamentRun,
    Vec<arena_runner::tournament::archive::IndexRecord>,
) {
    let root = temp_root(tag);
    let run = run_external_tournament(cfg, &root).expect("tournament runs");
    write_tournament_artifacts(cfg, &run, &root, None).expect("archive writes");
    let index_text = std::fs::read_to_string(root.join("index.tsv")).expect("index");
    let records = parse_index_tsv(&index_text).expect("index parses");
    (root, run, records)
}

// ---------------------------------------------------------------------------
// (11) SPAWN_FORFEIT in tournament mode.
// ---------------------------------------------------------------------------

#[test]
fn tournament_spawn_forfeit_is_technical_and_excluded_from_elo() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    let valid = external_entrant("valid", "probe-client", &[("PROBE_MODE", "valid")]);
    let no_spawn = external_entrant("nospawn", "probe-client", &[("PROBE_MODE", "no-spawn")]);
    let cfg = external_config(
        "spawn-forfeit",
        vec![valid, no_spawn],
        vec![TMatchup::new("valid-nospawn", 0, 1, 2)],
        4,
        ClientSandbox::Off,
        15_000,
    );
    let (root, run, records) = run_and_archive(&cfg, "spawnforfeit");

    // Both items are technical wins for the entrant that DID spawn.
    for outcome in &run.outcomes {
        match outcome {
            TournamentOutcome::TechnicalWin { winner, stage, .. } => {
                assert_eq!(*stage, Stage::SpawnForfeit);
                assert_eq!(*winner, 0, "the valid entrant wins technically");
            }
            other => panic!("expected SpawnForfeit, got {other:?}"),
        }
    }
    // Elo: nothing moved; the forfeit-win counter did.
    let ratings = parse_ratings_tsv(&std::fs::read_to_string(root.join("ratings.tsv")).unwrap())
        .expect("ratings parse");
    assert!(
        ratings.is_empty(),
        "technical outcomes never reach ratings.tsv"
    );
    let standings = std::fs::read_to_string(root.join("standings.txt")).unwrap();
    assert!(standings.contains("valid\t1500\t0\t0\t0\t2"), "{standings}");
    assert!(
        standings.contains("nospawn\t1500\t0\t0\t0\t0"),
        "{standings}"
    );

    // Archive shape: spawn forfeits carry meta + journal and NO replay
    // (D67 — no IARP is possible without both spawn sets).
    for record in &records {
        assert_eq!(record.outcome, "technical_win");
        assert_eq!(record.stage, "spawn_forfeit");
        assert_eq!(record.winner, "valid");
        assert_eq!(record.replay, "-");
        let match_dir = root.join("matches").join(format!("m{}", record.index));
        assert!(match_dir.join("meta.json").is_file(), "meta.json");
        assert!(match_dir.join("server/journal.txt").is_file(), "journal");
        assert!(!match_dir.join("server/replay.iarp").exists(), "no IARP");
        // The private reason is archived in meta.json.
        let meta = std::fs::read_to_string(match_dir.join("meta.json")).unwrap();
        assert!(meta.contains("\"stage\": \"spawn_forfeit\""), "{meta}");
        assert!(
            meta.contains("\"reason\": \"missed spawn deadline\""),
            "{meta}"
        );
        assert_eq!(record.rounds_completed, 0);
    }
}

// ---------------------------------------------------------------------------
// (12) CLIENT_FORFEIT in tournament mode (abnormal IARP archived).
// ---------------------------------------------------------------------------

#[test]
fn tournament_client_forfeit_archives_abnormal_replay() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    let valid = external_entrant("valid", "probe-client", &[("PROBE_MODE", "valid")]);
    let killer = external_entrant(
        "selfkill",
        "probe-client",
        &[("PROBE_MODE", "self-kill"), ("PROBE_ARG", "2")],
    );
    let cfg = external_config(
        "client-forfeit",
        vec![valid, killer],
        vec![TMatchup::new("valid-selfkill", 0, 1, 2)],
        6,
        ClientSandbox::Off,
        15_000,
    );
    let (root, run, records) = run_and_archive(&cfg, "clientforfeit");

    for outcome in &run.outcomes {
        match outcome {
            TournamentOutcome::TechnicalWin { winner, stage, .. } => {
                assert_eq!(*stage, Stage::ClientForfeit);
                assert_eq!(*winner, 0, "the surviving entrant wins technically");
            }
            other => panic!("expected ClientForfeit, got {other:?}"),
        }
    }
    for record in &records {
        assert_eq!(record.stage, "client_forfeit");
        // The match entered Round 1: an abnormal IARP exists and decodes.
        let match_dir = root.join("matches").join(format!("m{}", record.index));
        let replay = std::fs::read(match_dir.join("server/replay.iarp")).expect("abnormal IARP");
        let file = arena_replay::replay::decode_log(&replay).expect("decodes");
        match file.trailer() {
            Some(arena_replay::format::EndOfMatch::Abnormal { outcome_tag, .. }) => {
                assert_eq!(*outcome_tag, arena_replay::format::OUTCOME_CLIENT_FORFEIT);
            }
            other => panic!("expected abnormal trailer, got {other:?}"),
        }
        // The replayed rounds verify end to end.
        let replayed = arena_replay::replay::replay(&file).expect("replays");
        assert!(replayed.rounds_completed >= 2, "the kill landed at round 2");
    }
    // Excluded from Elo.
    let ratings = parse_ratings_tsv(&std::fs::read_to_string(root.join("ratings.tsv")).unwrap())
        .expect("ratings");
    assert!(ratings.is_empty());
}

// ---------------------------------------------------------------------------
// (10) Per-match budget enforcement: a stalling client is killed at the
// deadline and the item still records a deterministic outcome.
// ---------------------------------------------------------------------------

#[test]
fn budget_kill_yields_a_recorded_outcome_and_completes() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    let valid = external_entrant("valid", "probe-client", &[("PROBE_MODE", "valid")]);
    let stall = external_entrant("stall", "probe-client", &[("PROBE_MODE", "stall")]);
    // A 1.2 s budget: `stall` connects and never says HELLO; the server
    // would wait spawn_window + 5 s on its own — the budget kills first.
    let cfg = external_config(
        "budget",
        vec![valid, stall],
        vec![TMatchup::new("valid-stall", 0, 1, 2)],
        4,
        ClientSandbox::Off,
        1_200,
    );
    let started = Instant::now();
    let (root, run, records) = run_and_archive(&cfg, "budget");
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(20),
        "the run must not wait out the server's own 5.4 s handshake window per match twice: {elapsed:?}"
    );
    // Every item recorded SOMETHING (no crash of the run) and both were
    // budget kills.
    assert_eq!(run.outcomes.len(), 2);
    for (outcome, exceeded) in run.outcomes.iter().zip(run.budget_exceeded.iter()) {
        assert!(exceeded, "the stalling side forced the kill");
        assert!(
            !matches!(outcome, TournamentOutcome::Normal { .. }),
            "a killed match never masquerades as a score result: {outcome:?}"
        );
        // Deterministic category: the surviving side wins technically, or
        // the loss of both is unscored — never a crash, never a score.
        match outcome {
            TournamentOutcome::TechnicalWin { stage, .. } => {
                assert!(
                    *stage == Stage::ClientForfeit || *stage == Stage::SetupError,
                    "{stage:?}"
                )
            }
            TournamentOutcome::Unscored { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    // The archive is complete for every item.
    for record in &records {
        let match_dir = root.join("matches").join(format!("m{}", record.index));
        assert!(match_dir.join("meta.json").is_file());
        assert!(match_dir.join("server").is_dir());
    }
}

// ---------------------------------------------------------------------------
// (13) Sandboxed tournament match: env allowlist, scratch rw, JSON kept.
// ---------------------------------------------------------------------------

#[test]
fn sandboxed_tournament_match_runs_and_retains_the_sandbox_report() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    // PROBE_MODE=no-spawn is the positive env proof: if the variable did
    // not survive the sandbox's env scrub, the probe would play valid and
    // the match would end Normal instead of SPAWN_FORFEIT.
    let valid = external_entrant("valid", "probe-client", &[("PROBE_MODE", "valid")]);
    let probe = external_entrant("probe", "probe-client", &[("PROBE_MODE", "no-spawn")]);
    let cfg = external_config(
        "sandboxed",
        vec![valid, probe],
        vec![TMatchup::new("valid-probe", 0, 1, 2)],
        4,
        ClientSandbox::SandboxRunner {
            runner: target_bin("sandbox-runner"),
            wall_ms: 20_000,
            address_space_bytes: 2 << 30,
        },
        20_000,
    );
    let (root, run, records) = run_and_archive(&cfg, "sandboxed");

    for outcome in &run.outcomes {
        match outcome {
            TournamentOutcome::TechnicalWin { winner, stage, .. } => {
                assert_eq!(*stage, Stage::SpawnForfeit);
                assert_eq!(*winner, 0, "PROBE_MODE survived the env allowlist");
            }
            other => panic!("expected a spawn forfeit under sandbox, got {other:?}"),
        }
    }
    // Both sides ran under the sandbox runner and their JSON reports were
    // retained; the sandboxed scratch dir held the socket.
    for record in &records {
        let match_dir = root.join("matches").join(format!("m{}", record.index));
        for side in ["a", "b"] {
            let report = std::fs::read_to_string(match_dir.join(format!("sandbox_{side}.json")))
                .unwrap_or_else(|_| panic!("sandbox_{side}.json retained"));
            assert!(report.contains("\"classification\""), "{report}");
        }
        let scratch = match_dir.join("scratch");
        assert!(scratch.is_dir(), "the rw scratch dir (socket home) exists");
    }
}

// ---------------------------------------------------------------------------
// (15/16) The r1-shaped external gate + replay verification.
// ---------------------------------------------------------------------------

#[test]
fn external_tournament_gate_r1_shape_completes_and_ranks() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    let client_a = external_entrant("client_a", "probe-client", &[]);
    let client_c = external_entrant("client_c", "probe-client", &[]);
    let probe = external_entrant("probe", "probe-client", &[("PROBE_MODE", "valid")]);
    let cfg = TournamentConfig {
        name: "gate-r1".into(),
        master_seed: 0x006D_3600_0000_00F1,
        entrants: vec![client_a, client_c, probe],
        matchups: vec![
            TMatchup::new("client_a-client_c", 0, 1, 6),
            TMatchup::new("client_a-probe", 0, 2, 4),
            TMatchup::new("client_c-probe", 1, 2, 4),
        ],
        map_count: 4,
        rounds_per_match: 24,
        config: fast_windows(24),
        elo: EloConfig::default(),
        concurrency: 3,
        sandbox: ClientSandbox::Off,
        per_match_budget_ms: 20_000,
    };
    let started = Instant::now();
    let (root, run, records) = run_and_archive(&cfg, "gate");
    let elapsed = started.elapsed();

    // Roadmap M9: >=3 entrants, >=2 balanced matchups, >=12 external
    // matches, 0 unhandled crashes.
    assert!(cfg.entrants.len() >= 3);
    assert!(cfg.matchups.len() >= 2);
    assert!(run.items.len() >= 12, "{} matches", run.items.len());
    assert_eq!(records.len(), run.items.len());
    for outcome in &run.outcomes {
        assert!(
            matches!(outcome, TournamentOutcome::Normal { .. }),
            "healthy protocol fixtures must finish normally with proven faction attribution: {outcome:?}"
        );
    }
    // Balance per matchup (D55 through the whole external path).
    for matchup in &cfg.matchups {
        let mine: Vec<_> = run.items.iter().filter(|i| i.tag == matchup.tag).collect();
        let a_enl = mine.iter().filter(|i| i.a_is_enl).count();
        assert_eq!(a_enl, (matchup.count / 2) as usize);
    }
    // Standings computed: ratings move for scored matches only, ranks
    // ordered rating desc.
    let standings = std::fs::read_to_string(root.join("standings.txt")).unwrap();
    assert!(standings.starts_with("# arena-runner tournament standings: gate-r1"));
    let mut ratings: Vec<i64> = Vec::new();
    for line in standings.lines().skip(5) {
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() >= 3 {
            if let Ok(rating) = columns[2].parse::<i64>() {
                ratings.push(rating);
            }
        }
    }
    assert_eq!(ratings.len(), 3, "three ranked entrants: {standings}");
    assert!(
        ratings.windows(2).all(|pair| pair[0] >= pair[1]),
        "ranked rating desc: {ratings:?}"
    );
    let scored: usize = run.outcomes.iter().filter(|o| o.elo_relevant()).count();
    let ratings_rows =
        parse_ratings_tsv(&std::fs::read_to_string(root.join("ratings.tsv")).unwrap()).unwrap();
    assert_eq!(scored, ratings_rows.len());
    // The last row's post ratings reproduce the standings order.
    if let Some(last) = ratings_rows.last() {
        assert_eq!(last.0, run.items.iter().map(|i| i.index).max().unwrap());
    }

    // (16) Replay verification of the gate artifacts: every archived
    // IARP decodes, replays, and its final hash equals the recorded
    // final_state_hash in index.tsv.
    let matches_dir = root.join("matches");
    for record in &records {
        let match_dir = matches_dir.join(format!("m{}", record.index));
        assert!(match_dir.join("meta.json").is_file());
        if record.replay == "-" {
            // No replay only when the match never entered Round 1.
            assert_eq!(record.rounds_completed, 0, "{}", record.index);
            continue;
        }
        let bytes = std::fs::read(match_dir.join(&record.replay)).expect("replay");
        let file = arena_replay::replay::decode_log(&bytes).expect("replay decodes");
        let replayed = arena_replay::replay::replay(&file).expect("replay verifies");
        assert_eq!(record.rounds_completed as u32, replayed.rounds_completed);
        assert_eq!(
            record.final_state_hash,
            arena_replay::canon::hex(&replayed.final_state_hash),
            "index.tsv final hash == replayed final hash"
        );
    }
    // Traceability: every row carries the map hash, config identity, and
    // both provenance ids.
    for record in &records {
        assert_eq!(record.map_hash.len(), 64);
        assert_eq!(record.config_hash, cfg.config_identity());
        assert!(record.prov_a.starts_with("blake3:"), "{}", record.prov_a);
        assert!(record.prov_b.starts_with("blake3:"), "{}", record.prov_b);
    }
    // Wall-clock bounded (metadata assert; keeps the suite honest).
    assert!(
        elapsed < Duration::from_secs(120),
        "gate wall clock {elapsed:?}"
    );
}

// ---------------------------------------------------------------------------
// (3/5/9) tournament.json echo + index.tsv round-trip over a real run.
// ---------------------------------------------------------------------------

#[test]
fn tournament_json_echo_round_trips_and_records_provenance() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    let client_a = external_entrant("client_a", "probe-client", &[]);
    let probe = external_entrant(
        "probe",
        "probe-client",
        &[("PROBE_MODE", "no-spawn"), ("PROBE_ARG", "2")],
    );
    let cfg = external_config(
        "echo",
        vec![client_a, probe],
        vec![TMatchup::new("client_a-probe", 0, 1, 2)],
        3,
        ClientSandbox::Off,
        15_000,
    );
    let (root, run, _records) = run_and_archive(&cfg, "echo");

    // The echo parses back and every literal round-trips.
    let text = std::fs::read_to_string(root.join("tournament.json")).unwrap();
    let echo = parse_tournament_json(&text).expect("echo parses");
    assert_eq!(echo.name, cfg.name);
    assert_eq!(echo.master_seed, cfg.master_seed);
    assert_eq!(echo.schedule_seed, cfg.schedule_seed());
    assert_eq!(echo.config_hash, cfg.config_identity());
    assert_eq!(echo.rounds_per_match, cfg.rounds_per_match);
    assert_eq!(echo.map_count, cfg.map_count);
    assert_eq!(echo.elo, (cfg.elo.k, cfg.elo.initial));
    assert_eq!(echo.sandbox_mode, "off");
    assert_eq!(echo.per_match_budget_ms, cfg.per_match_budget_ms);
    assert_eq!(echo.concurrency, cfg.concurrency);
    assert_eq!(echo.matchups, cfg.matchups);
    // Every Benchmark Seed Set map seed + hash recorded (D80).
    assert_eq!(echo.maps.len(), cfg.map_count as usize);
    for (index, seed, hash) in &echo.maps {
        assert_eq!(*seed, cfg.map_seed(*index));
        assert_eq!(hash.len(), 64);
    }
    // Entrants: kinds, env pairs, and provenance literals verbatim.
    assert_eq!(echo.entrants.len(), 2);
    assert_eq!(echo.entrants[0].name, "client_a");
    assert_eq!(
        echo.entrants[1].kind,
        EntrantKind::External {
            binary: target_bin("probe-client"),
            env: vec![
                ("PROBE_MODE".to_string(), "no-spawn".to_string()),
                ("PROBE_ARG".to_string(), "2".to_string())
            ],
        }
    );
    let provenance = &echo.entrants[1].provenance;
    assert_eq!(provenance.tree_hash.as_deref(), Some("tree-literal"));
    assert_eq!(provenance.lock_hash.as_deref(), Some("lock-literal"));
    assert_eq!(
        provenance.toolchain_hash.as_deref(),
        Some("toolchain-literal")
    );
    assert_eq!(
        provenance.manifest_id.as_deref(),
        Some("runs/builds/test-manifest")
    );
    // The computed binary hash matches an independent hash of the file.
    assert_eq!(
        provenance.binary_blake3,
        Some(compute_binary_hash(&target_bin("probe-client")).unwrap())
    );

    // index.tsv round-trips exactly.
    let index_text = std::fs::read_to_string(root.join("index.tsv")).unwrap();
    let expected_records: Vec<_> = run
        .items
        .iter()
        .zip(run.outcomes.iter())
        .zip(run.replay_rel.iter().zip(run.elapsed_ms.iter()))
        .map(|((item, outcome), (replay, elapsed))| {
            index_record(item, outcome, &cfg, replay.as_deref(), *elapsed)
        })
        .collect();
    assert_eq!(index_records_to_tsv(&expected_records), index_text);
    assert_eq!(parse_index_tsv(&index_text).unwrap(), expected_records);
    // The emit function is byte-identical to the archived file.
    assert_eq!(tournament_json_text(&cfg, &run.items), text);
}

// ---------------------------------------------------------------------------
// (17) docs/reports/tournament-r1.md consistency with runs/tournaments/r1.
// `docs/` is tracked; `runs/` is LOCAL evidence (gitignored — the M6
// convention: reports under docs/ are committed, artifacts stay local).
// The doc must always exist; the artifact cross-checks run whenever the
// local r1 artifacts are present (this machine, or after any rerun).
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

#[test]
fn committed_tournament_doc_matches_the_r1_artifacts() {
    let doc = repo_root().join("docs/reports/tournament-r1.md");
    let standings = repo_root().join("runs/tournaments/r1/standings.txt");
    let doc_text = std::fs::read_to_string(&doc).unwrap_or_else(|error| {
        panic!(
            "docs/reports/tournament-r1.md must exist from an actual run ({}): {error}",
            doc.display()
        )
    });
    assert!(doc_text.contains("# Tournament R1"), "{doc_text:?}");
    let Ok(standings_text) = std::fs::read_to_string(&standings) else {
        eprintln!(
            "note: {} not present (runs/ is local evidence) — skipping the artifact cross-check",
            standings.display()
        );
        return;
    };
    // Every standings row appears verbatim in the doc's standings table.
    for line in standings_text.lines().filter(|line| line.contains('\t')) {
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() >= 3 && columns[0].chars().all(|c| c.is_ascii_digit()) {
            let row = format!(
                "| {} | {} | {} | {}-{}-{} | {} | {} |",
                columns[0],
                columns[1],
                columns[2],
                columns[3],
                columns[4],
                columns[5],
                columns[6],
                columns[8]
            );
            assert!(
                doc_text.contains(&row),
                "the committed doc's standings must match the artifacts: missing {row:?}"
            );
        }
    }
}

/// (16, on the committed artifacts) every archived r1 replay decodes,
/// replays, and its final hash equals the `index.tsv` row — the P5-GATE
/// "Replay 可归档且 Hash 校验稳定" claim, checked against the frozen run
/// itself, not a re-run.
#[test]
fn committed_r1_archives_replay_to_the_recorded_hashes() {
    let root = repo_root().join("runs/tournaments/r1");
    let Ok(index_text) = std::fs::read_to_string(root.join("index.tsv")) else {
        eprintln!(
            "note: {} not present (runs/ is local evidence) — skipping the committed-archive replay check",
            root.display()
        );
        return;
    };
    let records = parse_index_tsv(&index_text).expect("index parses");
    assert!(records.len() >= 12, "the frozen r1 has >= 12 matches");
    let mut scored = 0;
    for record in &records {
        let match_dir = root.join("matches").join(format!("m{}", record.index));
        assert!(match_dir.join("meta.json").is_file(), "meta.json");
        if record.replay == "-" {
            assert_eq!(record.rounds_completed, 0);
            continue;
        }
        scored += 1;
        let bytes = std::fs::read(match_dir.join(&record.replay)).expect("replay.iarp archived");
        let file = arena_replay::replay::decode_log(&bytes).expect("decodes");
        let replayed = arena_replay::replay::replay(&file).expect("replays");
        assert_eq!(record.rounds_completed as u32, replayed.rounds_completed);
        assert_eq!(
            record.final_state_hash,
            arena_replay::canon::hex(&replayed.final_state_hash),
            "match {} recorded hash == replayed hash",
            record.index
        );
    }
    assert!(
        scored >= 12,
        "scored matches with verified replays: {scored}"
    );
    // Every row is fully traceable (map hash, config identity, provenance).
    for record in &records {
        assert_eq!(record.map_hash.len(), 64);
        assert!(record.prov_a.starts_with("blake3:"));
        assert!(record.prov_b.starts_with("blake3:"));
    }
    // ratings.tsv reproduces the final standings: replaying the rows in
    // order (initial 1500) must land every entrant on their standing.
    let ratings_rows =
        parse_ratings_tsv(&std::fs::read_to_string(root.join("ratings.tsv")).unwrap()).unwrap();
    assert_eq!(ratings_rows.len(), scored);
    let mut current: std::collections::BTreeMap<String, i64> = Default::default();
    for row in &ratings_rows {
        current.insert(name_of(&records, row.0, 0), row.3);
        current.insert(name_of(&records, row.0, 1), row.5);
    }
    let standings_text = std::fs::read_to_string(root.join("standings.txt")).unwrap();
    for line in standings_text.lines().skip(5) {
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() >= 3 && columns[0].chars().all(|c| c.is_ascii_digit()) {
            let name = columns[1];
            let rating: i64 = columns[2].parse().unwrap();
            assert_eq!(
                current.get(name).copied().unwrap_or(1500),
                rating,
                "{name}'s standing == replayed final rating"
            );
        }
    }
}

/// The entrant names of one index row (a=0, b=1) — small helper for the
/// standings cross-check above.
fn name_of(
    records: &[arena_runner::tournament::archive::IndexRecord],
    index: u64,
    side: usize,
) -> String {
    let record = records
        .iter()
        .find(|record| record.index == index)
        .expect("index row");
    if side == 0 {
        record.entrant_a.clone()
    } else {
        record.entrant_b.clone()
    }
}

// ---------------------------------------------------------------------------
// Regression: a RELATIVE archive root must work under the sandbox (the
// sandbox sets the client cwd to the scratch dir; before the fix every
// client died with ENOENT on the relative socket path and the whole
// tournament collapsed into unscored SetupErrors — found live in the r1
// release run).
// ---------------------------------------------------------------------------

#[test]
fn sandboxed_tournament_accepts_a_relative_archive_root() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    let valid = external_entrant("valid", "probe-client", &[("PROBE_MODE", "valid")]);
    let no_spawn = external_entrant("nospawn", "probe-client", &[("PROBE_MODE", "no-spawn")]);
    let cfg = external_config(
        "relative-root",
        vec![valid, no_spawn],
        vec![TMatchup::new("valid-nospawn", 0, 1, 2)],
        2,
        ClientSandbox::SandboxRunner {
            runner: target_bin("sandbox-runner"),
            wall_ms: 20_000,
            address_space_bytes: 2 << 30,
        },
        20_000,
    );
    // A RELATIVE root (the crate dir is the test cwd).
    let root = std::path::PathBuf::from("target").join("m9-relative-root");
    let _ = std::fs::remove_dir_all(&root);
    let run = run_external_tournament(&cfg, &root).expect("tournament runs");
    for outcome in &run.outcomes {
        match outcome {
            TournamentOutcome::TechnicalWin { stage, .. } => {
                assert_eq!(*stage, Stage::SpawnForfeit);
            }
            other => panic!(
                "a relative root must not turn matches into unscored setup errors: {other:?}"
            ),
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// REVIEW FIX (M9 finding 1, D81 faction fidelity): the served faction
// assignment must be PROVEN, not assumed. The pre-fix verification
// compared the socket PATH's filesystem inode against SOCKFS fd-link
// inodes — two disjoint kernel inode spaces — so it NEVER succeeded:
// every r1 meta.json recorded connect_order_verified=false after a 3 s
// spin, and attribution rested on spawn order alone. Now the server
// records the accept order with each peer's SO_PEERCRED pid and the
// runner proves slot 0 (first accept = ENL, D61) against the
// ENL-scheduled side's process tree. This test would have failed since
// day one.
// ---------------------------------------------------------------------------

#[test]
fn normal_external_matches_prove_the_served_faction_assignment() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    let client_a = external_entrant("client_a", "probe-client", &[]);
    let probe = external_entrant("probe", "probe-client", &[("PROBE_MODE", "valid")]);
    let cfg = external_config(
        "connect-proof",
        vec![client_a, probe],
        vec![TMatchup::new("client_a-probe", 0, 1, 2)],
        6,
        ClientSandbox::Off,
        15_000,
    );
    let (root, run, records) = run_and_archive(&cfg, "connectproof");

    // Both matches completed and were SCORED — an unproven attribution
    // would have demoted them to Unscored{RunnerFault}.
    for outcome in &run.outcomes {
        assert!(
            matches!(outcome, TournamentOutcome::Normal { .. }),
            "expected a scored match: {outcome:?}"
        );
    }
    // THE regression pin: every match's accept order is proven.
    assert!(
        run.connect_order_verified.iter().all(|v| *v),
        "connect_order_verified must be true for normal external matches: {:?}",
        run.connect_order_verified
    );
    // meta.json records the proof flag; the server journal carries the
    // per-connection peer pids (traceability of the accept order).
    for record in &records {
        let match_dir = root.join("matches").join(format!("m{}", record.index));
        let meta = std::fs::read_to_string(match_dir.join("meta.json")).unwrap();
        assert!(meta.contains("\"connect_order_verified\": true"), "{meta}");
        let journal = std::fs::read_to_string(match_dir.join("server/journal.txt")).unwrap();
        assert!(
            journal.matches("peer_pid=").count() == 2,
            "both accepted connections journaled with their peer pid: {journal}"
        );
        assert!(
            !journal.contains("peer_pid=-"),
            "SO_PEERCRED must succeed for live child connections: {journal}"
        );
    }
    // The proven matches reached the ratings (draws count, 0.5 each).
    let ratings = parse_ratings_tsv(&std::fs::read_to_string(root.join("ratings.tsv")).unwrap())
        .expect("ratings parse");
    assert_eq!(ratings.len(), 2);
}

// ---------------------------------------------------------------------------
// REVIEW FIX (M9 finding 2, D74/D81 containment + wall budget): a
// budget-kill under ClientSandbox::SandboxRunner must not orphan the
// client. The sandboxed client is a GRANDCHILD in its own process group;
// the pre-fix deadline path std-killed only the sandbox-runner (killing
// its group watchdog with it) and the client kept running, still holding
// the match socket. Now the sandbox wall is clamped INSIDE the remaining
// budget (its own group watchdog fires first) and kill_all reaches the
// grandchild's whole process group via `sandbox-runner --kill-group`.
// ---------------------------------------------------------------------------

/// Any live process whose command line mentions `needle` (argv is
/// NUL-separated; read as lossy text).
fn any_process_cmdline_contains(needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(entry.path().join("cmdline")) {
            if String::from_utf8_lossy(&bytes).contains(needle) {
                return true;
            }
        }
    }
    false
}

#[test]
fn sandboxed_budget_kill_contains_the_client_processes() {
    if skip_live() {
        return;
    }
    let _live = live_slot();
    let valid = external_entrant("valid", "probe-client", &[("PROBE_MODE", "valid")]);
    let stall = external_entrant("stall", "probe-client", &[("PROBE_MODE", "stall")]);
    // A 60 s sandbox wall against a 5 s budget: the wall is clamped to
    // (remaining - margin), so the sandbox-runner's own group watchdog
    // ends the match strictly before the tournament deadline.
    let cfg = external_config(
        "sandbox-budget",
        vec![valid, stall],
        vec![TMatchup::new("valid-stall", 0, 1, 2)],
        4,
        ClientSandbox::SandboxRunner {
            runner: target_bin("sandbox-runner"),
            wall_ms: 60_000,
            address_space_bytes: 2 << 30,
        },
        5_000,
    );
    let started = Instant::now();
    let (root, run, records) = run_and_archive(&cfg, "sandboxbudget");
    // The budget (not the 60 s wall) bounded the run.
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "sandboxed budget kill must stay near the budget: {:?}",
        started.elapsed()
    );
    for (outcome, record) in run.outcomes.iter().zip(records.iter()) {
        // A killed match records a deterministic technical/unscored
        // outcome — never a score result.
        assert!(
            !matches!(outcome, TournamentOutcome::Normal { .. }),
            "a budget-killed match never masquerades as a score result: {outcome:?}"
        );
        assert_ne!(record.outcome, "normal");
        let match_dir = root.join("matches").join(format!("m{}", record.index));
        // Both sandbox reports survived (pre-fix the deadline path killed
        // the sandbox-runner mid-report and lost them) and the stalling
        // side hit the WALL watchdog, not a runner-side kill.
        let mut wall_timeouts = 0;
        for side in ["a", "b"] {
            let report = std::fs::read_to_string(match_dir.join(format!("sandbox_{side}.json")))
                .unwrap_or_else(|_| panic!("sandbox_{side}.json retained"));
            if report.contains("\"wall_timeout\"") {
                wall_timeouts += 1;
            }
        }
        assert!(
            wall_timeouts >= 1,
            "the sandbox wall (clamped inside the budget) ends the match: {record:?}"
        );
    }
    // CONTAINMENT: no process of this tournament survives — scan /proc
    // command lines for the run-unique archive root (the scratch dir is
    // under it, so both the sandbox-runners and the clients mention it).
    let needle = root.to_string_lossy().to_string();
    let deadline = Instant::now() + Duration::from_secs(5);
    while any_process_cmdline_contains(&needle) {
        assert!(
            Instant::now() < deadline,
            "a sandboxed client process survived the budget kill (D74 containment broken)"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

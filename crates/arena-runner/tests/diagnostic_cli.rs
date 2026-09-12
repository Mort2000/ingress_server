use arena_runner::tournament::archive::parse_json;
use std::{fs, process::Command};
#[test]
fn scenario_is_self_contained_and_decision_comparison_locates_first_difference() {
    use arena_core::{
        config::GameConfig, resolve::Resolver, spawn::validate_initial_spawn_set, state::GameState,
        types::Coord,
    };
    let root = std::env::temp_dir().join(format!(
        "arena-diagnostic-{}",
        arena_runner::tournament::exec_external::execution_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let cfg = GameConfig {
        total_rounds: 2,
        ..GameConfig::default()
    };
    let map = arena_core::map::generate_map(9, &cfg).unwrap();
    let (a, b) = GameState::initial_roster(&cfg);
    let spawn = |ids: Vec<_>| {
        validate_initial_spawn_set(
            &ids,
            &ids.iter()
                .map(|id| (*id, Coord::new(0, 0)))
                .collect::<Vec<_>>(),
            &cfg,
        )
        .unwrap()
    };
    let a = spawn(a);
    let b = spawn(b);
    let mut state = GameState::new(cfg, &map, &a, &b).unwrap();
    let mut writer = arena_replay::recorder::ReplayWriter::new(71, &cfg, &map, &a, &b);
    for _ in 0..2 {
        let report = Resolver::with_seed(71).run_round(&mut state, &[]);
        state.advance_round();
        writer.record_round(&report, &state).unwrap();
    }
    let replay = root.join("original.iarp");
    fs::write(&replay, writer.finish(&state).unwrap()).unwrap();
    let cli = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_arena-runner"))
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap()
    };
    let scenario = root.join("scenario");
    cli(&[
        "scenario",
        "--input",
        replay.to_str().unwrap(),
        "--faction",
        "enl",
        "--out",
        scenario.to_str().unwrap(),
    ]);
    fs::remove_file(&replay).unwrap();
    let history = fs::read_to_string(scenario.join("recorded.json")).unwrap();
    let j = parse_json(&history).unwrap();
    assert_eq!(
        j.get("scope").unwrap().as_str(),
        Some("recorded-observations")
    );
    assert_eq!(j.get("rounds").unwrap().as_array().unwrap().len(), 2);
    let compared = cli(&[
        "diagnose",
        "--input",
        scenario.join("recorded.json").to_str().unwrap(),
        "--against",
        scenario.join("recorded.json").to_str().unwrap(),
    ]);
    assert!(compared.contains("\"first_difference\":null"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn independent_example_reproduces_live_history_and_exports_timing_conditions() {
    let root = std::env::temp_dir().join(format!(
        "arena-example-{}",
        arena_runner::tournament::exec_external::execution_id().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let client = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/iteration-client")
        .canonicalize()
        .expect("cargo build --workspace first");
    let cli = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_arena-runner"))
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap()
    };
    let run = root.join("run");
    cli(&[
        "tournament",
        "--out",
        run.to_str().unwrap(),
        "--client",
        client.to_str().unwrap(),
        "--client",
        client.to_str().unwrap(),
        "--rounds",
        "2",
        "--fast",
    ]);
    let scenario = root.join("scenario");
    cli(&[
        "scenario",
        "--input",
        run.join("matches/m0/attempts/0001/server/replay.iarp")
            .to_str()
            .unwrap(),
        "--faction",
        "enl",
        "--out",
        scenario.to_str().unwrap(),
    ]);
    let actual = root.join("actual.json");
    let status = Command::new(&client)
        .args([
            "--arena-offline",
            scenario.to_str().unwrap(),
            actual.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let diff = cli(&[
        "diagnose",
        "--input",
        actual.to_str().unwrap(),
        "--against",
        scenario.join("recorded.json").to_str().unwrap(),
    ]);
    assert!(diff.contains("\"first_difference\":null"));
    let a = parse_json(&fs::read_to_string(&actual).unwrap()).unwrap();
    let rounds = a.get("rounds").unwrap().as_array().unwrap();
    assert!(
        rounds[1].get("trace").unwrap().as_array().unwrap()[0]
            .get("task")
            .unwrap()
            .as_str()
            .unwrap()
            .ends_with("step-2")
    );
    let timing = cli(&["timing-report", "--out", run.to_str().unwrap()]);
    let t = parse_json(&timing).unwrap();
    let matches = t.get("matches").unwrap().as_array().unwrap();
    assert_eq!(matches.len(), 2);
    assert!(
        !matches[0]
            .get("client_timings")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !matches[0]
            .get("server_receipts")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    fs::remove_dir_all(root).unwrap();
}

use arena_runner::tournament::archive::{Json, parse_json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "arena-batch2-{}",
            arena_runner::tournament::exec_external::execution_id().unwrap()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arena-runner"))
        .args(args)
        .output()
        .unwrap()
}
fn good(args: &[&str]) -> String {
    let o = cli(args);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}
fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn quoted(s: &str) -> String {
    format!("\"{}\"", arena_runner::tournament::archive::json_escape(s))
}
fn register(t: &Path, id: &str, version: &str, binary: &Path) -> PathBuf {
    let request = t.join(format!("{id}-{version}.json"));
    let out = t.join(format!("{id}-{version}"));
    fs::write(&request,format!(r#"{{"schema":"arena-build-request-v1","entrant_id":"{id}","version":"{version}","binary":{},"source_tree":"fixture","commit":"unknown","dirty":false,"lock_hash":"fixture","toolchain":"fixture","build_args":[],"audit":{{"status":"not-reviewed","evidence":"functional fixture"}}}}"#,quoted(path(binary)))).unwrap();
    good(&[
        "build-register",
        "--spec",
        path(&request),
        "--out",
        path(&out),
    ]);
    out
}
fn map_set(t: &Path) -> PathBuf {
    let config = t.join("rules.json");
    fs::write(&config,r#"{"portal_count":6,"total_rounds":1,"decision_window_ms":50,"initial_spawn_window_ms":2000}"#).unwrap();
    for layout in ["uniform", "corridor"] {
        good(&[
            "map-generate",
            "--config",
            path(&config),
            "--layout",
            layout,
            "--seed",
            "17",
            "--out",
            path(&t.join(format!("{layout}.json"))),
        ]);
    }
    let request = t.join("set-request.json");
    fs::write(&request,r#"{"schema":"arena-map-set-request-v1","purpose":"development","maps":["uniform.json","corridor.json"]}"#).unwrap();
    let set = t.join("set.json");
    good(&["map-set", "--spec", path(&request), "--out", path(&set)]);
    set
}
fn event(t: &Path, entries: &str, schedule: &str) -> PathBuf {
    let p = t.join("event.json");
    fs::write(&p,format!(r#"{{"schema":"arena-event-v1","name":"example","seed":"42","mode":"functional","map_set":"set.json","entrants":[{entries}],"schedule":{schedule},"resources":{{"workers":1,"per_match_budget_ms":10000,"sandbox":null,"address_space_bytes":536870912}},"attempt_limit":3}}"#)).unwrap();
    p
}

#[test]
fn resource_preflight_records_admission_caps_and_hard_limit_degradation() {
    let t = Temp::new();
    map_set(&t.0);
    for id in ["a", "b"] {
        register(
            &t.0,
            id,
            "v1",
            Path::new(env!("CARGO_BIN_EXE_arena-runner")),
        );
    }
    let spec = event(
        &t.0,
        r#"{"id":"a","name":"A","build":"a-v1","args":[],"env":{}},{"id":"b","name":"B","build":"b-v1","args":[],"env":{}}"#,
        r#"{"kind":"round-robin","repeats":1}"#,
    );
    let source = fs::read_to_string(&spec).unwrap().replace(
        "\"workers\":1",
        "\"workers\":4,\"cpu_slots\":2,\"memory_budget_bytes\":\"1207959552\"",
    );
    fs::write(&spec, source).unwrap();
    let out = t.0.join("plan");
    good(&["plan", "--spec", path(&spec), "--out", path(&out)]);
    let output = good(&["preflight", "--out", path(&out)]);
    let j = parse_json(&output).unwrap();
    assert_eq!(j.get("effective_workers"), Some(&Json::Int(1)));
    assert_eq!(
        j.get("batch_memory_hard_enforced"),
        Some(&Json::Bool(false))
    );
    assert_eq!(j.get("cpu_hard_enforced"), Some(&Json::Bool(false)));
    assert_eq!(j.get("scheduler").unwrap().as_str(), Some("dynamic"));
}

#[test]
fn dynamic_execution_claims_short_tasks_while_long_match_is_running() {
    let t = Temp::new();
    map_set(&t.0);
    let probe = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/probe-client")
        .canonicalize()
        .expect("cargo build --workspace first");
    for id in ["a", "b", "c"] {
        register(&t.0, id, "v1", &probe);
    }
    let spec = event(
        &t.0,
        r#"{"id":"a","name":"Slow","build":"a-v1","args":[],"env":{"PROBE_MODE":"no-spawn"}},{"id":"b","name":"Fast B","build":"b-v1","args":[],"env":{}},{"id":"c","name":"Fast C","build":"c-v1","args":[],"env":{}}"#,
        r#"{"kind":"round-robin","repeats":1}"#,
    );
    fs::write(
        &spec,
        fs::read_to_string(&spec)
            .unwrap()
            .replace("\"workers\":1", "\"workers\":2"),
    )
    .unwrap();
    let source = t.0.join("source");
    good(&["plan", "--spec", path(&spec), "--out", path(&source)]);
    // Two maps per pair: a-b occupies 0..3, a-c 4..7, b-c 8..11.
    let out = t.0.join("mixed");
    good(&[
        "select",
        "--from",
        path(&source),
        "--items",
        "0,8,9",
        "--out",
        path(&out),
    ]);
    good(&["resume", "--out", path(&out)]);
    let load = |i| {
        parse_json(
            &fs::read_to_string(out.join(format!("matches/m{i}/attempts/0001/resources.json")))
                .unwrap(),
        )
        .unwrap()
    };
    let slow = load(0);
    let last_short = load(2);
    assert!(
        last_short.get("started_unix_ms").unwrap().as_int().unwrap()
            < slow.get("finished_unix_ms").unwrap().as_int().unwrap(),
        "idle worker must claim the third item before slow first item ends"
    );
    assert!(good(&["status", "--out", path(&out)]).contains("completed"));
}

#[test]
fn cooperating_runner_processes_share_one_host_slot() {
    let t = Temp::new();
    map_set(&t.0);
    let pool = t.0.join("pool");
    good(&["resource-pool", "--out", path(&pool), "--slots", "1"]);
    let probe = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/probe-client")
        .canonicalize()
        .unwrap();
    for id in ["a", "b"] {
        register(&t.0, id, "v1", &probe);
    }
    let spec = event(
        &t.0,
        r#"{"id":"a","name":"Slow","build":"a-v1","args":[],"env":{"PROBE_MODE":"no-spawn"}},{"id":"b","name":"B","build":"b-v1","args":[],"env":{}}"#,
        r#"{"kind":"round-robin","repeats":1}"#,
    );
    fs::write(
        &spec,
        fs::read_to_string(&spec).unwrap().replace(
            "\"workers\":1",
            &format!("\"workers\":2,\"host_pool\":{}", quoted(path(&pool))),
        ),
    )
    .unwrap();
    let source = t.0.join("source");
    good(&["plan", "--spec", path(&spec), "--out", path(&source)]);
    let a = t.0.join("one");
    let b = t.0.join("two");
    for out in [&a, &b] {
        good(&[
            "select",
            "--from",
            path(&source),
            "--items",
            "0",
            "--out",
            path(out),
        ]);
    }
    let launch = |p: &Path| {
        Command::new(env!("CARGO_BIN_EXE_arena-runner"))
            .args(["resume", "--out", path(p)])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap()
    };
    let mut ca = launch(&a);
    let mut cb = launch(&b);
    assert!(ca.wait().unwrap().success());
    assert!(cb.wait().unwrap().success());
    let interval = |p: &Path| {
        let j = parse_json(
            &fs::read_to_string(p.join("matches/m0/attempts/0001/resources.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(j.get("host_slot"), Some(&Json::Int(0)));
        (
            j.get("started_unix_ms").unwrap().as_int().unwrap(),
            j.get("finished_unix_ms").unwrap().as_int().unwrap(),
        )
    };
    let ia = interval(&a);
    let ib = interval(&b);
    assert!(
        ia.1 <= ib.0 || ib.1 <= ia.0,
        "single-slot match execution intervals cannot overlap"
    );
    // Lock is released, including between successive invocations.
    good(&["rebuild", "--out", path(&a)]);
}
#[test]
fn declarative_plan_covers_every_map_and_is_independent_of_display_order() {
    let t = Temp::new();
    map_set(&t.0);
    for id in ["a", "b", "c"] {
        register(
            &t.0,
            id,
            "v1",
            Path::new(env!("CARGO_BIN_EXE_arena-runner")),
        );
    }
    let rows = [
        r#"{"id":"a","name":"Alpha","build":"a-v1","args":[],"env":{}}"#,
        r#"{"id":"b","name":"Beta","build":"b-v1","args":[],"env":{}}"#,
        r#"{"id":"c","name":"Gamma","build":"c-v1","args":[],"env":{}}"#,
    ];
    let spec = event(
        &t.0,
        &rows.join(","),
        r#"{"kind":"round-robin","repeats":1}"#,
    );
    let out = t.0.join("plan1");
    let preview = good(&["plan", "--spec", path(&spec), "--out", path(&out)]);
    assert!(preview.contains("matches=12"), "{preview}");
    assert!(
        preview.contains("address_space_bytes=536870912 address_space_enforced=false"),
        "{preview}"
    );
    let p = arena_runner::tournament::durable::load(&out).unwrap();
    assert_eq!(p.items.len(), 12);
    assert_eq!(
        p.config.entrants[0]
            .provenance
            .toolchain_hash
            .as_ref()
            .unwrap()
            .len(),
        64,
        "toolchain text must project to a hash, not a version description"
    );
    for pair in p.items.chunks_exact(2) {
        assert_eq!(pair[0].map_hash, pair[1].map_hash);
        assert_ne!(pair[0].a_is_enl, pair[1].a_is_enl);
        assert_eq!(pair[0].game_seed, pair[1].game_seed);
    }
    let spec = event(
        &t.0,
        &[rows[2], rows[0], rows[1]]
            .join(",")
            .replace("Alpha", "Renamed"),
        r#"{"kind":"round-robin","repeats":1}"#,
    );
    let other = t.0.join("plan2");
    good(&["plan", "--spec", path(&spec), "--out", path(&other)]);
    assert_eq!(
        p.items,
        arena_runner::tournament::durable::load(&other)
            .unwrap()
            .items
    );
    assert!(!out.join("matches").exists());
    let manifest = out.join("plan/manifest");
    let damaged = fs::read_to_string(&manifest)
        .unwrap()
        .lines()
        .filter(|l| !l.ends_with("batch2.json"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(manifest, damaged).unwrap();
    assert!(
        !cli(&["status", "--out", path(&out)]).status.success(),
        "uncovered frozen metadata must be rejected"
    );
}

#[test]
fn cli_registers_snapshot_not_a_link_to_development_binary() {
    let t = Temp::new();
    let binary = t.0.join("development");
    fs::copy(env!("CARGO_BIN_EXE_arena-runner"), &binary).unwrap();
    let spec = t.0.join("build.json");
    let out = t.0.join("registered");
    fs::write(&spec,format!(r#"{{"schema":"arena-build-request-v1","entrant_id":"bot","version":"v1","binary":{},"source_tree":"recorded-tree","commit":"abc","dirty":true,"lock_hash":"recorded-lock","toolchain":"rustc fixture","build_args":["--release","--locked"],"audit":{{"status":"not-reviewed","evidence":"fixture only"}}}}"#,quoted(path(&binary)))).unwrap();
    good(&["build-register", "--spec", path(&spec), "--out", path(&out)]);
    let original = fs::read(out.join("binary")).unwrap();
    fs::write(&binary, b"rebuilt").unwrap();
    assert_eq!(fs::read(out.join("binary")).unwrap(), original);
    good(&["build-inspect", "--input", path(&out)]);
    assert!(
        !cli(&["build-register", "--spec", path(&spec), "--out", path(&out)])
            .status
            .success()
    );
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(out.join("binary"), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(out.join("binary"), b"tampered").unwrap();
    assert!(
        !cli(&["build-inspect", "--input", path(&out)])
            .status
            .success()
    );
}

#[test]
fn subset_and_comparison_reuse_frozen_inputs_and_execute_archive_copies() {
    let t = Temp::new();
    map_set(&t.0);
    let probe = Path::new(env!("CARGO_BIN_EXE_arena-runner"))
        .parent()
        .unwrap()
        .join("probe-client");
    assert!(probe.is_file(), "cargo build --workspace first");
    for (id, v) in [("a", "v1"), ("a", "v2"), ("b", "v1"), ("c", "v1")] {
        register(&t.0, id, v, &probe);
    }
    let rows = r#"{"id":"a","name":"同名","build":"a-v1","args":[],"env":{}},{"id":"b","name":"同名","build":"b-v1","args":[],"env":{}},{"id":"c","name":"C","build":"c-v1","args":[],"env":{}}"#;
    let spec = event(
        &t.0,
        rows,
        r#"{"kind":"challenge","candidates":["a"],"baselines":["b","c"],"repeats":1}"#,
    );
    let original = t.0.join("original");
    good(&["plan", "--spec", path(&spec), "--out", path(&original)]);
    let subset = t.0.join("subset");
    good(&[
        "select",
        "--from",
        path(&original),
        "--items",
        "0,1,4,5",
        "--out",
        path(&subset),
    ]);
    let plan = arena_runner::tournament::durable::load(&original).unwrap();
    let selected = arena_runner::tournament::durable::load(&subset).unwrap();
    for (i, j) in [0, 1, 4, 5].into_iter().enumerate() {
        let mut expected = plan.items[j].clone();
        expected.index = i as u64;
        assert_eq!(selected.items[i], expected);
    }
    let compared = t.0.join("compare");
    good(&[
        "compare-plan",
        "--from",
        path(&subset),
        "--candidate",
        "a",
        "--build",
        path(&t.0.join("a-v2")),
        "--out",
        path(&compared),
    ]);
    let frozen = arena_runner::tournament::durable::load(&compared).unwrap();
    assert_eq!(frozen.items.len(), 8);
    for pair in frozen.items.chunks_exact(2) {
        assert_eq!(pair[0].game_seed, pair[1].game_seed);
        assert_eq!(pair[0].map_hash, pair[1].map_hash);
        assert_eq!(pair[0].client_seed, pair[1].client_seed);
        assert_eq!(pair[0].a_is_enl, pair[1].a_is_enl);
    }
    // Moving the archive and deleting input assets must not invalidate it.
    let moved = t.0.join("moved");
    fs::rename(&compared, &moved).unwrap();
    for p in ["a-v1", "a-v2", "b-v1", "c-v1", "original", "subset"] {
        fs::remove_dir_all(t.0.join(p)).unwrap();
    }
    good(&["resume", "--out", path(&moved)]);
    let status = good(&["status", "--out", path(&moved)]);
    assert!(status.contains("completed=8"), "{status}");
    good(&["compare", "--out", path(&moved)]);
    let report = fs::read(moved.join("comparison.json")).unwrap();
    let json = parse_json(std::str::from_utf8(&report).unwrap()).unwrap();
    assert_eq!(
        json.get("resources")
            .unwrap()
            .get("address_space_bytes")
            .and_then(Json::as_int),
        Some(536870912)
    );
    assert_eq!(json.get("paired_samples").and_then(Json::as_int), Some(4));
    assert_eq!(json.get("rows").and_then(Json::as_array).unwrap().len(), 8);
    good(&["compare", "--out", path(&moved)]);
    assert_eq!(report, fs::read(moved.join("comparison.json")).unwrap());
    for entry in json.get("rows").and_then(Json::as_array).unwrap() {
        let replay = entry.get("replay").and_then(Json::as_str).unwrap();
        let log = arena_replay::replay::decode_log(&fs::read(moved.join(replay)).unwrap()).unwrap();
        assert_eq!(
            log.header.map,
            frozen.maps[frozen.items[entry.get("item").and_then(Json::as_int).unwrap() as usize]
                .map_index as usize]
        );
        assert!(
            log.header
                .identity
                .unwrap()
                .participants
                .iter()
                .all(|p| p.binding_verified && !p.registered_version.is_empty())
        );
    }
}

#[test]
fn comparison_executes_manual_and_generated_maps_with_distinct_rules_and_technical_results() {
    let t = Temp::new();
    map_set(&t.0);
    let manual = t.0.join("manual.json");
    fs::write(&manual,r#"{"schema":"arena-map-v1","config":{"portal_count":3,"portal_cluster_count":1,"portal_min_distance_cm":10,"map_width_cm":10000,"map_height_cm":20000,"total_rounds":2,"decision_window_ms":50,"initial_spawn_window_ms":250},"portals":[[0,0],[9000,0],[0,18000]]}"#).unwrap();
    let request = t.0.join("mixed-request.json");
    fs::write(&request,r#"{"schema":"arena-map-set-request-v1","purpose":"regression","maps":["manual.json","uniform.json","corridor.json"]}"#).unwrap();
    let mixed = t.0.join("mixed.json");
    good(&["map-set", "--spec", path(&request), "--out", path(&mixed)]);
    let probe = Path::new(env!("CARGO_BIN_EXE_arena-runner"))
        .parent()
        .unwrap()
        .join("probe-client");
    for (id, v) in [("a", "v1"), ("a", "v2"), ("b", "v1")] {
        register(&t.0, id, v, &probe);
    }
    let rows = r#"{"id":"old","name":"A","build":"a-v1","args":[],"env":{}},{"id":"new","name":"A","build":"a-v2","args":[],"env":{"PROBE_MODE":"no-spawn"}},{"id":"base","name":"B","build":"b-v1","args":[],"env":{}}"#;
    let spec = event(
        &t.0,
        rows,
        r#"{"kind":"comparison","before":"old","after":"new","baselines":["base"],"repeats":1}"#,
    );
    let text = fs::read_to_string(&spec)
        .unwrap()
        .replace("set.json", "mixed.json");
    fs::write(&spec, text).unwrap();
    let out = t.0.join("run");
    good(&["tournament", "--spec", path(&spec), "--out", path(&out)]);
    good(&["compare", "--out", path(&out)]);
    let report = parse_json(&fs::read_to_string(out.join("comparison.json")).unwrap()).unwrap();
    assert_eq!(
        report.get("technical_results").and_then(Json::as_int),
        Some(6)
    );
    assert_eq!(
        report
            .get("incomplete_or_technical_pairs")
            .and_then(Json::as_int),
        Some(6)
    );
    let plan = arena_runner::tournament::durable::load(&out).unwrap();
    for row in report.get("rows").and_then(Json::as_array).unwrap() {
        if row.get("variant").and_then(Json::as_str) == Some("before") {
            let i = row.get("item").and_then(Json::as_int).unwrap() as usize;
            let log = arena_replay::replay::decode_log(
                &fs::read(out.join(row.get("replay").and_then(Json::as_str).unwrap())).unwrap(),
            )
            .unwrap();
            assert_eq!(
                log.header.config,
                plan.match_config(&plan.items[i]).unwrap().config
            );
            assert_eq!(log.header.map, plan.maps[plan.items[i].map_index as usize]);
        }
    }
}

#[test]
fn map_preview_and_illegal_inputs_are_explicit_before_plan_creation() {
    let t = Temp::new();
    map_set(&t.0);
    let svg = t.0.join("preview.svg");
    good(&[
        "map-preview",
        "--input",
        path(&t.0.join("uniform.json")),
        "--out",
        path(&svg),
    ]);
    assert!(fs::read_to_string(svg).unwrap().contains("<circle"));
    let bad = t.0.join("bad.json");
    fs::write(&bad,r#"{"schema":"arena-map-v1","config":{"portal_count":2,"portal_cluster_count":1},"portals":[[0,0],[0,0]]}"#).unwrap();
    let o = cli(&[
        "map-import",
        "--input",
        path(&bad),
        "--out",
        path(&t.0.join("should-not-exist")),
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("distance"));
    assert!(!t.0.join("should-not-exist").exists());
}

#[test]
fn structured_arguments_and_environment_reach_sandboxed_registered_builds() {
    let t = Temp::new();
    map_set(&t.0);
    let bin = Path::new(env!("CARGO_BIN_EXE_arena-runner"))
        .parent()
        .unwrap();
    for id in ["a", "b"] {
        register(&t.0, id, "v1", &bin.join("probe-client"));
    }
    let rows = r#"{"id":"a","name":"A","build":"a-v1","args":["space ; literal"],"env":{"PROBE_OBSERVATION_LOG":"1"}},{"id":"b","name":"B","build":"b-v1","args":[],"env":{}}"#;
    let spec = event(&t.0, rows, r#"{"kind":"round-robin","repeats":1}"#);
    let value = fs::read_to_string(&spec).unwrap().replace(
        "\"sandbox\":null",
        &format!("\"sandbox\":{}", quoted(path(&bin.join("sandbox-runner")))),
    );
    fs::write(&spec, value).unwrap();
    let out = t.0.join("sandboxed");
    good(&["tournament", "--spec", path(&spec), "--out", path(&out)]);
    let status = good(&["status", "--out", path(&out)]);
    assert!(
        status.contains("completed=4"),
        "{status}\n{:?}",
        (0..4)
            .map(|i| fs::read_to_string(out.join(format!("matches/m{i}/meta.json"))).unwrap())
            .collect::<Vec<_>>()
    );
    let report = fs::read_to_string(out.join("matches/m0/attempts/0001/sandbox_a.json")).unwrap();
    assert!(report.contains("space ; literal"), "{report}");
}

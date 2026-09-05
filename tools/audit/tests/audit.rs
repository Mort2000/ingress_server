//! Audit tests (P4-CAPABILITY): lexer pins, manifest parser pins, the
//! mutation fixtures, the real-participant CI-style pass, the operator
//! exemption boundary, the closure native report, and the CLI exit
//! codes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use audit::lexer::{contains_substring, contains_word, strip_source};
use audit::toml::dependency_entries;
use audit::{ALLOWED_EXTERNAL, NativeEntry, Report, RuleId, Severity, run};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture_report(name: &str) -> Report {
    let dir = fixtures().join(name);
    run(&fixtures(), &[dir], &[])
}

// ---------------------------------------------------------------- lexer

#[test]
fn comments_and_literals_never_hide_or_fake_tokens() {
    let cases: &[(&str, bool)] = &[
        // hidden (must NOT flag)
        ("let s = \"unsafe\";", false),
        ("// unsafe", false),
        ("/* unsafe /* nested */ still */", false),
        ("let r = r#\"unsafe\"#;", false),
        ("let c = 'u';", false),
        ("let b = b\"unsafe\";", false),
        // real (must flag)
        ("unsafe { }", true),
        ("let r#unsafe = 1;", true),
        ("fn f() -> u8 { unsafe { 0 } }", true),
    ];
    for (src, expected) in cases {
        let stripped = strip_source(src);
        assert_eq!(
            contains_word(&stripped, "unsafe"),
            *expected,
            "src={src:?} stripped={stripped:?}"
        );
    }
}

#[test]
fn lifetimes_and_word_boundaries() {
    let code = strip_source("fn f<'a>(x: &'a str) -> &'a str { x }");
    assert!(code.contains("'a"), "lifetimes kept: {code:?}");
    let code = strip_source("let unsafe_code = 1;");
    assert!(!contains_word(&code, "unsafe"));
    assert!(contains_word(&code, "unsafe_code"));
}

// ------------------------------------------------------------ manifest

#[test]
fn dependency_entries_parse_sections_and_multiline_tables() {
    let manifest = r#"
[package]
name = "x"

[dependencies]
blake3 = "1.8"
serde = { version = "1",
          features = ["derive"] }

[target.'cfg(unix)'.dependencies]
libc = "0.2"

[dev-dependencies]
arena-core = { path = "../core" }

[workspace.dependencies]
planus = "1.3"
"#;
    let entries = dependency_entries(manifest);
    let names: Vec<(&str, &str)> = entries
        .iter()
        .map(|e| (e.name.as_str(), e.section.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("blake3", "dependencies"),
            ("serde", "dependencies"),
            ("libc", "target.cfg.dependencies"),
            ("arena-core", "dev-dependencies"),
            ("planus", "workspace.dependencies"),
        ]
    );
    let serde = &entries[1];
    assert!(serde.value.contains("features"), "multiline value joined");
}

/// REVIEW FIX pins (M8 review, findings 2+5): dotted single-entry
/// tables and triple target sections parse into ordinary entries, and
/// key extraction is spacing-tolerant (`git="..."`).
#[test]
fn dotted_headers_and_triple_targets_parse_as_entries() {
    let manifest = r#"
[dependencies.evilgit]
git = "https://github.com/example/evil"

[target.x86_64-unknown-linux-gnu.build-dependencies.cc]
version = "1"

[dev-dependencies.probe-helper]
path = "../crates/arena-core"
"#;
    let entries = dependency_entries(manifest);
    let triples: Vec<(&str, &str, &str)> = entries
        .iter()
        .map(|e| (e.name.as_str(), e.section.as_str(), e.value.as_str()))
        .collect();
    assert_eq!(
        triples,
        vec![
            (
                "evilgit",
                "dependencies",
                "git = \"https://github.com/example/evil\""
            ),
            (
                "cc",
                "target.x86_64-unknown-linux-gnu.build-dependencies",
                "version = \"1\""
            ),
            (
                "probe-helper",
                "dev-dependencies",
                "path = \"../crates/arena-core\""
            ),
        ]
    );
    use audit::toml::string_value;
    assert_eq!(
        string_value("git=\"https://x\"", "git").as_deref(),
        Some("https://x")
    );
    assert_eq!(
        string_value("{ version = \"1\", features = [\"git-cli\"] }", "git"),
        None,
        "a feature string is not the git key"
    );
    assert_eq!(
        string_value("{ path = \"../x\", registry = \"custom\" }", "registry").as_deref(),
        Some("custom")
    );
    assert_eq!(string_value("workspace = true", "path"), None);
}

// ------------------------------------------------------------- rules

#[test]
fn clean_fixture_passes_every_rule() {
    let report = fixture_report("clean");
    assert!(report.pass(), "{}", report.render());
    assert!(report.findings.is_empty());
}

#[test]
fn unsafe_block_and_missing_forbid_fail_r1() {
    for fixture in ["unsafe-block", "missing-forbid"] {
        let report = fixture_report(fixture);
        assert!(!report.pass(), "{fixture} must fail");
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == RuleId::R1ForbidUnsafe && f.severity == Severity::Fail),
            "{fixture}: {}",
            report.render()
        );
    }
}

#[test]
fn bad_dep_and_git_dep_fail_r2() {
    let bad = fixture_report("bad-dep");
    assert!(bad.failing_rules().contains(&"R2"), "{}", bad.render());
    let git = fixture_report("git-dep");
    assert!(git.failing_rules().contains(&"R2"), "{}", git.render());
}

#[test]
fn renamed_and_registry_sources_fail_r2_even_with_allowed_names() {
    for (label, manifest) in [
        (
            "renamed",
            "[dependencies]\nmyblake = { package = \"blake3\", version = \"1\" }\n",
        ),
        (
            "registry",
            "[dependencies]\nblake3 = { version = \"1\", registry = \"custom\" }\n",
        ),
    ] {
        let tmp = temp_fixture(label);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("Cargo.toml"), manifest).unwrap();
        fs::write(tmp.join("src/lib.rs"), "#![forbid(unsafe_code)]\n").unwrap();
        let report = run(&fixtures(), &[tmp], &[]);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == RuleId::R2DepAllowlist && f.severity == Severity::Fail),
            "{label}: {}",
            report.render()
        );
    }
}

#[test]
fn participant_build_rs_fails_r3() {
    let report = fixture_report("participant-build-rs");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::R3Native && f.severity == Severity::Fail),
        "{}",
        report.render()
    );
}

#[test]
fn net_import_fails_r4() {
    let report = fixture_report("net-import");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::R4NetAbsence && f.severity == Severity::Fail),
        "{}",
        report.render()
    );
}

/// REVIEW FIX (M8 review, finding 1): the token scans used to cover
/// only `src/**/*.rs`, so a module pulled in via
/// `#[path = "../hidden/evil.rs"]` escaped R1/R4/R5 entirely (verified
/// live: unsafe block + TcpStream::connect behind a clean crate root
/// audited PASS). The scan now covers EVERY `.rs` under the participant
/// crate directory and the inclusion attribute/macro itself is flagged
/// for operator review.
#[test]
fn path_included_module_outside_src_fails_r1_r4() {
    let report = fixture_report("include-path");
    let rendered = report.render();
    assert!(!report.pass(), "the mutant must fail: {rendered}");
    // The hidden file is scanned like any other source: its unsafe
    // block trips R1 and its TcpStream trips R4.
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::R1ForbidUnsafe
                && f.severity == Severity::Fail
                && f.path.ends_with("hidden/evil.rs")),
        "unsafe inside the #[path]-included file is flagged: {rendered}"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::R4NetAbsence
                && f.severity == Severity::Fail
                && f.path.ends_with("hidden/evil.rs")),
        "network token inside the #[path]-included file is flagged: {rendered}"
    );
    // The inclusion itself is flagged: #[path]/include! can also name
    // files OUTSIDE the crate directory, where no scan runs.
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::R1ForbidUnsafe
                && f.severity == Severity::Fail
                && f.detail.contains("#[path]")),
        "the inclusion attribute is its own finding: {rendered}"
    );
}

/// The `include!` macro form of the same inclusion escape.
#[test]
fn include_macro_fails_r1_as_operator_review() {
    let tmp = temp_fixture("include-macro");
    fs::create_dir_all(tmp.join("src")).unwrap();
    fs::write(tmp.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    fs::write(
        tmp.join("src/main.rs"),
        "#![forbid(unsafe_code)]\ninclude!(\"../hidden/x.rs\");\nfn main() {}\n",
    )
    .unwrap();
    let report = run(&tmp, &[tmp.clone()], &[]);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::R1ForbidUnsafe
                && f.severity == Severity::Fail
                && f.detail.contains("include")),
        "include! is flagged: {}",
        report.render()
    );
}

/// REVIEW FIX (M8 review, finding 2): dotted single-entry tables
/// (`[dependencies.evilgit] git = ...`) and TRIPLE target sections
/// (`[target.x86_64-unknown-linux-gnu.dependencies]`) used to be
/// invisible to the manifest parser (verified live: both audited
/// PASS).
#[test]
fn dotted_and_triple_target_dep_sections_fail_r2() {
    for (label, manifest) in [
        (
            "dotted-git",
            "[dependencies.evilgit]\ngit = \"https://github.com/example/evil\"\n",
        ),
        (
            "dotted-registry",
            "[dependencies.blake3]\nversion = \"1\"\nregistry = \"custom\"\n",
        ),
        (
            "triple-target",
            "[target.x86_64-unknown-linux-gnu.dependencies]\nserde = \"1\"\n",
        ),
    ] {
        let tmp = temp_fixture(label);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("Cargo.toml"), manifest).unwrap();
        fs::write(tmp.join("src/lib.rs"), "#![forbid(unsafe_code)]\n").unwrap();
        let report = run(&tmp, &[tmp.clone()], &[]);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == RuleId::R2DepAllowlist && f.severity == Severity::Fail),
            "{label}: {}",
            report.render()
        );
    }
}

/// REVIEW FIX (M8 review, finding 5): `path =` deps used to be skipped
/// as "workspace-internal" with NO containment check, so
/// `cheat = { path = "../../ext/cheat" }` linked unaudited code into
/// the audited build while the participant crate itself stayed clean
/// (verified live). A path dep must resolve to a workspace member
/// crate (crates/*, bots/*, tools/* under the audit root).
#[test]
fn path_dep_escaping_the_workspace_fails_r2() {
    for (label, path) in [
        ("out-of-tree", "../../ext/cheat"),
        ("absolute", "/ext/cheat"),
    ] {
        let tmp = temp_fixture(label);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(
            tmp.join("Cargo.toml"),
            format!("[package]\nname = \"x\"\n\n[dependencies]\ncheat = {{ path = \"{path}\" }}\n"),
        )
        .unwrap();
        fs::write(tmp.join("src/lib.rs"), "#![forbid(unsafe_code)]\n").unwrap();
        let report = run(&tmp, &[tmp.clone()], &[]);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == RuleId::R2DepAllowlist
                    && f.severity == Severity::Fail
                    && f.detail.contains("escapes the audited workspace")),
            "{label}: {}",
            report.render()
        );
    }
}

/// REVIEW FIX (M8 review, finding 2, backstop half): the lockfile
/// external-name scan used to count only `registry` sources, so a
/// git-sourced dep was invisible to allowlist equality too. Every
/// `source =` entry counts now: appending ONE git-sourced package to
/// the real lockfile must break the frozen-allowlist equality check.
#[test]
fn git_sourced_lock_entries_count_as_external() {
    let tmp = temp_fixture("lock-git");
    let mut lock =
        fs::read_to_string(workspace_root().join("Cargo.lock")).expect("real Cargo.lock");
    lock.push_str(
        "\n[[package]]\nname = \"evilgit\"\nversion = \"1.0.0\"\nsource = \
         \"git+https://github.com/example/evil#deadbeef\"\n",
    );
    fs::write(tmp.join("Cargo.lock"), lock).unwrap();
    let err = audit::allowlist_matches_lock(&tmp).expect_err("git source must count");
    assert!(err.contains("evilgit"), "{err}");
    // And the real, unmutated lockfile still matches exactly.
    audit::allowlist_matches_lock(&workspace_root()).expect("real lock == allowlist");
}

#[test]
fn shm_ffi_fixture_fails_r5() {
    let report = fixture_report("shm-ffi");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::R5ShmFfiAbsence && f.severity == Severity::Fail),
        "{}",
        report.render()
    );
}

fn temp_fixture(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m8-audit")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ------------------------------------------------- closure native report

#[test]
fn closure_native_report_flags_unknown_and_sanctions_the_frozen_pair() {
    let root = temp_fixture("vendor");
    fs::create_dir_all(root.join("vendor/blake3-1.8.7/src")).unwrap();
    fs::write(root.join("vendor/blake3-1.8.7/build.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("vendor/blake3-1.8.7/Cargo.toml"),
        "[package]\nname = \"blake3\"\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("vendor/libc-0.2.189/src")).unwrap();
    fs::write(root.join("vendor/libc-0.2.189/build.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("vendor/libc-0.2.189/Cargo.toml"),
        "[package]\nname = \"libc\"\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("vendor/mystery-9.9.9/src")).unwrap();
    fs::write(root.join("vendor/mystery-9.9.9/build.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("vendor/mystery-9.9.9/Cargo.toml"),
        "[package]\nname = \"mystery\"\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("vendor/evil-proc-1.0.0/src")).unwrap();
    fs::write(
        root.join("vendor/evil-proc-1.0.0/Cargo.toml"),
        "[package]\nname = \"evil-proc\"\n\n[lib]\nproc-macro = true\n",
    )
    .unwrap();
    let report = run(&root, &[], &[]);
    let sanctioned: Vec<&NativeEntry> = report.natives.iter().filter(|n| n.sanctioned).collect();
    assert!(
        sanctioned
            .iter()
            .any(|n| n.crate_name == "blake3" && n.kind == "build.rs"),
        "{:?}",
        report.natives
    );
    assert!(
        sanctioned
            .iter()
            .any(|n| n.crate_name == "libc" && n.kind == "build.rs"),
        "{:?}",
        report.natives
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.severity == Severity::Fail && f.detail.contains("mystery")),
        "unknown build.rs fails: {}",
        report.render()
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.severity == Severity::Fail && f.detail.contains("proc-macro")),
        "closure proc-macro fails: {}",
        report.render()
    );
}

// ------------------------------------------- real participants + boundary

/// The CI-style gate: the audit PASSES on the real `bots/*` crates.
#[test]
fn audit_passes_on_real_participants() {
    let root = workspace_root();
    let participants = audit::participant_dirs(&root);
    assert!(participants.len() >= 3, "bots exist");
    let report = run(&root, &participants, &audit::operator_dirs(&root));
    assert!(report.pass(), "{}", report.render());
}

/// The frozen allowlist equals the real external closure of the
/// committed Cargo.lock (they cannot drift silently).
#[test]
fn frozen_allowlist_matches_the_lockfile_closure() {
    audit::allowlist_matches_lock(&workspace_root()).expect("allowlist == lock externals");
}

/// The SDK exemption boundary, pinned from both sides: the operator
/// SDK/server crates DO carry the sanctioned `extern "C"` tokens (they
/// would fail if audited as participants), and they are excluded from
/// the participant set by `participant_dirs` (bots/* only).
#[test]
fn operator_crates_are_the_documented_exemption() {
    let root = workspace_root();
    let operators = audit::operator_dirs(&root);
    let sdk = operators
        .iter()
        .find(|p| p.file_name().unwrap() == "arena-client-sdk")
        .expect("sdk is an operator crate");
    // Audited AS a participant it fails R5 (extern/shm_open/mmap live in
    // its sanctioned shm.rs) — the boundary is the scope split, not the
    // tokens themselves.
    let as_participant = run(&root, &[sdk.clone()], &[]);
    assert!(
        as_participant
            .findings
            .iter()
            .any(|f| f.rule == RuleId::R5ShmFfiAbsence),
        "the SDK's sanctioned shm.rs trips R5 when audited as a participant: {}",
        as_participant.render()
    );
    // And every real participant is under bots/.
    for p in audit::participant_dirs(&root) {
        assert!(p.starts_with(root.join("bots")), "{}", p.display());
    }
}

/// Real closure facts as they exist today (D94: no vendored closure —
/// the dependency closure IS the Cargo.lock external set): the frozen
/// allowlist equals the lockfile external set exactly, and the only
/// native entry is the lockfile-derived sanctioned cc build-dep of
/// blake3; nothing unknown native.
#[test]
fn real_closure_has_no_unknown_native_capability() {
    let root = workspace_root();
    audit::allowlist_matches_lock(&root)
        .expect("frozen allowlist tracks the Cargo.lock external set");
    let participants = audit::participant_dirs(&root);
    let report = run(&root, &participants, &audit::operator_dirs(&root));
    for native in &report.natives {
        if native.kind == "build.rs" {
            assert!(
                native.sanctioned,
                "unsanctioned build script in the closure: {:?}",
                native
            );
        }
    }
    assert!(
        report.natives.iter().all(|n| n.sanctioned),
        "no unsanctioned native capability in the closure: {:?}",
        report.natives
    );
    assert!(
        report
            .natives
            .iter()
            .any(|n| n.crate_name == "cc" && n.sanctioned),
        "the lockfile-derived sanctioned cc build-dep entry is present: {:?}",
        report.natives
    );
    assert!(report.pass(), "{}", report.render());
}

// ------------------------------------------------------------- CLI

#[test]
fn audit_bin_exit_codes_and_output() {
    let bin = env!("CARGO_BIN_EXE_audit");
    // Real repo: pass.
    let ok = Command::new(bin)
        .arg("--root")
        .arg(workspace_root())
        .output()
        .expect("run audit bin");
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let text = String::from_utf8_lossy(&ok.stdout);
    assert!(text.contains("VERDICT: PASS"), "{text}");
    assert!(text.contains("SANCTIONED"), "native report printed: {text}");

    // Mutant tree: fail with the rule id on stdout.
    let tmp = temp_fixture("bin-fail");
    let bots = tmp.join("bots/unsafe-block/src");
    fs::create_dir_all(&bots).unwrap();
    fs::copy(
        fixtures().join("unsafe-block/Cargo.toml"),
        tmp.join("bots/unsafe-block/Cargo.toml"),
    )
    .unwrap();
    fs::copy(
        fixtures().join("unsafe-block/src/lib.rs"),
        bots.join("lib.rs"),
    )
    .unwrap();
    fs::copy(workspace_root().join("Cargo.lock"), tmp.join("Cargo.lock")).unwrap();
    let bad = Command::new(bin)
        .arg("--root")
        .arg(&tmp)
        .output()
        .expect("run");
    assert_eq!(bad.status.code(), Some(1));
    let text = String::from_utf8_lossy(&bad.stdout);
    assert!(text.contains("R1"), "rule id in report: {text}");
    assert!(text.contains("VERDICT: FAIL"), "{text}");
}

/// Substring/word scanner sanity for the token lists themselves.
#[test]
fn net_tokens_flag_the_canonical_import() {
    let code = strip_source("use std::net::TcpStream;\nfn f() {}\n");
    assert!(contains_substring(&code, "std::net"));
    assert!(contains_substring(&code, "TcpStream"));
}

/// The frozen external list is exactly 15 names (D73 /).
#[test]
fn allowlist_shape() {
    assert_eq!(ALLOWED_EXTERNAL.len(), 15);
    assert!(ALLOWED_EXTERNAL.contains(&"blake3"));
    assert!(ALLOWED_EXTERNAL.contains(&"planus"));
    assert!(!ALLOWED_EXTERNAL.contains(&"serde"));
}

//! White-box audit of participant crates (P4-CAPABILITY, D73).
//! `#![forbid(unsafe_code)]` in this crate: the audit itself must be
//! safe Rust and stay OUT of the audited closure (std-only, no deps).
//!
//! # Ruleset (frozen, D73)
//!
//! * **R1 FORBID-UNSAFE** — every participant crate root must carry
//!   `#![forbid(unsafe_code)]`, and after the frozen lexical strip no
//!   `unsafe` token may remain in ANY `.rs` file under the participant
//!   crate directory (not only `src/` — modules can be pulled in from
//!   anywhere in the crate via `#[path]`/`include!`). The inclusion
//!   attributes/macros themselves are flagged for operator review:
//!   they can also name files OUTSIDE the crate directory, where no
//!   scan runs.
//! * **R2 DEP-ALLOWLIST** — every dependency entry of a participant
//!   manifest (plain, dotted `[dependencies.name]`, and
//!   `target.*.dependencies` tables alike) must be a `path =` dep that
//!   resolves to a workspace MEMBER crate (`crates/*`, `bots/*`,
//!   `tools/*`) or one of the frozen 15 sanctioned external names;
//!   `git =`/registry/renamed (`package =`) sources FAIL even with
//!   allowed names, and out-of-workspace path deps FAIL (they would
//!   link unaudited code into the audited build).
//! * **R3 NATIVE-FLAG** — participant-owned `build.rs`, build-deps,
//!   proc-macros, `links =`, and staticlib/cdylib crate types FAIL;
//!   native code inside the dependency closure is REPORTED with a
//!   verdict: SANCTIONED (blake3 build.rs + cc-built C intrinsics,
//!   libc build.rs — ADR-0013) or UNKNOWN (any other native capability
//!   that has not been separately audited and approved). Since D94 the
//!   repo carries no vendored closure: the lockfile half still reports
//!   the sanctioned cc build-dep of blake3, and a `vendor/` directory,
//!   if present, resumes the full source scan.
//! * **R4 NET-ABSENCE** — networking tokens (`std::net`, TcpStream, ...)
//!   must be absent from participant sources.
//! * **R5 SHM/FFI-ABSENCE** — raw system-boundary tokens (`extern`,
//!   `shm_open`, `mmap`, `MAP_SHARED`, `/dev/shm`, `syscall`) must be
//!   absent from participant sources; SHM/RPC/clock access is SDK-only
//!   (operator side, exempt by the frozen operator list).
//!
//! Over-flagging (lexical, not AST) is the safe direction.

#![forbid(unsafe_code)]

pub mod lexer;
pub mod toml;

use std::fs;
use std::path::{Path, PathBuf};

/// The frozen sanctioned external dependency set: exactly the 15
/// external crates of the committed Cargo.lock closure (blake3 +
/// planus and their transitive deps — ADR-0013).
pub const ALLOWED_EXTERNAL: [&str; 15] = [
    "allocator-api2",
    "array-init-cursor",
    "arrayvec",
    "blake3",
    "cc",
    "cfg-if",
    "constant_time_eq",
    "cpufeatures",
    "equivalent",
    "find-msvc-tools",
    "foldhash",
    "hashbrown",
    "libc",
    "planus",
    "shlex",
];

/// External crates whose native code is sanctioned (ADR-0013): blake3
/// (build.rs compiling C/asm intrinsics via cc) and libc (build.rs).
/// Consulted only by the vendor-source scan — dormant since D94 (the
/// repo resolves via mirror; the lockfile half is
/// `Report::lock_external_names`). Anything else native is UNKNOWN and
/// fails the audit.
const SANCTIONED_BUILD_SCRIPTS: [&str; 2] = ["blake3", "libc"];

/// R4 frozen token list (substring scans over stripped sources).
pub const NET_TOKENS: [&str; 7] = [
    "std::net",
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "SocketAddr",
    "ToSocketAddrs",
    "lookup_host",
];

/// R5 frozen token list.
pub const FFI_TOKENS: [&str; 6] = [
    "extern",
    "shm_open",
    "mmap",
    "MAP_SHARED",
    "/dev/shm",
    "syscall",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleId {
    R1ForbidUnsafe,
    R2DepAllowlist,
    R3Native,
    R4NetAbsence,
    R5ShmFfiAbsence,
}

impl RuleId {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleId::R1ForbidUnsafe => "R1",
            RuleId::R2DepAllowlist => "R2",
            RuleId::R3Native => "R3",
            RuleId::R4NetAbsence => "R4",
            RuleId::R5ShmFfiAbsence => "R5",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Fails the audit.
    Fail,
    /// Present, pre-approved, reported for the record.
    Sanctioned,
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub rule: RuleId,
    pub severity: Severity,
    pub path: String,
    pub detail: String,
}

/// One native-capability entry in the closure-wide R3 report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeEntry {
    pub crate_name: String,
    pub kind: &'static str,
    pub sanctioned: bool,
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub root: PathBuf,
    pub participants: Vec<PathBuf>,
    pub operators: Vec<PathBuf>,
    pub findings: Vec<Finding>,
    pub natives: Vec<NativeEntry>,
}

impl Report {
    pub fn pass(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Fail)
    }

    pub fn failing_rules(&self) -> Vec<&'static str> {
        let mut rules: Vec<&'static str> = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Fail)
            .map(|f| f.rule.as_str())
            .collect();
        rules.sort();
        rules.dedup();
        rules
    }

    /// Human-readable rendering (also used by the bin).
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("audit root: {}\n", self.root.display()));
        out.push_str(&format!(
            "participants: {}\n",
            self.participants
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str("native closure report (R3):\n");
        for native in &self.natives {
            out.push_str(&format!(
                "  [{}] {} {} ({})\n",
                if native.sanctioned {
                    "SANCTIONED"
                } else {
                    "UNKNOWN   "
                },
                native.crate_name,
                native.kind,
                native.path
            ));
        }
        out.push_str("findings:\n");
        for finding in &self.findings {
            out.push_str(&format!(
                "  [{}] {:?} {}: {}\n",
                if finding.severity == Severity::Fail {
                    "FAIL"
                } else {
                    "OK  "
                },
                finding.rule,
                finding.path,
                finding.detail
            ));
        }
        out.push_str(if self.pass() {
            "VERDICT: PASS\n"
        } else {
            "VERDICT: FAIL\n"
        });
        out
    }
}

/// Participant crate directories: every `bots/*` subdir with a manifest.
pub fn participant_dirs(root: &Path) -> Vec<PathBuf> {
    crate_dirs(&root.join("bots"))
}

/// Operator crate directories: `crates/*` and `tools/*` (SDK, server,
/// and tooling — their one sanctioned module each is exempt per the
/// frozen operator list; P4-CAPABILITY scopes the participant rules to
/// `bots/*`).
pub fn operator_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = crate_dirs(&root.join("crates"));
    dirs.extend(crate_dirs(&root.join("tools")));
    dirs
}

fn crate_dirs(parent: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let Ok(entries) = fs::read_dir(parent) else {
        return dirs;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() && path.join("Cargo.toml").is_file() {
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs
}

/// Workspace member crate directories (the workspace glob of the root
/// `Cargo.toml`): `crates/*`, `bots/*`, `tools/*`. These — and only
/// these — are legal `path =` dependency targets for a participant:
/// every member is inside the operator-built, provenance-hashed tree
/// and operator crates are on the exempt side of the boundary.
fn workspace_member_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = crate_dirs(&root.join("crates"));
    dirs.extend(crate_dirs(&root.join("bots")));
    dirs.extend(crate_dirs(&root.join("tools")));
    dirs.into_iter().map(|d| normalize_path(&d)).collect()
}

/// Lexically normalizes a path (resolves `.`/`..` components without
/// touching the filesystem, so half-existing fixture trees work). A
/// `..` above the root clamps at the root.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

/// True when `resolved` is exactly one of the workspace member crates
/// (symlinks resolved when both sides exist, lexical equality else).
fn is_workspace_member(members: &[PathBuf], resolved: &Path) -> bool {
    members.iter().any(|member| {
        if member == resolved {
            return true;
        }
        match (member.canonicalize(), resolved.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    })
}

/// Runs the full ruleset. `root` is the repository root; `participants`
/// are audited; `workspace` (operator crates) are recorded as the
/// exempt side of the boundary.
pub fn run(root: &Path, participants: &[PathBuf], workspace: &[PathBuf]) -> Report {
    let mut report = Report {
        root: root.to_path_buf(),
        participants: participants.to_vec(),
        operators: workspace.to_vec(),
        findings: Vec::new(),
        natives: Vec::new(),
    };
    for participant in participants {
        audit_participant(participant, &mut report);
    }
    audit_closure(root, &mut report);
    report
}

fn audit_participant(crate_dir: &Path, report: &mut Report) {
    let rel = |p: &Path| -> String {
        p.strip_prefix(&report.root)
            .unwrap_or(p)
            .display()
            .to_string()
    };
    let src = crate_dir.join("src");
    let manifest_path = crate_dir.join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap_or_default();
    let crate_label = rel(&crate_dir);

    // ---- R1: attribute + token scan over every participant .rs ----
    let roots: Vec<PathBuf> = ["lib.rs", "main.rs"]
        .iter()
        .map(|r| src.join(r))
        .filter(|p| p.is_file())
        .collect();
    if roots.is_empty() {
        report.findings.push(Finding {
            rule: RuleId::R1ForbidUnsafe,
            severity: Severity::Fail,
            path: crate_label.clone(),
            detail: "no src/lib.rs or src/main.rs crate root".to_string(),
        });
    }
    for root_file in &roots {
        let text = fs::read_to_string(root_file).unwrap_or_default();
        let stripped = lexer::strip_source(&text);
        if !stripped.contains("#![forbid(unsafe_code)]") {
            report.findings.push(Finding {
                rule: RuleId::R1ForbidUnsafe,
                severity: Severity::Fail,
                path: rel(root_file),
                detail: "crate root is missing #![forbid(unsafe_code)]".to_string(),
            });
        }
    }
    // Scan EVERY `.rs` under the participant crate directory, not only
    // `src/`: `#[path = ...] mod x;` and `include!(...)` can pull source
    // from anywhere inside the crate, and those files are compiled into
    // the audited binary exactly like `src/` files.
    for file in rust_files(crate_dir) {
        let text = fs::read_to_string(&file).unwrap_or_default();
        let stripped = lexer::strip_source(&text);
        let label = rel(&file);
        if lexer::contains_word(&stripped, "unsafe") {
            report.findings.push(Finding {
                rule: RuleId::R1ForbidUnsafe,
                severity: Severity::Fail,
                path: label.clone(),
                detail: "`unsafe` token present after lexical strip".to_string(),
            });
        }
        // Inclusion attribute/macro: the one way participant source can
        // name a file OUTSIDE the crate directory, where no scan runs.
        // Over-flagging (every `include*!` form) is the safe direction;
        // the operator reviews the finding.
        if stripped.contains("#[path") || stripped.contains("# [path") {
            report.findings.push(Finding {
                rule: RuleId::R1ForbidUnsafe,
                severity: Severity::Fail,
                path: label.clone(),
                detail: "`#[path]` module inclusion can name files outside the \
                         participant crate directory (no scan covers them) — \
                         requires operator review"
                    .to_string(),
            });
        }
        if lexer::contains_word(&stripped, "include") {
            report.findings.push(Finding {
                rule: RuleId::R1ForbidUnsafe,
                severity: Severity::Fail,
                path: label.clone(),
                detail: "`include*!` macro can name files outside the \
                         participant crate directory (no scan covers them) — \
                         requires operator review"
                    .to_string(),
            });
        }
        // ---- R4: networking absence ----
        for token in NET_TOKENS {
            if lexer::contains_substring(&stripped, token) {
                report.findings.push(Finding {
                    rule: RuleId::R4NetAbsence,
                    severity: Severity::Fail,
                    path: label.clone(),
                    detail: format!("networking token `{token}` present"),
                });
            }
        }
        // ---- R5: raw SHM/FFI absence ----
        for token in FFI_TOKENS {
            let hit = if token == "extern" || token == "syscall" {
                lexer::contains_word(&stripped, token)
            } else {
                lexer::contains_substring(&stripped, token)
            };
            if hit {
                report.findings.push(Finding {
                    rule: RuleId::R5ShmFfiAbsence,
                    severity: Severity::Fail,
                    path: label.clone(),
                    detail: format!("raw system-boundary token `{token}` present"),
                });
            }
        }
    }

    // ---- R2: dependency allowlist ----
    let members = workspace_member_dirs(&report.root);
    for entry in toml::dependency_entries(&manifest) {
        let value = entry.value.as_str();
        let detail_fail = |what: String| Finding {
            rule: RuleId::R2DepAllowlist,
            severity: Severity::Fail,
            path: format!("{crate_label}/Cargo.toml"),
            detail: format!("[{}] `{}` {}", entry.section, entry.name, what),
        };
        // Key-based extraction (spacing-tolerant): `git="..."` must not
        // slip past a `contains("git =")` check.
        if let Some(path) = toml::string_value(value, "path") {
            // Path deps are the one sanctioned non-registry source, but
            // ONLY when the target is an audited workspace member crate
            // — an out-of-tree target links unaudited code (free to be
            // `unsafe`/FFI/networked; it is never scanned) into the
            // audited build, and path deps carry no registry source in
            // Cargo.lock, so the allowlist-equality backstop cannot see
            // them either.
            let resolved = normalize_path(&crate_dir.join(&path));
            if !is_workspace_member(&members, &resolved) {
                report.findings.push(detail_fail(format!(
                    "path dep `{path}` escapes the audited workspace (resolves \
                     to {}, not a crates/*/bots/*/tools/* member)",
                    resolved.display()
                )));
            }
            continue;
        }
        if ["git", "branch", "tag", "rev"]
            .iter()
            .any(|key| toml::string_value(value, key).is_some())
        {
            report
                .findings
                .push(detail_fail("uses a git source".into()));
            continue;
        }
        if toml::string_value(value, "registry").is_some() {
            report
                .findings
                .push(detail_fail("uses an alternative registry source".into()));
            continue;
        }
        if toml::string_value(value, "package").is_some() {
            report
                .findings
                .push(detail_fail("is a renamed package source".into()));
            continue;
        }
        if !ALLOWED_EXTERNAL.contains(&entry.name.as_str()) {
            report.findings.push(detail_fail(
                "is not in the sanctioned dependency set (workspace-member \
                 path deps and the frozen 15-crate closure only)"
                    .into(),
            ));
        }
    }
    if toml::has_patch_or_replace(&manifest) {
        report.findings.push(Finding {
            rule: RuleId::R2DepAllowlist,
            severity: Severity::Fail,
            path: format!("{crate_label}/Cargo.toml"),
            detail: "[patch]/[replace] source substitution is not sanctioned".to_string(),
        });
    }

    // ---- R3 (participant half): no native capability ----
    let has_build_rs = crate_dir.join("build.rs").is_file();
    let has_build_deps = toml::dependency_entries(&manifest)
        .iter()
        .any(|e| e.section.contains("build-dependencies"));
    let lib_crate_type = toml::table_value(&manifest, "lib", "crate-type");
    let proc_macro = toml::table_value(&manifest, "lib", "proc-macro");
    if has_build_rs {
        report.findings.push(Finding {
            rule: RuleId::R3Native,
            severity: Severity::Fail,
            path: format!("{crate_label}/build.rs"),
            detail: "participant-owned build script requires separate audit".to_string(),
        });
    }
    if has_build_deps {
        report.findings.push(Finding {
            rule: RuleId::R3Native,
            severity: Severity::Fail,
            path: format!("{crate_label}/Cargo.toml"),
            detail: "participant [build-dependencies] require separate audit".to_string(),
        });
    }
    if let Some(value) = lib_crate_type {
        if value.contains("staticlib") || value.contains("cdylib") {
            report.findings.push(Finding {
                rule: RuleId::R3Native,
                severity: Severity::Fail,
                path: format!("{crate_label}/Cargo.toml"),
                detail: format!("native crate-type {value}"),
            });
        }
    }
    if proc_macro.as_deref() == Some("true") {
        report.findings.push(Finding {
            rule: RuleId::R3Native,
            severity: Severity::Fail,
            path: format!("{crate_label}/Cargo.toml"),
            detail: "participant proc-macro crate requires separate audit".to_string(),
        });
    }
}

/// R3 (closure half): report every native capability under `vendor/`
/// when a vendored closure is present (dormant since D94), plus the
/// lockfile-derived cc build-dep of blake3.
fn audit_closure(root: &Path, report: &mut Report) {
    let vendor = root.join("vendor");
    let mut crates_seen: Vec<PathBuf> = Vec::new();
    if vendor.is_dir() {
        let mut dirs: Vec<PathBuf> = fs::read_dir(&vendor)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default();
        dirs.sort();
        crates_seen = dirs;
    }
    for dir in crates_seen {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        // Vendored names are `name-version` when --versioned-dirs is
        // used; split at the final '-'.
        let base = match name.rsplit_once('-') {
            Some((head, _)) if head.chars().any(|c| c.is_ascii_alphabetic()) => head.to_string(),
            _ => name.clone(),
        };
        let rel = dir.strip_prefix(root).unwrap_or(&dir).display().to_string();
        if dir.join("build.rs").is_file() {
            let sanctioned = SANCTIONED_BUILD_SCRIPTS.contains(&base.as_str());
            report.natives.push(NativeEntry {
                crate_name: base.clone(),
                kind: "build.rs",
                sanctioned,
                path: format!("{rel}/build.rs"),
            });
            if !sanctioned {
                report.findings.push(Finding {
                    rule: RuleId::R3Native,
                    severity: Severity::Fail,
                    path: format!("{rel}/build.rs"),
                    detail: format!("unsanctioned build script in dependency `{base}`"),
                });
            }
        }
        let manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
        if toml::table_value(&manifest, "lib", "proc-macro").as_deref() == Some("true") {
            report.natives.push(NativeEntry {
                crate_name: base.clone(),
                kind: "proc-macro",
                sanctioned: false,
                path: format!("{rel}/Cargo.toml"),
            });
            report.findings.push(Finding {
                rule: RuleId::R3Native,
                severity: Severity::Fail,
                path: format!("{rel}/Cargo.toml"),
                detail: format!("proc-macro crate `{base}` in the closure"),
            });
        }
        if let Some(links) = toml::table_value(&manifest, "package", "links") {
            report.natives.push(NativeEntry {
                crate_name: base.clone(),
                kind: "links",
                sanctioned: false,
                path: format!("{rel}/Cargo.toml"),
            });
            report.findings.push(Finding {
                rule: RuleId::R3Native,
                severity: Severity::Fail,
                path: format!("{rel}/Cargo.toml"),
                detail: format!("`links = {links}` native library binding"),
            });
        }
        if base.ends_with("-sys") || base.ends_with("-ffi") {
            report.natives.push(NativeEntry {
                crate_name: base.clone(),
                kind: "sys/ffi-suffix",
                sanctioned: false,
                path: format!("{rel}/Cargo.toml"),
            });
            report.findings.push(Finding {
                rule: RuleId::R3Native,
                severity: Severity::Fail,
                path: format!("{rel}/Cargo.toml"),
                detail: format!("`{base}` looks like a native binding crate"),
            });
        }
    }
    // cc is a sanctioned BUILD dependency of blake3 (the C/asm
    // intrinsics); record it when present in the closure.
    let closure_has_cc = report.lock_external_names(root).iter().any(|n| n == "cc");
    if closure_has_cc {
        report.natives.push(NativeEntry {
            crate_name: "cc".to_string(),
            kind: "build-dep of blake3 (C/asm intrinsics)",
            sanctioned: true,
            path: "Cargo.lock".to_string(),
        });
    }
}

impl Report {
    /// External crate names in Cargo.lock: EVERY entry that carries a
    /// `source =` line (registry, git, sparse — path/workspace deps
    /// carry none and stay workspace-internal). This is the backstop
    /// behind R2: a git-sourced dep smuggled into the lockfile shows up
    /// here and breaks allowlist equality. Used to keep the frozen
    /// allowlist honest.
    fn lock_external_names(&self, root: &Path) -> Vec<String> {
        let mut names = Vec::new();
        let Ok(text) = fs::read_to_string(root.join("Cargo.lock")) else {
            return names;
        };
        let mut current: Option<String> = None;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("name = ") {
                current = Some(rest.trim_matches('"').to_string());
            } else if line.starts_with("source = ") {
                if let Some(name) = current.take() {
                    names.push(name);
                }
            }
        }
        names.sort();
        names
    }
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs(dir, &mut files);
    files.sort();
    files
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            // Build outputs never contain audited source.
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Asserts the frozen allowlist equals the actual external closure in
/// Cargo.lock (used by tests and available to the operator).
pub fn allowlist_matches_lock(root: &Path) -> Result<(), String> {
    let dummy = Report {
        root: root.to_path_buf(),
        participants: vec![],
        operators: vec![],
        findings: vec![],
        natives: vec![],
    };
    let external = dummy.lock_external_names(root);
    let mut allowed: Vec<&str> = ALLOWED_EXTERNAL.to_vec();
    allowed.sort();
    if external != allowed {
        return Err(format!(
            "Cargo.lock external set {external:?} != frozen allowlist {allowed:?}"
        ));
    }
    Ok(())
}

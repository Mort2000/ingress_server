//! Compile-fail harness (P4-CAPABILITY, no trybuild): each snippet in
//! `tests/compile_fail/` is built as a throwaway cargo project with a
//! path dependency on this SDK and a SHARED target dir; the build must
//! fail with the expected error code from its `// error-code:` header.
//! Runtime siblings (context close, phase violation) live in
//! `tests/runtime.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

fn snippets() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/compile_fail");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("compile_fail dir")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "compile-fail snippets exist");
    files
}

/// The expected failure, from a `// error-code: EXXXX` or
/// `// error: <substring>` header (some lifetime diagnostics have no
/// error code).
#[derive(Debug)]
enum Expected {
    Code(String),
    Message(String),
}

fn expected_error(path: &Path) -> Expected {
    for line in std::fs::read_to_string(path).expect("snippet").lines() {
        if let Some(code) = line.strip_prefix("// error-code: ") {
            return Expected::Code(code.trim().to_string());
        }
        if let Some(message) = line.strip_prefix("// error: ") {
            return Expected::Message(message.trim().to_string());
        }
    }
    panic!(
        "{} lacks a `// error-code: EXXXX` or `// error: ...` header",
        path.display()
    );
}

#[test]
fn observation_view_cannot_escape() {
    let shared_target =
        std::env::temp_dir().join(format!("ingress-m7-cf-target-{}", std::process::id()));
    std::fs::create_dir_all(&shared_target).expect("shared target dir");
    let sdk = workspace_root().join("crates/arena-client-sdk");
    let core = workspace_root().join("crates/arena-core");

    for snippet in snippets() {
        let expected = expected_error(&snippet);
        let name = snippet.file_stem().unwrap().to_string_lossy().to_string();
        let project =
            std::env::temp_dir().join(format!("ingress-m7-cf-{}-{}", name, std::process::id()));
        let src = project.join("src");
        std::fs::create_dir_all(&src).expect("project src");
        std::fs::write(
            project.join("Cargo.toml"),
            format!(
                "[package]\nname = \"cf_{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
                 [dependencies]\narena-client-sdk = {{ path = {sdk:?} }}\n\
                 arena-core = {{ path = {core:?} }}\n[workspace]\n"
            ),
        )
        .expect("Cargo.toml");
        std::fs::copy(&snippet, src.join("main.rs")).expect("main.rs");

        let output = Command::new(env!("CARGO"))
            .arg("build")
            .current_dir(&project)
            .env("CARGO_TARGET_DIR", &shared_target)
            .output()
            .expect("cargo build runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "{name}: expected a compile error, got a successful build"
        );
        let matches = match &expected {
            Expected::Code(code) => {
                stderr.contains(&format!("error[{code}]"))
                    || stderr.contains(&format!("error: {code}"))
            }
            Expected::Message(message) => stderr.contains(message.as_str()),
        };
        assert!(matches, "{name}: expected {expected:?}, stderr:\n{stderr}");
        let _ = std::fs::remove_dir_all(&project);
    }
}

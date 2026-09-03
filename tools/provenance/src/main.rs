//! provenance CLI: records the audited-build manifest (P4-BUILD, D71).
//!
//! ```text
//! provenance [--root <repo>] [--target-dir target] [--profile release]
//!            [--out-dir runs/builds]
//! ```
//!
//! Writes `<out-dir>/<build-id>/manifest.json` and prints a human
//! summary. Run AFTER `cargo build --release --locked --offline` so all
//! recorded products exist.

use std::path::PathBuf;

fn main() {
    let mut root = std::env::current_dir().expect("cwd");
    let mut target_dir = PathBuf::from("target");
    let mut profile = String::from("release");
    let mut out_dir = PathBuf::from("runs/builds");

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match flag.as_str() {
            "--root" => root = PathBuf::from(value()),
            "--target-dir" => target_dir = PathBuf::from(value()),
            "--profile" => profile = value(),
            "--out-dir" => out_dir = PathBuf::from(value()),
            other => {
                eprintln!("unknown flag {other:?}");
                std::process::exit(1);
            }
        }
    }

    let provenance = match provenance::compute(&root) {
        Ok(p) => p,
        Err(error) => {
            eprintln!("provenance: {error}");
            std::process::exit(1);
        }
    };
    let binaries = match provenance::collect_binaries(
        &root.join(&target_dir),
        &profile,
        &provenance::DEFAULT_BINARIES,
    ) {
        Ok(b) => b,
        Err(error) => {
            eprintln!("provenance: {error} (build the workspace first)");
            std::process::exit(1);
        }
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let id = provenance::build_id(&provenance, now);
    let dir = out_dir.join(&id);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("provenance: cannot create {}: {error}", dir.display());
        std::process::exit(1);
    }
    let manifest = provenance::manifest_json(&provenance, &binaries);
    let path = dir.join("manifest.json");
    std::fs::write(&path, &manifest).expect("write manifest");
    std::fs::write(dir.join("id.txt"), &id).expect("write id");

    println!("build-id      {id}");
    println!("tree-hash     {}", provenance.tree_hash);
    println!("files-hashed  {}", provenance.file_count);
    println!("lock-hash     {}", provenance.lock_hash);
    println!(
        "toolchain     {} ({})",
        provenance.toolchain.commit_hash,
        provenance
            .toolchain
            .channel
            .as_deref()
            .unwrap_or("<unpinned>")
    );
    println!("binaries      {}", binaries.len());
    println!("manifest      {}", path.display());
}

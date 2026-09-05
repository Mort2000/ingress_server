//! Build provenance for the audited build closure (P4-BUILD, D71).
//!
//! Every match product must be traceable to the exact audited inputs:
//! the source tree, the lockfile, the toolchain identity, and the final
//! binaries (ADR-0005). This crate computes those identities with
//! domain-separated BLAKE3 hashes and writes an ordered, deterministic
//! manifest (hand-rolled std-only JSON, M6 pattern — no serde).
//!
//! # D71 canonicalization (frozen)
//!
//! * **tree_hash** = blake3 `Hasher::new_der_key("ingress-arena-tree-v0.1")`
//!   over the stream: for each regular file in ascending order **sorted by
//!   the raw path bytes** (POSIX `/`-relative UTF-8 paths):
//!   `u32le(path_len) ‖ path ‖ u64le(content_len) ‖ content ‖ u8(exec_bit)`;
//!   terminated by `u64le(file_count)`. Walk root = the repository. The
//!   frozen exclusion set prunes any directory named `.git`, `target`,
//!   `runs`, or `.scratch` (build outputs, evidence dirs, planning
//!   scratch). Everything else in the tree participates — whatever the
//!   tree contains is the audited input (since D94 there is no
//!   `vendor/`; dependencies resolve via mirror per the lockfile).
//!   Symlinks are an error (never followed).
//!   `exec_bit = (mode & 0o111 != 0) as u8` — git's only tracked mode bit.
//! * **lock_hash** = `new_der_key("ingress-arena-lock-v0.1")` over the raw
//!   `Cargo.lock` bytes.
//! * **toolchain_hash** = `new_der_key("ingress-arena-toolchain-v0.1")`
//!   over `rustc -Vv` stdout — an identity proxy, not a full toolchain
//!   content hash (documented honestly; the rust-toolchain.toml pin plus
//!   the recorded `rustc -Vv` text is what the operator verifies).
//! * **binary hash** = `new_der_key("ingress-arena-bin-v0.1")` over the
//!   file bytes, recorded as `name ‖ size ‖ hex`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Frozen exclusion set: directory names never hashed (D71).
pub const EXCLUDED_DIRS: [&str; 4] = [".git", "target", "runs", ".scratch"];

/// Domain labels (D71).
///
/// Domain separation uses the official BLAKE3 key-derivation
/// construction `Hasher::new_keyed(&derive_key(context, &[]))` — the
/// pinned blake3 1.8.7 predates the `new_der_key` convenience wrapper
/// for exactly this construction (noted in progress.md deviations).
pub const TREE_DOMAIN: &str = "ingress-arena-tree-v0.1";
pub const LOCK_DOMAIN: &str = "ingress-arena-lock-v0.1";
pub const TOOLCHAIN_DOMAIN: &str = "ingress-arena-toolchain-v0.1";
pub const BIN_DOMAIN: &str = "ingress-arena-bin-v0.1";

/// Manifest schema tag.
pub const MANIFEST_SCHEMA: &str = "ingress-provenance-v1";

/// Errors of the provenance layer.
#[derive(Debug)]
pub enum ProvenanceError {
    Io(std::io::Error),
    /// A path under the root is not valid UTF-8 (canonical paths are
    /// UTF-8 by D71).
    NonUtf8Path(PathBuf),
    /// A symlink was encountered; D71 never follows symlinks.
    Symlink(PathBuf),
    /// A non-regular, non-directory entry was encountered.
    SpecialFile(PathBuf),
    /// A required input file is missing (e.g. Cargo.lock).
    Missing(PathBuf),
    /// A required binary was not found in the target dir.
    MissingBinary {
        name: String,
        searched: PathBuf,
    },
}

impl core::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProvenanceError::Io(e) => write!(f, "io error: {e}"),
            ProvenanceError::NonUtf8Path(p) => write!(f, "path is not utf-8: {}", p.display()),
            ProvenanceError::Symlink(p) => {
                write!(f, "symlink refused (never followed): {}", p.display())
            }
            ProvenanceError::SpecialFile(p) => {
                write!(f, "special file not hashed: {}", p.display())
            }
            ProvenanceError::Missing(p) => write!(f, "required input missing: {}", p.display()),
            ProvenanceError::MissingBinary { name, searched } => {
                write!(f, "binary {name:?} not found under {}", searched.display())
            }
        }
    }
}

impl std::error::Error for ProvenanceError {}

impl From<std::io::Error> for ProvenanceError {
    fn from(value: std::io::Error) -> Self {
        ProvenanceError::Io(value)
    }
}

/// Toolchain identity of the audited build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolchainInfo {
    /// `rustc -Vv` output, verbatim (trailing newline trimmed).
    pub rustc_vv: String,
    /// The `commit-hash:` line, verbatim.
    pub commit_hash: String,
    /// Channel string from `rust-toolchain.toml`, if the file exists.
    pub channel: Option<String>,
    /// blake3(`TOOLCHAIN_DOMAIN`, `rustc -Vv` stdout) hex.
    pub toolchain_hash: String,
}

/// One recorded build product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryRecord {
    pub name: String,
    pub size: u64,
    pub blake3_hex: String,
}

/// The provenance of one tree state (binaries are collected separately:
/// they exist only after a build).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub tree_hash: String,
    pub file_count: u64,
    pub lock_hash: String,
    pub toolchain: ToolchainInfo,
}

/// Returns the repository files in D71 canonical order (ascending raw
/// path bytes), applying the frozen exclusion set. Public so tests can
/// pin the byte-order rule (no locale collation).
pub fn ordered_files(root: &Path) -> Result<Vec<PathBuf>, ProvenanceError> {
    let mut files = Vec::new();
    walk(root, PathBuf::new(), &mut files)?;
    // Ascending by raw path bytes — Rust string/Path ordering IS bytewise
    // on Unix; sort by the encoded form to make that explicit and stable.
    files.sort_by(|a, b| {
        a.as_os_str()
            .as_encoded_bytes()
            .cmp(b.as_os_str().as_encoded_bytes())
    });
    Ok(files)
}

fn walk(root: &Path, rel: PathBuf, out: &mut Vec<PathBuf>) -> Result<(), ProvenanceError> {
    let dir = root.join(&rel);
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| {
            ProvenanceError::Io(std::io::Error::other(format!(
                "reading {}: {e}",
                dir.display()
            )))
        })?
        .filter_map(|entry| entry.ok())
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| ProvenanceError::NonUtf8Path(dir.join(&name)))?;
        let ft = entry.file_type().map_err(ProvenanceError::Io)?;
        let child_rel = rel.join(name);
        if ft.is_symlink() {
            return Err(ProvenanceError::Symlink(root.join(&child_rel)));
        }
        if ft.is_dir() {
            if EXCLUDED_DIRS.contains(&name) {
                continue; // frozen exclusion set (D71)
            }
            walk(root, child_rel, out)?;
        } else if ft.is_file() {
            out.push(child_rel);
        } else {
            return Err(ProvenanceError::SpecialFile(root.join(&child_rel)));
        }
    }
    Ok(())
}

fn domain_hasher(domain: &str) -> blake3::Hasher {
    blake3::Hasher::new_keyed(&blake3::derive_key(domain, &[]))
}

fn hash_file(domain: &str, path: &Path) -> Result<blake3::Hash, ProvenanceError> {
    let bytes = fs::read(path).map_err(ProvenanceError::Io)?;
    Ok(domain_hasher(domain).update(&bytes).finalize())
}

/// Computes tree/lock/toolchain provenance for the tree rooted at
/// `root` (the repository root).
pub fn compute(root: &Path) -> Result<Provenance, ProvenanceError> {
    let files = ordered_files(root)?;
    let mut tree = domain_hasher(TREE_DOMAIN);
    let mut count: u64 = 0;
    for rel in &files {
        let path = root.join(rel);
        let posix = rel
            .to_str()
            .ok_or_else(|| ProvenanceError::NonUtf8Path(path.clone()))?;
        let bytes = fs::read(&path).map_err(ProvenanceError::Io)?;
        let exec = fs::metadata(&path)
            .map_err(ProvenanceError::Io)?
            .permissions()
            .mode()
            & 0o111;
        let path_bytes = posix.as_bytes();
        tree.update(&(path_bytes.len() as u32).to_le_bytes());
        tree.update(path_bytes);
        tree.update(&(bytes.len() as u64).to_le_bytes());
        tree.update(&bytes);
        tree.update(&[u8::from(exec != 0)]);
        count += 1;
    }
    tree.update(&count.to_le_bytes());

    let lock_path = root.join("Cargo.lock");
    if !lock_path.is_file() {
        return Err(ProvenanceError::Missing(lock_path));
    }
    let lock_hash = hash_file(LOCK_DOMAIN, &lock_path)?.to_hex().to_string();

    let toolchain = toolchain_info(root)?;
    Ok(Provenance {
        tree_hash: tree.finalize().to_hex().to_string(),
        file_count: count,
        lock_hash,
        toolchain,
    })
}

/// `rustc -Vv` identity plus the rust-toolchain.toml channel pin.
pub fn toolchain_info(root: &Path) -> Result<ToolchainInfo, ProvenanceError> {
    let output = std::process::Command::new("rustc")
        .arg("-Vv")
        .output()
        .map_err(|e| {
            ProvenanceError::Io(std::io::Error::other(format!("running rustc -Vv: {e}")))
        })?;
    if !output.status.success() {
        return Err(ProvenanceError::Io(std::io::Error::other(format!(
            "rustc -Vv failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))));
    }
    let vv = String::from_utf8(output.stdout)
        .map_err(|_| ProvenanceError::Io(std::io::Error::other("rustc -Vv not utf-8")))?
        .trim_end()
        .to_string();
    let commit_hash = vv
        .lines()
        .find(|l| l.starts_with("commit-hash:"))
        .unwrap_or("commit-hash: <none>")
        .to_string();
    let channel = channel_pin(root);
    let toolchain_hash = domain_hasher(TOOLCHAIN_DOMAIN)
        .update(vv.as_bytes())
        .finalize()
        .to_hex()
        .to_string();
    Ok(ToolchainInfo {
        rustc_vv: vv,
        commit_hash,
        channel,
        toolchain_hash,
    })
}

/// Reads `channel = "..."` from rust-toolchain.toml (minimal parser; the
/// file is two lines of standard rustup config today).
pub fn channel_pin(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("rust-toolchain.toml")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("channel") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let value = rest.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// Default product set recorded by the build proof: the server,
/// every participant client, and the operator tools.
pub const DEFAULT_BINARIES: [&str; 12] = [
    "arena-server",
    "nearest-client",
    "hybrid-client",
    "probe-client",
    "provenance",
    "audit",
    "sandbox-runner",
    "sb-malloc-bomb",
    "sb-spin",
    "sb-net-probe",
    "sb-fs-probe",
    "sb-env-report",
];

/// Hashes the named products under `<target_dir>/<profile>/`.
pub fn collect_binaries(
    target_dir: &Path,
    profile: &str,
    names: &[&str],
) -> Result<Vec<BinaryRecord>, ProvenanceError> {
    let dir = target_dir.join(profile);
    let mut records = Vec::new();
    for name in names {
        let path = dir.join(name);
        if !path.is_file() {
            return Err(ProvenanceError::MissingBinary {
                name: (*name).to_string(),
                searched: dir.clone(),
            });
        }
        let size = fs::metadata(&path).map_err(ProvenanceError::Io)?.len();
        let hex = hash_file(BIN_DOMAIN, &path)?.to_hex().to_string();
        records.push(BinaryRecord {
            name: (*name).to_string(),
            size,
            blake3_hex: hex,
        });
    }
    Ok(records)
}

/// Deterministic manifest body (no wall-clock fields: two computations
/// over the same tree produce byte-identical output).
pub fn manifest_json(provenance: &Provenance, binaries: &[BinaryRecord]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"schema\": {},\n", json_str(MANIFEST_SCHEMA)));
    out.push_str("  \"tree\": {\n");
    out.push_str(&format!(
        "    \"hash\": {},\n",
        json_str(&provenance.tree_hash)
    ));
    out.push_str(&format!("    \"file_count\": {},\n", provenance.file_count));
    out.push_str("    \"excluded_dirs\": [");
    let excluded: Vec<String> = EXCLUDED_DIRS.iter().map(|d| json_str(d)).collect();
    out.push_str(&excluded.join(", "));
    out.push_str("]\n  },\n");
    out.push_str("  \"lock\": {\n");
    out.push_str(&format!(
        "    \"hash\": {}\n",
        json_str(&provenance.lock_hash)
    ));
    out.push_str("  },\n");
    out.push_str("  \"toolchain\": {\n");
    out.push_str(&format!(
        "    \"channel\": {},\n",
        json_str(
            provenance
                .toolchain
                .channel
                .as_deref()
                .unwrap_or("<unpinned>")
        )
    ));
    out.push_str(&format!(
        "    \"commit_hash\": {},\n",
        json_str(&provenance.toolchain.commit_hash)
    ));
    out.push_str(&format!(
        "    \"rustc_vv\": {},\n",
        json_str(&provenance.toolchain.rustc_vv)
    ));
    out.push_str(&format!(
        "    \"hash\": {}\n",
        json_str(&provenance.toolchain.toolchain_hash)
    ));
    out.push_str("  },\n");
    out.push_str("  \"binaries\": [\n");
    for (i, record) in binaries.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"name\": {},\n", json_str(&record.name)));
        out.push_str(&format!("      \"size\": {},\n", record.size));
        out.push_str(&format!(
            "      \"blake3\": {}\n",
            json_str(&record.blake3_hex)
        ));
        out.push_str(if i + 1 == binaries.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

/// JSON string literal with minimal escaping.
pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Build id: `YYYYMMDD-HHMM-<tree_hash[0..12]>` (the timestamp lives in
/// the id/directory name only, never in the manifest body).
pub fn build_id(provenance: &Provenance, unix_secs: u64) -> String {
    // Civil-time conversion (days since epoch -> y/m/d) without external
    // crates; valid for 1970..~9999.
    let days = unix_secs / 86_400;
    let secs_of_day = unix_secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{:04}{:02}{:02}-{:02}{:02}-{}",
        year,
        month,
        day,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        &provenance.tree_hash[..12]
    )
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

//! Entrant registry and provenance-as-data (D79, P5): an entrant is
//! an external client binary. Provenance is PURE
//! DATA — the runner computes only the binary hash itself (blake3 over
//! the file bytes under the frozen m9 domain); the tree/lock/toolchain
//! hashes and the manifest id are RECORDED LITERALS from `tools/provenance`
//! output supplied in the config, so arena-runner never depends on the
//! provenance tool (keeps the crate blake3 + workspace paths only).

use std::io;
use std::path::Path;

use crate::tournament::BINARY_HASH_DOMAIN;

/// Index into `TournamentConfig::entrants`.
pub type EntrantId = u32;

/// One registered competitor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entrant {
    /// Unique within a tournament (asserted by `validate_entrants`).
    pub name: String,
    pub kind: EntrantKind,
    pub provenance: EntrantProvenance,
}

/// How an entrant is executed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntrantKind {
    /// A real client binary: argv is `<socket> <client_seed>`; `env` carries
    /// the entrant's own variables (e.g. `PROBE_MODE` for the probe
    /// fixture) through the sandbox's frozen allowlist.
    External {
        binary: std::path::PathBuf,
        env: Vec<(String, String)>,
    },
}

impl EntrantKind {
    pub fn tag(&self) -> &'static str {
        match self {
            EntrantKind::External { .. } => "external",
        }
    }
}

/// Recorded provenance fields (all optional; all DATA — see the module
/// docs). Mirrors the P5 requirement that every Match Outcome is
/// traceable to both clients' source tree, lockfile, toolchain, and
/// binary hash.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct EntrantProvenance {
    /// blake3 hex over the binary file bytes under
    /// `"ingress-arena-m9-binary"` — COMPUTED by the runner (external
    /// entrants only).
    pub binary_blake3: Option<String>,
    /// `tools/provenance` tree hash (recorded literal).
    pub tree_hash: Option<String>,
    /// `tools/provenance` lock hash (recorded literal).
    pub lock_hash: Option<String>,
    /// `tools/provenance` toolchain hash (recorded literal).
    pub toolchain_hash: Option<String>,
    /// Build-manifest location, e.g. `runs/builds/<id>` (recorded literal).
    pub manifest_id: Option<String>,
    /// Entrant self-reported strategy version.
    pub bot_version: Option<String>,
}

impl EntrantProvenance {
    /// Recorded literals from a `tools/provenance` manifest.
    pub fn recorded(
        tree_hash: Option<String>,
        lock_hash: Option<String>,
        toolchain_hash: Option<String>,
        manifest_id: Option<String>,
    ) -> Self {
        EntrantProvenance {
            tree_hash,
            lock_hash,
            toolchain_hash,
            manifest_id,
            ..Default::default()
        }
    }

    /// Compact per-match provenance id for `index.tsv`: the binary hash
    /// (first 16 hex) for external entrants.
    pub fn id_tag(&self, kind: &EntrantKind) -> String {
        match kind {
            EntrantKind::External { .. } => match &self.binary_blake3 {
                Some(hash) => format!("blake3:{}", &hash[..hash.len().min(16)]),
                None => "blake3:unhashed".to_string(),
            },
        }
    }
}

impl Entrant {
    /// Computes `provenance.binary_blake3` for the executable.
    pub fn compute_binary_hash(&mut self) -> io::Result<()> {
        let EntrantKind::External { binary, .. } = &self.kind;
        self.provenance.binary_blake3 = Some(compute_binary_hash(binary)?);
        Ok(())
    }
}

/// blake3 hex over the file bytes under the frozen m9 binary domain.
/// Deliberately NOT the `tools/provenance` `ingress-arena-bin-v0.1`
/// keyed construction: the runner hashes plain bytes so an independent
/// `blake3 <file>` verification (plus the domain label) suffices.
pub fn compute_binary_hash(path: &Path) -> io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINARY_HASH_DOMAIN);
    hasher.update(&bytes);
    Ok(hasher.finalize().to_hex().to_string())
}

/// Registry invariants: names unique, external binaries exist.
pub fn validate_entrants(entrants: &[Entrant]) -> Result<(), String> {
    let mut names: Vec<&str> = entrants.iter().map(|e| e.name.as_str()).collect();
    names.sort();
    for pair in names.windows(2) {
        if pair[0] == pair[1] {
            return Err(format!("duplicate entrant name {:?}", pair[0]));
        }
    }
    for entrant in entrants {
        let EntrantKind::External { binary, .. } = &entrant.kind;
        if !binary.is_file() {
            return Err(format!(
                "entrant {:?} binary missing: {}",
                entrant.name,
                binary.display()
            ));
        }
    }
    Ok(())
}

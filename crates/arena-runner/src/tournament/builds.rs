//! Immutable-by-contract build copies with content validation, not authentication.
use super::{archive::Json, document::*, durable::atomic_write, entrant::compute_binary_hash};
use std::{fs, io, os::unix::fs::PermissionsExt, path::Path};

pub fn register(spec: &Path, out: &Path) -> io::Result<Json> {
    let request = read(spec)?;
    keys(
        &request,
        &[
            "schema",
            "entrant_id",
            "version",
            "binary",
            "source_tree",
            "commit",
            "dirty",
            "lock_hash",
            "toolchain",
            "build_args",
            "audit",
        ],
    )?;
    if text(&request, "schema")? != "arena-build-request-v1" {
        return Err(invalid("unsupported build request"));
    }
    identifier(text(&request, "entrant_id")?)?;
    for key in ["version", "source_tree", "commit", "lock_hash", "toolchain"] {
        if text(&request, key)?.is_empty() {
            return Err(invalid(format!(
                "{key} must explicitly record a value or 'unknown'"
            )));
        }
    }
    if !matches!(request.get("dirty"), Some(Json::Bool(_))) {
        return Err(invalid("dirty must be boolean"));
    }
    for arg in array(&request, "build_args")? {
        arg.as_str()
            .ok_or_else(|| invalid("build_args must be string array"))?;
    }
    let audit = request
        .get("audit")
        .ok_or_else(|| invalid("audit required"))?;
    keys(audit, &["status", "evidence"])?;
    if !matches!(text(audit, "status")?, "pass" | "fail" | "not-reviewed") {
        return Err(invalid("audit.status must be pass, fail or not-reviewed"));
    }
    if text(audit, "evidence")?.is_empty() {
        return Err(invalid("audit evidence must be recorded"));
    }
    let binary = spec
        .parent()
        .unwrap_or(Path::new("."))
        .join(text(&request, "binary")?)
        .canonicalize()?;
    let bytes = fs::read(&binary)?;
    let mode = fs::metadata(&binary)?.permissions().mode();
    if mode & 0o111 == 0 {
        return Err(invalid("binary is not executable"));
    }
    fs::create_dir(out)?;
    atomic_write(&out.join("binary"), &bytes)?;
    fs::set_permissions(out.join("binary"), fs::Permissions::from_mode(0o555))?;
    fs::File::open(out.join("binary"))?.sync_all()?;
    let manifest = object(vec![
        ("schema", string("arena-build-v1")),
        (
            "binary_hash",
            string(compute_binary_hash(&out.join("binary"))?),
        ),
        ("request", request),
    ]);
    atomic_write(&out.join("manifest.json"), encode(&manifest).as_bytes())?;
    atomic_write(
        &out.join("identity"),
        blake3::hash(encode(&manifest).as_bytes())
            .to_hex()
            .as_bytes(),
    )?;
    Ok(manifest)
}
pub fn inspect(dir: &Path) -> io::Result<Json> {
    let manifest = read(&dir.join("manifest.json"))?;
    if text(&manifest, "schema")? != "arena-build-v1" {
        return Err(invalid("unsupported build schema"));
    }
    if fs::read_to_string(dir.join("identity"))?
        != blake3::hash(encode(&manifest).as_bytes()).to_hex().as_str()
    {
        return Err(invalid("build manifest identity mismatch"));
    }
    if text(&manifest, "binary_hash")? != compute_binary_hash(&dir.join("binary"))? {
        return Err(invalid("build binary hash mismatch"));
    }
    Ok(manifest)
}

/// Project the registered evidence into the legacy provenance interface. The
/// toolchain field contains recorded rustc text, so hash it under D71's domain;
/// never put a version description into a field named toolchain_hash.
pub(crate) fn provenance(manifest: &Json) -> io::Result<super::EntrantProvenance> {
    let request = manifest
        .get("request")
        .ok_or_else(|| invalid("missing build request"))?;
    let recorded = |key: &str| -> io::Result<Option<String>> {
        let s = text(request, key)?;
        Ok((s != "unknown").then(|| s.to_owned()))
    };
    let toolchain_hash = recorded("toolchain")?.map(|s| {
        let mut h =
            blake3::Hasher::new_keyed(&blake3::derive_key("ingress-arena-toolchain-v0.1", &[]));
        h.update(s.as_bytes());
        h.finalize().to_hex().to_string()
    });
    Ok(super::EntrantProvenance {
        binary_blake3: Some(text(manifest, "binary_hash")?.into()),
        tree_hash: recorded("source_tree")?,
        lock_hash: recorded("lock_hash")?,
        toolchain_hash,
        manifest_id: Some(
            blake3::hash(encode(manifest).as_bytes())
                .to_hex()
                .to_string(),
        ),
        bot_version: Some(text(request, "version")?.into()),
    })
}
/// Copy bytes, never hard-link a build into a batch. Archive relocation resolves
/// these paths again at load time; the original development directory is unused.
pub(crate) fn copy_into(source: &Path, dest: &Path) -> io::Result<()> {
    inspect(source)?;
    fs::create_dir(dest)?;
    for file in ["binary", "manifest.json", "identity"] {
        atomic_write(&dest.join(file), &fs::read(source.join(file))?)?;
    }
    fs::set_permissions(dest.join("binary"), fs::Permissions::from_mode(0o555))?;
    fs::File::open(dest.join("binary"))?.sync_all()?;
    inspect(dest)?;
    Ok(())
}

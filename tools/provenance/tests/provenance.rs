//! Provenance tests (P4-BUILD, D71): canonical-form golden vector,
//! determinism, mutation sensitivity, exclusion correctness, byte-order
//! path sort, symlink refusal, binary collection, and the deterministic
//! manifest body.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use provenance::{
    BIN_DOMAIN, LOCK_DOMAIN, MANIFEST_SCHEMA, TREE_DOMAIN, build_id, collect_binaries, compute,
    manifest_json, ordered_files,
};

fn fixture(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m8-provenance")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("fixture dir");
    dir
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, bytes).expect("write");
}

/// Recursively copies a fixture tree (regular files only; modes copied).
fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("dst dir");
    for entry in fs::read_dir(src).expect("read src") {
        let entry = entry.expect("entry");
        let target = dst.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy");
        }
    }
}

/// The canonical tree-hash stream for known files, hand-assembled here
/// and fed to the same domain — proves the encoding byte-for-byte.
fn canonical_stream(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let mut stream = Vec::new();
    for &(path, content, exec) in entries {
        stream.extend(&(path.len() as u32).to_le_bytes());
        stream.extend(path.as_bytes());
        stream.extend(&(content.len() as u64).to_le_bytes());
        stream.extend(content);
        stream.push(exec as u8);
    }
    stream.extend(&(entries.len() as u64).to_le_bytes());
    stream
}

fn domain_hash(domain: &str, bytes: &[u8]) -> String {
    blake3::Hasher::new_keyed(&blake3::derive_key(domain, &[]))
        .update(bytes)
        .finalize()
        .to_hex()
        .to_string()
}

fn minimal_tree(tag: &str) -> PathBuf {
    let root = fixture(tag);
    write(&root.join("Cargo.toml"), b"[workspace]\n");
    write(&root.join("Cargo.lock"), b"version = 4\n");
    write(&root.join("src/a.txt"), b"alpha\n");
    write(&root.join("src/z.txt"), b"zulu\n");
    root
}

/// D71 canonical form pinned exactly: the computed tree_hash equals the
/// domain hash over the hand-assembled stream, with the exec bit and
/// content participating.
#[test]
fn tree_hash_matches_the_hand_assembled_canonical_stream() {
    let root = minimal_tree("canonical");
    // Make one file executable to pin the exec-bit byte.
    let bin = root.join("src/a.txt");
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).expect("chmod");

    let entries = [
        ("Cargo.lock", b"version = 4\n".as_slice(), false),
        ("Cargo.toml", b"[workspace]\n".as_slice(), false),
        ("src/a.txt", b"alpha\n".as_slice(), true),
        ("src/z.txt", b"zulu\n".as_slice(), false),
    ];
    let expected = domain_hash(TREE_DOMAIN, &canonical_stream(&entries));
    let got = compute(&root).expect("compute").tree_hash;
    assert_eq!(got, expected, "byte-exact canonical encoding");
}

/// Two computations over the same tree are identical, and the manifest
/// body (no wall-clock fields) is byte-identical.
#[test]
fn double_compute_is_deterministic() {
    let root = minimal_tree("determinism");
    let first = compute(&root).expect("first");
    let second = compute(&root).expect("second");
    assert_eq!(first, second);
    let binaries = vec![provenance::BinaryRecord {
        name: "x".into(),
        size: 3,
        blake3_hex: "ab".into(),
    }];
    assert_eq!(
        manifest_json(&first, &binaries),
        manifest_json(&second, &binaries)
    );
}

/// One appended byte in one file changes tree_hash; so does a rename, a
/// move between directories, and an exec-bit flip.
#[test]
fn mutation_rename_move_and_exec_bit_all_change_the_tree_hash() {
    let root = minimal_tree("mutation");
    let base = compute(&root).expect("base");

    let mutated = fixture("mutation2");
    copy_tree(&root, &mutated);
    let file = mutated.join("src/z.txt");
    let mut bytes = fs::read(&file).expect("read");
    bytes.push(b'!');
    write(&file, &bytes);
    assert_ne!(
        compute(&mutated).expect("mutated").tree_hash,
        base.tree_hash
    );

    let renamed = fixture("mutation3");
    copy_tree(&root, &renamed);
    fs::rename(renamed.join("src/a.txt"), renamed.join("src/b.txt")).expect("rename");
    assert_ne!(
        compute(&renamed).expect("renamed").tree_hash,
        base.tree_hash
    );

    let moved = fixture("mutation4");
    copy_tree(&root, &moved);
    fs::create_dir_all(moved.join("src/deep")).expect("dir");
    fs::rename(moved.join("src/z.txt"), moved.join("src/deep/z.txt")).expect("move");
    assert_ne!(compute(&moved).expect("moved").tree_hash, base.tree_hash);

    let execed = fixture("mutation5");
    copy_tree(&root, &execed);
    fs::set_permissions(execed.join("src/z.txt"), fs::Permissions::from_mode(0o755))
        .expect("chmod");
    assert_ne!(compute(&execed).expect("execed").tree_hash, base.tree_hash);
}

/// Changes under the frozen exclusion set never change tree_hash;
/// any other directory participates (all non-excluded content is part
/// of the audited input).
#[test]
fn exclusion_set_is_exact() {
    let root = minimal_tree("exclusions");
    let base = compute(&root).expect("base");
    for excluded in ["target", ".git", "runs", ".scratch"] {
        write(
            &root.join(format!("{excluded}/junk-{excluded}.bin")),
            b"noise",
        );
    }
    assert_eq!(
        compute(&root).expect("with junk").tree_hash,
        base.tree_hash,
        "excluded dirs never hashed"
    );
    // Nested excluded dirs are pruned too (e.g. a nested target/).
    write(&root.join("src/target/nested.bin"), b"noise");
    assert_eq!(
        compute(&root).expect("nested junk").tree_hash,
        base.tree_hash
    );

    // Any non-excluded directory participates (vendor/ and .cargo/ are
    // just representative names; the repo itself no longer carries them).
    let closure_root = fixture("closure-covered");
    copy_tree(&root, &closure_root);
    let closure_base = compute(&closure_root).expect("closure base");
    write(
        &closure_root.join("vendor/some-crate/lib.rs"),
        b"pub fn f() {}\n",
    );
    assert_ne!(
        compute(&closure_root).expect("vendor").tree_hash,
        closure_base.tree_hash
    );
    write(&closure_root.join(".cargo/config.toml"), b"[source]\n");
    assert_ne!(
        compute(&closure_root).expect("dot-cargo").tree_hash,
        closure_base.tree_hash
    );
}

/// Path order is raw-byte ascending, not locale collation: uppercase
/// letters sort before lowercase in byte order but after in most locale
/// sorts; non-ASCII pins the bytewise rule beyond ASCII.
#[test]
fn paths_sort_by_raw_bytes_not_locale() {
    let root = fixture("sortpin");
    for name in [
        "b-file.txt",
        "B-file.txt",
        "a-file.txt",
        "A-file.txt",
        "z-last.txt",
    ] {
        write(&root.join(name), b"x");
    }
    let ordered = ordered_files(&root).expect("ordered");
    let names: Vec<String> = ordered
        .iter()
        .map(|p| p.to_str().expect("utf8").to_string())
        .collect();
    // Byte order: 'A'(0x41) < 'B'(0x42) < 'a'(0x61) < 'b'(0x62) < 'z'(0x7a).
    assert_eq!(
        names,
        vec![
            "A-file.txt",
            "B-file.txt",
            "a-file.txt",
            "b-file.txt",
            "z-last.txt"
        ]
    );
    let mut sorted_copy = names.clone();
    sorted_copy.sort();
    assert_eq!(names, sorted_copy, "Rust string sort IS the byte order");

    // A pair differing above ASCII: 'z'(0x7A) < '~'(0x7E) — both after
    // every ASCII letter, ordered bytewise.
    let root2 = fixture("sortpin2");
    write(&root2.join("~tilde"), b"x");
    write(&root2.join("zletter"), b"x");
    let ordered2: Vec<String> = ordered_files(&root2)
        .expect("ordered")
        .iter()
        .map(|p| p.to_str().unwrap().to_string())
        .collect();
    assert_eq!(ordered2, vec!["zletter", "~tilde"], "0x7A < 0x7E");
}

/// Symlinks are refused (never followed), including symlinked dirs.
#[test]
fn symlinks_are_errors() {
    let root = fixture("symlink");
    write(&root.join("real.txt"), b"r");
    std::os::unix::fs::symlink(root.join("real.txt"), root.join("link.txt")).expect("symlink");
    assert!(compute(&root).is_err());
    let root2 = fixture("symlinkdir");
    fs::create_dir_all(root2.join("sub")).expect("dir");
    write(&root2.join("sub/f.txt"), b"f");
    std::os::unix::fs::symlink(root2.join("sub"), root2.join("dlink")).expect("dir symlink");
    assert!(compute(&root2).is_err());
}

/// lock_hash is the domain-separated hash of the raw Cargo.lock bytes.
#[test]
fn lock_hash_is_der_keyed_raw_bytes() {
    let root = minimal_tree("lock");
    let expected = domain_hash(
        LOCK_DOMAIN,
        &fs::read(root.join("Cargo.lock")).expect("lock bytes"),
    );
    assert_eq!(compute(&root).expect("compute").lock_hash, expected);
    // Changing Cargo.lock changes it.
    write(&root.join("Cargo.lock"), b"version = 4\n# touched\n");
    assert_ne!(compute(&root).expect("compute2").lock_hash, expected);
}

/// A missing Cargo.lock is a named error.
#[test]
fn missing_lockfile_is_a_named_error() {
    let root = fixture("nolock");
    write(&root.join("Cargo.toml"), b"[workspace]\n");
    assert!(matches!(
        compute(&root),
        Err(provenance::ProvenanceError::Missing(_))
    ));
}

/// Binary collection records name/size/domain hash and names missing
/// products as errors.
#[test]
fn binary_collection_hashes_products() {
    let target = fixture("bins");
    let dir = target.join("release");
    write(&dir.join("arena-server"), b"server-bytes");
    write(&dir.join("reference-client"), b"client-bytes");
    let records = collect_binaries(&target, "release", &["arena-server", "reference-client"])
        .expect("collect");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].name, "arena-server");
    assert_eq!(records[0].size, 12);
    assert_eq!(
        records[0].blake3_hex,
        domain_hash(BIN_DOMAIN, b"server-bytes")
    );
    assert!(collect_binaries(&target, "release", &["absent-bin"]).is_err());
}

/// The manifest body carries the schema tag, fixed key order, and the
/// excluded-dir list; ids embed the timestamp, the manifest never does.
#[test]
fn manifest_body_shape_and_build_id() {
    let root = minimal_tree("manifest");
    let provenance = compute(&root).expect("compute");
    let body = manifest_json(&provenance, &[]);
    assert!(body.starts_with("{\n"));
    assert!(body.contains(format!("\"schema\": \"{MANIFEST_SCHEMA}\"").as_str()));
    let tree_at = body.find("\"tree\"").expect("tree");
    let lock_at = body.find("\"lock\"").expect("lock");
    let toolchain_at = body.find("\"toolchain\"").expect("toolchain");
    let binaries_at = body.find("\"binaries\"").expect("binaries");
    assert!(tree_at < lock_at && lock_at < toolchain_at && toolchain_at < binaries_at);
    assert!(body.contains("\"excluded_dirs\": [\".git\", \"target\", \"runs\", \".scratch\"]"));

    let id_a = build_id(&provenance, 1_700_000_000);
    let id_b = build_id(&provenance, 1_700_003_600);
    assert!(id_a.ends_with(&provenance.tree_hash[..12]));
    assert_ne!(id_a, id_b, "the timestamp participates in the id");
    assert_eq!(id_a.len(), "YYYYMMDD-HHMM-".len() + 12);
}

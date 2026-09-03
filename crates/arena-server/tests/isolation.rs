//! Information-isolation tests (A-ISOLATION, D77): the SHM regions of
//! a match do not exist before the server publishes (nothing for a
//! pre-match process to race), and they are unlinked when the match
//! ends — a fresh match id leaves no residue in /dev/shm. The
//! mapping-level guarantees (PROT_READ-only, no enemy partition in
//! the SDK process) are pinned cross-crate in
//! crates/arena-client-sdk/tests/isolation.rs; the same-UID open
//! residual is the documented AUDIT boundary (D77, docs/sandbox.md).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn target_bin(name: &str) -> PathBuf {
    if let Ok(path) = std::env::var(format!("CARGO_BIN_EXE_{name}")) {
        return PathBuf::from(path);
    }
    let target = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target");
    for profile in ["debug", "release"] {
        let candidate = target.join(profile).join(name);
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("binary {name} not built (run cargo build --workspace)");
}

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ingress-m8-isolation")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

struct Server {
    child: Child,
    #[allow(dead_code)]
    socket: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_server(tag: &str, rounds: u32) -> Server {
    let dir = temp(tag);
    let socket = dir.join("arena.sock");
    let child = Command::new(target_bin("arena-server"))
        .arg("--socket")
        .arg(&socket)
        .arg("--seed")
        .arg("0x77")
        .arg("--map-seed")
        .arg("11")
        .arg("--config")
        .arg("fast")
        .arg("--rounds")
        .arg(rounds.to_string())
        .arg("--match-id")
        .arg(tag)
        .arg("--out")
        .arg(dir.join("out"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("server spawns");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !socket.exists() {
        assert!(Instant::now() < deadline, "socket never appeared");
        std::thread::sleep(Duration::from_millis(5));
    }
    Server { child, socket }
}

fn region_files(match_id: &str) -> Vec<PathBuf> {
    let pattern = format!("ingress-{match_id}");
    let mut hits = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/shm") {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_name().to_string_lossy().starts_with(&pattern) {
                hits.push(entry.path());
            }
        }
    }
    hits
}

fn spawn_probe(socket: &Path, mode: &str) -> Child {
    Command::new(target_bin("probe-client"))
        .arg(socket)
        .env("PROBE_MODE", mode)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("probe spawns")
}

/// Before INITIAL_SPAWN_START — in fact before BOTH client HELLOs
/// arrive — no region file for the match exists: the glob over
/// /dev/shm for the fresh, unguessable match id finds nothing, even
/// with one client already connected (stalled pre-handshake). The
/// regions are created by the publish itself, not by the match id.
#[test]
fn no_region_files_exist_before_first_publish() {
    let server = spawn_server("isopre", 3);
    let mut stalled = spawn_probe(&server.socket, "stall");
    // Give the connection time to land; the stall probe never HELLOs,
    // so the server cannot have authenticated two clients (D61) and
    // must not have published anything.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        region_files("isopre").is_empty(),
        "no region may exist before the first publish"
    );
    let _ = stalled.kill();
    let _ = stalled.wait();
    drop(server);
    assert!(region_files("isopre").is_empty());
}

/// A fully completed match unlinks all three regions (D63: unlinked on
/// drop): after the server exits cleanly the /dev/shm glob for the
/// match id is empty again.
#[test]
fn regions_are_unlinked_when_the_match_ends() {
    let mut server = spawn_server("isopost", 3);
    let mut a = spawn_probe(&server.socket, "valid");
    let mut b = spawn_probe(&server.socket, "valid");
    let _ = a.wait().expect("a exits");
    let _ = b.wait().expect("b exits");
    let status = server.child.wait().expect("server exits");
    assert!(status.success(), "normal completion");
    a.kill().ok();
    b.kill().ok();
    drop(server);
    assert!(
        region_files("isopost").is_empty(),
        "regions unlinked at match end"
    );
}

/// Region names embed the match id and nothing else guessable-by-position:
/// the three names for one match are distinct and all carry the id
/// (D63). This pins the unguessability input: the id is chosen per
/// match by the operator, never derived from a counter a client sees.
#[test]
fn region_names_carry_the_match_id_and_partition_suffixes() {
    let (public, enl, res) = arena_server::shm::region_names("mid-xyz");
    assert_eq!(public, "/ingress-mid-xyz-public");
    assert_eq!(enl, "/ingress-mid-xyz-enl");
    assert_eq!(res, "/ingress-mid-xyz-res");
    assert_eq!(
        region_files("mid-xyz").len(),
        0,
        "no such match is running; names are derived, never created here"
    );
}

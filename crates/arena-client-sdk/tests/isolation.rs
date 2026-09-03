//! SDK-side information isolation (A-ISOLATION / A-PHASE-INTEGRITY,
//! D77): what a compliant SDK session actually maps. Assertions run
//! against `/proc/self/maps` of THIS process while an SDK Session for
//! the ENL faction is live over server-published regions:
//!
//! * the public and OWN private partitions are mapped, and every
//!   ingress mapping is `r--` (PROT_READ — the SDK twin never requests
//!   PROT_WRITE; ADR-0006 zero-copy observation);
//! * the ENEMY (RES) partition is never mapped by the Session;
//! * the same-UID open of the enemy region NAME — which v0.1 does NOT
//!   technically prevent — is the documented AUDIT boundary (D77): a
//!   non-compliant client could open it, which is exactly why R1/R5
//!   (forbid unsafe, raw-token absence) and the dep allowlist exist.
//!   This test performs the open to RECORD the boundary and drops the
//!   mapping immediately; the SDK Session API itself has no code path
//!   that does this (pinned by the compile-fail snippets).

use arena_client_sdk::round::Session;
use arena_client_sdk::shm::region_names;
use arena_core::types::Faction;
use arena_protocol::decode::decode_public;
use arena_protocol::{PUBLIC_SNAPSHOT_VERSION, build};

use arena_server::shm::{REGION_KIND_ENL, REGION_KIND_PUBLIC, REGION_KIND_RES, ShmPublisher};

fn maps_lines() -> Vec<String> {
    std::fs::read_to_string("/proc/self/maps")
        .expect("maps")
        .lines()
        .map(|l| l.to_string())
        .collect()
}

fn ingress_mappings(match_id: &str) -> Vec<(String, String)> {
    // (permissions, path) for every /dev/shm ingress region mapping.
    maps_lines()
        .iter()
        .filter(|l| l.contains(&format!("/dev/shm/ingress-{match_id}")))
        .filter_map(|l| {
            let mut parts = l.splitn(6, ' ');
            let _range = parts.next()?;
            let perms = parts.next()?.to_string();
            let path = l.rsplit(' ').next()?.to_string();
            Some((perms, path))
        })
        .collect()
}

/// The read-only (`r--`) ingress mappings — the ones an SDK READER
/// created. (The test process also hosts the server-side publishers,
/// whose `rw-s` mappings are the sanctioned writer side; they must
/// never be confused with the SDK's claims.)
fn sdk_readonly_mappings(match_id: &str) -> Vec<(String, String)> {
    ingress_mappings(match_id)
        .into_iter()
        .filter(|(perms, _)| perms.starts_with("r--"))
        .collect()
}

/// The core mapping-level guarantee, from inside the SDK process.
#[test]
fn session_maps_only_own_partitions_and_every_mapping_is_read_only() {
    let match_id = format!("iso-{}", std::process::id());
    let (public_name, enl_name, res_name) = region_names(&match_id);
    let cfg = arena_core::config::GameConfig::default();
    let map = arena_core::map::MapDefinition::new(Vec::new());

    let mut public = ShmPublisher::create(
        &public_name,
        REGION_KIND_PUBLIC,
        PUBLIC_SNAPSHOT_VERSION,
        8 << 20,
    )
    .expect("public");
    let mut enl = ShmPublisher::create(
        &enl_name,
        REGION_KIND_ENL,
        arena_protocol::PRIVATE_SNAPSHOT_VERSION,
        2 << 20,
    )
    .expect("enl");
    let mut res = ShmPublisher::create(
        &res_name,
        REGION_KIND_RES,
        arena_protocol::PRIVATE_SNAPSHOT_VERSION,
        2 << 20,
    )
    .expect("res");
    public
        .publish(&build::map_generation_bytes(&map, 0, 0), 0, 0, 0)
        .expect("publish public");
    enl.publish(&build::private_roster_bytes(&cfg, Faction::Enl), 0, 0, 0)
        .expect("publish enl");
    res.publish(&build::private_roster_bytes(&cfg, Faction::Res), 0, 0, 0)
        .expect("publish res");

    // Sanity: the public payload decodes before the session exists.
    {
        let reader = arena_client_sdk::shm::ShmReader::open(
            &public_name,
            arena_client_sdk::shm::RegionHeader::public(),
        )
        .expect("open");
        let snapshot = reader.read().expect("read");
        assert!(decode_public(snapshot.payload).is_ok());
    }

    // Session::new eagerly opens BOTH partitions it is entitled to
    // (public + own private) and nothing else.
    let session = Session::new(&match_id, Faction::Enl, cfg.clone()).expect("session");

    let mappings = sdk_readonly_mappings(&match_id);
    assert!(
        mappings.len() >= 2,
        "public + own private are mapped read-only: {mappings:?}"
    );
    for (perms, path) in &mappings {
        assert!(
            perms.starts_with("r--"),
            "PROT_READ only, never writable: {perms} {path}"
        );
    }
    assert!(
        mappings.iter().any(|(_, p)| p.contains("-public")),
        "public partition mapped: {mappings:?}"
    );
    assert!(
        mappings.iter().any(|(_, p)| p.contains("-enl")),
        "own private partition mapped: {mappings:?}"
    );
    assert!(
        !mappings.iter().any(|(_, p)| p.contains("-res")),
        "the Session NEVER maps the enemy partition: {mappings:?}"
    );

    // The documented residual (D77): opening the enemy NAME is not
    // technically prevented for same-UID code in v0.1 — it is exactly
    // the audit boundary. Record it, then unmap immediately.
    let enemy = arena_client_sdk::shm::ShmReader::open(
        &res_name,
        arena_client_sdk::shm::RegionHeader::private(REGION_KIND_RES),
    );
    if let Ok(enemy) = &enemy {
        let _ = enemy
            .read()
            .expect("same-uid open can read v0.1 (documented)");
    }
    drop(enemy);
    let after = sdk_readonly_mappings(&match_id);
    assert!(
        !after.iter().any(|(_, p)| p.contains("-res")),
        "dropping the reader unmaps it again: {after:?}"
    );

    drop((session, public, enl, res));
    assert!(
        ingress_mappings(&match_id).is_empty(),
        "everything unmapped at teardown"
    );
}

/// Sessions never map the enemy partition even when created for BOTH
/// factions in one process (each maps exactly public + own).
#[test]
fn both_faction_sessions_map_their_own_partition_only() {
    let match_id = format!("iso2-{}", std::process::id());
    let (public_name, enl_name, res_name) = region_names(&match_id);
    let cfg = arena_core::config::GameConfig::default();
    let map = arena_core::map::MapDefinition::new(Vec::new());
    let mut public = ShmPublisher::create(
        &public_name,
        REGION_KIND_PUBLIC,
        PUBLIC_SNAPSHOT_VERSION,
        8 << 20,
    )
    .expect("public");
    let mut enl = ShmPublisher::create(
        &enl_name,
        REGION_KIND_ENL,
        arena_protocol::PRIVATE_SNAPSHOT_VERSION,
        2 << 20,
    )
    .expect("enl");
    let mut res = ShmPublisher::create(
        &res_name,
        REGION_KIND_RES,
        arena_protocol::PRIVATE_SNAPSHOT_VERSION,
        2 << 20,
    )
    .expect("res");
    public
        .publish(&build::map_generation_bytes(&map, 0, 0), 0, 0, 0)
        .expect("publish");
    enl.publish(&build::private_roster_bytes(&cfg, Faction::Enl), 0, 0, 0)
        .expect("publish");
    res.publish(&build::private_roster_bytes(&cfg, Faction::Res), 0, 0, 0)
        .expect("publish");

    let enl_session = Session::new(&match_id, Faction::Enl, cfg.clone()).expect("enl session");
    let res_session = Session::new(&match_id, Faction::Res, cfg.clone()).expect("res session");
    let mappings = sdk_readonly_mappings(&match_id);
    let enl_hits = mappings.iter().filter(|(_, p)| p.contains("-enl")).count();
    let res_hits = mappings.iter().filter(|(_, p)| p.contains("-res")).count();
    let public_hits = mappings
        .iter()
        .filter(|(_, p)| p.contains("-public"))
        .count();
    // Each private partition is mapped exactly once (by its owning
    // session); the public partition once per session (two readers).
    assert_eq!(enl_hits, 1, "{mappings:?}");
    assert_eq!(res_hits, 1, "{mappings:?}");
    assert_eq!(public_hits, 2, "{mappings:?}");
    for (perms, _) in &mappings {
        assert!(perms.starts_with("r--"), "{perms}");
    }
    drop((enl_session, res_session, public, enl, res));
}

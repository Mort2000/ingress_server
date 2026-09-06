//! probe-client: a test/diagnostic fixture binary (D69), NOT a
//! competitor. It speaks the raw protocol (arena-protocol + the SDK's
//! connection/shm wrappers) and misbehaves on demand via environment
//! variables so the arena-server integration tests can exercise transport
//! edge cases from real child processes:
//!
//! ```text
//! PROBE_MODE=valid|slow-rounds|self-kill|late-half-frame|bad-version|
//!            duplicate-connect|giant-name|no-spawn|bad-spawn|stall
//! PROBE_ARG=<n>        # e.g. the kill round, or which version to bump
//! ```
//!
//! `#![forbid(unsafe_code)]` per P4-CAPABILITY; all system boundary access
//! goes through the official SDK.

#![forbid(unsafe_code)]

use std::time::Duration;

use arena_client_sdk::conn::Connection;
use arena_client_sdk::shm::{RegionHeader, ShmReader};
use arena_core::types::{AgentId, Coord};
use arena_protocol::build::envelope_bytes;
use arena_protocol::decode::{Envelope, HELLO_OK, MatchInfo, SpawnEntry};
use arena_protocol::frame::{MAX_FRAME_PAYLOAD_BYTES, encode_frame};
use arena_protocol::{PRIVATE_SNAPSHOT_VERSION, PROTOCOL_VERSION, PUBLIC_SNAPSHOT_VERSION};

fn env_mode() -> String {
    std::env::var("PROBE_MODE").unwrap_or_else(|_| "valid".into())
}

fn env_arg() -> u64 {
    std::env::var("PROBE_ARG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

fn main() {
    let mode = env_mode();
    let mut args = std::env::args().skip(1);
    let socket = args
        .next()
        .unwrap_or_else(|| "/tmp/ingress-arena.sock".into());
    let arg = env_arg();
    let code = match probe(&mode, &socket, arg) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("probe-client ({mode}) failed: {error}");
            2
        }
    };
    std::process::exit(code);
}

/// Sends best-effort: once the match ends mid-behavior, writing fails
/// (the server closed) and the probe simply stops.
fn send(conn: &mut Connection, envelope: &Envelope) -> bool {
    conn.send(envelope).is_ok()
}

fn hello_envelope(name: &str, bump: u32) -> Envelope {
    let mut triple = [
        PROTOCOL_VERSION,
        PUBLIC_SNAPSHOT_VERSION,
        PRIVATE_SNAPSHOT_VERSION,
    ];
    if bump > 0 {
        triple[((bump - 1) % 3) as usize] += 1;
    }
    Envelope::ClientHello {
        client_name: name.into(),
        client_version: "probe".into(),
        protocol_version: triple[0],
        public_snapshot_version: triple[1],
        private_snapshot_version: triple[2],
    }
}

fn probe(mode: &str, socket: &str, arg: u64) -> Result<i32, Box<dyn std::error::Error>> {
    let bump = arg;
    let mut conn = Connection::connect(std::path::Path::new(socket))?;

    // --- Handshake-phase misbehaviors ------------------------------
    match mode {
        "stall" => {
            // Connect and never say anything: the handshake window must
            // expire into an unscored setup error.
            std::thread::sleep(Duration::from_secs(30));
            return Ok(0);
        }
        "bad-version" => {
            send(&mut conn, &hello_envelope("probe-bad-version", bump as u32));
            // Expect the rejection, then the server closes.
            let _ = conn.next_envelope(Duration::from_secs(10));
            return Ok(0);
        }
        "giant-name" => {
            // Binary-search a name length whose CLIENT_HELLO payload is
            // exactly MAX_FRAME_PAYLOAD_BYTES bytes.
            let mut lo = 1usize;
            let mut hi = MAX_FRAME_PAYLOAD_BYTES;
            while lo < hi {
                let mid = (lo + hi + 1) / 2;
                let payload = envelope_bytes(&hello_envelope(&"P".repeat(mid), 0));
                if payload.len() <= MAX_FRAME_PAYLOAD_BYTES {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            let payload = envelope_bytes(&hello_envelope(&"P".repeat(lo), 0));
            assert_eq!(
                payload.len(),
                MAX_FRAME_PAYLOAD_BYTES,
                "exact legal maximum"
            );
            let mut frame = Vec::new();
            encode_frame(&payload, &mut frame)?;
            conn.send_raw(&frame)?;
        }
        _ => {
            send(&mut conn, &hello_envelope("probe", 0));
        }
    }

    // Expect SERVER_HELLO.
    let (status, faction, match_id) = match conn.next_envelope(Duration::from_secs(10))? {
        Some(Envelope::ServerHello {
            status,
            assigned_faction,
            match_id,
        }) => (status, assigned_faction, match_id),
        Some(other) => return Err(format!("expected SERVER_HELLO, got {other:?}").into()),
        None => return Ok(0), // server closed (e.g. after a mismatch abort)
    };
    if status != HELLO_OK {
        // Version mismatch path: server aborts; we just observe.
        return Ok(0);
    }

    // Duplicate connection from an already-assigned client: open a second
    // stream, HELLO, expect status=2.
    if mode == "duplicate-connect" {
        let mut second = Connection::connect(std::path::Path::new(socket))?;
        send(&mut second, &hello_envelope("probe-duplicate", 0));
        match second.next_envelope(Duration::from_secs(5))? {
            Some(Envelope::ServerHello { status, .. }) => {
                assert_eq!(status, 2, "surplus connection rejected");
            }
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    // INITIAL_SPAWN_START.
    let info: MatchInfo = match conn.next_envelope(Duration::from_secs(10))? {
        Some(Envelope::InitialSpawnStart { info, .. }) => info,
        Some(other) => return Err(format!("expected INITIAL_SPAWN_START, got {other:?}").into()),
        None => return Ok(0),
    };
    let decision_ms = info.decision_window_ms as u64;
    let agents: u32 = info.initial_agents_per_faction;
    let roster: Vec<AgentId> = match faction {
        arena_core::types::Faction::Enl => (0..agents).map(AgentId).collect(),
        arena_core::types::Faction::Res => (agents..2 * agents).map(AgentId).collect(),
    };

    // --- Spawn-phase misbehaviors -----------------------------------
    match mode {
        "no-spawn" => {
            // Never submit; the server must SPAWN_FORFEIT us at the
            // deadline. Wait for the technical GAME_END.
            drain_until_game_end(&mut conn, Duration::from_secs(30));
            return Ok(0);
        }
        "bad-spawn" => {
            let positions = if bump == 2 {
                // Missing one roster agent.
                roster[..roster.len() - 1]
                    .iter()
                    .map(|&id| SpawnEntry {
                        agent_id: id,
                        position: Coord::new(1, 1),
                    })
                    .collect()
            } else {
                // Out-of-bounds coordinate.
                roster
                    .iter()
                    .map(|&id| SpawnEntry {
                        agent_id: id,
                        position: Coord::new(info.map_width_cm + 1, 0),
                    })
                    .collect()
            };
            send(&mut conn, &Envelope::SubmitInitialSpawns { positions });
            drain_until_game_end(&mut conn, Duration::from_secs(30));
            return Ok(0);
        }
        _ => {
            let positions: Vec<SpawnEntry> = roster
                .iter()
                .map(|&id| SpawnEntry {
                    agent_id: id,
                    position: Coord::new(1, 1),
                })
                .collect();
            send(&mut conn, &Envelope::SubmitInitialSpawns { positions });
            match conn.next_envelope(Duration::from_secs(10))? {
                Some(Envelope::InitialSpawnResult { status: 0, .. }) => {}
                Some(Envelope::InitialSpawnResult { status, detail }) => {
                    return Err(format!("spawn rejected: {status} {detail}").into());
                }
                Some(other) => return Err(format!("expected spawn result, got {other:?}").into()),
                None => return Ok(0),
            }
        }
    }

    // Open the SHM regions like a real client (also proves the regions are
    // readable cross-process with the SDK reader).
    let (public_name, enl_name, res_name) = arena_client_sdk::shm::region_names(&match_id);
    let private_name = match faction {
        arena_core::types::Faction::Enl => enl_name,
        arena_core::types::Faction::Res => res_name,
    };
    let public_reader = ShmReader::open(&public_name, RegionHeader::public())?;
    let private_kind = match faction {
        arena_core::types::Faction::Enl => arena_client_sdk::shm::REGION_KIND_ENL,
        arena_core::types::Faction::Res => arena_client_sdk::shm::REGION_KIND_RES,
    };
    let private_reader = ShmReader::open(&private_name, RegionHeader::private(private_kind))?;

    // --- Round loop ---------------------------------------------------
    let mut round: u32 = 0;
    loop {
        let envelope = match conn.next_envelope(Duration::from_secs(60)) {
            Ok(envelope) => envelope,
            Err(_) => return Ok(0), // server closed: match over
        };
        match envelope {
            Some(Envelope::RoundStartSignal) => {
                round += 1;
                // The SHM generation never lags the ROUND_START_SIGNAL;
                // in slow modes the server may already have advanced one
                // further round by the time we look.
                let public = public_reader.read()?;
                assert!(
                    public.current_round >= round,
                    "SHM round {} trails the signal for round {round}",
                    public.current_round
                );
                let private = private_reader.read()?;
                assert!(private.current_round >= round);

                if mode == "self-kill" && round as u64 == arg {
                    // Hard-kill the process: the server must see EOF and
                    // award CLIENT_FORFEIT against us.
                    eprintln!("probe-client: self-kill at round {round}");
                    std::process::exit(9);
                }
                if mode == "slow-rounds" {
                    // Sleep past the decision window; the server must
                    // settle without us.
                    std::thread::sleep(Duration::from_millis(decision_ms + 60));
                }
                if mode == "late-half-frame" && round == 1 {
                    // Split one SubmitCommandStream frame across the
                    // deadline: first half now, remainder after.
                    let payload = envelope_bytes(&Envelope::SubmitCommandStream {
                        round,
                        agent_id: roster[0],
                        commands: vec![],
                    });
                    let mut frame = Vec::new();
                    encode_frame(&payload, &mut frame)?;
                    let split = frame.len() - 3;
                    let tail = frame.split_off(split);
                    conn.send_raw(&frame)?;
                    std::thread::sleep(Duration::from_millis(decision_ms + 60));
                    conn.send_raw(&tail)?;
                    continue;
                }
                // Default participation: submit nothing, commit.
                if !send(&mut conn, &Envelope::CommitRound { round }) {
                    return Ok(0);
                }
            }
            Some(Envelope::RoundClosed { .. }) => {}
            Some(Envelope::SubmissionResult { .. }) => {}
            Some(end @ Envelope::GameEnd { .. }) => {
                println!("outcome={end:?}");
                return Ok(0);
            }
            Some(other) => return Err(format!("unexpected {other:?}").into()),
            None => return Ok(0),
        }
    }
}

fn drain_until_game_end(conn: &mut Connection, timeout: Duration) {
    let deadline = std::time::Instant::now() + timeout;
    while let Ok(Some(envelope)) =
        conn.next_envelope(deadline.saturating_duration_since(std::time::Instant::now()))
    {
        if matches!(envelope, Envelope::GameEnd { .. }) {
            return;
        }
    }
}

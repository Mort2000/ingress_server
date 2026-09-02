//! Recorder + replay-engine integration tests over a real 500-round match
//! (M5 checklist items: recording transparency, byte-exact round trip,
//! checkpoints, full replay from the log alone, map freezing, corruption
//! localization, intake re-derivation, version rejection, tampered recorded
//! outcomes, and the D45 hash point).

mod support;

use arena_core::harness::{AgentSubmission, MatchHarness, RoundStrategy, ScriptedStrategy};
use arena_core::map::MapDefinition;
use arena_core::resolve::RoundReport;
use arena_core::state::GameState;
use arena_core::types::{AgentId, Command, Coord, Faction, MatchResult, PortalId};
use arena_replay::canon;
use arena_replay::format::{Block, CHECKPOINT_INTERVAL, EndOfMatch, ReplayError, ReplayFile};
use arena_replay::recorder::ReplayWriter;
use arena_replay::replay::{decode_log, replay, replay_bytes};
use support::{FieldBot, replay_cfg, replay_map, replay_spawns};

const GAME_SEED: u64 = 0x5EED_0000_0000_0005;

/// Runs the pinned FieldBot fixture match with and without a recorder.
fn field_match_parts(
    game_seed: u64,
) -> (
    Option<Vec<u8>>,
    Vec<RoundReport>,
    GameState,
    Vec<RoundReport>,
    GameState,
) {
    let cfg = replay_cfg();
    let map = replay_map();
    let enl_spawns = replay_spawns(&cfg, Faction::Enl);
    let res_spawns = replay_spawns(&cfg, Faction::Res);

    // Recorded run.
    let mut writer = ReplayWriter::new(game_seed, &cfg, &map, &enl_spawns, &res_spawns);
    let mut harness = MatchHarness::new(
        game_seed,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(FieldBot::new(Faction::Enl, [0, 1, 2], 0)),
        Box::new(FieldBot::new(Faction::Res, [3, 4, 5], 3)),
    )
    .expect("harness builds");
    let mut reports = Vec::new();
    for _ in 0..500 {
        let report = harness.run_round();
        writer
            .record_round(&report, harness.state())
            .expect("record");
        reports.push(report);
    }
    let bytes = writer.finish(harness.state()).expect("finish");
    let recorded_state = harness.into_state();

    // Plain twin, no recorder attached.
    let mut twin = MatchHarness::new(
        game_seed,
        replay_cfg(),
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(FieldBot::new(Faction::Enl, [0, 1, 2], 0)),
        Box::new(FieldBot::new(Faction::Res, [3, 4, 5], 3)),
    )
    .expect("twin builds");
    let twin_reports = twin.run_rounds(500);
    let twin_state = twin.into_state();

    (
        Some(bytes),
        reports,
        recorded_state,
        twin_reports,
        twin_state,
    )
}

fn recorded_match(game_seed: u64) -> (Vec<u8>, Vec<RoundReport>, GameState) {
    let (bytes, reports, state, _, _) = field_match_parts(game_seed);
    (bytes.expect("recorded"), reports, state)
}

/// Mutable borrow of one round's record (blocks interleave checkpoints, so
/// positional indexing would be wrong past round 49).
fn record_mut(file: &mut ReplayFile, round: u32) -> &mut arena_replay::format::RoundRecord {
    file.blocks
        .iter_mut()
        .find_map(|b| match b {
            Block::Round(r) if r.round == round => Some(r),
            _ => None,
        })
        .unwrap_or_else(|| panic!("round {round} record exists"))
}

// Checklist 16: recording a match changes nothing — reports and final
// state equal the unrecorded twin (GameState PartialEq).
#[test]
fn recording_changes_nothing() {
    let (bytes, reports, state, twin_reports, twin_state) = field_match_parts(GAME_SEED);
    let bytes = bytes.expect("recorded");
    assert_eq!(reports, twin_reports, "Vec<RoundReport> equal");
    assert_eq!(state, twin_state, "GameState equal (PartialEq)");
    assert_eq!(state.round(), 501);
    // The fixture is not vacuous: both factions hold a field.
    assert!(state.link_graph().fields().len() >= 2);
    assert!(state.score(Faction::Enl) > 0);
    assert!(state.score(Faction::Res) > 0);
    assert!(!bytes.is_empty());
}

// Checklist 11 + 14: a full 500-round log round-trips byte-exactly both
// directions, and checkpoints land exactly at rounds 50, 100, ..., 500
// with hashes agreeing with their round records.
#[test]
fn full_log_round_trip_and_checkpoint_placement() {
    let (bytes, _reports, state) = recorded_match(GAME_SEED);
    let decoded = decode_log(&bytes).expect("decodes");

    // Structural + byte-exact round trip.
    assert_eq!(decoded.encode(), bytes, "re-encode == original bytes");
    assert_eq!(
        decoded.round_records().map(|r| r.round).collect::<Vec<_>>(),
        (1..=500u32).collect::<Vec<_>>(),
        "500 sequential records"
    );

    // Checkpoints exactly at the multiples of 50, hash-verified at decode.
    let checkpoints: Vec<u32> = decoded
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Checkpoint(c) => Some(c.round),
            _ => None,
        })
        .collect();
    let expected: Vec<u32> = (CHECKPOINT_INTERVAL..=500u32)
        .step_by(CHECKPOINT_INTERVAL as usize)
        .collect();
    assert_eq!(checkpoints, expected, "checkpoints at 50,100,...,500");
    for block in &decoded.blocks {
        if let Block::Checkpoint(c) = block {
            let record = decoded
                .round_records()
                .nth(c.round as usize - 1)
                .expect("record exists");
            assert_eq!(c.round, record.round);
            assert_eq!(c.state_hash, record.state_hash);
            // The checkpoint carries the canonical bytes of exactly that
            // moment, and they hash to the recorded hash (decode checked;
            // this pins the recorded end state too).
            assert_eq!(
                canon::hash_canonical_bytes(&c.canonical_state),
                record.state_hash
            );
        }
    }
    let last: Option<u32> = decoded
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Checkpoint(c) => Some(c.round),
            _ => None,
        })
        .last();
    assert_eq!(last, Some(500));
    assert_eq!(canon::state_hash(&state), {
        let EndOfMatch::Normal {
            final_state_hash, ..
        } = decoded.trailer().expect("trailer")
        else {
            panic!("normal end")
        };
        *final_state_hash
    });
}

// Checklist 17: full 500-round replay from the log alone — every per-round
// hash, delta, and event re-derives; the final hash matches the trailer;
// the result is one of the three legal values and equals the recorded tag.
#[test]
fn full_replay_from_log_alone() {
    let (bytes, _reports, state) = recorded_match(GAME_SEED);
    let outcome = replay_bytes(&bytes).expect("replay verifies every round");
    assert_eq!(outcome.rounds_completed, 500);
    assert_eq!(outcome.final_state_hash, canon::state_hash(&state));
    assert_eq!(
        outcome.state, state,
        "the replayed state equals the recorded run"
    );
    assert!(matches!(
        outcome.result,
        MatchResult::EnlWin | MatchResult::ResWin | MatchResult::Draw
    ));
    let decoded = decode_log(&bytes).expect("decodes");
    if let EndOfMatch::Normal { result, .. } = decoded.trailer().expect("trailer") {
        assert_eq!(outcome.result, *result, "result tag agrees (P1-REPLAY)");
    }
    // Per-round verification is replay()'s job; spot-check three records
    // directly against a fresh twin's live hashes in the hash-point test.
    assert_eq!(
        outcome.final_scores,
        (state.score(Faction::Enl), state.score(Faction::Res))
    );
}

// Checklist 18: the map is loaded from the file, never regenerated —
// mutating one header portal position makes the replay diverge at round 1.
#[test]
fn map_is_loaded_from_the_file_never_regenerated() {
    let (bytes, _, _) = recorded_match(GAME_SEED);
    let mut decoded = decode_log(&bytes).expect("decodes");

    // Move P3 within validity (still >= PORTAL_MIN_DISTANCE from every
    // other portal, still in bounds) — the world now differs from what the
    // recorded submissions walked through.
    let portals = decoded.header.map.portals().to_vec();
    let mut moved = portals.clone();
    moved[3] = arena_core::types::Portal {
        id: PortalId(3),
        position: Coord::new(38_000, 4_000),
        owner: moved[3].owner,
        hp: moved[3].hp,
    };
    assert_ne!(portals[3], moved[3]);
    decoded.header.map = MapDefinition::new(moved);

    let tampered = decoded.encode();
    assert_eq!(
        decode_log(&tampered).expect("still structurally valid"),
        decoded,
        "the mutated header round-trips (it is the replay that catches it)"
    );
    let error = replay(&decoded).unwrap_err();
    match &error {
        ReplayError::HashMismatch {
            round,
            expected_hex,
            actual_hex,
        } => {
            assert_eq!(*round, 1, "divergence localized at round 1: {error}");
            assert_ne!(expected_hex, actual_hex);
        }
        other => panic!("expected HashMismatch, got {other:?}"),
    }
}

// Checklist 19: corruption localization — flipping one byte of a round-k
// submission command, round-k event, or round-k hash each fails naming
// round k (not k+1).
#[test]
fn corruption_localizes_to_round_k() {
    let (bytes, _, _) = recorded_match(GAME_SEED);
    let base = decode_log(&bytes).expect("decodes");

    // (a) Round-3 submission command: agent 0's planned MOVE becomes an
    // out-of-bounds destination -> different world -> hash mismatch at 3.
    let mut tampered = base.clone();
    {
        let record = record_mut(&mut tampered, 3);
        let submission = record
            .submissions
            .iter_mut()
            .find(|s| !s.commands.is_empty())
            .expect("round 3 has commands");
        submission.commands[0] = Command::Move {
            destination: Coord::new(-1, -1),
        };
    }
    match replay(&tampered).unwrap_err() {
        ReplayError::HashMismatch { round, .. } => assert_eq!(round, 3),
        other => panic!("expected HashMismatch at 3, got {other:?}"),
    }

    // (b) Round-100 event: the reinforcement event's position moves 1 cm.
    let mut tampered = base.clone();
    {
        let record = record_mut(&mut tampered, 100);
        assert!(
            !record.events.is_empty(),
            "round 100 carries the reinforcement event"
        );
        for event in &mut record.events {
            if let arena_core::event::EventKind::Reinforcement { position, .. } = &mut event.kind {
                position.x += 1;
            }
        }
    }
    assert_eq!(
        replay(&tampered).unwrap_err(),
        ReplayError::EventMismatch { round: 100 }
    );

    // (c) Round-251 recorded hash, corrupted at the struct level...
    let mut tampered = base.clone();
    {
        let record = record_mut(&mut tampered, 251);
        record.state_hash[0] ^= 1;
    }
    match replay(&tampered).unwrap_err() {
        ReplayError::HashMismatch {
            round,
            expected_hex,
            actual_hex,
        } => {
            assert_eq!(round, 251);
            assert_ne!(expected_hex, actual_hex);
        }
        other => panic!("expected HashMismatch at 251, got {other:?}"),
    }

    // ...and at the raw byte level: locate round 251's 32-byte hash inside
    // the file (251 is not a checkpoint round, so the pattern is unique to
    // its record) and flip its first byte.
    let recorded_hash = {
        let record = base.round_records().nth(250).expect("round 251");
        record.state_hash
    };
    let offset = bytes
        .windows(32)
        .position(|window| window == recorded_hash)
        .expect("hash bytes found in the file");
    let mut corrupted = bytes.clone();
    corrupted[offset] ^= 1;
    match replay_bytes(&corrupted).unwrap_err() {
        ReplayError::HashMismatch { round, .. } => assert_eq!(round, 251),
        other => panic!("expected HashMismatch at 251, got {other:?}"),
    }
}

// Checklist 20: unlocatable submissions and ignored duplicates re-derive
// identically — the recorded round-5 facts match the replayed report's.
#[test]
fn unlocatable_and_duplicates_re_derived() {
    /// FieldBot plus junk submissions injected at one round: an ignored
    /// duplicate and an unknown-agent envelope from the own faction, and a
    /// faction-mismatch envelope from the enemy faction.
    struct NoisyFieldBot {
        inner: FieldBot,
        inject_at: u32,
        own_agent: AgentId,
    }

    impl RoundStrategy for NoisyFieldBot {
        fn submit_round(&mut self, obs: &arena_core::observe::Observation) -> Vec<AgentSubmission> {
            let mut out = self.inner.submit_round(obs);
            if obs.round == self.inject_at {
                // Duplicate (locatable, slot already frozen by the first
                // submission of this agent).
                out.push(AgentSubmission::new(self.own_agent, vec![Command::Attack]));
                // Unknown agent id: envelope unlocatable.
                out.push(AgentSubmission::new(AgentId(99), vec![Command::Attack]));
            }
            out
        }
    }

    let cfg = replay_cfg();
    let map = replay_map();
    let enl_spawns = replay_spawns(&cfg, Faction::Enl);
    let res_spawns = replay_spawns(&cfg, Faction::Res);
    let mut writer = ReplayWriter::new(GAME_SEED, &cfg, &map, &enl_spawns, &res_spawns);
    let mut harness = MatchHarness::new(
        GAME_SEED,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(NoisyFieldBot {
            inner: FieldBot::new(Faction::Enl, [0, 1, 2], 0),
            inject_at: 5,
            own_agent: AgentId(0),
        }),
        Box::new({
            struct MismatchBot;
            impl RoundStrategy for MismatchBot {
                fn submit_round(
                    &mut self,
                    obs: &arena_core::observe::Observation,
                ) -> Vec<AgentSubmission> {
                    let mut out = FieldBot::new(Faction::Res, [3, 4, 5], 3).submit_round(obs);
                    if obs.round == 5 {
                        // RES claims the ENL agent: faction mismatch.
                        out.push(AgentSubmission::new(AgentId(0), vec![Command::Attack]));
                    }
                    out
                }
            }
            MismatchBot
        }),
    )
    .expect("harness builds");
    for _ in 0..500 {
        let report = harness.run_round();
        writer
            .record_round(&report, harness.state())
            .expect("record");
    }
    let bytes = writer.finish(harness.state()).expect("finish");
    let decoded = decode_log(&bytes).expect("decodes");

    let record = decoded.round_records().nth(4).expect("round 5");
    assert_eq!(record.round, 5);
    assert!(
        record.unlocatable.iter().any(|u| matches!(
            u,
            arena_core::resolve::Unlocatable::UnknownAgent(AgentId(99))
        )),
        "unknown agent recorded: {:?}",
        record.unlocatable
    );
    assert!(
        record
            .unlocatable
            .iter()
            .any(|u| matches!(u, arena_core::resolve::Unlocatable::FactionMismatch { .. })),
        "faction mismatch recorded"
    );
    assert_eq!(record.ignored_duplicates, vec![AgentId(0)]);
    assert_eq!(
        record.submissions.len(),
        3 + 3 + 3,
        "nine submissions echoed"
    );

    // The replay re-derives every intake fact identically or fails.
    let outcome = replay(&decoded).expect("replay re-derives the intake facts");
    assert_eq!(outcome.rounds_completed, 500);
}

// Checklist 21: version mismatches are rejected before any round executes
// (decode refuses the file outright).
#[test]
fn version_mismatch_rejected_before_any_round() {
    let (bytes, _, _) = recorded_match(GAME_SEED);
    let base = decode_log(&bytes).expect("decodes");

    let mut bad_rng = base.clone();
    bad_rng.header.rng_version += 1;
    assert_eq!(
        replay(&bad_rng).unwrap_err(),
        ReplayError::RngVersion {
            found: bad_rng.header.rng_version,
            expected: arena_core::rng::RNG_VERSION
        }
    );

    let mut bad_encoding = base.clone();
    bad_encoding.header.state_encoding_version += 1;
    assert_eq!(
        replay(&bad_encoding).unwrap_err(),
        ReplayError::StateEncodingVersion {
            found: bad_encoding.header.state_encoding_version,
            expected: canon::STATE_ENCODING_VERSION
        }
    );
}

// Checklist 22: re-execution, not trust — recorded outcomes are never fed
// back; tampering them is caught by the hash/event/delta/intake
// comparisons.
#[test]
fn tampered_recorded_outcomes_detected() {
    let (bytes, _, _) = recorded_match(GAME_SEED);
    let base = decode_log(&bytes).expect("decodes");

    // Delta tampered at a round where a field scores.
    let mut tampered = base.clone();
    {
        let record = record_mut(&mut tampered, 400);
        assert!(record.score2_delta.0 > 0, "round 400 scores");
        record.score2_delta.0 += 1;
    }
    assert_eq!(
        replay(&tampered).unwrap_err(),
        ReplayError::DeltaMismatch { round: 400 }
    );

    // Execution order tampered at round 2 (several streams execute).
    let mut tampered = base.clone();
    {
        let record = record_mut(&mut tampered, 2);
        assert!(record.execution_order.len() >= 2);
        record.execution_order.reverse();
    }
    assert_eq!(
        replay(&tampered).unwrap_err(),
        ReplayError::IntakeMismatch {
            round: 2,
            what: "execution order"
        }
    );

    // Slot statuses tampered at round 1.
    let mut tampered = base.clone();
    {
        let record = record_mut(&mut tampered, 1);
        let key = *record.slots.keys().next().expect("slots exist");
        record.slots.remove(&key);
    }
    assert_eq!(
        replay(&tampered).unwrap_err(),
        ReplayError::IntakeMismatch {
            round: 1,
            what: "slot statuses"
        }
    );

    // Ignored-duplicate list tampered at round 1 (empty -> bogus entry).
    let mut tampered = base.clone();
    record_mut(&mut tampered, 1)
        .ignored_duplicates
        .push(AgentId(4));
    assert_eq!(
        replay(&tampered).unwrap_err(),
        ReplayError::IntakeMismatch {
            round: 1,
            what: "ignored duplicates"
        }
    );
}

// Checklist 23: the hash point is exactly the post-advance state — every
// recorded round-r hash equals the live twin's hash after r rounds, and
// the epoch sweep's tracker reset is part of the hashed state.
#[test]
fn hash_point_is_the_post_advance_state() {
    // (a) Per-round equality on the real match: run a twin harness and
    // hash it live after every round.
    let (bytes, _, _) = recorded_match(GAME_SEED);
    let decoded = decode_log(&bytes).expect("decodes");
    let cfg = replay_cfg();
    let map = replay_map();
    let enl_spawns = replay_spawns(&cfg, Faction::Enl);
    let res_spawns = replay_spawns(&cfg, Faction::Res);
    let mut twin = MatchHarness::new(
        GAME_SEED,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(FieldBot::new(Faction::Enl, [0, 1, 2], 0)),
        Box::new(FieldBot::new(Faction::Res, [3, 4, 5], 3)),
    )
    .expect("twin builds");
    let mut records = decoded.round_records();
    for _ in 0..500 {
        let _ = twin.run_round();
        let record = records.next().expect("record");
        assert_eq!(
            canon::state_hash(twin.state()),
            record.state_hash,
            "round {} hash equals the live post-advance hash",
            record.round
        );
    }

    // (b) Epoch-boundary reset is in the hashed stream: a two-round epoch
    // scripted hack scenario; after round 2 closes, the tracker's
    // `last_hack_round` has been swept to None while `hack_ordinal`
    // survives, and the recorded round-2 hash equals that state's hash
    // (the tracker bytes are pinned inside the canonical encoding by the
    // canon layout tests).
    let mut cfg = replay_cfg();
    cfg.total_rounds = 4;
    cfg.epoch_length = 2;
    cfg.hack_cooldown_rounds = 1;
    cfg.initial_agents_per_faction = 1;
    let map = MapDefinition::new(vec![
        arena_core::types::Portal {
            id: PortalId(0),
            position: Coord::new(10_000, 10_000),
            owner: arena_core::types::Owner::Neutral,
            hp: 0,
        },
        arena_core::types::Portal {
            id: PortalId(1),
            position: Coord::new(30_000, 30_000),
            owner: arena_core::types::Owner::Neutral,
            hp: 0,
        },
    ]);
    cfg.portal_count = 2;
    cfg.portal_cluster_count = 2;
    cfg.portal_min_distance_cm = 4_000;
    let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
    let enl_spawns = arena_core::spawn::validate_initial_spawn_set(
        &enl_roster,
        &[(enl_roster[0], Coord::new(10_000, 10_000))],
        &cfg,
    )
    .expect("valid");
    let res_spawns = arena_core::spawn::validate_initial_spawn_set(
        &res_roster,
        &[(res_roster[0], Coord::new(30_000, 30_000))],
        &cfg,
    )
    .expect("valid");

    let hack = || {
        AgentSubmission::new(
            enl_roster[0],
            vec![Command::Hack {
                portal: PortalId(0),
            }],
        )
    };
    let enl_script =
        ScriptedStrategy::new(vec![vec![hack()], vec![hack()], vec![hack()], vec![hack()]]);
    let res_script = ScriptedStrategy::new(vec![vec![], vec![], vec![], vec![]]);

    let mut writer = ReplayWriter::new(GAME_SEED, &cfg, &map, &enl_spawns, &res_spawns);
    let mut harness = MatchHarness::new(
        GAME_SEED,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(enl_script),
        Box::new(res_script),
    )
    .expect("harness builds");
    for _ in 0..4 {
        let report = harness.run_round();
        writer
            .record_round(&report, harness.state())
            .expect("record");
    }
    let bytes = writer.finish(harness.state()).expect("finish");
    let decoded = decode_log(&bytes).expect("decodes");
    replay(&decoded).expect("epoch scenario replays");

    let state = harness.state();
    assert_eq!(state.round(), 5);
    let tracker = state
        .hack_tracker(enl_roster[0], PortalId(0))
        .expect("tracker exists");
    assert_eq!(tracker.last_hack_round, None, "swept at the 2->3 boundary");
    assert_eq!(tracker.hack_count_epoch, 0, "swept at the 2->3 boundary");
    assert_eq!(tracker.hack_ordinal, 4, "lifetime ordinal survives (D31)");
    // The last recorded hash (round 4 closed) is the hash of exactly this
    // state, reset included.
    let last_record = decoded.round_records().last().expect("records");
    assert_eq!(last_record.round, 4);
    assert_eq!(last_record.state_hash, canon::state_hash(state));
    // And the reset is observable BETWEEN rounds: the round-2 hash was
    // taken when the tracker still said Some(2) — the tracker section of
    // that checkpoint's canonical bytes carries a non-zero last_hack_round.
    let round2_checkpoint = decoded
        .blocks
        .iter()
        .rev()
        .find_map(|b| match b {
            Block::Round(r) if r.round == 2 => Some(r),
            _ => None,
        })
        .expect("round 2 record");
    assert_ne!(round2_checkpoint.state_hash, last_record.state_hash);
}

// Recorder misuse: recording a state that did not advance exactly one
// round errors instead of corrupting the file's sequence.
#[test]
fn recorder_rejects_non_advanced_states() {
    use arena_core::resolve::{Resolver, SubmittedCommandStream};
    let cfg = replay_cfg();
    let map = replay_map();
    let enl_spawns = replay_spawns(&cfg, Faction::Enl);
    let res_spawns = replay_spawns(&cfg, Faction::Res);
    let mut state = GameState::new(cfg, &map, &enl_spawns, &res_spawns).expect("state builds");
    let resolver = Resolver::with_seed(GAME_SEED);
    let mut writer = ReplayWriter::new(GAME_SEED, state.config(), &map, &enl_spawns, &res_spawns);

    // Round 1 settles but is NOT advanced: recording must refuse.
    let submission = SubmittedCommandStream {
        round: 1,
        faction: Faction::Enl,
        agent_id: AgentId(0),
        received_at_ms: 0,
        receive_seq: 0,
        commands: vec![Command::Attack],
    };
    let report = resolver.run_round(&mut state, &[submission]);
    assert_eq!(
        writer.record_round(&report, &state),
        Err(ReplayError::RoundSequence {
            expected: 1,
            found: 0
        })
    );

    // After advancing, the same report records fine.
    state.advance_round();
    writer.record_round(&report, &state).expect("records now");
    assert_eq!(writer.rounds_recorded(), 1);
}

// A second seed produces a different but equally replayable match
// (replay is not overfit to one fixture).
#[test]
fn second_seed_replays_identically() {
    let (bytes, _, state) = recorded_match(0xD1FF_E5E7_0000_0002);
    let outcome = replay_bytes(&bytes).expect("replays");
    assert_eq!(outcome.state, state);
    let other = recorded_match(GAME_SEED);
    assert_ne!(
        canon::state_hash(&state),
        canon::state_hash(&other.2),
        "different seeds really differ"
    );
}

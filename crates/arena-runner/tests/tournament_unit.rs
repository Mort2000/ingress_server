//! M9 tournament unit tests (P5-SCHEDULING / P5-RANKING): entrant
//! registry + provenance echo, config identity, schedule determinism and
//! faction balance, Benchmark Seed Set separation, outcome normalization
//! mapping table, and the integer Elo golden vectors. No processes, no
//! I/O outside `std::env::temp_dir`.

use arena_core::config::GameConfig;
use arena_core::types::{Faction, MatchResult};
use arena_runner::tournament::config::{ClientSandbox, EloConfig, TMatchup, TournamentConfig};
use arena_runner::tournament::elo::{EloTable, expected_score_q, round_half_away};
use arena_runner::tournament::entrant::{
    Entrant, EntrantKind, EntrantProvenance, compute_binary_hash, validate_entrants,
};
use arena_runner::tournament::outcome::{Stage, TournamentOutcome, from_server, winner_entrant};
use arena_runner::tournament::schedule::{
    MAP_SEED_LABEL, TournamentItem, build_items, map_codec_hash, map_pool,
};
use arena_runner::tournament::{CLIENT_SEED_LABEL, GAME_SEED_LABEL, SCHEDULE_SEED_LABEL};
use arena_server::MatchOutcome;

fn entrant(name: &str, kind: EntrantKind) -> Entrant {
    Entrant {
        name: name.to_string(),
        kind,
        provenance: EntrantProvenance::default(),
    }
}

fn fixture_config() -> TournamentConfig {
    TournamentConfig {
        name: "unit".into(),
        master_seed: 0x006D_3600_0000_0093,
        entrants: vec![
            entrant(
                "client_a",
                EntrantKind::External {
                    binary: std::env::current_exe().unwrap(),
                    env: vec![],
                },
            ),
            entrant(
                "client_b",
                EntrantKind::External {
                    binary: std::env::current_exe().unwrap(),
                    env: vec![],
                },
            ),
            entrant(
                "client_c",
                EntrantKind::External {
                    binary: std::env::current_exe().unwrap(),
                    env: vec![],
                },
            ),
        ],
        matchups: vec![
            TMatchup::new("client_a-client_b", 0, 1, 6),
            TMatchup::new("client_b-client_c", 1, 2, 4),
        ],
        map_count: 4,
        rounds_per_match: 8,
        config: {
            let mut cfg = GameConfig::default();
            cfg.total_rounds = 8;
            cfg
        },
        elo: EloConfig::default(),
        concurrency: 2,
        sandbox: ClientSandbox::Off,
        per_match_budget_ms: 5_000,
    }
}

// ---------------------------------------------------------------------------
// 1/2. Schedule determinism + faction balance (P5-SCHEDULING, D79/D80).
// ---------------------------------------------------------------------------

#[test]
fn schedule_is_deterministic_and_seed_sensitive() {
    let cfg = fixture_config();
    let items = build_items(&cfg).expect("items");
    let again = build_items(&cfg).expect("items");
    assert_eq!(items, again, "identical item list for the same config");
    assert_eq!(items.len() as u64, cfg.total_matches());
    assert_eq!(cfg.total_matches(), 10);

    // A different master seed changes the faction assignment (the whole
    // schedule derives from it).
    let mut other = fixture_config();
    other.master_seed ^= 1;
    let other_items = build_items(&other).expect("items");
    let assignments: Vec<bool> = items.iter().map(|i| i.a_is_enl).collect();
    let other_assignments: Vec<bool> = other_items.iter().map(|i| i.a_is_enl).collect();
    assert_ne!(assignments, other_assignments, "seed changes assignment");

    // Shape: one match per item, contiguous global indices, exactly one
    // item per (tag, j) — no auto-swap rematch anywhere in the list.
    for (position, item) in items.iter().enumerate() {
        assert_eq!(item.index, position as u64);
    }
    let mut seen: Vec<(String, u32)> = items.iter().map(|i| (i.tag.clone(), i.j)).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), items.len(), "one match per scheduling item");
}

#[test]
fn schedule_balances_factions_exactly_per_matchup() {
    let cfg = fixture_config();
    let items = build_items(&cfg).expect("items");
    for matchup in &cfg.matchups {
        let mine: Vec<&TournamentItem> = items.iter().filter(|i| i.tag == matchup.tag).collect();
        assert_eq!(mine.len() as u32, matchup.count);
        let a_enl = mine.iter().filter(|i| i.a_is_enl).count() as u32;
        // D55 reuse: EXACTLY count/2 per side — |ENL-RES| = 0 <= 1.
        assert_eq!(a_enl, matchup.count / 2, "entrant A ENL exposure");
        assert_eq!(matchup.count - a_enl, matchup.count / 2, "RES exposure");
        // Map index cycles the Benchmark Seed Set.
        for (j, item) in mine.iter().enumerate() {
            assert_eq!(item.j, j as u32);
            assert_eq!(item.map_index, item.j % cfg.map_count);
        }
    }
}

#[test]
fn benchmark_seed_set_is_disjoint_from_the_m6_tuning_pool() {
    let cfg = fixture_config();
    // The M9 Benchmark Seed Set uses a NEW domain (D80): formal scoring
    // samples are separated from the M6 tuning pool. Different labels (and
    // a master-bound payload) must give different values for equal k.
    assert_eq!(MAP_SEED_LABEL, b"ingress-arena-m9-benchmap");
    assert_ne!(MAP_SEED_LABEL, b"ingress-arena-m6-map");
    for k in 0u32..8 {
        assert_ne!(
            cfg.map_seed(k),
            arena_runner::derive_seed(b"ingress-arena-m6-map", &k.to_le_bytes())
        );
    }
    // Master binding: the same k under a different master changes the map.
    let mut other = fixture_config();
    other.master_seed ^= 0xABCD;
    assert_ne!(cfg.map_seed(0), other.map_seed(0));

    // build_items records the map hash of the actual generated definition
    // (D46 map codec under the m9-mapdef domain).
    let items = build_items(&cfg).expect("items");
    let pool = map_pool(&cfg).expect("pool");
    for item in &items {
        let map = &pool[item.map_index as usize];
        assert_eq!(item.map_hash, map_codec_hash(map));
        assert_eq!(item.map_seed, cfg.map_seed(item.map_index));
    }
}

// ---------------------------------------------------------------------------
// 4/5. Config identity extraction + entrant registry / provenance echo.
// ---------------------------------------------------------------------------

#[test]
fn alpha_config_has_new_identity_and_legacy_encoding_is_unchanged() {
    let cfg = GameConfig::default();
    let mut legacy = cfg;
    legacy.total_rounds = 500;
    legacy.max_reinforcement_agents_per_faction = u32::MAX;
    legacy.recharge_amount = 15;
    legacy.max_link_distance_cm = 200_000;
    assert_eq!(
        arena_runner::tournament::config::config_identity(&legacy),
        0x9ead_0522_19fa_7ff3
    );
    assert_ne!(
        arena_runner::tournament::config::config_identity(&cfg),
        0x9ead_0522_19fa_7ff3
    );
    let tournament = fixture_config();
    assert_eq!(
        tournament.config_identity(),
        arena_runner::tournament::config::config_identity(&tournament.config),
        "same GameConfig -> same identity through the tournament config"
    );
}

#[test]
fn entrant_registry_rejects_duplicates_and_hashes_binaries() {
    let mut entrants = vec![
        entrant(
            "a",
            EntrantKind::External {
                binary: std::env::current_exe().unwrap(),
                env: vec![],
            },
        ),
        entrant(
            "a",
            EntrantKind::External {
                binary: std::env::current_exe().unwrap(),
                env: vec![],
            },
        ),
    ];
    assert!(validate_entrants(&entrants).is_err(), "duplicate names");
    entrants[1].name = "b".into();
    assert!(validate_entrants(&entrants).is_ok());

    // The runner computes the binary hash itself over the file bytes under
    // the frozen m9 domain; an independent blake3 of the same bytes agrees.
    let dir = std::env::temp_dir().join(format!("ingress-m9-unit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let bin = dir.join("fake-client");
    std::fs::write(&bin, b"tournament unit test payload").expect("write");
    let got = compute_binary_hash(&bin).expect("hash");
    let mut expect_hasher = blake3::Hasher::new();
    expect_hasher.update(b"ingress-arena-m9-binary");
    expect_hasher.update(b"tournament unit test payload");
    assert_eq!(got, expect_hasher.finalize().to_hex().to_string());

    let mut external = entrant(
        "probe",
        EntrantKind::External {
            binary: bin.clone(),
            env: vec![("PROBE_MODE".to_string(), "valid".to_string())],
        },
    );
    assert!(external.provenance.binary_blake3.is_none());
    external.compute_binary_hash().expect("fill");
    assert_eq!(
        external.provenance.binary_blake3.as_deref(),
        Some(got.as_str())
    );

    // Recorded provenance literals are data: they round-trip verbatim.
    external.provenance = EntrantProvenance::recorded(
        Some("22710a8b".to_string()),
        Some("e721b841".to_string()),
        Some("fb9cd866".to_string()),
        Some("runs/builds/20260903-0038-22710a8be811".to_string()),
    );
    let EntrantProvenance {
        tree_hash,
        lock_hash,
        toolchain_hash,
        manifest_id,
        ..
    } = &external.provenance;
    assert_eq!(tree_hash.as_deref(), Some("22710a8b"));
    assert_eq!(lock_hash.as_deref(), Some("e721b841"));
    assert_eq!(toolchain_hash.as_deref(), Some("fb9cd866"));
    assert_eq!(
        manifest_id.as_deref(),
        Some("runs/builds/20260903-0038-22710a8be811")
    );
    let _ = std::fs::remove_file(&bin);
}

#[test]
fn tournament_config_validation_rejects_defects() {
    let mut cfg = fixture_config();
    assert!(cfg.validate().is_ok());
    cfg.matchups[0].count = 7;
    assert!(cfg.validate().is_err(), "odd counts break D55 balance");
    let mut cfg = fixture_config();
    cfg.matchups[0].b = 9;
    assert!(cfg.validate().is_err(), "entrant id out of range");
    let mut cfg = fixture_config();
    cfg.config.total_rounds = 5;
    assert!(
        cfg.validate().is_err(),
        "config.total_rounds must equal rounds_per_match"
    );
    let mut cfg = fixture_config();
    cfg.entrants[1].name = cfg.entrants[0].name.clone();
    assert!(cfg.validate().is_err(), "names are unique");
}

// ---------------------------------------------------------------------------
// 6. Outcome normalization mapping table (CONTEXT Match Outcome: a
// technical termination never masquerades as a score result).
// ---------------------------------------------------------------------------

fn faction_entrant(a: u32, b: u32, a_is_enl: bool, faction: Faction) -> u32 {
    if (faction == Faction::Enl) == a_is_enl {
        a
    } else {
        b
    }
}

#[test]
fn outcome_mapping_table_covers_every_server_outcome() {
    let (a, b) = (3u32, 7u32);
    let normal = MatchOutcome::Normal {
        result: MatchResult::EnlWin,
        score_enl: 10,
        score_res: 4,
        rounds_completed: 8,
    };
    for a_is_enl in [true, false] {
        let mapped = from_server(&normal, a, b, a_is_enl);
        match &mapped {
            TournamentOutcome::Normal {
                result,
                score_enl,
                score_res,
                rounds_completed,
                ..
            } => {
                assert_eq!(*result, MatchResult::EnlWin);
                assert_eq!(*score_enl, 10);
                assert_eq!(*score_res, 4);
                assert_eq!(*rounds_completed, 8);
            }
            other => panic!("Normal stays Normal, got {other:?}"),
        }
        assert!(mapped.elo_relevant());
        assert_eq!(
            winner_entrant(MatchResult::EnlWin, a, b, a_is_enl),
            Some(faction_entrant(a, b, a_is_enl, Faction::Enl))
        );
        assert_eq!(
            winner_entrant(MatchResult::ResWin, a, b, a_is_enl),
            Some(faction_entrant(a, b, a_is_enl, Faction::Res))
        );
        assert_eq!(winner_entrant(MatchResult::Draw, a, b, a_is_enl), None);

        // Technical wins map the winning FACTION back to the winning
        // ENTRANT through a_is_enl, both polarities, both stages.
        for (outcome, stage, reason) in [
            (
                MatchOutcome::SpawnForfeit {
                    winner: Some(Faction::Res),
                    reason_losing: "invalid initial spawn set".into(),
                },
                Stage::SpawnForfeit,
                "invalid initial spawn set",
            ),
            (
                MatchOutcome::SpawnForfeit {
                    winner: Some(Faction::Enl),
                    reason_losing: "missed spawn deadline".into(),
                },
                Stage::SpawnForfeit,
                "missed spawn deadline",
            ),
            (
                MatchOutcome::ClientForfeit {
                    winner: Some(Faction::Enl),
                    at_round: 3,
                    reason_losing: "client disconnected".into(),
                },
                Stage::ClientForfeit,
                "client disconnected",
            ),
            (
                MatchOutcome::ClientForfeit {
                    winner: Some(Faction::Res),
                    at_round: 2,
                    reason_losing: "over-limit frame".into(),
                },
                Stage::ClientForfeit,
                "over-limit frame",
            ),
        ] {
            let mapped = from_server(&outcome, a, b, a_is_enl);
            match &mapped {
                TournamentOutcome::TechnicalWin {
                    winner,
                    stage: s,
                    reason: r,
                } => {
                    let winning = match &outcome {
                        MatchOutcome::SpawnForfeit { winner, .. }
                        | MatchOutcome::ClientForfeit { winner, .. } => winner.unwrap(),
                        _ => unreachable!(),
                    };
                    assert_eq!(*winner, faction_entrant(a, b, a_is_enl, winning));
                    assert_eq!(*s, stage);
                    assert_eq!(r, reason);
                }
                other => panic!("technical stays technical, got {other:?}"),
            }
            assert!(!mapped.elo_relevant());
        }

        // Winner-less / infra outcomes are unscored, never technical wins.
        for (outcome, stage) in [
            (
                MatchOutcome::SpawnForfeit {
                    winner: None,
                    reason_losing: "both missed the deadline".into(),
                },
                Stage::SpawnForfeit,
            ),
            (
                MatchOutcome::ClientForfeit {
                    winner: None,
                    at_round: 0,
                    reason_losing: "both lost".into(),
                },
                Stage::ClientForfeit,
            ),
            (
                MatchOutcome::SetupError {
                    reason: "handshake window expired".into(),
                },
                Stage::SetupError,
            ),
        ] {
            let mapped = from_server(&outcome, a, b, a_is_enl);
            match &mapped {
                TournamentOutcome::Unscored { stage: s, reason } => {
                    assert_eq!(*s, stage);
                    assert!(!reason.is_empty());
                }
                other => panic!("unscored stays unscored, got {other:?}"),
            }
            assert!(!mapped.elo_relevant());
        }
    }
}

#[test]
fn outcome_tags_round_trip() {
    use TournamentOutcome::*;
    let cases: Vec<(TournamentOutcome, &str, Option<Stage>)> = vec![
        (
            Normal {
                result: MatchResult::Draw,
                score_enl: 1,
                score_res: 1,
                rounds_completed: 2,
                final_state_hash: [0u8; 32],
            },
            "draw",
            None,
        ),
        (
            Normal {
                result: MatchResult::EnlWin,
                score_enl: 1,
                score_res: 0,
                rounds_completed: 2,
                final_state_hash: [0u8; 32],
            },
            "enl_win",
            None,
        ),
        (
            Normal {
                result: MatchResult::ResWin,
                score_enl: 0,
                score_res: 1,
                rounds_completed: 2,
                final_state_hash: [0u8; 32],
            },
            "res_win",
            None,
        ),
        (
            TechnicalWin {
                winner: 1,
                stage: Stage::SpawnForfeit,
                reason: "r".into(),
            },
            "technical_win",
            Some(Stage::SpawnForfeit),
        ),
        (
            TechnicalWin {
                winner: 1,
                stage: Stage::ClientForfeit,
                reason: "r".into(),
            },
            "technical_win",
            Some(Stage::ClientForfeit),
        ),
        (
            Unscored {
                stage: Stage::RunnerFault,
                reason: "r".into(),
            },
            "unscored",
            Some(Stage::RunnerFault),
        ),
    ];
    for (outcome, tag, stage) in cases {
        assert_eq!(outcome.outcome_tag(), tag);
        assert_eq!(outcome.stage(), stage);
        if let Some(stage) = stage {
            assert_eq!(Stage::from_tag(stage.tag()), Some(stage));
        }
    }
    for stage in [
        Stage::SpawnForfeit,
        Stage::ClientForfeit,
        Stage::SetupError,
        Stage::RunnerFault,
    ] {
        assert_eq!(Stage::from_tag(stage.tag()), Some(stage));
    }
}

// ---------------------------------------------------------------------------
// 7. Elo golden vectors (P5-RANKING: draw = 0.5 each, integers only).
// ---------------------------------------------------------------------------

/// |q - want| <= tolerance_nano (in units of 2^-32 ulps), all integers.
fn assert_close(q: i64, want_nano: i64, tolerance_nano: i64, what: &str) {
    let want_q = (want_nano as i128 * (1i128 << 32)) / 1_000_000_000;
    let diff = (q as i128 - want_q).abs();
    let bound = ((tolerance_nano as i128) * (1i128 << 32)) / 1_000_000_000;
    assert!(
        diff <= bound,
        "{what}: q={q} want~{want_q} diff={diff} bound={bound}"
    );
}

#[test]
fn elo_expected_score_golden_vectors() {
    // Delta 0: exact 0.5 (Q == 1 << 31).
    assert_eq!(expected_score_q(1500, 1500), 1 << 31);
    // Symmetry: E(-d) = 1 - E(d) up to one rounding ulp each (the pair
    // sums to Q_ONE == 2^32).
    for d in [1i64, 17, 100, 200, 400] {
        let plus = expected_score_q(1500, 1500 + d);
        let minus = expected_score_q(1500, 1500 - d);
        assert!(
            (plus + minus - (1i64 << 32)).abs() <= 2,
            "complementarity within one ulp each, d={d}: {} {}",
            plus,
            minus
        );
    }
    // Published Elo table: +/-100 and +/-200 within 1e-6, +/-400 within
    // 2^-28 of the exact rational values.
    //   E(+100) = 0.359935000197, E(-100) = 0.640064999803
    //   E(+200) = 0.240253073352, E(-200) = 0.759746926648
    //   E(+400) = 1/11,           E(-400) = 10/11
    assert_close(expected_score_q(1500, 1600), 359_935_000, 1_000, "E(+100)");
    assert_close(expected_score_q(1600, 1500), 640_064_999, 1_000, "E(-100)");
    assert_close(expected_score_q(1500, 1700), 240_253_073, 1_000, "E(+200)");
    assert_close(expected_score_q(1700, 1500), 759_746_926, 1_000, "E(-200)");
    assert_close(
        expected_score_q(1500, 1900),
        90_909_090,
        3_725,
        "E(+400) within 2^-28",
    );
    assert_close(
        expected_score_q(1900, 1500),
        909_090_909,
        3_725,
        "E(-400) within 2^-28",
    );
}

#[test]
fn round_half_away_pins() {
    assert_eq!(round_half_away(5, 2), 3);
    assert_eq!(round_half_away(-5, 2), -3);
    assert_eq!(round_half_away(4, 2), 2);
    assert_eq!(round_half_away(-4, 2), -2);
    assert_eq!(round_half_away(3, 2), 2);
    assert_eq!(round_half_away(-3, 2), -2);
    assert_eq!(round_half_away(1, 2), 1);
    assert_eq!(round_half_away(-1, 2), -1);
    assert_eq!(round_half_away(0, 7), 0);
}

#[test]
fn elo_four_match_hand_worked_sequence_is_pinned() {
    // Hand-computed (independent awk cross-check; see elo.rs docs):
    // both 1500, K=32; match 4 overrides K=16.
    //   m1 A beats B : E=0.5              A 1516 B 1484
    //   m2 draw      : E=0.545921922780   A 1515 B 1485
    //   m3 B beats A : E=0.543066492022   A 1498 B 1502
    //   m4 A beats B : E=0.494243791589   A 1506 B 1494  (K=16)
    // A = entrant 0 = matchup entrant `a`; the result tag is derived from
    // the item's own faction assignment so the sequence is seed-proof.
    let cfg = fixture_config();
    let items = build_items(&cfg).expect("items");
    let ab: Vec<&TournamentItem> = items
        .iter()
        .filter(|i| i.tag == "client_a-client_b")
        .collect();
    let a_wins = |item: &TournamentItem| TournamentOutcome::Normal {
        result: if item.a_is_enl {
            MatchResult::EnlWin
        } else {
            MatchResult::ResWin
        },
        score_enl: 3,
        score_res: 1,
        rounds_completed: 8,
        final_state_hash: [7u8; 32],
    };
    let b_wins = |item: &TournamentItem| TournamentOutcome::Normal {
        result: if item.a_is_enl {
            MatchResult::ResWin
        } else {
            MatchResult::EnlWin
        },
        score_enl: 1,
        score_res: 3,
        rounds_completed: 8,
        final_state_hash: [7u8; 32],
    };
    let draw = TournamentOutcome::Normal {
        result: MatchResult::Draw,
        score_enl: 2,
        score_res: 2,
        rounds_completed: 8,
        final_state_hash: [7u8; 32],
    };
    let mut table = EloTable::new(&EloConfig::default(), 3);
    table.apply(ab[0], &a_wins(ab[0]));
    assert_eq!((table.rating(0), table.rating(1)), (1516, 1484));
    table.apply(ab[1], &draw);
    assert_eq!((table.rating(0), table.rating(1)), (1515, 1485));
    table.apply(ab[2], &b_wins(ab[2]));
    assert_eq!((table.rating(0), table.rating(1)), (1498, 1502));
    table.apply_k(ab[3], &a_wins(ab[3]), 16);
    assert_eq!((table.rating(0), table.rating(1)), (1506, 1494));
    // W/D/L counters moved exactly with the four scored matches.
    assert_eq!(
        (table.wins(0), table.draws(0), table.losses(0)),
        (2, 1, 1),
        "entrant 0: W2 D1 L1"
    );
    assert_eq!((table.wins(1), table.draws(1), table.losses(1)), (1, 1, 2));
}

// ---------------------------------------------------------------------------
// 8. Technical exclusion + schedule-order sensitivity.
// ---------------------------------------------------------------------------

#[test]
fn technical_outcomes_never_move_ratings_or_counts() {
    let cfg = fixture_config();
    let items = build_items(&cfg).expect("items");
    let ab: Vec<&TournamentItem> = items
        .iter()
        .filter(|i| i.tag == "client_a-client_b")
        .collect();
    let mut table = EloTable::new(&EloConfig::default(), 3);
    let before: Vec<i64> = table.ratings().to_vec();
    table.apply(
        ab[0],
        &TournamentOutcome::TechnicalWin {
            winner: 0,
            stage: Stage::SpawnForfeit,
            reason: "missed spawn deadline".into(),
        },
    );
    table.apply(
        ab[1],
        &TournamentOutcome::TechnicalWin {
            winner: 1,
            stage: Stage::ClientForfeit,
            reason: "client disconnected".into(),
        },
    );
    table.apply(
        ab[2],
        &TournamentOutcome::Unscored {
            stage: Stage::SetupError,
            reason: "handshake".into(),
        },
    );
    table.apply(
        ab[3],
        &TournamentOutcome::Unscored {
            stage: Stage::RunnerFault,
            reason: "panic".into(),
        },
    );
    assert_eq!(table.ratings(), &before[..], "no rating moved");
    for id in 0..3u32 {
        assert_eq!(
            (table.wins(id), table.draws(id), table.losses(id)),
            (0, 0, 0)
        );
    }
    assert_eq!(table.forfeit_wins(0), 1);
    assert_eq!(table.forfeit_wins(1), 1);
    assert_eq!(table.forfeit_wins(2), 0);
    assert_eq!(table.forfeit_losses(1), 1);
    assert_eq!(table.forfeit_losses(0), 1);
    // An unscored match counts for BOTH entrants: 2 matches x 2 = 4.
    assert_eq!(table.unscored(0) + table.unscored(1) + table.unscored(2), 4);
}

/// A fully synthetic item (the 0-vs-2 and 2-vs-1 pairings below are not
/// scheduled by the frozen config; the schedule machinery itself is pinned
/// by the determinism tests above). `a` is always the ENL side.
fn synthetic_item(index: u64, a: u32, b: u32) -> TournamentItem {
    TournamentItem {
        index,
        tag: format!("synthetic-{index}"),
        j: index as u32,
        a,
        b,
        a_is_enl: true,
        map_index: 0,
        map_seed: index,
        map_hash: String::new(),
        game_seed: index,
        client_seed: index ^ 0x5555,
        schedule_seed: 42,
        config_hash: 1,
    }
}

#[test]
fn ratings_replay_in_item_index_order_and_order_matters() {
    // Matches (all Normal, entrant A always the ENL side): m1 A beats B,
    // m2 A draws C, m3 C beats B. Hand-computed (awk cross-check):
    //   m1-then-m2-then-m3 : A=1515 B=1469 C=1516
    //   m2-then-m1-then-m3 : A=1516 B=1469 C=1515
    let draw = TournamentOutcome::Normal {
        result: MatchResult::Draw,
        score_enl: 1,
        score_res: 1,
        rounds_completed: 8,
        final_state_hash: [0u8; 32],
    };
    let enl_wins = TournamentOutcome::Normal {
        result: MatchResult::EnlWin,
        score_enl: 2,
        score_res: 1,
        rounds_completed: 8,
        final_state_hash: [0u8; 32],
    };
    let m1 = synthetic_item(0, 0, 1); // A(0) plays ENL vs B(1)
    let m2 = synthetic_item(1, 0, 2); // A(0) plays ENL vs C(2)
    let m3 = synthetic_item(2, 2, 1); // C(2) plays ENL vs B(1)

    let mut order1 = EloTable::new(&EloConfig::default(), 3);
    order1.apply(&m1, &enl_wins); // A beats B
    order1.apply(&m2, &draw); // A draws C
    order1.apply(&m3, &enl_wins); // C beats B (C is the ENL side of m3)
    assert_eq!(order1.ratings(), &vec![1515, 1469, 1516][..]);

    let mut order2 = EloTable::new(&EloConfig::default(), 3);
    order2.apply(&m2, &draw);
    order2.apply(&m1, &enl_wins);
    order2.apply(&m3, &enl_wins);
    assert_eq!(order2.ratings(), &vec![1516, 1469, 1515][..]);
    assert_ne!(
        order1.ratings(),
        order2.ratings(),
        "reordering scored items changes ratings"
    );
}

// ---------------------------------------------------------------------------
// Seed-domain hygiene: every derived stream has its own label.
// ---------------------------------------------------------------------------

#[test]
fn seed_domains_are_disjoint_labels() {
    assert_eq!(SCHEDULE_SEED_LABEL, b"ingress-arena-m9-sched");
    assert_eq!(GAME_SEED_LABEL, b"ingress-arena-m9-game");
    assert_eq!(CLIENT_SEED_LABEL, b"ingress-arena-m9-client");
    let labels: Vec<&[u8]> = vec![
        SCHEDULE_SEED_LABEL,
        GAME_SEED_LABEL,
        CLIENT_SEED_LABEL,
        MAP_SEED_LABEL,
    ];
    let mut sorted = labels.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), labels.len(), "four distinct domains");
    let cfg = fixture_config();
    let items = build_items(&cfg).expect("items");
    for item in &items {
        assert_eq!(item.game_seed, cfg.game_seed(&item.tag, item.j));
        assert_eq!(item.client_seed, cfg.client_seed(&item.tag, item.j));
        assert_ne!(item.game_seed, item.client_seed);
        assert_eq!(item.schedule_seed, cfg.schedule_seed());
        assert_eq!(item.config_hash, cfg.config_identity());
    }
}

// ---------------------------------------------------------------------------
// REVIEW FIX (M9, D81 faction fidelity): the connect-order PROOF and its
// enforcement. The server records each accepted connection's SO_PEERCRED
// pid in accept order; the runner matches slot 0 (first accept = ENL,
// D61) against the ENL-scheduled side's process tree and slot 1 against
// the RES side's. A match whose attribution cannot be proven NEVER
// reaches the ratings.
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use arena_runner::tournament::exec_external::{enforce_connect_proof, verify_accept_order};

fn tree(pids: &[u32]) -> BTreeSet<u32> {
    pids.iter().copied().collect()
}

#[test]
fn verify_accept_order_proves_and_refutes() {
    let enl = tree(&[100, 101]); // runner 100 + sandboxed client 101
    let res = tree(&[200, 201]);
    // The proven case: first accept from the ENL tree, second from RES.
    assert!(verify_accept_order([Some(101), Some(200)], &enl, &res));
    assert!(verify_accept_order([Some(100), Some(201)], &enl, &res));
    // Swapped accept order (RES connected first): the schedule's
    // assignment does NOT hold — this is exactly the silent misattribution
    // the proof exists to catch.
    assert!(!verify_accept_order([Some(200), Some(100)], &enl, &res));
    // A connection whose peer is in neither tree (or a missing second
    // connection) cannot be proven.
    assert!(!verify_accept_order([Some(999), Some(200)], &enl, &res));
    assert!(!verify_accept_order([Some(100), None], &enl, &res));
    assert!(!verify_accept_order([None, Some(200)], &enl, &res));
    // One process connecting twice (e.g. a duplicate-connect probe taking
    // the second slot) must NOT verify as the scheduled opponent.
    assert!(!verify_accept_order([Some(100), Some(101)], &enl, &res));
    assert!(!verify_accept_order([Some(101), Some(101)], &enl, &res));
}

#[test]
fn unproven_connect_order_demotes_scoreable_outcomes_only() {
    let normal = TournamentOutcome::Normal {
        result: MatchResult::EnlWin,
        score_enl: 2,
        score_res: 1,
        rounds_completed: 8,
        final_state_hash: [7u8; 32],
    };
    // A proven match keeps its outcome byte-for-byte.
    assert_eq!(enforce_connect_proof(normal.clone(), true), normal);
    // An unproven Normal NEVER scores: wrong winner attribution must not
    // reach ratings/W-D-L.
    match enforce_connect_proof(normal, false) {
        TournamentOutcome::Unscored { stage, reason } => {
            assert_eq!(stage, Stage::RunnerFault);
            assert!(reason.contains("connect order not proven"), "{reason}");
        }
        other => panic!("unproven normal must demote, got {other:?}"),
    }
    // Same for an attributed technical win ...
    let technical = TournamentOutcome::TechnicalWin {
        winner: 0,
        stage: Stage::SpawnForfeit,
        reason: "missed spawn deadline".into(),
    };
    assert_eq!(enforce_connect_proof(technical.clone(), true), technical);
    assert!(matches!(
        enforce_connect_proof(technical, false),
        TournamentOutcome::Unscored {
            stage: Stage::RunnerFault,
            ..
        }
    ));
    // ... but an already-unscored outcome keeps its more precise stage
    // (e.g. SetupError from a handshake that never completed — the
    // attribution question is moot when nothing was scored anyway).
    let unscored = TournamentOutcome::Unscored {
        stage: Stage::SetupError,
        reason: "handshake window expired".into(),
    };
    assert_eq!(enforce_connect_proof(unscored.clone(), false), unscored);
}

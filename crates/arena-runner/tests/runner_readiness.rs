//! B2 readiness-layer tests (`docs/ai-balance-plan.md` sections 10.1 and
//! 13.2/13.3, ADR-0016): the Stage::Readiness plan rules, every gate
//! evaluator on hand-built passing AND failing fixtures, the frozen
//! active-blocking proxy over a hand-scripted replay with known truth,
//! the mirror spawn gate over real replay headers, the Transfer fold
//! through `run_match_recorded`, and worker-count invariance of a real
//! R2 batch.

use arena_bots::policy::PolicyId;
use arena_core::types::Score2;
use arena_runner::balance::plan::{ConfigVariant, Gate, PolicyMatchup, SeedSet, Stage};
use arena_runner::balance::readiness::{
    GateCount, b2_blocking_plan, b2_idle_plan, b2_round_robin_plan, blocking_gate, drop_gate,
    idle_gate, mirror_gate, records_gate, sustained_scoring_gate, transfer_gate,
};
use arena_runner::balance::record::{BalanceMatchRecord, BalanceOutcome};
use arena_runner::balance::schedule::MatchCell;
use arena_runner::metrics::{FactionMatchStats, MatchStats, Outcome};
use arena_runner::policy::BucketPolicy;

// ---------------------------------------------------------------------
// Hand-built records
// ---------------------------------------------------------------------

/// A minimal played balance record with the given sides, outcome and
/// per-faction stats. Index is the cell index; identities are echoed
/// consistently.
fn record(
    index: u64,
    a: PolicyId,
    b: PolicyId,
    a_is_enl: bool,
    enl_score2: Score2,
    res_score2: Score2,
    enl: FactionMatchStats,
    res: FactionMatchStats,
) -> BalanceMatchRecord {
    let (spec_a, spec_b) = (a.r2_spec(), b.r2_spec());
    let mut cell = MatchCell {
        index,
        experiment_id: "unit-readiness".to_string(),
        seed_set_tag: "development".to_string(),
        variant_id: "default".to_string(),
        pair_tag: format!("{}--{}", a.tag(), b.tag()),
        pair_id: format!("{}--{}#m{:03}", a.tag(), b.tag(), index / 2),
        leg: (index % 2) as u8,
        map_id: format!("development-{:03}", index / 2),
        map_index: (index / 2) as u32,
        policy_a: a,
        policy_b: b,
        policy_a_is_enl: a_is_enl,
        map_seed: index as u64,
        game_seed: index as u64 + 1,
        bot_seed: index as u64 + 2,
        schedule_seed: 7,
        bucket_policy: BucketPolicy::Flat(0),
        config_hash: 42,
    };
    cell.leg = 0;
    cell.policy_a_is_enl = true;
    BalanceMatchRecord {
        index,
        experiment_id: "unit-readiness".to_string(),
        seed_set_tag: "development".to_string(),
        variant_id: "default".to_string(),
        pair_tag: cell.pair_tag.clone(),
        pair_id: cell.pair_id.clone(),
        leg: 0,
        map_id: cell.map_id.clone(),
        map_index: cell.map_index,
        policy_a: a,
        policy_a_family: spec_a.family,
        policy_a_version: spec_a.version.to_string(),
        policy_b: b,
        policy_b_family: spec_b.family,
        policy_b_version: spec_b.version.to_string(),
        policy_a_is_enl: true,
        map_seed: cell.map_seed,
        game_seed: cell.game_seed,
        bot_seed: cell.bot_seed,
        schedule_seed: 7,
        bucket_policy: BucketPolicy::Flat(0),
        config_hash: 42,
        result: BalanceOutcome::Played(if enl_score2 >= res_score2 {
            Outcome::EnlWin
        } else {
            Outcome::ResWin
        }),
        enl_score2,
        res_score2,
        final_state_hash: [0xAB; 32],
        stats: MatchStats {
            enl,
            res,
            checkpoints: arena_runner::metrics::CHECKPOINTS.map(|round| {
                arena_runner::metrics::ScoreSnapshot {
                    round,
                    enl: enl_score2.min(Score2::MAX),
                    res: res_score2.min(Score2::MAX),
                    leader: Some(arena_core::types::Faction::Enl),
                }
            }),
        },
        elapsed_ms: 0,
    }
}

fn scoring_faction(fields: u64) -> FactionMatchStats {
    FactionMatchStats {
        commands_submitted: 100,
        commands_ok: 100,
        fields_created: fields,
        ..FactionMatchStats::default()
    }
}

fn transfer_faction(transfers: u64) -> FactionMatchStats {
    FactionMatchStats {
        commands_submitted: 100,
        commands_ok: 100,
        transfers,
        ..FactionMatchStats::default()
    }
}

// ---------------------------------------------------------------------
// Stage::Readiness plan rules
// ---------------------------------------------------------------------

fn readiness_plan() -> arena_runner::balance::plan::ExperimentPlan {
    b2_blocking_plan(2, "unit-readiness-block")
}

#[test]
fn readiness_stage_is_default_config_only_and_round_trips() {
    assert_eq!(Stage::Readiness.tag(), "readiness");
    assert_eq!(Stage::from_tag("readiness"), Some(Stage::Readiness));
    let plan = readiness_plan();
    assert_eq!(plan.validate(), Ok(()), "default-only readiness validates");
    // A declared config change is refused...
    let mut plan = readiness_plan();
    let mut cfg = arena_core::config::GameConfig::default();
    cfg.move_budget_cm = 24_000;
    plan.variants.push(ConfigVariant::new(
        "move-budget-240m",
        cfg,
        vec!["move_budget_cm"],
    ));
    assert!(plan.validate().is_err(), "readiness refuses rule changes");
    // ...and so is an UNDECLARED one (belt-and-braces).
    let mut plan = readiness_plan();
    let mut cfg = arena_core::config::GameConfig::default();
    cfg.attack_damage = 25;
    plan.variants
        .push(ConfigVariant::new("sneaky", cfg, Vec::new()));
    assert!(plan.validate().is_err(), "undeclared changes are caught");
    // The round-robin and idle plans validate too, with all 21/6
    // pairings and pre-registered gates.
    let rr = b2_round_robin_plan(1, "unit-readiness-rr");
    assert_eq!(rr.validate(), Ok(()));
    assert_eq!(rr.matchups.len(), 21);
    assert_eq!(rr.matchups.iter().filter(|m| m.a == m.b).count(), 6);
    assert_eq!(rr.gates.len(), 5);
    let idle = b2_idle_plan(1, "unit-readiness-idle");
    assert_eq!(idle.validate(), Ok(()));
    assert_eq!(idle.matchups.len(), 6);
}

// ---------------------------------------------------------------------
// Gate evaluators: passing and failing fixtures
// ---------------------------------------------------------------------

/// Four competitive policies from three families all scoring and
/// fielding in every match: the sustained-scoring gate passes with the
/// raw counts verbatim.
#[test]
fn sustained_scoring_gate_passes_and_fails_on_raw_counts() {
    let quorum = [
        PolicyId::GreedyCoordinatedR2,
        PolicyId::BuilderDefenderR2,
        PolicyId::BuilderDisruptorR2,
        PolicyId::LogisticsBuilderR2,
    ];
    let mut records = Vec::new();
    let mut index = 0;
    for (i, &a) in quorum.iter().enumerate() {
        for &b in quorum.iter().skip(i + 1) {
            records.push(record(
                index,
                a,
                b,
                true,
                100,
                50,
                scoring_faction(2),
                scoring_faction(1),
            ));
            index += 1;
        }
    }
    let specs: Vec<_> = quorum.iter().map(|id| id.r2_spec()).collect();
    let gate = sustained_scoring_gate(&records, &specs);
    assert!(gate.passed, "4 policies, 4 families, all scoring");
    assert_eq!(gate.numerator, 4);
    assert_eq!(gate.denominator, 4);
    assert_eq!(gate.permille, 1000, "4 of the 4-policy quorum");
    // One policy never fields: only 3 qualify -> gate fails (needs 4).
    let mut records = records;
    records.push(record(
        index,
        PolicyId::BlockerBuilderR2,
        PolicyId::AdaptiveHybridR2,
        true,
        100,
        0,
        scoring_faction(0),
        scoring_faction(5),
    ));
    let gate = sustained_scoring_gate(&records, &specs);
    assert!(gate.passed, "still the same 4 qualifiers");

    // Three families only, four policies: fails the family side.
    let trio = [
        PolicyId::GreedyCoordinatedR2,
        PolicyId::BuilderDefenderR2,
        PolicyId::BuilderDisruptorR2,
    ];
    let specs: Vec<_> = trio.iter().map(|id| id.r2_spec()).collect();
    let gate = sustained_scoring_gate(&records, &specs);
    assert!(!gate.passed, "3 policies cannot reach the quorum of 4");
}

#[test]
fn idle_gate_requires_perfect_scoring_and_fielding() {
    let mut records = Vec::new();
    for (index, &policy) in PolicyId::R2_COMPETITIVE.iter().enumerate() {
        records.push(record(
            index as u64,
            policy,
            PolicyId::Idle,
            true,
            500,
            0,
            scoring_faction(3),
            FactionMatchStats::default(),
        ));
    }
    let gate = idle_gate(&records);
    assert!(gate.passed);
    assert_eq!(gate.numerator, 6);
    assert_eq!(gate.denominator, 6);
    // One policy fails to field in one match: the gate fails, numerator
    // and denominator carry the raw counts.
    records[0].stats.enl.fields_created = 0;
    let gate = idle_gate(&records);
    assert!(!gate.passed);
    assert_eq!(gate.numerator, 5);
    assert_eq!(gate.denominator, 6);
}

#[test]
fn transfer_gate_passes_at_half_the_matches() {
    let a = PolicyId::LogisticsBuilderR2;
    let b = PolicyId::GreedyCoordinatedR2;
    let mut records = Vec::new();
    for i in 0..4u64 {
        let transfers = if i < 2 {
            transfer_faction(3)
        } else {
            transfer_faction(0)
        };
        records.push(record(
            i,
            a,
            b,
            true,
            100,
            90,
            transfers,
            transfer_faction(0),
        ));
    }
    let gate = transfer_gate(&records);
    assert!(gate.passed, "2/4 matches with transfers meets the 50% bar");
    assert_eq!(gate.numerator, 2);
    assert_eq!(gate.denominator, 4);
    let mut records = records;
    records[0].stats.enl.transfers = 0;
    let gate = transfer_gate(&records);
    assert!(!gate.passed, "1/4 is below the bar");
    assert_eq!(gate.numerator, 1);
    assert_eq!(gate.denominator, 4);
}

#[test]
fn drop_gate_pools_competitive_matches_only_and_enforces_the_threshold() {
    let a = PolicyId::GreedyCoordinatedR2;
    let b = PolicyId::AdaptiveHybridR2;
    let mut dropped_stats = FactionMatchStats {
        commands_submitted: 100,
        commands_dropped: 3,
        ..FactionMatchStats::default()
    };
    dropped_stats.commands_ok = 97;
    let clean = FactionMatchStats {
        commands_submitted: 100,
        commands_ok: 100,
        ..FactionMatchStats::default()
    };
    // One idle match (excluded) and one competitive match: the pooled
    // rate counts only the competitive legs.
    let records = vec![
        record(
            0,
            a,
            PolicyId::Idle,
            true,
            10,
            0,
            clean.clone(),
            clean.clone(),
        ),
        record(1, a, b, true, 10, 5, dropped_stats.clone(), clean.clone()),
        record(2, a, b, false, 10, 5, clean.clone(), dropped_stats),
    ];
    let gate = drop_gate(&records);
    assert!(gate.passed, "6/400 = 15 permille pooled");
    assert_eq!(gate.numerator, 6);
    assert_eq!(gate.denominator, 400);
    // A pathological dropper fails the 50-permille threshold.
    let mut bad = FactionMatchStats {
        commands_submitted: 100,
        commands_dropped: 51,
        ..FactionMatchStats::default()
    };
    bad.commands_ok = 49;
    let records = vec![record(3, a, b, true, 10, 0, bad, clean)];
    let gate = drop_gate(&records);
    assert!(!gate.passed);
    assert_eq!(gate.permille, 255, "51 of 200 pooled commands");
}

#[test]
fn records_gate_detects_family_and_version_mismatches() {
    let plan = b2_blocking_plan(1, "unit-readiness-block");
    let mut ok = record(
        0,
        PolicyId::BlockerBuilderR2,
        PolicyId::AdaptiveHybridR2,
        true,
        1,
        2,
        scoring_faction(1),
        scoring_faction(1),
    );
    ok.experiment_id = plan.experiment_id.clone();
    assert!(records_gate(&[ok.clone()], &plan).passed);
    let mut wrong_family = ok.clone();
    wrong_family.policy_a_family = arena_bots::policy::PolicyFamily::FieldBuilder;
    assert!(!records_gate(&[wrong_family], &plan).passed);
    let mut wrong_version = ok;
    wrong_version.policy_b_version = "r2.1".to_string();
    assert!(!records_gate(&[wrong_version], &plan).passed);
}

// ---------------------------------------------------------------------
// The active-blocking proxy and the mirror gate over REAL replays
// ---------------------------------------------------------------------

/// Drives one scripted 1-round-per-command match on the hand-placed
/// blocking fixture map and returns its replay bytes: ENL captures P6
/// and P7 (the crossing anchors) and links P6->P7 across the enemy
/// candidate pair (P1, P2); RES captures P1 and P2 first. The link
/// blocks EXACTLY one enemy candidate pair by the frozen definition.
#[test]
fn blocking_proxy_counts_the_known_crossing_link() {
    use arena_core::types::{AgentId, Command, Coord, Owner, Portal, PortalId};
    let mut cfg = arena_core::config::GameConfig::default();
    cfg.total_rounds = 6;
    cfg.initial_agents_per_faction = 2;
    cfg.portal_count = 5;
    cfg.portal_cluster_count = 5;
    cfg.portal_min_distance_cm = 4_000;
    cfg.map_width_cm = 40_000;
    cfg.map_height_cm = 40_000;
    let km = 1_000;
    // Enemy pair P1-P2 on the map diagonal; our anchors P3/P4 cross it.
    // (MapDefinition indexes portals by dense id: 0..4.)
    let map = arena_core::map::MapDefinition::new(vec![
        Portal {
            id: PortalId(0),
            position: Coord::new(2 * km, 2 * km),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(1),
            position: Coord::new(38 * km, 2 * km),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(2),
            position: Coord::new(2 * km, 38 * km),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(3),
            position: Coord::new(8 * km, 26 * km),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(4),
            position: Coord::new(8 * km, 34 * km),
            owner: Owner::Neutral,
            hp: 0,
        },
    ]);
    let enl_spawn = vec![
        (AgentId(0), map.portal(PortalId(3)).unwrap().position),
        (AgentId(1), map.portal(PortalId(4)).unwrap().position),
    ];
    let res_spawn = vec![
        (AgentId(2), map.portal(PortalId(1)).unwrap().position),
        (AgentId(3), map.portal(PortalId(2)).unwrap().position),
    ];
    let enl_spawns =
        arena_core::spawn::validate_initial_spawn_set(&[AgentId(0), AgentId(1)], &enl_spawn, &cfg)
            .expect("enl spawns");
    let res_spawns =
        arena_core::spawn::validate_initial_spawn_set(&[AgentId(2), AgentId(3)], &res_spawn, &cfg)
            .expect("res spawns");

    // Scripted: r1 both factions capture their portals; r2 ENL hacks P7
    // for a key; r3 the hacker walks to P6; r4 the hacker links P6->P7.
    use arena_core::harness::{AgentSubmission, RoundStrategy, ScriptedStrategy};
    let _ = AgentSubmission::new;
    let make_enl = |map: &arena_core::map::MapDefinition| {
        let map = map.clone();
        move |round: u32| -> Vec<AgentSubmission> {
            match round {
                1 => vec![
                    AgentSubmission::new(
                        AgentId(0),
                        vec![Command::Capture {
                            portal: PortalId(3),
                        }],
                    ),
                    AgentSubmission::new(
                        AgentId(1),
                        vec![Command::Capture {
                            portal: PortalId(4),
                        }],
                    ),
                ],
                2 => vec![AgentSubmission::new(
                    AgentId(1),
                    vec![Command::Hack {
                        portal: PortalId(4),
                    }],
                )],
                3 => vec![AgentSubmission::new(
                    AgentId(1),
                    vec![Command::Move {
                        destination: map.portal(PortalId(3)).unwrap().position,
                    }],
                )],
                4 => vec![AgentSubmission::new(
                    AgentId(1),
                    vec![Command::Link {
                        source_portal: PortalId(3),
                        target_portal: PortalId(4),
                    }],
                )],
                _ => Vec::new(),
            }
        }
    };
    let make_res = move |round: u32| -> Vec<AgentSubmission> {
        if round == 1 {
            vec![
                AgentSubmission::new(
                    AgentId(2),
                    vec![Command::Capture {
                        portal: PortalId(1),
                    }],
                ),
                AgentSubmission::new(
                    AgentId(3),
                    vec![Command::Capture {
                        portal: PortalId(2),
                    }],
                ),
            ]
        } else {
            Vec::new()
        }
    };
    struct Scripted<F: Fn(u32) -> Vec<AgentSubmission>>(F);
    impl<F: Fn(u32) -> Vec<AgentSubmission>> RoundStrategy for Scripted<F> {
        fn submit_round(&mut self, obs: &arena_core::observe::Observation) -> Vec<AgentSubmission> {
            (self.0)(obs.round)
        }
    }
    let mut harness = arena_core::harness::MatchHarness::new(
        0x8_0C1,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(Scripted(make_enl(&map))),
        Box::new(Scripted(make_res)),
    )
    .expect("fixture match builds");
    let mut rounds = Vec::new();
    for _ in 0..cfg.total_rounds {
        rounds.push(harness.run_round());
    }
    let state = harness.into_state();
    // The link really exists and ENL owns its endpoints.
    assert!(
        state
            .link_graph()
            .links()
            .iter()
            .any(|(id, link)| id.min == PortalId(3)
                && id.max == PortalId(4)
                && link.faction == arena_core::types::Faction::Enl)
    );

    // Re-record the same match into a replay through the recorded runner.
    let descriptor = arena_runner::batch::MatchDescriptor {
        index: 0,
        matchup_index: 0,
        matchup_tag: "block-fixture".to_string(),
        j: 0,
        bot_a: "blocker-builder-r2".to_string(),
        bot_b: "adaptive-hybrid-r2".to_string(),
        bot_a_is_enl: true,
        game_seed: 0x8_0C1,
        map_seed: 1,
        map_index: 0,
        bot_seed: 1,
        schedule_seed: 1,
        bucket_policy: BucketPolicy::Flat(0),
        config_hash: 9,
        bot_a_version: "r2.0",
        bot_b_version: "r2.0",
    };
    let _ = ScriptedStrategy::new(vec![]);
    struct Scripted2<F: Fn(u32) -> Vec<AgentSubmission>>(F);
    impl<F: Fn(u32) -> Vec<AgentSubmission>> RoundStrategy for Scripted2<F> {
        fn submit_round(&mut self, obs: &arena_core::observe::Observation) -> Vec<AgentSubmission> {
            (self.0)(obs.round)
        }
    }
    let played = arena_runner::run::run_match_recorded(
        &descriptor,
        &cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(Scripted2(make_enl(&map))),
        Box::new(Scripted2(make_res)),
        true,
    );
    let replay_bytes = played.replay.clone().expect("replay captured");
    let file = arena_replay::replay::decode_log(&replay_bytes).expect("replay decodes");
    // EXACTLY ONE active block by ENL: the P6-P7 link crossing the enemy
    // candidate pair (P1, P2); RES creates no links.
    assert_eq!(
        arena_runner::balance::readiness::count_active_blocks(
            &file,
            arena_core::types::Faction::Enl
        ),
        1,
        "the known crossing link blocks exactly one enemy candidate pair"
    );
    assert_eq!(
        arena_runner::balance::readiness::count_active_blocks(
            &file,
            arena_core::types::Faction::Res
        ),
        0
    );

    // The blocking gate over this single replay passes the >=10% share
    // but FAILS the aggregate >=5 (raw counts verbatim).
    let mut cell_record = record(
        0,
        PolicyId::BlockerBuilderR2,
        PolicyId::AdaptiveHybridR2,
        true,
        1,
        2,
        scoring_faction(1),
        scoring_faction(1),
    );
    cell_record.index = 0;
    let replay = arena_runner::balance::execute::BalanceReplay {
        cell_index: 0,
        variant_id: "default".to_string(),
        pair_id: "pair".to_string(),
        leg: 0,
        map_id: "development-000".to_string(),
        bytes: replay_bytes.clone(),
    };
    let gate = blocking_gate(&[cell_record], &[replay], PolicyId::BlockerBuilderR2);
    assert!(
        !gate.passed,
        "1 block in 1 match meets the share but not the aggregate of 5"
    );
    assert_eq!(gate.numerator, 1);
    assert_eq!(gate.denominator, 1);

    // Five such matches pass the whole gate.
    let records: Vec<BalanceMatchRecord> = (0..5u64)
        .map(|i| {
            let mut r = record(
                i,
                PolicyId::BlockerBuilderR2,
                PolicyId::AdaptiveHybridR2,
                true,
                1,
                2,
                scoring_faction(1),
                scoring_faction(1),
            );
            r.index = i;
            r
        })
        .collect();
    let replays: Vec<arena_runner::balance::execute::BalanceReplay> = (0..5u64)
        .map(|i| arena_runner::balance::execute::BalanceReplay {
            cell_index: i,
            variant_id: "default".to_string(),
            pair_id: format!("pair{i}"),
            leg: 0,
            map_id: format!("development-{i:03}"),
            bytes: replay_bytes.clone(),
        })
        .collect();
    let gate = blocking_gate(&records, &replays, PolicyId::BlockerBuilderR2);
    assert!(
        gate.passed,
        "5/5 matches with a block and 5 aggregate blocks"
    );
    assert_eq!(gate.numerator, 5);
    assert_eq!(gate.denominator, 5);
}

/// The mirror gate reads spawn sets from REAL replay headers: identical
/// faction spawn sets are counted (failing fixture), different ones pass.
#[test]
fn mirror_gate_counts_identical_spawn_legs_from_replay_headers() {
    let mut cfg = arena_core::config::GameConfig::default();
    cfg.total_rounds = 1;
    cfg.initial_agents_per_faction = 2;
    let map = arena_core::map::generate_map(0, &arena_core::config::GameConfig::default())
        .expect("map generates");
    let enl_roster = vec![arena_core::types::AgentId(0), arena_core::types::AgentId(1)];
    let res_roster = vec![arena_core::types::AgentId(2), arena_core::types::AgentId(3)];
    let same = arena_core::types::Coord::new(1, 1);
    let other = arena_core::types::Coord::new(5_000, 5_000);
    let mk = |enl: arena_core::types::Coord, res: arena_core::types::Coord, index: u64| {
        let enl_spawns = arena_core::spawn::validate_initial_spawn_set(
            &enl_roster,
            &[(enl_roster[0], enl), (enl_roster[1], enl)],
            &cfg,
        )
        .expect("enl spawns");
        let res_spawns = arena_core::spawn::validate_initial_spawn_set(
            &res_roster,
            &[(res_roster[0], res), (res_roster[1], res)],
            &cfg,
        )
        .expect("res spawns");
        struct Idle;
        impl arena_core::harness::RoundStrategy for Idle {
            fn submit_round(
                &mut self,
                _obs: &arena_core::observe::Observation,
            ) -> Vec<arena_core::harness::AgentSubmission> {
                Vec::new()
            }
        }
        let descriptor = arena_runner::batch::MatchDescriptor {
            index,
            matchup_index: 0,
            matchup_tag: "mirror-fixture".to_string(),
            j: 0,
            bot_a: "greedy-coordinated-r2".to_string(),
            bot_b: "greedy-coordinated-r2".to_string(),
            bot_a_is_enl: true,
            game_seed: 1,
            map_seed: 1,
            map_index: 0,
            bot_seed: 1,
            schedule_seed: 1,
            bucket_policy: BucketPolicy::Flat(0),
            config_hash: 9,
            bot_a_version: "r2.0",
            bot_b_version: "r2.0",
        };
        arena_runner::run::run_match_recorded(
            &descriptor,
            &cfg,
            &map,
            &enl_spawns,
            &res_spawns,
            Box::new(Idle),
            Box::new(Idle),
            true,
        )
        .replay
        .expect("replay captured")
    };
    let mk_record = |index: u64| {
        let mut r = record(
            index,
            PolicyId::GreedyCoordinatedR2,
            PolicyId::GreedyCoordinatedR2,
            true,
            1,
            2,
            scoring_faction(1),
            scoring_faction(1),
        );
        r.pair_id = format!("unit--unit#m{:03}", index);
        r
    };
    // Two DIFFERENT-spawn mirror legs: passes, zero identical.
    let records = vec![mk_record(0), mk_record(1)];
    let replays = vec![
        arena_runner::balance::execute::BalanceReplay {
            cell_index: 0,
            variant_id: "default".to_string(),
            pair_id: "p".to_string(),
            leg: 0,
            map_id: "development-000".to_string(),
            bytes: mk(other, same, 0),
        },
        arena_runner::balance::execute::BalanceReplay {
            cell_index: 1,
            variant_id: "default".to_string(),
            pair_id: "p".to_string(),
            leg: 1,
            map_id: "development-000".to_string(),
            bytes: mk(same, other, 1),
        },
    ];
    let gate = mirror_gate(&records, &replays);
    assert!(gate.passed);
    assert_eq!(gate.numerator, 0);
    assert_eq!(gate.denominator, 2);
    // One leg with IDENTICAL faction spawn sets: fails, count 1/2.
    let replays = vec![
        arena_runner::balance::execute::BalanceReplay {
            cell_index: 0,
            variant_id: "default".to_string(),
            pair_id: "p".to_string(),
            leg: 0,
            map_id: "development-000".to_string(),
            bytes: mk(same, same, 0),
        },
        replays[1].clone(),
    ];
    let gate = mirror_gate(&records, &replays);
    assert!(!gate.passed);
    assert_eq!(gate.numerator, 1);
    assert_eq!(gate.denominator, 2);
}

// ---------------------------------------------------------------------
// run_match_recorded fold + worker-count invariance on a real R2 batch
// ---------------------------------------------------------------------

/// The plan 10.1 transfer evidence through the SHARED recorder fold:
/// a logistics vs greedy competitive match records `transfers > 0` in its
/// MatchStats.
#[test]
fn recorded_stats_fold_counts_real_transfers() {
    let mut cfg = arena_core::config::GameConfig::default();
    cfg.total_rounds = 150;
    let map = arena_core::map::generate_map(0, &arena_core::config::GameConfig::default())
        .expect("map generates");
    let spec_enl = PolicyId::LogisticsBuilderR2.r2_spec();
    let spec_res = PolicyId::GreedyCoordinatedR2.r2_spec();
    let spawns = |spec: &arena_bots::policy::PolicySpec, faction: arena_core::types::Faction| {
        let mut player = arena_bots::policy::make_policy_player(spec, faction, 0x7A5C, &cfg, &map);
        let roster = arena_core::state::GameState::initial_roster(&cfg);
        let roster = if faction == arena_core::types::Faction::Enl {
            roster.0
        } else {
            roster.1
        };
        arena_core::spawn::validate_initial_spawn_set(
            &roster,
            &player.choose_spawns(&cfg, &map, &roster),
            &cfg,
        )
        .expect("spawns validate")
    };
    let enl_spawns = spawns(&spec_enl, arena_core::types::Faction::Enl);
    let res_spawns = spawns(&spec_res, arena_core::types::Faction::Res);
    let enl = arena_bots::policy::make_policy_player(
        &spec_enl,
        arena_core::types::Faction::Enl,
        0x7A5C,
        &cfg,
        &map,
    );
    let res = arena_bots::policy::make_policy_player(
        &spec_res,
        arena_core::types::Faction::Res,
        0x7A5C,
        &cfg,
        &map,
    );
    let descriptor = arena_runner::batch::MatchDescriptor {
        index: 0,
        matchup_index: 0,
        matchup_tag: "logistics--greedy".to_string(),
        j: 0,
        bot_a: "logistics-builder-r2".to_string(),
        bot_b: "greedy-coordinated-r2".to_string(),
        bot_a_is_enl: true,
        game_seed: 3,
        map_seed: 3,
        map_index: 0,
        bot_seed: 0x7A5C,
        schedule_seed: 3,
        bucket_policy: BucketPolicy::Flat(0),
        config_hash: 9,
        bot_a_version: "r2.0",
        bot_b_version: "r2.0",
    };
    let played = arena_runner::run::run_match_recorded(
        &descriptor,
        &cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        enl.into_strategy(),
        res.into_strategy(),
        false,
    );
    assert_eq!(played.record.result, Outcome::EnlWin);
    assert!(
        played.record.stats.enl.transfers > 0,
        "the shared recorder fold counts logistics transfers (got {})",
        played.record.stats.enl.transfers
    );
    assert!(played.record.stats.enl.fields_created >= 1);
}

/// A tiny REAL readiness batch is worker-count invariant (plan section
/// 13.3): identical records modulo the metadata-only elapsed time, and
/// identical captured replay bytes.
#[test]
fn tiny_r2_batch_is_identical_under_one_and_many_workers() {
    let plan = b2_blocking_plan(2, "unit-readiness-block-workers");
    let run_one = arena_runner::balance::execute::run_balance_batch_with(
        &plan,
        1,
        &arena_runner::balance::execute::default_cell_setup,
        &|_cell| true,
    )
    .expect("1-worker batch runs");
    let run_four = arena_runner::balance::execute::run_balance_batch_with(
        &plan,
        4,
        &arena_runner::balance::execute::default_cell_setup,
        &|_cell| true,
    )
    .expect("4-worker batch runs");
    assert_eq!(run_one.records.len(), run_four.records.len());
    for (a, b) in run_one.records.iter().zip(run_four.records.iter()) {
        assert!(
            a.same_ignoring_elapsed(b),
            "record {} differs across worker counts",
            a.index
        );
    }
    assert_eq!(run_one.replays.len(), run_four.replays.len());
    for (a, b) in run_one.replays.iter().zip(run_four.replays.iter()) {
        assert_eq!(a.cell_index, b.cell_index);
        assert_eq!(a.bytes, b.bytes, "replay bytes identical across workers");
    }
    // The gates over the two runs agree.
    let gates_one: Vec<GateCount> = vec![blocking_gate(
        &run_one.records,
        &run_one.replays,
        PolicyId::BlockerBuilderR2,
    )];
    let gates_four: Vec<GateCount> = vec![blocking_gate(
        &run_four.records,
        &run_four.replays,
        PolicyId::BlockerBuilderR2,
    )];
    for (a, b) in gates_one.iter().zip(gates_four.iter()) {
        assert_eq!(a.count_line(), b.count_line());
    }
}

// The plan gate slot stays pre-registered (Pending) until evaluation.
#[test]
fn readiness_plans_pre_register_their_gates() {
    for plan in [
        b2_round_robin_plan(1, "unit-rr"),
        b2_idle_plan(1, "unit-idle"),
        b2_blocking_plan(1, "unit-block"),
    ] {
        assert!(!plan.gates.is_empty());
        for gate in &plan.gates {
            assert!(matches!(gate, Gate::Pending { .. }));
        }
        assert!(matches!(plan.stage, Stage::Readiness));
        assert_eq!(plan.seed_set, SeedSet::Development);
        assert!(plan.policies.iter().all(|s| s.validate().is_ok()));
    }
    let _ = PolicyMatchup::new("x", PolicyId::Idle, PolicyId::Idle);
}

//! Runner unit gates (checklist 12, 13, 16, 17): schedule-seed faction
//! assignment, bucket-policy semantics end-to-end, metric math on
//! hand-built report/event sequences, and BatchSummary aggregation.

use std::collections::BTreeMap;

use arena_bots::bot::BotKind;
use arena_core::config::GameConfig;
use arena_core::event::{Event, EventKind};
use arena_core::harness::{AgentSubmission, MatchHarness, RoundStrategy};
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::resolve::{AttackHit, CommandEffect, CommandOutcome, ExecutedStream, RoundReport};
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::{
    AgentId, Command, Coord, Faction, FieldId, LinkId, Owner, Portal, PortalId,
};
use arena_runner::batch::{BatchConfig, Matchup};
use arena_runner::metrics::{
    CHECKPOINTS, FactionMatchStats, MatchFold, MatchRecord, MatchStats, Outcome, ScoreSnapshot,
};
use arena_runner::policy::{BucketPolicy, PolicyStrategy};
use arena_runner::report::{BatchSummary, fraction_str, mean_u64_str, permille_str, summary_text};
use arena_runner::run::{BatchError, run_batch};
use arena_runner::schedule::a_is_enl_decisions;

// ---------------------------------------------------------------------
// Checklist 12: schedule-seed faction assignment (D55)
// ---------------------------------------------------------------------

#[test]
fn schedule_assigns_exactly_half_enl_per_bot() {
    let schedule_seed = 0x5EED_0001;
    for (tag, count) in [
        ("Random-Random", 1000),
        ("Nearest-Nearest", 1500),
        ("Nearest-Greedy", 1750),
        ("Greedy-DestroyAnchor", 1750),
        ("Greedy-Hybrid", 1750),
        ("Hybrid-Hybrid", 2250),
        ("tiny", 2),
        ("small", 8),
    ] {
        let first = a_is_enl_decisions(schedule_seed, tag, count);
        assert_eq!(first.len() as u32, count, "{tag}");
        let enl = first.iter().filter(|enl| **enl).count() as i32;
        let res = count as i32 - enl;
        assert_eq!(
            enl - res,
            0,
            "{tag}: |ENL - RES| must be 0 (got {enl} vs {res})"
        );
        // Deterministic in (schedule seed, matchup, j).
        let second = a_is_enl_decisions(schedule_seed, tag, count);
        assert_eq!(first, second, "{tag} deterministic");
        // The schedule seed changes the assignment (with more bits than a
        // 2-match toy matchup has outcomes).
        if count >= 8 {
            let other = a_is_enl_decisions(schedule_seed ^ 1, tag, count);
            assert_ne!(first, other, "{tag} changes with the schedule seed");
        }
        // Randomized order: not all ENL slots first (a degenerate
        // deterministic split would still satisfy the count, but the frozen
        // schedule must interleave). Skipped for toy counts whose first
        // third is empty.
        if count >= 8 {
            let first_majority = first[..count as usize / 3]
                .iter()
                .filter(|enl| **enl)
                .count();
            assert!(
                first_majority < count as usize / 3,
                "{tag}: ENL assignments are spread, not front-loaded"
            );
        }
    }
}

#[test]
#[should_panic(expected = "must be even")]
fn schedule_rejects_odd_counts() {
    a_is_enl_decisions(1, "odd", 7);
}

// ---------------------------------------------------------------------
// Checklist 13: bucket policy semantics end-to-end (D19/D20/D56)
// ---------------------------------------------------------------------

/// Always submits one legal Move-to-self per agent (cheap, legal, keeps
/// both streams executable so the merge order is observable).
struct WiggleStrategy {
    agents: Vec<AgentId>,
}

impl RoundStrategy for WiggleStrategy {
    fn submit_round(&mut self, obs: &Observation) -> Vec<AgentSubmission> {
        obs.own_agents
            .iter()
            .map(|me| {
                let _ = &self.agents;
                AgentSubmission::new(
                    me.id,
                    vec![Command::Move {
                        destination: me.position,
                    }],
                )
            })
            .collect()
    }
}

fn policy_cfg() -> GameConfig {
    let mut cfg = GameConfig::default();
    cfg.map_width_cm = 30_000;
    cfg.map_height_cm = 30_000;
    cfg.portal_count = 2;
    cfg.portal_cluster_count = 2;
    cfg.portal_min_distance_cm = 4_000;
    cfg.initial_agents_per_faction = 1;
    cfg.total_rounds = 6;
    cfg
}

fn policy_map() -> MapDefinition {
    MapDefinition::new(vec![
        Portal {
            id: PortalId(0),
            position: Coord::new(10_000, 10_000),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(1),
            position: Coord::new(25_000, 25_000),
            owner: Owner::Neutral,
            hp: 0,
        },
    ])
}

fn policy_match(policy: BucketPolicy) -> Vec<RoundReport> {
    let cfg = policy_cfg();
    let map = policy_map();
    let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
    let entries = |roster: &[AgentId], coord: Coord| -> Vec<(AgentId, Coord)> {
        roster.iter().map(|&id| (id, coord)).collect()
    };
    let enl_spawns = validate_initial_spawn_set(
        &enl_roster,
        &entries(&enl_roster, Coord::new(10_000, 5_000)),
        &cfg,
    )
    .expect("spawns");
    let res_spawns = validate_initial_spawn_set(
        &res_roster,
        &entries(&res_roster, Coord::new(6_000, 10_000)),
        &cfg,
    )
    .expect("spawns");
    let enl = PolicyStrategy::new(
        Box::new(WiggleStrategy {
            agents: enl_roster.clone(),
        }),
        Faction::Enl,
        policy,
    );
    let res = PolicyStrategy::new(
        Box::new(WiggleStrategy {
            agents: res_roster.clone(),
        }),
        Faction::Res,
        policy,
    );
    let mut harness = MatchHarness::new(
        0,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        Box::new(enl),
        Box::new(res),
    )
    .expect("match builds");
    harness.run_rounds(6)
}

#[test]
fn flat_bucket_zero_shows_parity_alternation() {
    let reports = policy_match(BucketPolicy::Flat(0));
    assert_eq!(reports.len(), 6);
    for (index, report) in reports.iter().enumerate() {
        let round = index as u32 + 1;
        let factions: Vec<Faction> = report.execution_order.iter().map(|s| s.faction).collect();
        assert_eq!(factions.len(), 2, "both streams execute every round");
        if round % 2 == 1 {
            assert_eq!(
                factions,
                vec![Faction::Enl, Faction::Res],
                "odd round {round}: ENL settles first (D20)"
            );
        } else {
            assert_eq!(
                factions,
                vec![Faction::Res, Faction::Enl],
                "even round {round}: RES settles first (D20)"
            );
        }
        for stream in &report.execution_order {
            assert_eq!(stream.receive_bucket, 0);
        }
    }
}

#[test]
fn cross_bucket_policy_puts_every_enl_stream_first_every_round() {
    let reports = policy_match(BucketPolicy::Factions { enl: 0, res: 1 });
    for report in &reports {
        let factions: Vec<Faction> = report.execution_order.iter().map(|s| s.faction).collect();
        assert_eq!(
            factions,
            vec![Faction::Enl, Faction::Res],
            "bucket 0 (ENL) always settles before bucket 1 (RES), every round"
        );
        assert_eq!(report.execution_order[0].receive_bucket, 0);
        assert_eq!(report.execution_order[1].receive_bucket, 1);
    }
    // The bucket order dominates parity: compare with Flat(0), where the
    // even rounds flip (the cross-bucket rows never flip).
    let flat = policy_match(BucketPolicy::Flat(0));
    assert_ne!(
        faction_shape(&flat),
        faction_shape(&reports),
        "the two policies produce observably different orderings"
    );
}

fn faction_shape(reports: &[RoundReport]) -> Vec<Vec<Faction>> {
    reports
        .iter()
        .map(|r| r.execution_order.iter().map(|s| s.faction).collect())
        .collect()
}

// ---------------------------------------------------------------------
// Checklist 16: metrics math on hand-built sequences
// ---------------------------------------------------------------------

fn metrics_map() -> MapDefinition {
    // P0..P4: a 12k x 12k right triangle plus two outliers.
    MapDefinition::new(vec![
        Portal {
            id: PortalId(0),
            position: Coord::new(4_000, 4_000),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(1),
            position: Coord::new(16_000, 4_000),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(2),
            position: Coord::new(4_000, 16_000),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(3),
            position: Coord::new(16_000, 16_000),
            owner: Owner::Neutral,
            hp: 0,
        },
        Portal {
            id: PortalId(4),
            position: Coord::new(28_000, 16_000),
            owner: Owner::Neutral,
            hp: 0,
        },
    ])
}

/// A one-stream report with the given outcomes for `faction`.
fn synthetic_report(faction: Faction, outcomes: Vec<CommandOutcome>) -> RoundReport {
    RoundReport {
        submissions: Vec::new(),
        slots: BTreeMap::new(),
        execution_order: vec![ExecutedStream {
            faction,
            agent_id: AgentId(0),
            receive_bucket: 0,
            receive_seq: 0,
            outcomes,
        }],
        ignored_duplicates: Vec::new(),
        unlocatable: Vec::new(),
        score2_delta: BTreeMap::new(),
    }
}

fn delta(report: &mut RoundReport, enl: u128, res: u128) {
    report.score2_delta = BTreeMap::from([(Faction::Enl, enl), (Faction::Res, res)]);
}

fn capture_event(faction: Faction, portal: u32) -> Event {
    Event {
        seq: 0,
        round: 1,
        kind: EventKind::PortalCaptured {
            faction,
            agent_id: AgentId(0),
            portal: PortalId(portal),
        },
    }
}

#[test]
fn metrics_first_field_area_lifetimes_and_layering() {
    let map = metrics_map();
    let cfg = GameConfig::default();
    let mut fold = MatchFold::new();
    let field_a = FieldId::new(PortalId(0), PortalId(1), PortalId(2)).expect("distinct");
    let field_b = FieldId::new(PortalId(0), PortalId(3), PortalId(4)).expect("distinct");
    let link = LinkId::new(PortalId(0), PortalId(1)).expect("distinct");

    // Round 10: one ENL link closes TWO fields sharing P0 -> layering 2,
    // areas from map geometry (144M each for a/b).
    let mut report = synthetic_report(
        Faction::Enl,
        vec![CommandOutcome::Ok(CommandEffect::Linked {
            link,
            fields: vec![field_a, field_b],
        })],
    );
    delta(&mut report, 288_000_000, 0);
    fold.round(10, &report, &[], &map, &cfg);
    assert_eq!(fold.stats.enl.first_field_round, Some(10));
    assert_eq!(fold.stats.enl.fields_created, 2);
    assert_eq!(fold.stats.enl.field_area2_sum, 288_000_000);
    assert_eq!(fold.stats.enl.layering_depth_max, 2, "fields share P0");

    // Round 20: RES neutralizes P0 -> both fields die (lifetime 10 each).
    let report = synthetic_report(
        Faction::Res,
        vec![CommandOutcome::Ok(CommandEffect::Attacked {
            hits: vec![AttackHit {
                portal: PortalId(0),
                hp_after: 0,
                neutralized: true,
            }],
        })],
    );
    fold.round(20, &report, &[], &map, &cfg);
    assert_eq!(fold.stats.enl.fields_destroyed, 2);
    assert_eq!(
        fold.stats.enl.destroyed_lifetime_rounds_sum, 20,
        "2x(20-10)"
    );
    assert_eq!(
        fold.stats.res.anchor_attack_hits, 1,
        "P0 carried live ENL fields"
    );
    assert_eq!(fold.stats.res.attack_hits, 1);

    // Round 30: a surviving field; supply at match end from a real state.
    let field_c = FieldId::new(PortalId(1), PortalId(2), PortalId(3)).expect("distinct");
    let report = synthetic_report(
        Faction::Res,
        vec![CommandOutcome::Ok(CommandEffect::Linked {
            link,
            fields: vec![field_c],
        })],
    );
    fold.round(30, &report, &[], &map, &cfg);
    let state = metrics_state();
    fold.finish(&state, &cfg);
    assert_eq!(fold.stats.res.fields_surviving, 1);
    assert_eq!(fold.stats.res.surviving_lifetime_rounds_sum, 470, "500-30");
    // The synthetic final state is the one-agent-per-faction policy
    // fixture; nothing spent supply in this synthetic match.
    assert_eq!(fold.stats.enl.supply_remaining, 20);
    assert_eq!(fold.stats.res.supply_remaining, 20);
}

fn metrics_state() -> GameState {
    let cfg = policy_cfg();
    let map = policy_map();
    let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
    let entries = |roster: &[AgentId], coord: Coord| -> Vec<(AgentId, Coord)> {
        roster.iter().map(|&id| (id, coord)).collect()
    };
    let enl = validate_initial_spawn_set(
        &enl_roster,
        &entries(&enl_roster, Coord::new(1_000, 1_000)),
        &cfg,
    )
    .expect("spawns");
    let res = validate_initial_spawn_set(
        &res_roster,
        &entries(&res_roster, Coord::new(2_000, 2_000)),
        &cfg,
    )
    .expect("spawns");
    GameState::new(cfg, &map, &enl, &res).expect("state")
}

#[test]
fn metrics_checkpoints_key_utilization_and_turnover() {
    let map = metrics_map();
    let cfg = GameConfig::default();
    let mut fold = MatchFold::new();
    let field = FieldId::new(PortalId(0), PortalId(1), PortalId(2)).expect("distinct");
    let link = LinkId::new(PortalId(0), PortalId(1)).expect("distinct");

    // Round 50: ENL leads 100 vs 25.
    let mut report = synthetic_report(
        Faction::Enl,
        vec![CommandOutcome::Ok(CommandEffect::Linked {
            link,
            fields: vec![field],
        })],
    );
    delta(&mut report, 100, 25);
    fold.round(50, &report, &[capture_event(Faction::Enl, 0)], &map, &cfg);
    // Round 100: RES takes the lead 25 vs 200 (leader flips).
    let mut report = RoundReport::default();
    delta(&mut report, 0, 175);
    fold.round(100, &report, &[capture_event(Faction::Res, 1)], &map, &cfg);

    let snapshot50 = &fold.stats.checkpoints[0];
    assert_eq!(
        (snapshot50.round, snapshot50.enl, snapshot50.res),
        (50, 100, 25)
    );
    assert_eq!(snapshot50.leader, Some(Faction::Enl));
    let snapshot100 = &fold.stats.checkpoints[1];
    assert_eq!((snapshot100.enl, snapshot100.res), (100, 200));
    assert_eq!(snapshot100.leader, Some(Faction::Res));
    // Untouched checkpoints stay zeroed with no leader.
    assert_eq!(fold.stats.checkpoints[3].leader, None);

    // Keys: 3 hacked, 1 spent on the link -> utilization 1/3.
    let hack = CommandOutcome::Ok(CommandEffect::Hacked {
        portal: PortalId(3),
        keys_gained: 3,
        supply_granted: 1,
        supply_after: 21,
    });
    let report = synthetic_report(Faction::Enl, vec![hack]);
    fold.round(101, &report, &[], &map, &cfg);
    assert_eq!(fold.stats.enl.keys_gained, 3);
    assert_eq!(fold.stats.enl.links_created, 1);
    assert_eq!(fold.stats.enl.key_utilization_permille(), 333);

    // Turnover: 2 captures over 5 portals = 400 permille per the fraction
    // helper (captures are per faction; each captured one portal).
    assert_eq!(fold.stats.enl.captures, 1);
    assert_eq!(fold.stats.res.captures, 1);
    assert_eq!(fraction_str(1, 5), "0.200");
}

#[test]
fn metrics_reinforcement_share_and_drop_rate() {
    let map = metrics_map();
    let cfg = GameConfig::default();
    let mut fold = MatchFold::new();
    let field = FieldId::new(PortalId(0), PortalId(1), PortalId(2)).expect("distinct");
    let link = LinkId::new(PortalId(0), PortalId(1)).expect("distinct");
    // A reinforcement agent (id 6 >= 2n with n = 3) creates the only link.
    let mut report = RoundReport {
        submissions: Vec::new(),
        slots: BTreeMap::new(),
        execution_order: vec![ExecutedStream {
            faction: Faction::Enl,
            agent_id: AgentId(6),
            receive_bucket: 0,
            receive_seq: 0,
            outcomes: vec![CommandOutcome::Ok(CommandEffect::Linked {
                link,
                fields: vec![field],
            })],
        }],
        ignored_duplicates: Vec::new(),
        unlocatable: Vec::new(),
        score2_delta: BTreeMap::new(),
    };
    delta(&mut report, 144_000_000, 0);
    fold.round(110, &report, &[], &map, &cfg);
    assert_eq!(fold.stats.enl.links_by_reinforcements, 1);
    assert_eq!(fold.stats.enl.reinforcement_share_permille(), 1000);

    // Drop rate: 1 drop of 3 submitted = 333 permille.
    let mut stats = FactionMatchStats {
        commands_submitted: 3,
        commands_dropped: 1,
        ..Default::default()
    };
    assert_eq!(stats.drop_rate_permille(), 333);
    stats.commands_submitted = 0;
    stats.commands_dropped = 0;
    assert_eq!(stats.drop_rate_permille(), 0);
}

// ---------------------------------------------------------------------
// Checklist 17: aggregation over synthetic records
// ---------------------------------------------------------------------

fn synthetic_record(
    index: u64,
    bot_a_is_enl: bool,
    result: Outcome,
    leader100: Option<Faction>,
) -> MatchRecord {
    let stats = MatchStats {
        checkpoints: CHECKPOINTS.map(|round| ScoreSnapshot {
            round,
            enl: if leader100 == Some(Faction::Enl) {
                10
            } else {
                5
            },
            res: if leader100 == Some(Faction::Res) {
                10
            } else {
                5
            },
            leader: if round == 100 { leader100 } else { None },
        }),
        ..Default::default()
    };
    MatchRecord {
        index,
        matchup_index: 0,
        matchup_tag: "Test-Test".to_string(),
        j: index as u32,
        bot_a: "hybrid".to_string(),
        bot_b: "greedy-field".to_string(),
        bot_a_is_enl,
        game_seed: index,
        map_seed: index,
        map_index: 0,
        bot_seed: index,
        schedule_seed: 7,
        bucket_policy: BucketPolicy::Flat(0),
        config_hash: 42,
        bot_a_version: "m6.0".to_string(),
        bot_b_version: "m6.0".to_string(),
        result,
        enl_score2: 0,
        res_score2: 0,
        final_state_hash: [0u8; 32],
        stats,
        elapsed_ms: 0,
    }
}

#[test]
fn batch_summary_matches_hand_computed_win_rates_and_splits() {
    let matchups = vec![Matchup::new(
        "Test-Test",
        BotKind::Hybrid,
        BotKind::GreedyField,
        4,
    )];
    let records = vec![
        synthetic_record(0, true, Outcome::EnlWin, Some(Faction::Enl)),
        synthetic_record(1, true, Outcome::ResWin, Some(Faction::Enl)),
        synthetic_record(2, false, Outcome::EnlWin, Some(Faction::Enl)),
        synthetic_record(3, false, Outcome::ResWin, None), // tie at R100
    ];
    let summary = BatchSummary::from_records(&records, &matchups);
    let row = &summary.matchups[0];
    assert_eq!(row.count, 4);
    assert_eq!(row.bot_a_wins, 2, "bot A won matches 0 and 3");
    assert_eq!(row.bot_b_wins, 2);
    assert_eq!(row.enl_wins, 2);
    assert_eq!(row.res_wins, 2);
    assert_eq!(row.draws, 0);
    assert_eq!(row.crashes, 0);
    assert_eq!(row.bot_a_win_rate_permille(), 500);

    // Snowball at R100: three decided leaders, two of them won; one tie.
    let checkpoint = &row.snowball[1];
    assert_eq!(checkpoint.round, 100);
    assert_eq!(checkpoint.decided, 3);
    assert_eq!(
        checkpoint.leader_won, 2,
        "the R100 leader won matches 0 and 2"
    );
    assert_eq!(checkpoint.ties, 1);
    assert_eq!(checkpoint.win_rate_permille(), 666);

    // Pooled faction split.
    assert_eq!(
        (
            summary.total,
            summary.total_enl_wins,
            summary.total_res_wins
        ),
        (4, 2, 2)
    );

    // Formatting helpers: permille and means as integer fixed point.
    assert_eq!(permille_str(666), "0.666");
    assert_eq!(permille_str(1000), "1.000");
    assert_eq!(mean_u64_str(10, 4), "2.500");
    assert_eq!(mean_u64_str(0, 0), "-");
}

// R-BALANCE-02 (Portal Turnover Rate) in summary.txt must be a PER-MATCH
// rate: the matchup-wide capture sum divided by (played matches x portals
// in one map). Regression: the turnover column used to divide by
// portal_count only, so it scaled linearly with the matchup's match count
// (RR 1000 matches vs HH 2250) and cross-matchup comparisons were
// meaningless even though every other mean on the same line is per match.
#[test]
fn summary_turnover_is_a_per_match_rate() {
    let record_with_captures = |matchup_index: u32, index: u64, result: Outcome| {
        let mut record = synthetic_record(index, true, result, None);
        record.matchup_index = matchup_index;
        // 30 captures per faction per match on both factions.
        record.stats.enl.captures = 30;
        record.stats.res.captures = 30;
        record
    };
    let matchups = vec![
        Matchup::new("Small", BotKind::Hybrid, BotKind::GreedyField, 2),
        Matchup::new("Big", BotKind::Hybrid, BotKind::GreedyField, 4),
        Matchup::new("Crashed", BotKind::Hybrid, BotKind::GreedyField, 2),
    ];
    let records = vec![
        record_with_captures(0, 0, Outcome::Draw),
        record_with_captures(0, 1, Outcome::Draw),
        record_with_captures(1, 2, Outcome::Draw),
        record_with_captures(1, 3, Outcome::Draw),
        record_with_captures(1, 4, Outcome::Draw),
        record_with_captures(1, 5, Outcome::Draw),
        // Crashed matches fold zero stats (`MatchRecord::crashed`); with 0
        // played matches nothing is printed ("-"), not a division by zero.
        {
            let mut crashed = synthetic_record(6, true, Outcome::Crash, None);
            crashed.matchup_index = 2;
            crashed
        },
        {
            let mut crashed = synthetic_record(7, true, Outcome::Crash, None);
            crashed.matchup_index = 2;
            crashed
        },
    ];
    let summary = BatchSummary::from_records(&records, &matchups);
    let text = summary_text(&summary, 100);

    // The `enl:` metric line of each matchup section, turnover field.
    let turnovers: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("  enl: "))
        .map(|line| {
            line.split("turnover=")
                .nth(1)
                .and_then(|rest| rest.split(' ').next())
                .expect("turnover field")
        })
        .collect();
    // Identical per-match behavior must print an identical rate regardless
    // of the matchup's match count: 2x30 captures over (2 matches x 100
    // portals) = 0.300 and 4x30 over (4 x 100) = 0.300. Before the fix
    // these printed 0.600 and 1.200. All-crash prints "-".
    assert_eq!(turnovers, vec!["0.300", "0.300", "-"]);
    assert!(!text.contains("turnover=0.600"));
    assert!(!text.contains("turnover=1.200"));
    // The per-faction absolute capture counts stay raw totals (unchanged
    // behavior, pinned so the normalization cannot wander into them).
    assert!(text.contains("captures=60 "));
    assert!(text.contains("captures=120 "));
}

// The small batch used by several runner tests.
pub fn small_batch() -> BatchConfig {
    BatchConfig {
        batch_id: "runner-unit".to_string(),
        matchups: vec![
            Matchup::new(
                "Nearest-Nearest",
                BotKind::NearestPortal,
                BotKind::NearestPortal,
                4,
            ),
            Matchup::new("Hybrid-Random", BotKind::Hybrid, BotKind::Random, 4),
        ],
        map_count: 3,
        master_seed: 0xABBA_0001,
        bucket_policy: BucketPolicy::Flat(0),
        config: GameConfig::default(),
    }
}

#[test]
fn descriptors_carry_derived_seeds_and_schedule() {
    let batch = small_batch();
    let descriptors = batch.descriptors();
    assert_eq!(descriptors.len(), 8);
    assert_eq!(batch.total_matches(), 8);
    for (index, descriptor) in descriptors.iter().enumerate() {
        assert_eq!(descriptor.index, index as u64);
        assert_eq!(
            descriptor.game_seed,
            batch.game_seed(&descriptor.matchup_tag, descriptor.j)
        );
        assert_eq!(
            descriptor.bot_seed,
            batch.bot_seed(&descriptor.matchup_tag, descriptor.j)
        );
        assert_eq!(descriptor.map_seed, batch.map_seed(descriptor.map_index));
        assert_eq!(descriptor.map_index, descriptor.j % batch.map_count);
        assert_eq!(descriptor.config_hash, batch.config_hash());
        assert_eq!(descriptor.schedule_seed, batch.schedule_seed());
        // Distinct seed domains never collide across the batch.
        assert_ne!(descriptor.game_seed, descriptor.bot_seed);
    }
    // The two matchups' schedules are independent decisions of one seed.
    let tags: Vec<&String> = descriptors.iter().map(|d| &d.matchup_tag).collect();
    assert!(tags.contains(&&"Nearest-Nearest".to_string()));
    assert!(tags.contains(&&"Hybrid-Random".to_string()));
}

#[test]
fn small_batch_runs_and_records_metadata() {
    // Smoke-level: a tiny real batch end-to-end (also exercises run_batch's
    // stride merge).
    let batch = small_batch();
    let run = run_batch(&batch, 4).expect("small batch runs");
    assert_eq!(run.records.len(), 8);
    for record in &run.records {
        assert_ne!(record.result, Outcome::Crash);
        assert!(record.elapsed_ms < 60_000);
    }
}

// BatchError implements Display + std::error::Error (the ReplayError/SdkError
// house style) and a genuinely infeasible config surfaces it from run_batch.
#[test]
fn batch_error_displays_and_infeasible_maps_surface_it() {
    let error = BatchError::MapInfeasible { map_index: 2 };
    assert!(error.to_string().contains("map 2"), "got: {error}");
    let boxed: Box<dyn std::error::Error> = Box::new(error);
    assert!(boxed.to_string().contains("infeasible"));

    // A config that cannot be laid out (50 portals, min distance 1000 cm on
    // a 1000x1000 map — at most 4 mutually spaced points exist) aborts the
    // batch as a config defect instead of skipping matches (no map skip
    // rule, D58).
    let mut cfg = GameConfig::default();
    cfg.map_width_cm = 1_000;
    cfg.map_height_cm = 1_000;
    cfg.portal_count = 50;
    cfg.portal_min_distance_cm = 1_000;
    let batch = BatchConfig {
        batch_id: "infeasible".to_string(),
        matchups: vec![Matchup::new(
            "NN",
            BotKind::NearestPortal,
            BotKind::NearestPortal,
            2,
        )],
        map_count: 1,
        master_seed: 0x1BAD_BEEF,
        bucket_policy: BucketPolicy::Flat(0),
        config: cfg,
    };
    let error = match run_batch(&batch, 1) {
        Ok(_) => panic!("the config cannot be laid out"),
        Err(error) => error,
    };
    assert_eq!(error, BatchError::MapInfeasible { map_index: 0 });
    assert!(error.to_string().contains("config defect"), "got: {error}");
}

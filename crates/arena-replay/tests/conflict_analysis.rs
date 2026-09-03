//! A-LATENCY conflict-analysis tests (D70 realized): hand-constructed
//! scripted matches whose conflicts and winners are known by construction,
//! recorded through the real recorder and analyzed through the real
//! verify-then-analyze pipeline. Plus a sweep over the archived r1
//! tournament replays wherever `runs/` exists (the M9 skip-on-fresh-clone
//! convention).

use arena_core::config::GameConfig;
use arena_core::harness::{AgentSubmission, MatchHarness, ScriptedStrategy};
use arena_core::map::MapDefinition;
use arena_core::resolve::DropReason;
use arena_core::spawn::{InitialSpawns, validate_initial_spawn_set};
use arena_core::state::GameState;
use arena_core::types::{AgentId, Command, Coord, Faction, Owner, Portal, PortalId};
use arena_replay::analysis::{self, BucketAdvantage, ConflictKind, ConflictObject, ConflictReport};
use arena_replay::recorder::ReplayWriter;

const GAME_SEED: u64 = 0x5EED_C0FF_E000_0001;

/// A small-map config: one agent per faction, 40 km map, 4 km minimum
/// portal spacing, `rounds` rounds total.
fn scenario_cfg(rounds: u32, portals: u32) -> GameConfig {
    let mut cfg = GameConfig::default();
    cfg.total_rounds = rounds;
    cfg.map_width_cm = 40_000;
    cfg.map_height_cm = 40_000;
    cfg.portal_count = portals;
    cfg.portal_cluster_count = portals;
    cfg.portal_min_distance_cm = 4_000;
    cfg.initial_agents_per_faction = 1;
    cfg
}

fn portal(id: u32, x: i32, y: i32, owner: Owner, hp: u32) -> Portal {
    Portal {
        id: PortalId(id),
        position: Coord::new(x, y),
        owner,
        hp,
    }
}

fn spawns_at(cfg: &GameConfig, faction: Faction, position: Coord) -> InitialSpawns {
    let (enl_roster, res_roster) = GameState::initial_roster(cfg);
    let roster: Vec<AgentId> = if faction == Faction::Enl {
        enl_roster
    } else {
        res_roster
    };
    validate_initial_spawn_set(&roster, &[(roster[0], position)], cfg).expect("spawn valid")
}

/// Runs a fully scripted match, records it, and returns the finished IARP.
fn scripted_match(
    cfg: GameConfig,
    map: &MapDefinition,
    enl_at: Coord,
    res_at: Coord,
    enl_rounds: Vec<Vec<AgentSubmission>>,
    res_rounds: Vec<Vec<AgentSubmission>>,
) -> Vec<u8> {
    assert_eq!(enl_rounds.len() as u32, cfg.total_rounds);
    assert_eq!(res_rounds.len() as u32, cfg.total_rounds);
    let rounds = cfg.total_rounds;
    let enl_spawns = spawns_at(&cfg, Faction::Enl, enl_at);
    let res_spawns = spawns_at(&cfg, Faction::Res, res_at);
    let mut writer = ReplayWriter::new(GAME_SEED, &cfg, map, &enl_spawns, &res_spawns);
    let mut harness = MatchHarness::new(
        GAME_SEED,
        cfg,
        map,
        &enl_spawns,
        &res_spawns,
        Box::new(ScriptedStrategy::new(enl_rounds)),
        Box::new(ScriptedStrategy::new(res_rounds)),
    )
    .expect("harness builds");
    for _ in 0..rounds {
        let report = harness.run_round();
        writer
            .record_round(&report, harness.state())
            .expect("record");
    }
    writer.finish(harness.state()).expect("finish")
}

fn submit(agent: u32, commands: Vec<Command>) -> AgentSubmission {
    AgentSubmission::new(AgentId(agent), commands)
}

fn submit_bucket(agent: u32, bucket: u64, commands: Vec<Command>) -> AgentSubmission {
    AgentSubmission {
        agent_id: AgentId(agent),
        bucket,
        commands,
    }
}

// ---------------------------------------------------------------------------
// Double capture: same bucket (first execution wins) and split buckets
// (the earlier bucket wins regardless of parity)
// ---------------------------------------------------------------------------

#[test]
fn double_capture_winner_follows_bucket_then_arrival() {
    // P0 contested under both agents' feet; P2 four km away for the
    // bucket-split round.
    let cfg = scenario_cfg(2, 3);
    let map = MapDefinition::new(vec![
        portal(0, 20_000, 20_000, Owner::Neutral, 0),
        portal(1, 4_000, 4_000, Owner::Neutral, 0),
        portal(2, 24_000, 20_000, Owner::Neutral, 0),
    ]);
    let on_p0 = Coord::new(20_000, 20_000);

    // Round 1: both CAPTURE P0 in bucket 0. Round 1 is odd, so ENL
    // settles first (D20) — ENL captures, RES drops on the pre-emptied
    // (no longer neutral) portal.
    // Round 2: both CAPTURE P2, but ENL states bucket 1 and RES bucket 0 —
    // the bucket layer decides (R-RESOLVE-03): RES executes first and wins.
    let enl = vec![
        vec![submit(
            0,
            vec![Command::Capture {
                portal: PortalId(0),
            }],
        )],
        vec![submit_bucket(
            0,
            1,
            vec![Command::Capture {
                portal: PortalId(2),
            }],
        )],
    ];
    let res = vec![
        vec![submit(
            1,
            vec![Command::Capture {
                portal: PortalId(0),
            }],
        )],
        vec![submit_bucket(
            1,
            0,
            vec![Command::Capture {
                portal: PortalId(2),
            }],
        )],
    ];
    let iarp = scripted_match(cfg, &map, on_p0, on_p0, enl, res);

    let report = analysis::analyze_conflicts_bytes(&iarp).expect("analysis");
    assert_eq!(report.rounds_analyzed, 2);
    assert_eq!(report.conflicts.len(), 2, "one conflict per round");
    assert_eq!(report.aggregate.decisive, 2);

    // Round 1: same bucket, ENL executed first.
    let first = &report.conflicts[0];
    assert_eq!(first.round, 1);
    assert_eq!(first.kind, ConflictKind::DoubleCapture);
    assert_eq!(first.object, ConflictObject::Portal(PortalId(0)));
    assert_eq!(first.winner.faction, Faction::Enl);
    assert_eq!(first.winner.agent, AgentId(0));
    assert_eq!(first.loser.faction, Faction::Res);
    assert_eq!(
        first.advantage,
        BucketAdvantage::WinnerSameBucketFirstArrival
    );
    assert!(first.loser_pre_emptied, "R-RESOLVE-04 shape");
    assert!(matches!(
        first.loser_drop,
        Some(DropReason::CaptureTargetNotNeutral { .. })
    ));

    // Round 2: strictly earlier bucket wins even though the round's parity
    // would have settled ENL first.
    let second = &report.conflicts[1];
    assert_eq!(second.round, 2);
    assert_eq!(second.winner.faction, Faction::Res);
    assert_eq!(second.winner.bucket, 0);
    assert_eq!(second.loser.bucket, 1);
    assert_eq!(second.advantage, BucketAdvantage::WinnerEarlierBucket);
    assert!(second.loser_pre_emptied);
    assert!(matches!(
        second.loser_drop,
        Some(DropReason::CaptureTargetNotNeutral { .. })
    ));

    // Aggregate arithmetic is consistent with the detailed records.
    assert_eq!(report.aggregate.winner_same_bucket_first_arrival, 1);
    assert_eq!(report.aggregate.winner_earlier_bucket, 1);
    assert_eq!(report.aggregate.loser_pre_empted, 2);
    assert_eq!(report.aggregate.indecisive, 0);
    assert_eq!(
        report.aggregate.by_kind.get(&ConflictKind::DoubleCapture),
        Some(&2)
    );
}

// ---------------------------------------------------------------------------
// Crossing links: one vertical and one horizontal link over the same
// crossing point, both attempted in the same round
// ---------------------------------------------------------------------------

#[test]
fn crossing_links_first_link_wins_second_drops() {
    // A=P0 (6k,4k) -> B=P1 (6k,16k): vertical. C=P2 (2k,10k) -> D=P3
    // (10k,10k): horizontal. The segments cross at (6k,10k); no portal
    // sits on the other faction's segment.
    let cfg = scenario_cfg(6, 4);
    let map = MapDefinition::new(vec![
        portal(0, 6_000, 4_000, Owner::Neutral, 0),
        portal(1, 6_000, 16_000, Owner::Neutral, 0),
        portal(2, 2_000, 10_000, Owner::Neutral, 0),
        portal(3, 10_000, 10_000, Owner::Neutral, 0),
    ]);
    let at_a = Coord::new(6_000, 4_000);
    let at_c = Coord::new(2_000, 10_000);

    // Both factions tour capture -> travel -> capture+hack the far portal
    // -> travel back -> LINK in round 5. Round 5 is odd: ENL settles
    // first, its link exists, RES's crossing link DROPs.
    let enl = vec![
        vec![submit(
            0,
            vec![Command::Capture {
                portal: PortalId(0),
            }],
        )],
        vec![submit(
            0,
            vec![Command::Move {
                destination: Coord::new(6_000, 16_000),
            }],
        )],
        vec![submit(
            0,
            vec![
                Command::Capture {
                    portal: PortalId(1),
                },
                Command::Hack {
                    portal: PortalId(1),
                },
            ],
        )],
        vec![submit(0, vec![Command::Move { destination: at_a }])],
        vec![submit(
            0,
            vec![Command::Link {
                source_portal: PortalId(0),
                target_portal: PortalId(1),
            }],
        )],
        vec![],
    ];
    let res = vec![
        vec![submit(
            1,
            vec![Command::Capture {
                portal: PortalId(2),
            }],
        )],
        vec![submit(
            1,
            vec![Command::Move {
                destination: Coord::new(10_000, 10_000),
            }],
        )],
        vec![submit(
            1,
            vec![
                Command::Capture {
                    portal: PortalId(3),
                },
                Command::Hack {
                    portal: PortalId(3),
                },
            ],
        )],
        vec![submit(1, vec![Command::Move { destination: at_c }])],
        vec![submit(
            1,
            vec![Command::Link {
                source_portal: PortalId(2),
                target_portal: PortalId(3),
            }],
        )],
        vec![],
    ];
    let iarp = scripted_match(cfg, &map, at_a, at_c, enl, res);
    let report = analysis::analyze_conflicts_bytes(&iarp).expect("analysis");

    // Exactly one conflict, in round 5 only.
    assert_eq!(report.conflicts.len(), 1, "{:?}", report.conflicts);
    let conflict = &report.conflicts[0];
    assert_eq!(conflict.round, 5);
    assert_eq!(conflict.kind, ConflictKind::CrossingLinks);
    assert_eq!(
        conflict.object,
        ConflictObject::Links(
            arena_core::types::LinkId::new(PortalId(0), PortalId(1)).unwrap(),
            arena_core::types::LinkId::new(PortalId(2), PortalId(3)).unwrap(),
        )
    );
    assert_eq!(conflict.winner.faction, Faction::Enl);
    assert_eq!(conflict.loser.faction, Faction::Res);
    assert_eq!(
        conflict.advantage,
        BucketAdvantage::WinnerSameBucketFirstArrival
    );
    assert!(conflict.loser_pre_emptied, "R-RESOLVE-04 shape");
    assert!(matches!(
        conflict.loser_drop,
        Some(DropReason::LinkIntersectsLink { .. })
    ));
    // The rounds before the links (captures of DIFFERENT portals, hacks,
    // moves, transfers of nothing) conflict-free.
    assert_eq!(report.aggregate.decisive, 1);
    assert_eq!(report.aggregate.indecisive, 0);
}

// ---------------------------------------------------------------------------
// Attack vs recharge: the decisive neutralize-pre-empts-recharge case, the
// indecisive both-executed case, and rounds that must NOT count (attacks
// with no opposing recharge; a TRANSFER against nothing)
// ---------------------------------------------------------------------------

#[test]
fn attack_vs_recharge_and_non_conflicts() {
    // P0 starts RES-owned at full hp right under both agents; P1 is a
    // neutral filler far away.
    let cfg = scenario_cfg(8, 2);
    let map = MapDefinition::new(vec![
        portal(0, 20_000, 20_000, Owner::Res, 100),
        portal(1, 34_000, 34_000, Owner::Neutral, 0),
    ]);
    let on_p0 = Coord::new(20_000, 20_000);

    // ENL attacks P0 every round it can (attack damage 20): rounds 1-4
    // wear it 100 -> 20, round 5 neutralizes (20 -> 0). Round 6 is a
    // self-TRANSFER (must never count). Round 7 attacks the recaptured
    // portal WITHOUT neutralizing.
    let enl = vec![
        vec![submit(0, vec![Command::Attack])],
        vec![submit(0, vec![Command::Attack])],
        vec![submit(0, vec![Command::Attack])],
        vec![submit(0, vec![Command::Attack])],
        vec![submit(0, vec![Command::Attack])],
        vec![submit(
            0,
            vec![Command::Transfer {
                target_agent: AgentId(0),
                supply: 0,
                keys: Default::default(),
            }],
        )],
        vec![submit(0, vec![Command::Attack])],
        vec![],
    ];
    // RES: idle while being worn down; round 5 recharges the portal the
    // same round the fifth attack neutralizes it (drops on the pre-emptied
    // state); round 6 recaptures it; round 7 recharges the recaptured
    // portal after the (non-neutralizing) attack — both effects execute.
    let res = vec![
        vec![],
        vec![],
        vec![],
        vec![],
        vec![submit(
            1,
            vec![Command::Recharge {
                portal: PortalId(0),
            }],
        )],
        vec![submit(
            1,
            vec![Command::Capture {
                portal: PortalId(0),
            }],
        )],
        vec![submit(
            1,
            vec![Command::Recharge {
                portal: PortalId(0),
            }],
        )],
        vec![],
    ];
    let iarp = scripted_match(cfg, &map, on_p0, on_p0, enl, res);
    let report = analysis::analyze_conflicts_bytes(&iarp).expect("analysis");

    // Exactly ONE decisive conflict: round 5, attack won, recharge dropped
    // on the just-neutralized portal.
    assert_eq!(report.conflicts.len(), 1, "{:?}", report.conflicts);
    let conflict = &report.conflicts[0];
    assert_eq!(conflict.round, 5);
    assert_eq!(conflict.kind, ConflictKind::AttackVsRecharge);
    assert_eq!(conflict.object, ConflictObject::Portal(PortalId(0)));
    assert_eq!(conflict.winner.faction, Faction::Enl);
    assert_eq!(conflict.loser.faction, Faction::Res);
    assert!(conflict.loser_pre_emptied);
    assert!(matches!(
        conflict.loser_drop,
        Some(DropReason::RechargePortalNotOwned { .. })
    ));

    // Round 7 is the indecisive both-executed exchange.
    assert_eq!(report.aggregate.indecisive, 1);
    assert_eq!(report.aggregate.decisive, 1);
    // Rounds 1-4 (attacks with no opposing recharge), round 6 (TRANSFER
    // against a capture of a DIFFERENT... unopposed portal), and round 8
    // contribute nothing.
    assert_eq!(
        report
            .conflicts
            .iter()
            .filter(|conflict| conflict.round != 5)
            .count(),
        0
    );
}

// ---------------------------------------------------------------------------
// No-conflict control: moves, hacks, and transfers alone never conflict
// ---------------------------------------------------------------------------

#[test]
fn quiet_rounds_produce_no_conflicts() {
    let cfg = scenario_cfg(3, 2);
    let map = MapDefinition::new(vec![
        portal(0, 20_000, 20_000, Owner::Neutral, 0),
        portal(1, 34_000, 34_000, Owner::Neutral, 0),
    ]);
    let on_p0 = Coord::new(20_000, 20_000);
    let enl = vec![
        vec![submit(
            0,
            vec![Command::Hack {
                portal: PortalId(0),
            }],
        )],
        vec![submit(
            0,
            vec![Command::Transfer {
                target_agent: AgentId(0),
                supply: 3,
                keys: Default::default(),
            }],
        )],
        vec![submit(
            0,
            vec![Command::Move {
                destination: Coord::new(21_000, 20_000),
            }],
        )],
    ];
    let res = vec![
        vec![submit(
            1,
            vec![Command::Hack {
                portal: PortalId(0),
            }],
        )],
        vec![submit(
            1,
            vec![Command::Hack {
                portal: PortalId(0),
            }],
        )],
        vec![submit(
            1,
            vec![Command::Transfer {
                target_agent: AgentId(1),
                supply: 5,
                keys: Default::default(),
            }],
        )],
    ];
    let iarp = scripted_match(cfg, &map, on_p0, on_p0, enl, res);
    let report = analysis::analyze_conflicts_bytes(&iarp).expect("analysis");
    // HACK/TRANSFER/MOVE carry no opposing intent over a game object, even
    // when both factions act on the same portal in the same round.
    assert!(report.conflicts.is_empty());
    assert_eq!(report.aggregate.decisive, 0);
    assert_eq!(report.aggregate.indecisive, 0);
    assert_eq!(report.rounds_analyzed, 3);
}

// ---------------------------------------------------------------------------
// Unverified input is refused (the D88 convention applies to analysis too)
// ---------------------------------------------------------------------------

#[test]
fn unverified_log_is_refused_not_analyzed() {
    let cfg = scenario_cfg(1, 2);
    let map = MapDefinition::new(vec![
        portal(0, 20_000, 20_000, Owner::Neutral, 0),
        portal(1, 34_000, 34_000, Owner::Neutral, 0),
    ]);
    let on_p0 = Coord::new(20_000, 20_000);
    let iarp = scripted_match(
        cfg,
        &map,
        on_p0,
        on_p0,
        vec![vec![submit(0, vec![Command::Attack])]],
        vec![vec![]],
    );
    // Flip one byte of the recorded round-1 state hash (the D88 export
    // test's corruption recipe): the file stays structurally decodable but
    // the verification gate refuses before any analysis runs.
    let log = arena_replay::replay::decode_log(&iarp).expect("decodes");
    let mut bytes = log.encode();
    let hash = log
        .round_records()
        .next()
        .expect("round 1 record")
        .state_hash;
    let at = bytes
        .windows(32)
        .position(|window| window == hash)
        .expect("hash bytes located");
    bytes[at] ^= 1;
    match analysis::analyze_conflicts_bytes(&bytes) {
        Ok(report) => panic!("unverified log analyzed: {}", report.summary()),
        Err(error) => assert!(
            format!("{error}").contains("mismatch") || format!("{error}").contains("hash"),
            "expected a verification failure, got: {error}"
        ),
    }
}

// ---------------------------------------------------------------------------
// The archived r1 tournament replays (skip on a fresh clone, M9
// convention): every one verifies and analyzes; the printed summaries are
// the A-LATENCY acceptance evidence recorded in docs/acceptance-mvp.md.
// ---------------------------------------------------------------------------

fn archived_iarps() -> Vec<std::path::PathBuf> {
    // The M9 repo-root convention: test binaries run from the package
    // directory, `runs/` lives at the workspace root.
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths = Vec::new();
    for root in ["runs/tournaments/r1", "runs/tournaments/inproc-r1"] {
        let matches = repo_root.join(root).join("matches");
        let Ok(entries) = std::fs::read_dir(&matches) else {
            continue;
        };
        for entry in entries.flatten() {
            for candidate in ["server/replay.iarp", "replay.iarp"] {
                let path = entry.path().join(candidate);
                if path.is_file() {
                    paths.push(path);
                }
            }
        }
    }
    paths.sort();
    paths
}

#[test]
fn archived_r1_replays_analyze() {
    let paths = archived_iarps();
    if paths.is_empty() {
        eprintln!("SKIP: runs/tournaments artifacts absent (M9 convention)");
        return;
    }
    let mut totals = ConflictAggregateTotals::default();
    for path in &paths {
        let bytes = std::fs::read(path).expect("read archived replay");
        let report = analysis::analyze_conflicts_bytes(&bytes)
            .unwrap_or_else(|error| panic!("{} failed: {error}", path.display()));
        eprintln!("{}: {}", path.display(), report.summary());
        totals.add(&report);
    }
    eprintln!("TOTAL over {} replays: {}", paths.len(), totals.text());
    // Non-vacuity: the archived external matches really contain decisive
    // conflicts and pre-empted losers (the numbers are pinned in
    // docs/acceptance-mvp.md).
    assert!(
        totals.decisive > 0,
        "the r1 archive must contain decisive conflicts"
    );
    assert!(
        totals.loser_pre_empted > 0,
        "the r1 archive must contain pre-empted losers"
    );
}

/// Cross-replay sum for the sweep test (kept local to the test binary).
#[derive(Default)]
struct ConflictAggregateTotals {
    rounds: u64,
    decisive: u64,
    indecisive: u64,
    winner_earlier_bucket: u64,
    winner_same_bucket_first_arrival: u64,
    loser_pre_empted: u64,
}

impl ConflictAggregateTotals {
    fn add(&mut self, report: &ConflictReport) {
        let a = &report.aggregate;
        self.rounds += report.rounds_analyzed as u64;
        self.decisive += a.decisive;
        self.indecisive += a.indecisive;
        self.winner_earlier_bucket += a.winner_earlier_bucket;
        self.winner_same_bucket_first_arrival += a.winner_same_bucket_first_arrival;
        self.loser_pre_empted += a.loser_pre_empted;
    }

    fn text(&self) -> String {
        format!(
            "rounds={} decisive={} indecisive={} winner_earlier_bucket={} ({}) \
             winner_same_bucket_first_arrival={} ({}) loser_pre_empted={} ({})",
            self.rounds,
            self.decisive,
            self.indecisive,
            self.winner_earlier_bucket,
            ConflictReport::permille(self.winner_earlier_bucket, self.decisive),
            self.winner_same_bucket_first_arrival,
            ConflictReport::permille(self.winner_same_bucket_first_arrival, self.decisive),
            self.loser_pre_empted,
            ConflictReport::permille(self.loser_pre_empted, self.decisive),
        )
    }
}

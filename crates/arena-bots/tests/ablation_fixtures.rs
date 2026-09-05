//! B3 ablation-arm fixtures (, `docs/balance/ai-balance-plan.md` section
//! 7.2): the three measurement arms as first-class policies — identity
//! round-trips, the spawn pin against the frozen Hybrid, the
//! byte-identity pin of `hybrid-builder-attack` (defense branch removed
//! and nothing else), the builder-only behavioral pin, and the
//! canonical-spawn collision probe.

mod support;

use std::collections::BTreeMap;

use arena_bots::ablation::{B3_VERSION, canonical_core_params};
use arena_bots::bot::{BotKind, BotStrategy, make_bot};
use arena_bots::policy::{
    PolicyFamily, PolicyId, PolicyParameters, PolicySpec, ablation_specs, make_policy_player,
};
use arena_bots::r2::build::CoreParams;
use arena_core::config::GameConfig;
use arena_core::harness::MatchHarness;
use arena_core::spawn::validate_initial_spawn_set;
use arena_core::state::GameState;
use arena_core::types::{AgentId, Command, Coord, Faction, Score2};
use support::{fixture_map, generated_map, roster, validate_bot_spawns};

// ---------------------------------------------------------------------
// Policy identity
// ---------------------------------------------------------------------

// The three arms exist as diagnostic policies with their own versions,
// full-value parameter records that round-trip with identity sensitivity,
// no BotKind, and a constructible PolicyPlayer per arm.
#[test]
fn ablation_arm_specs_round_trip_and_validate() {
    let specs = ablation_specs();
    assert_eq!(specs.len(), 3, "the three B3 arms");
    let by_tag: BTreeMap<&str, &PolicySpec> =
        specs.iter().map(|s| (s.policy_id.tag(), s)).collect();
    for tag in [
        "hybrid-builder-only",
        "hybrid-builder-attack",
        "greedy-coordinated-canonical",
    ] {
        assert!(by_tag.contains_key(tag), "{tag} registered");
    }
    for spec in &specs {
        assert_eq!(
            spec.validate(),
            Ok(()),
            "{} validates",
            spec.policy_id.tag()
        );
        assert_eq!(spec.family, PolicyFamily::Ablation);
        assert!(spec.family.is_diagnostic(), "ablation arms are diagnostic");
        assert_eq!(
            spec.policy_id.bot_kind(),
            None,
            "ablation arms are not BotKinds"
        );
        assert_eq!(
            PolicyId::from_tag(spec.policy_id.tag()),
            Some(spec.policy_id)
        );
        // Full-value parameter records round-trip exactly.
        let tag = spec.parameters.tag();
        assert_eq!(
            PolicyParameters::from_tag(&tag),
            Some(spec.parameters),
            "{tag} round-trips"
        );
    }
    // Versions: the Hybrid arms are b3.0, the canonical arm IS the r2 core.
    assert_eq!(
        by_tag["hybrid-builder-only"].version, B3_VERSION,
        "hybrid arms carry b3.0"
    );
    assert_eq!(by_tag["hybrid-builder-attack"].version, B3_VERSION);
    assert_eq!(by_tag["greedy-coordinated-canonical"].version, "r2.0");
    // The canonical arm's parameters are the r2 core with permille 0 and
    // no spawn jitter, everything else default.
    assert_eq!(
        by_tag["greedy-coordinated-canonical"].parameters,
        PolicyParameters::GreedyCoordinated(canonical_core_params())
    );
    assert_eq!(canonical_core_params().near_equal_permille, 0);
    assert_eq!(canonical_core_params().spawn_jitter_cm, 0);
    assert_eq!(
        canonical_core_params().replan_retry_rounds,
        CoreParams::default().replan_retry_rounds
    );

    // Identity sensitivity: an edited parameter record parses to a
    // DIFFERENT parameter set (or not at all), never back to the original.
    let builder_only = by_tag["hybrid-builder-only"].parameters;
    let edited =
        PolicyParameters::from_tag(&builder_only.tag().replace("builders=3", "builders=4"));
    assert_ne!(edited, Some(builder_only));
    let switched = PolicyParameters::from_tag(
        &builder_only
            .tag()
            .replace("roles=builder-only", "roles=builder-attack"),
    );
    assert_eq!(
        switched,
        Some(by_tag["hybrid-builder-attack"].parameters),
        "switching roles yields exactly the other arm's parameters"
    );
    for malformed in [
        "hybrid-ablation|builders=3;roles=builder-only;hacksupply=3",
        "hybrid-ablation|roles=builder-only;builders=3",
        "hybrid-ablation|roles=builder-only;builders=three;hacksupply=3",
        "hybrid-ablation|roles=defender;builders=3;hacksupply=3",
        "hybrid-ablation|",
        "hybrid-builder-only|roles=builder-only;builders=3;hacksupply=3",
    ] {
        assert_eq!(
            PolicyParameters::from_tag(malformed),
            None,
            "malformed record {malformed:?} must not parse"
        );
    }

    // make_policy_player constructs every arm, both factions, without panic.
    let cfg = GameConfig::default();
    let map = generated_map();
    for spec in &specs {
        for faction in [Faction::Enl, Faction::Res] {
            let mut player = make_policy_player(spec, faction, 0xAB, &cfg, &map);
            let spawns = player.choose_spawns(&cfg, &map, &roster(&cfg, faction));
            validate_bot_spawns(&cfg, faction, &map, &spawns);
        }
    }

    // A structurally inconsistent arm spec is refused (and make_policy_player
    // panics per its contract).
    let mut wrong = by_tag["hybrid-builder-only"].clone();
    wrong.parameters = by_tag["hybrid-builder-attack"].parameters;
    assert!(wrong.validate().is_err());
    let mut wrong = by_tag["greedy-coordinated-canonical"].clone();
    wrong.family = PolicyFamily::FieldBuilder;
    assert!(wrong.validate().is_err());
}

// ---------------------------------------------------------------------
// The Hybrid arms share the frozen Hybrid's spawn (ladder steps differ
// by roles only)
// ---------------------------------------------------------------------

#[test]
fn arm_spawn_sets_equal_frozen_hybrid_spawns() {
    let cfg = GameConfig::default();
    for map in [generated_map(), fixture_map()] {
        for faction in [Faction::Enl, Faction::Res] {
            let roster_ids = roster(&cfg, faction);
            let frozen = make_bot(BotKind::Hybrid, faction, 0x5E_ED_00, &cfg, &map).choose_spawns(
                &cfg,
                &map,
                &roster_ids,
            );
            for spec in [
                PolicyId::HybridBuilderOnly.ablation_spec(),
                PolicyId::HybridBuilderAttack.ablation_spec(),
            ] {
                let spawns = make_policy_player(&spec, faction, 0x5E_ED_00, &cfg, &map)
                    .choose_spawns(&cfg, &map, &roster_ids);
                assert_eq!(
                    spawns,
                    frozen,
                    "{} spawns equal the frozen Hybrid's ({:?} / {:?})",
                    spec.policy_id.tag(),
                    faction,
                    map.portals().len()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------
// hybrid-builder-attack: byte-identical to the frozen Hybrid whenever
// the defense dispatch cannot fire
// ---------------------------------------------------------------------

/// One side of the identity comparison: either a frozen m6.0 kind or a
/// policy spec, played as ENL against the frozen GreedyField as RES (a
/// never-attacking opponent — no PortalAttacked event can exist, so the
/// frozen Hybrid's defense dispatch never fires) over the FULL default
/// 500-round match on the pinned generated map.
enum EnlSide {
    Frozen(BotKind),
    Spec(PolicySpec),
}

fn drive_arm_vs_greedy_field(
    enl_side: &EnlSide,
) -> (
    Vec<Vec<arena_core::resolve::SubmittedCommandStream>>,
    (Score2, Score2),
) {
    let cfg = GameConfig::default();
    let map = generated_map();
    let bot_seed = 0x0B3_0000_0000_0001u64;
    let game_seed = 0x6A1E_5EED_0000_0B3Au64;
    let (enl_roster, res_roster) = GameState::initial_roster(&cfg);
    let (enl_strategy, enl_entries) = match enl_side {
        EnlSide::Frozen(kind) => {
            let mut bot = make_bot(*kind, Faction::Enl, bot_seed, &cfg, &map);
            let entries = bot.choose_spawns(&cfg, &map, &enl_roster);
            (
                Box::new(BotStrategy(bot)) as Box<dyn arena_core::harness::RoundStrategy>,
                entries,
            )
        }
        EnlSide::Spec(spec) => {
            let mut player = make_policy_player(spec, Faction::Enl, bot_seed, &cfg, &map);
            let entries = player.choose_spawns(&cfg, &map, &enl_roster);
            (player.into_strategy(), entries)
        }
    };
    let enl_spawns = validate_initial_spawn_set(&enl_roster, &enl_entries, &cfg).expect("spawns");
    let mut res_bot = make_bot(BotKind::GreedyField, Faction::Res, bot_seed, &cfg, &map);
    let res_entries = res_bot.choose_spawns(&cfg, &map, &res_roster);
    let res_spawns = validate_initial_spawn_set(&res_roster, &res_entries, &cfg).expect("spawns");
    let mut harness = MatchHarness::new(
        game_seed,
        cfg,
        &map,
        &enl_spawns,
        &res_spawns,
        enl_strategy,
        Box::new(BotStrategy(res_bot)),
    )
    .expect("match builds");
    let mut rounds = Vec::new();
    for _ in 0..cfg.total_rounds {
        let report = harness.run_round();
        rounds.push(report.submissions.clone());
    }
    let state = harness.state();
    (
        rounds,
        (state.score(Faction::Enl), state.score(Faction::Res)),
    )
}

// GreedyField never attacks, so no PortalAttacked event ever exists and
// the frozen Hybrid's defense dispatch never fires — over the WHOLE
// 500-round match (reinforcements included), hybrid-builder-attack must
// submit byte-identical streams to the frozen HybridBot.
#[test]
fn hybrid_builder_attack_is_byte_identical_to_frozen_hybrid_when_defense_never_dispatches() {
    let (frozen_rounds, frozen_scores) =
        drive_arm_vs_greedy_field(&EnlSide::Frozen(BotKind::Hybrid));
    let (arm_rounds, arm_scores) = drive_arm_vs_greedy_field(&EnlSide::Spec(
        PolicyId::HybridBuilderAttack.ablation_spec(),
    ));
    assert_eq!(frozen_rounds.len(), arm_rounds.len());
    for (round, (frozen, arm)) in frozen_rounds.iter().zip(arm_rounds.iter()).enumerate() {
        assert_eq!(
            frozen,
            arm,
            "hybrid-builder-attack diverged from the frozen Hybrid at round {}",
            round + 1
        );
    }
    assert_eq!(frozen_scores, arm_scores, "final scores identical");
    // The match actually exercised attackers: reinforcements arrived (one
    // every 100 rounds), so rounds after 100 have four or more agents.
    let late = &arm_rounds[149];
    assert!(
        late.iter().map(|s| s.agent_id).collect::<Vec<_>>().len() >= 4,
        "reinforcement agents joined the match"
    );
}

// ---------------------------------------------------------------------
// hybrid-builder-only: never attacks (nor recharges), still builds
// ---------------------------------------------------------------------

#[test]
fn hybrid_builder_only_never_attacks_and_builds() {
    let (rounds, scores) =
        drive_arm_vs_greedy_field(&EnlSide::Spec(PolicyId::HybridBuilderOnly.ablation_spec()));
    let mut attacks_or_recharges = 0u64;
    for round in &rounds {
        for submission in round {
            for command in &submission.commands {
                if matches!(command, Command::Attack | Command::Recharge { .. }) {
                    attacks_or_recharges += 1;
                }
            }
        }
    }
    assert_eq!(
        attacks_or_recharges, 0,
        "builder-only never emits Attack or Recharge"
    );
    assert!(
        scores.0 > 0,
        "builder-only scores (>0 score2 requires a completed field); got {}",
        scores.0
    );
    // Reinforcements stayed builders too: by round 150 a fourth agent has
    // joined (one every 100 rounds) and none of them attacked (already
    // asserted globally).
    let late_agents = rounds[149]
        .iter()
        .map(|s| s.agent_id)
        .collect::<Vec<_>>()
        .len();
    assert!(late_agents >= 4, "reinforcements joined as builders");
}

// ---------------------------------------------------------------------
// The canonical-spawn arm: determinism as a measurable variable
// ---------------------------------------------------------------------

fn sorted_spawns(entries: &[(AgentId, Coord)]) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = entries.iter().map(|(_, c)| (c.x, c.y)).collect();
    out.sort();
    out
}

// Two mirror factions of the canonical arm choose IDENTICAL spawn
// coordinates (best triangle, no jitter — the collision the seeded
// top-K arm exists to break), on fixed maps; the same construction with
// the r2 defaults does not collide; every run is deterministic.
#[test]
fn canonical_arm_mirror_factions_choose_identical_spawn_sets() {
    let cfg = GameConfig::default();
    let spec = PolicyId::GreedyCoordinatedCanonical.ablation_spec();
    let seeded = PolicyId::GreedyCoordinatedR2.r2_spec();
    let mut seeded_collisions = 0u32;
    let mut maps_checked = 0u32;
    for seed in 0u64..6 {
        let map = arena_core::map::generate_map(seed, &cfg).expect("map generates");
        maps_checked += 1;
        let enl = make_policy_player(&spec, Faction::Enl, 0xCA0, &cfg, &map).choose_spawns(
            &cfg,
            &map,
            &roster(&cfg, Faction::Enl),
        );
        let res = make_policy_player(&spec, Faction::Res, 0xCA0, &cfg, &map).choose_spawns(
            &cfg,
            &map,
            &roster(&cfg, Faction::Res),
        );
        assert_eq!(
            sorted_spawns(&enl),
            sorted_spawns(&res),
            "canonical mirror factions collide on map seed {seed} (the measured artifact)"
        );
        // Determinism: same inputs, same spawns, every run.
        let enl_again = make_policy_player(&spec, Faction::Enl, 0xCA0, &cfg, &map).choose_spawns(
            &cfg,
            &map,
            &roster(&cfg, Faction::Enl),
        );
        assert_eq!(enl, enl_again, "canonical spawns are deterministic");
        // The seeded top-K arm (r2 defaults) does NOT collide.
        let seeded_enl = make_policy_player(&seeded, Faction::Enl, 0xCA0, &cfg, &map)
            .choose_spawns(&cfg, &map, &roster(&cfg, Faction::Enl));
        let seeded_res = make_policy_player(&seeded, Faction::Res, 0xCA0, &cfg, &map)
            .choose_spawns(&cfg, &map, &roster(&cfg, Faction::Res));
        if sorted_spawns(&seeded_enl) == sorted_spawns(&seeded_res) {
            seeded_collisions += 1;
        }
    }
    assert!(maps_checked >= 6);
    assert!(
        seeded_collisions < maps_checked,
        "the seeded arm must break the collision on at least one fixed map (got {seeded_collisions}/{maps_checked} collisions)"
    );
}

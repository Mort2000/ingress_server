//! R2 shared-capability decision unit tests (`docs/ai-balance-plan.md`
//! sections 5.1, 6.2, 6.3 and 13.1): the value-model scorers, the
//! role/task allocator, seeded tie-break sampling and the R2 RNG namespace,
//! all on hand-built fixture observations. Every scorer is a pure function
//! of `(Observation, GameConfig, parameters)` — determinism here means the
//! policies built on top are deterministic given `(observation, seed,
//! faction, version)` by construction (plan section 6.3).

mod support;

use arena_bots::bot::BotKind;
use arena_bots::rng::bot_rng_key;
use arena_bots::rng::kind_tag as m6_kind_tag;
use arena_core::types::Faction;

// ---------------------------------------------------------------------
// R2 RNG namespace (plan 5.1 "reproducible seeded tie-break"; ADR-0015:
// kind-tag bytes 1-5 belong permanently to the frozen m6 kinds, R2
// policies take fresh bytes >= 6 on the SAME frozen D53 key shape)
// ---------------------------------------------------------------------

use arena_bots::r2::rng::{R2Rng, R2Stream, r2_rng_key};

#[test]
fn r2_key_layout_is_the_frozen_d53_shape_with_stream_tag() {
    // seed 0x0102030405060708, RES (faction tag 1), greedy-coordinated-r2
    // (stream tag 6): b"ABOT" || BOT_RNG_VERSION u32 LE || bot_seed u64 LE
    // || faction tag || stream tag.
    let key = r2_rng_key(
        0x0102_0304_0506_0708,
        Faction::Res,
        R2Stream::GreedyCoordinated,
    );
    assert_eq!(
        key,
        vec![
            b'A', b'B', b'O', b'T', // domain
            1, 0, 0, 0, // BOT_RNG_VERSION (frozen m6 value)
            8, 7, 6, 5, 4, 3, 2, 1, // bot_seed LE
            1, // faction: Res
            6, // stream: greedy-coordinated-r2
        ]
    );
    assert_eq!(key.len(), 18);
    // The idle diagnostic stream carries the highest tag, 12.
    assert_eq!(
        r2_rng_key(1, Faction::Enl, R2Stream::Idle).last(),
        Some(&12)
    );
}

#[test]
fn r2_stream_tags_are_exactly_6_to_12_and_disjoint_from_m6_kinds() {
    let tags: Vec<u8> = R2Stream::ALL.iter().map(|s| s.kind_tag()).collect();
    let mut sorted = tags.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(tags.len(), sorted.len(), "stream tags are distinct");
    assert_eq!(sorted, vec![6, 7, 8, 9, 10, 11, 12]);
    // No stream tag collides with any frozen m6 kind tag (1..=5).
    for kind in BotKind::ALL {
        assert!(
            !sorted.contains(&m6_kind_tag(kind)),
            "r2 namespace must stay disjoint from the m6 kind {kind:?}"
        );
    }
    // One registry, seven streams: the six plan-6.1 policies plus idle.
    assert_eq!(R2Stream::ALL.len(), 7);
}

#[test]
fn r2_streams_are_pairwise_disjoint_for_a_fixed_seed_and_faction() {
    let seed = 0x0707_0707_0000_00B2;
    // Keys are pairwise distinct across all streams AND all m6 kinds.
    let mut keys: Vec<Vec<u8>> = R2Stream::ALL
        .iter()
        .map(|&s| r2_rng_key(seed, Faction::Enl, s))
        .collect();
    for kind in BotKind::ALL {
        keys.push(bot_rng_key(seed, Faction::Enl, kind));
    }
    for (i, a) in keys.iter().enumerate() {
        for b in keys.iter().skip(i + 1) {
            assert_ne!(a, b, "two streams share a key");
        }
    }
    // And the drawn words themselves diverge: the first 8 bytes of every
    // stream's XOF output are distinct.
    let mut first_words: Vec<[u8; 8]> = R2Stream::ALL
        .iter()
        .map(|&s| {
            let mut rng = R2Rng::new(seed, Faction::Res, s);
            let mut buf = [0u8; 8];
            rng.fill(&mut buf);
            buf
        })
        .collect();
    let count = first_words.len();
    first_words.sort_unstable();
    first_words.dedup();
    assert_eq!(first_words.len(), count, "XOF outputs are disjoint");
}

#[test]
fn r2_rng_is_deterministic_and_seed_and_faction_sensitive() {
    let seed = 0x5EED_5EED_0000_0001;
    let draws = |bot_seed: u64, faction: Faction| -> Vec<u32> {
        let mut rng = R2Rng::new(bot_seed, faction, R2Stream::BuilderDefender);
        (0..32).map(|_| rng.uniform(1000)).collect()
    };
    // Determinism: identical construction draws an identical sequence.
    assert_eq!(draws(seed, Faction::Enl), draws(seed, Faction::Enl));
    // Seed sensitivity: a different bot_seed diverges somewhere in the
    // sequence.
    assert_ne!(
        draws(seed, Faction::Enl),
        draws(seed ^ 1, Faction::Enl),
        "bot_seed must key the stream"
    );
    // Faction keying: ENL and RES instances of the same policy draw
    // differently — this is what makes mirror spawn sets diverge (plan
    // 10.1 mirror gate).
    assert_ne!(
        draws(seed, Faction::Enl),
        draws(seed, Faction::Res),
        "mirror factions must not share a stream"
    );
}

// ---------------------------------------------------------------------
// Shared integer value model (plan 6.2 / 13.1): candidate generation and
// the five scorers as pure functions over the legal Observation and the
// public event history, integer scores only
// ---------------------------------------------------------------------

use arena_bots::r2::value::{
    Economy, FieldCandidate, ValueParams, attack_value, block_value, defense_value,
    field_candidates, field_value, key_rounds, link_order_rounds, rank_field_candidates,
    transfer_value, travel_rounds_to,
};
use arena_core::config::GameConfig;
use arena_core::event::{Event, EventKind};
use arena_core::types::{AgentId, Coord, Owner, PortalId};
use support::ObsBuilder;

const KM: i32 = 1_000;

/// An enemy-faction attack event on `portal` in `round`.
fn enemy_attack(seq: u64, round: u32, portal: u32, hp_after: u32) -> Event {
    Event {
        seq,
        round,
        kind: EventKind::PortalAttacked {
            faction: Faction::Res,
            agent_id: AgentId(9),
            portal: PortalId(portal),
            damage: 20,
            hp_after,
        },
    }
}

fn own_attack(seq: u64, round: u32, portal: u32) -> Event {
    Event {
        seq,
        round,
        kind: EventKind::PortalAttacked {
            faction: Faction::Enl,
            agent_id: AgentId(0),
            portal: PortalId(portal),
            damage: 20,
            hp_after: 80,
        },
    }
}

/// The hand-placed two-triangle fixture world (support::fixture_map
/// geometry) as a round-1 ENL observation: triangle A = (P0, P1, P2),
/// triangle B = (P3, P4, P5), both area2 = 144_000_000.
fn two_triangle_world(round: u32) -> ObsBuilder {
    let mut b = ObsBuilder::new(round);
    for (id, (x, y)) in [
        (0, (4, 4)),
        (1, (16, 4)),
        (2, (4, 16)),
        (3, (36, 4)),
        (4, (36, 16)),
        (5, (24, 16)),
    ] {
        b = b.portal(id, Coord::new(x * KM, y * KM), Owner::Neutral, 0);
    }
    b
}

fn candidate_of(candidates: &[FieldCandidate], a: u32, b: u32, c: u32) -> FieldCandidate {
    let mut want = [PortalId(a), PortalId(b), PortalId(c)];
    want.sort_unstable();
    *candidates
        .iter()
        .find(|cand| cand.vertices == want)
        .unwrap_or_else(|| panic!("candidate {a}/{b}/{c} missing"))
}

#[test]
fn field_candidates_find_viable_triangles_and_rank_by_area2() {
    let cfg = support::fixture_cfg();
    let params = ValueParams::default();
    let obs = two_triangle_world(1)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .build();
    let candidates = field_candidates(&obs, Faction::Enl, &cfg, &params);
    let triangle_a = candidate_of(&candidates, 0, 1, 2);
    let triangle_b = candidate_of(&candidates, 3, 4, 5);
    assert_eq!(triangle_a.area2, 144_000_000);
    assert_eq!(triangle_b.area2, 144_000_000);
    // Generation ranks area2 descending, ties by ascending triple.
    let areas: Vec<u64> = candidates.iter().map(|c| c.area2).collect();
    let mut sorted = areas.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(areas, sorted, "area2 descending");
}

#[test]
fn field_value_prefers_near_candidates_and_is_exact_integer_math() {
    let cfg = support::fixture_cfg();
    let params = ValueParams::default();
    // Agents standing on triangle A's vertices: travel 0, keys missing
    // (3 x hack cooldown = 9), all vertices neutral (3 waiting links),
    // horizon = min(100, 500) = 100 => 144_000_000 x 88.
    let obs = two_triangle_world(1)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .build();
    let candidates = field_candidates(&obs, Faction::Enl, &cfg, &params);
    let near = field_value(
        &obs,
        Faction::Enl,
        &cfg,
        &params,
        &candidate_of(&candidates, 0, 1, 2),
    );
    let far = field_value(
        &obs,
        Faction::Enl,
        &cfg,
        &params,
        &candidate_of(&candidates, 3, 4, 5),
    );
    assert_eq!(near, 144_000_000i64 * 88);
    assert!(near > far, "equal-area candidate nearer to the agents wins");
    // Direction (plan 13.1): a larger MOVE_BUDGET raises the far
    // candidate's value (its binding travel shrinks).
    let mut wide = cfg;
    wide.move_budget_cm = 40_000;
    let far_wide = field_value(
        &obs,
        Faction::Enl,
        &wide,
        &params,
        &candidate_of(&candidates, 3, 4, 5),
    );
    assert!(far_wide > far);
}

#[test]
fn field_value_rises_when_keys_are_held_and_vertices_are_owned() {
    let cfg = support::fixture_cfg();
    let params = ValueParams::default();
    let base = two_triangle_world(1)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20);
    let without_keys = base.clone().build();
    let with_keys = base
        .clone()
        .agent_keys(0, 0, 1)
        .agent_keys(1, 1, 1)
        .agent_keys(2, 2, 1)
        .build();
    let candidates = field_candidates(&without_keys, Faction::Enl, &cfg, &params);
    let a = candidate_of(&candidates, 0, 1, 2);
    assert_eq!(key_rounds(&without_keys, Faction::Enl, &cfg, &a), 9);
    assert_eq!(key_rounds(&with_keys, Faction::Enl, &cfg, &a), 0);
    assert!(
        field_value(&with_keys, Faction::Enl, &cfg, &params, &a)
            > field_value(&without_keys, Faction::Enl, &cfg, &params, &a)
    );
    // Direction: a longer HACK cooldown lowers a key-less candidate.
    let mut slow_hack = cfg;
    slow_hack.hack_cooldown_rounds = 9;
    assert_eq!(key_rounds(&without_keys, Faction::Enl, &slow_hack, &a), 27);
    assert!(
        field_value(&without_keys, Faction::Enl, &slow_hack, &params, &a)
            < field_value(&without_keys, Faction::Enl, &cfg, &params, &a)
    );
    // Link-order cost: own vertices remove the waiting links.
    let owned = ObsBuilder::new(1)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 100)
        .portal(1, Coord::new(16 * KM, 4 * KM), Owner::Enl, 100)
        .portal(2, Coord::new(4 * KM, 16 * KM), Owner::Enl, 100)
        .portal(3, Coord::new(36 * KM, 4 * KM), Owner::Neutral, 0)
        .portal(4, Coord::new(36 * KM, 16 * KM), Owner::Neutral, 0)
        .portal(5, Coord::new(24 * KM, 16 * KM), Owner::Neutral, 0)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .build();
    assert_eq!(link_order_rounds(&owned, Faction::Enl, &a), 0);
    assert_eq!(link_order_rounds(&without_keys, Faction::Enl, &a), 3);
    assert!(
        field_value(&owned, Faction::Enl, &cfg, &params, &a)
            > field_value(&without_keys, Faction::Enl, &cfg, &params, &a)
    );
}

#[test]
fn field_value_discounts_enemy_pressure_from_public_history_only() {
    let cfg = support::fixture_cfg();
    let params = ValueParams::default();
    let threatened = two_triangle_world(10)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .event(enemy_attack(0, 10, 0, 80)) // this round: within the window
        .build();
    let calm = two_triangle_world(10)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .build();
    let candidates = field_candidates(&calm, Faction::Enl, &cfg, &params);
    let a = candidate_of(&candidates, 0, 1, 2);
    let calm_value = field_value(&calm, Faction::Enl, &cfg, &params, &a);
    assert!(
        field_value(&threatened, Faction::Enl, &cfg, &params, &a) < calm_value,
        "an enemy attack on a vertex discounts the candidate"
    );
    // Exactly one risk-rounds block per threatened vertex (10 rounds here).
    assert_eq!(
        calm_value - field_value(&threatened, Faction::Enl, &cfg, &params, &a),
        144_000_000i64 * 10
    );
    // Own-faction attacks are not pressure, and stale enemy attacks fall
    // outside the window (round 10, window 8: events older than 8 rounds
    // do not count).
    let own_pressure = two_triangle_world(10)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .event(own_attack(0, 10, 0))
        .build();
    assert_eq!(
        field_value(&own_pressure, Faction::Enl, &cfg, &params, &a),
        calm_value
    );
    let stale = two_triangle_world(10)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .event(enemy_attack(0, 1, 0, 80)) // 9 rounds old: outside
        .build();
    assert_eq!(
        field_value(&stale, Faction::Enl, &cfg, &params, &a),
        calm_value
    );
}

/// A BIG far neutral triangle (area2 784_000_000, heavy setup) against a
/// SMALL own, keyed, agent-occupied triangle (area2 36_000_000, zero
/// setup) on a 120 km map whose MAX_LINK_DISTANCE (60_000) keeps the two
/// clusters mutually unreachable: exactly two candidates exist, and the
/// remaining-rounds horizon decides the ranking.
fn big_small_world() -> (GameConfig, ObsBuilder, ObsBuilder) {
    let mut cfg = support::fixture_cfg();
    cfg.map_width_cm = 120_000;
    cfg.map_height_cm = 120_000;
    cfg.max_link_distance_cm = 60_000;
    let mut early = ObsBuilder::new(1);
    let mut late = ObsBuilder::new(495);
    for (id, (x, y), owner, hp) in [
        (0u32, (2i32, 2i32), Owner::Neutral, 0),
        (1, (30, 2), Owner::Neutral, 0),
        (2, (2, 30), Owner::Neutral, 0),
        (3, (80, 80), Owner::Enl, 100),
        (4, (86, 80), Owner::Enl, 100),
        (5, (80, 86), Owner::Enl, 100),
    ] {
        early = early.portal(id, Coord::new(x * KM, y * KM), owner, hp);
        late = late.portal(id, Coord::new(x * KM, y * KM), owner, hp);
    }
    for b in [&mut early, &mut late] {
        *b = b
            .clone()
            .agent(0, Coord::new(80 * KM, 80 * KM), 20)
            .agent(1, Coord::new(86 * KM, 80 * KM), 20)
            .agent(2, Coord::new(80 * KM, 86 * KM), 20)
            .agent_keys(0, 3, 1)
            .agent_keys(1, 4, 1)
            .agent_keys(2, 5, 1);
    }
    (cfg, early, late)
}

#[test]
fn remaining_rounds_flip_the_big_vs_small_candidate_ranking() {
    let (cfg, early, late) = big_small_world();
    let params = ValueParams::default();
    let early_obs = early.build();
    let late_obs = late.build();
    for (label, obs) in [("early", &early_obs), ("late", &late_obs)] {
        let ranked = rank_field_candidates(obs, Faction::Enl, &cfg, &params);
        assert_eq!(ranked.len(), 2, "two viable candidates {label}");
        let top = ranked[0].1.vertices;
        if label == "early" {
            // Horizon 100: the big triangle's setup (travel 3 + keys 9 +
            // order 3 = 15 rounds) is affordable.
            assert_eq!(top, [PortalId(0), PortalId(1), PortalId(2)]);
        } else {
            // Horizon 6 (round 495 of 500): the big triangle's value is
            // negative; the ready small triangle wins.
            assert_eq!(top, [PortalId(3), PortalId(4), PortalId(5)]);
            assert!(ranked[0].0 > 0);
            assert!(ranked[1].0 < 0);
        }
    }
}

#[test]
fn equal_value_candidates_tie_break_ascending_deterministically() {
    let cfg = support::fixture_cfg();
    let params = ValueParams::default();
    // Agents on every portal vertex: the two fixture triangles have equal
    // area2 and identical setup, so their values tie exactly; the ranking
    // breaks ties by ascending vertex triple and is a pure function of the
    // observation (repeat call, identical result).
    let obs = two_triangle_world(1)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .agent(3, Coord::new(36 * KM, 4 * KM), 20)
        .agent(4, Coord::new(36 * KM, 16 * KM), 20)
        .agent(5, Coord::new(24 * KM, 16 * KM), 20)
        .build();
    let first = rank_field_candidates(&obs, Faction::Enl, &cfg, &params);
    let second = rank_field_candidates(&obs, Faction::Enl, &cfg, &params);
    assert_eq!(first, second);
    let index_of = |triple: [PortalId; 3]| {
        first
            .iter()
            .position(|(_, c)| c.vertices == triple)
            .unwrap_or_else(|| panic!("candidate {triple:?} missing"))
    };
    let index_a = index_of([PortalId(0), PortalId(1), PortalId(2)]);
    let index_b = index_of([PortalId(3), PortalId(4), PortalId(5)]);
    assert_eq!(first[index_a].0, first[index_b].0, "genuinely equal values");
    assert!(
        index_a < index_b,
        "equal-value ties break by ascending vertex triple"
    );
}

#[test]
fn economy_horizon_is_the_remaining_match_capped_by_parameters() {
    let cfg = support::fixture_cfg();
    let params = ValueParams::default();
    assert_eq!(
        Economy::for_round(&cfg, 1, &params, 10_000_000).horizon_rounds,
        100,
        "capped far below the full 500"
    );
    assert_eq!(
        Economy::for_round(&cfg, 490, &params, 10_000_000).horizon_rounds,
        11
    );
    assert_eq!(
        Economy::for_round(&cfg, 500, &params, 10_000_000).horizon_rounds,
        1
    );
    assert_eq!(
        Economy::for_round(&cfg, 600, &params, 10_000_000).horizon_rounds,
        1,
        "saturates after the final round"
    );
    assert_eq!(
        Economy::for_round(&cfg, 1, &params, 10_000_000).opportunity_per_round,
        10_000_000
    );
}

#[test]
fn attack_value_ranks_anchors_over_empty_portals_and_falls_with_cost() {
    let mut cfg = support::fixture_cfg();
    let mut params = ValueParams::default();
    params.capture_gain_share_permille = 0; // isolate the denial term
    let economy = Economy {
        horizon_rounds: 100,
        opportunity_per_round: 10_000_000,
    };
    let attack_world = |anchor_hp: u32, field_area2: u64, agent_at: Coord| {
        ObsBuilder::new(1)
            .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Res, anchor_hp)
            .portal(1, Coord::new(16 * KM, 4 * KM), Owner::Res, 100)
            .portal(2, Coord::new(4 * KM, 16 * KM), Owner::Res, 100)
            .portal(3, Coord::new(36 * KM, 36 * KM), Owner::Res, 100)
            .field([0, 1, 2], Faction::Res, field_area2)
            .agent(0, agent_at, 20)
            .build()
    };
    let obs = attack_world(100, 100_000_000, Coord::new(4 * KM, 4 * KM));
    // Exact integer: preventable 100_000_000 x 100, travel 0, attacks
    // ceil(100 / 20) = 5 rounds x 10_000_000.
    assert_eq!(
        attack_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(0)),
        100_000_000i64 * 100 - 5 * 10_000_000
    );
    assert!(
        attack_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(0))
            > attack_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(3)),
        "an enemy anchor outranks an empty enemy portal"
    );
    // Bigger anchored field => higher denial value.
    let heavy = attack_world(100, 200_000_000, Coord::new(4 * KM, 4 * KM));
    assert!(
        attack_value(&heavy, Faction::Enl, &cfg, &params, &economy, PortalId(0))
            > attack_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(0))
    );
    // Direction (plan 13.1): fewer HP (fewer attacks needed) and higher
    // ATTACK_DAMAGE both raise the value.
    let wounded = attack_world(40, 100_000_000, Coord::new(4 * KM, 4 * KM));
    assert!(
        attack_value(&wounded, Faction::Enl, &cfg, &params, &economy, PortalId(0))
            > attack_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(0))
    );
    cfg.attack_damage = 25;
    assert!(
        attack_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(0))
            > attack_value(
                &obs,
                Faction::Enl,
                &support::fixture_cfg(),
                &params,
                &economy,
                PortalId(0)
            ),
        "ATTACK_DAMAGE 25 needs only 4 attacks"
    );
    // Farther target ranks lower (travel rounds).
    let far_agent = attack_world(100, 100_000_000, Coord::new(36 * KM, 36 * KM));
    assert!(
        attack_value(
            &obs,
            Faction::Enl,
            &support::fixture_cfg(),
            &params,
            &economy,
            PortalId(0)
        ) > attack_value(
            &far_agent,
            Faction::Enl,
            &support::fixture_cfg(),
            &params,
            &economy,
            PortalId(0)
        )
    );
}

#[test]
fn attack_value_counts_capture_gain_of_the_best_new_own_field() {
    // No enemy fields: only the capture-gain term differs. MAX_LINK_DISTANCE
    // 30_000 keeps the isolated corner portal out of every triangle.
    let mut cfg = support::fixture_cfg();
    cfg.max_link_distance_cm = 30_000;
    let params = ValueParams::default(); // full capture share
    let economy = Economy {
        horizon_rounds: 100,
        opportunity_per_round: 1_000_000,
    };
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Res, 100)
        .portal(1, Coord::new(20 * KM, 4 * KM), Owner::Neutral, 0)
        .portal(2, Coord::new(4 * KM, 20 * KM), Owner::Neutral, 0)
        .portal(3, Coord::new(38 * KM, 38 * KM), Owner::Res, 100)
        .agent(0, Coord::new(4 * KM, 2 * KM), 20)
        .build();
    assert_eq!(travel_rounds_to(&obs, &cfg, PortalId(3)), 3);
    assert!(
        attack_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(0))
            > attack_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(3)),
        "capturing the portal that unlocks a big own field outranks the barren one"
    );
}

#[test]
fn defense_value_recharges_only_urgent_anchors_ranked_by_live_field_area2() {
    let cfg = support::fixture_cfg();
    let params = ValueParams::default();
    let economy = Economy {
        horizon_rounds: 100,
        opportunity_per_round: 10_000_000,
    };
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 40) // urgent: below half HP
        .portal(1, Coord::new(16 * KM, 4 * KM), Owner::Enl, 40) // urgent twin
        .portal(2, Coord::new(4 * KM, 16 * KM), Owner::Enl, 100) // healthy
        .portal(3, Coord::new(36 * KM, 4 * KM), Owner::Enl, 100) // healthy but attacked
        .field([0, 1, 2], Faction::Enl, 200_000_000)
        .field([0, 1, 3], Faction::Enl, 50_000_000)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .build();
    // P0 carries both fields (250M), P1 carries both too (250M): equal
    // live income; exact integer for P0: 250M x 100 - (1 major round +
    // 1 supply x 500 permille) x 10M.
    assert_eq!(
        defense_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(0)),
        250_000_000i64 * 100 - 10_000_000 - 5_000_000
    );
    assert_eq!(
        defense_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(0)),
        defense_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(1)),
        "equal live area2 and urgency rank equal"
    );
    // The healthy anchor with the same live fields is not urgent => zero.
    assert_eq!(
        defense_value(&obs, Faction::Enl, &cfg, &params, &economy, PortalId(2)),
        0
    );
    // A fresh enemy attack makes a full-HP anchor urgent (public history).
    let attacked = ObsBuilder::new(4)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 100)
        .field([0, 1, 2], Faction::Enl, 200_000_000)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .event(enemy_attack(0, 4, 0, 100))
        .build();
    let attacked_value = defense_value(
        &attacked,
        Faction::Enl,
        &cfg,
        &params,
        &economy,
        PortalId(0),
    );
    assert!(attacked_value > 0);
    // Direction: pricier Supply lowers the value; a farther defender too.
    let mut pricey = cfg;
    pricey.recharge_cost_supply = 5;
    assert!(
        defense_value(
            &attacked,
            Faction::Enl,
            &pricey,
            &params,
            &economy,
            PortalId(0)
        ) < attacked_value
    );
    let far = ObsBuilder::new(4)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 100)
        .field([0, 1, 2], Faction::Enl, 200_000_000)
        .agent(0, Coord::new(36 * KM, 36 * KM), 20)
        .event(enemy_attack(0, 4, 0, 100))
        .build();
    assert!(
        defense_value(&far, Faction::Enl, &cfg, &params, &economy, PortalId(0)) < attacked_value
    );
}

#[test]
fn transfer_value_scales_with_acceleration_and_field_area2() {
    let economy = Economy {
        horizon_rounds: 100,
        opportunity_per_round: 10_000_000,
    };
    let base = transfer_value(&economy, 100_000_000, 3, 0);
    assert_eq!(base, 300_000_000);
    assert!(transfer_value(&economy, 100_000_000, 5, 0) > base);
    assert!(transfer_value(&economy, 200_000_000, 3, 0) > base);
    // The sender's walk into TRANSFER_RANGE is deducted at the opportunity
    // rate.
    assert_eq!(
        transfer_value(&economy, 100_000_000, 3, 2),
        base - 20_000_000
    );
}

#[test]
fn block_value_scores_denied_enemy_candidates_minus_own_geometric_loss() {
    let cfg = support::fixture_cfg();
    let params = ValueParams::default();
    // No neutral portals: enemy candidates come only from the RES triangle
    // (P0, P1, P2), own candidates only from ENL portals, so each proposed
    // link's crossings are enumerable by hand.
    let obs = ObsBuilder::new(1)
        // The enemy candidate triangle, high on the left: area2 144_000_000.
        .portal(0, Coord::new(4 * KM, 30 * KM), Owner::Res, 100)
        .portal(1, Coord::new(16 * KM, 30 * KM), Owner::Res, 100)
        .portal(2, Coord::new(4 * KM, 18 * KM), Owner::Res, 100)
        // Anchors of the crossing link: the vertical segment x = 8,
        // y in [26, 34] cuts the enemy pair P0-P1 (y = 30, x in [4, 16])
        // and no own candidate segment (every other own portal sits at
        // x >= 30).
        .portal(6, Coord::new(8 * KM, 26 * KM), Owner::Enl, 100)
        .portal(7, Coord::new(8 * KM, 34 * KM), Owner::Enl, 100)
        // A small own candidate triangle top-right: (P8, P9, P10).
        .portal(8, Coord::new(34 * KM, 34 * KM), Owner::Enl, 100)
        .portal(9, Coord::new(38 * KM, 34 * KM), Owner::Enl, 100)
        .portal(10, Coord::new(38 * KM, 38 * KM), Owner::Enl, 100)
        // Anchors of the own-harm link: the vertical segment x = 36,
        // y in [32, 40] cuts the own pair P8-P9 (y = 34, x in [34, 38]).
        .portal(11, Coord::new(36 * KM, 32 * KM), Owner::Enl, 100)
        .portal(12, Coord::new(36 * KM, 40 * KM), Owner::Enl, 100)
        // Anchors of an empty link crossing nothing.
        .portal(13, Coord::new(30 * KM, 2 * KM), Owner::Enl, 100)
        .portal(14, Coord::new(34 * KM, 2 * KM), Owner::Enl, 100)
        .agent(0, Coord::new(8 * KM, 26 * KM), 20)
        .build();
    let crossing = block_value(&obs, Faction::Enl, &cfg, &params, PortalId(6), PortalId(7));
    let empty = block_value(
        &obs,
        Faction::Enl,
        &cfg,
        &params,
        PortalId(13),
        PortalId(14),
    );
    let own_harm = block_value(
        &obs,
        Faction::Enl,
        &cfg,
        &params,
        PortalId(11),
        PortalId(12),
    );
    // Exact integer: the only enemy candidate is denied for its full
    // horizon (144_000_000 x 100) at the default deny share 500 permille.
    assert_eq!(crossing, 7_200_000_000i64);
    assert_eq!(empty, 0, "a link crossing nothing denies nothing");
    assert!(
        own_harm < 0,
        "crossing our own candidates is a geometric loss"
    );
    assert!(crossing > empty);
    assert!(empty > own_harm);
}

#[test]
fn value_params_tag_round_trips_and_rejects_malformed_records() {
    let default = ValueParams::default();
    assert_eq!(ValueParams::from_tag(&default.tag()), Some(default));
    let mut tuned = default;
    tuned.horizon_cap_rounds = 60;
    tuned.deny_share_permille = 900;
    tuned.max_field_candidates = 8;
    assert_eq!(ValueParams::from_tag(&tuned.tag()), Some(tuned));
    assert_ne!(tuned.tag(), default.tag());
    // Strictly canonical: unknown keys, missing keys, reordered keys,
    // non-numeric values and junk all fail to parse.
    for malformed in [
        "",
        "horizon=100",
        "bogus=1;window=8;risk=10;defend=500;deny=500;ownloss=500;capture=500;supply=500;maxcand=24",
        "window=8;horizon=100;risk=10;defend=500;deny=500;ownloss=500;capture=500;supply=500;maxcand=24",
        "horizon=many;window=8;risk=10;defend=500;deny=500;ownloss=500;capture=500;supply=500;maxcand=24",
        "horizon=100;window=8;risk=10;defend=500;deny=500;ownloss=500;capture=500;supply=500;maxcand=24;junk=1",
        "horizon=100;;window=8;risk=10;defend=500;deny=500;ownloss=500;capture=500;supply=500;maxcand=24",
    ] {
        assert_eq!(
            ValueParams::from_tag(malformed),
            None,
            "malformed record {malformed:?} must not parse"
        );
    }
}

// ---------------------------------------------------------------------
// Role/task allocator with per-round target reservation (plan 6.3):
// one-shot uniqueness, the explicit coordinated-attack exception, and
// reinforcement through the same allocation path
// ---------------------------------------------------------------------

use arena_bots::r2::tasks::{Task, TaskBoard, TaskRequest, allocate};
use std::collections::BTreeMap;

fn request(task: Task, priority: i64) -> TaskRequest {
    TaskRequest {
        task,
        priority,
        assignee_capacity: 1,
    }
}

fn coordinated(task: Task, priority: i64, capacity: u32) -> TaskRequest {
    TaskRequest {
        task,
        priority,
        assignee_capacity: capacity,
    }
}

#[test]
fn one_shot_tasks_admit_exactly_one_assignee_per_round() {
    let capture = Task::Capture {
        portal: PortalId(4),
    };
    let mut board = TaskBoard::new(7);
    assert!(board.claim(AgentId(0), capture, 1));
    assert!(
        !board.claim(AgentId(1), capture, 1),
        "a one-shot reservation blocks a second claim within the round"
    );
    assert_eq!(board.assignees(&capture), &[AgentId(0)]);
    assert_eq!(board.remaining_capacity(&capture), 0);
    // A fresh board of the next round starts clean (reservations are
    // per-round).
    assert_eq!(TaskBoard::new(8).remaining_capacity(&capture), 0);
    assert!(!TaskBoard::new(8).is_assigned(&capture));
    assert_eq!(TaskBoard::new(8).round(), 8);
}

#[test]
fn coordinated_attacks_are_the_explicit_multi_assignee_exception() {
    let strike = Task::Attack {
        portal: PortalId(2),
    };
    let mut board = TaskBoard::new(1);
    assert!(board.claim(AgentId(0), strike, 2));
    assert!(
        board.claim(AgentId(1), strike, 2),
        "a coordinated attack admits its declared capacity"
    );
    assert!(
        !board.claim(AgentId(2), strike, 2),
        "but no agent beyond the capacity"
    );
    assert_eq!(board.assignees(&strike), &[AgentId(0), AgentId(1)]);
}

#[test]
fn allocate_grants_each_agent_one_task_by_priority_then_agent_id() {
    let capture_a = Task::Capture {
        portal: PortalId(0),
    };
    let capture_b = Task::Capture {
        portal: PortalId(1),
    };
    let mut requests = BTreeMap::new();
    requests.insert(
        AgentId(0),
        vec![request(capture_a, 10), request(capture_b, 5)],
    );
    requests.insert(
        AgentId(1),
        vec![request(capture_a, 10), request(capture_b, 9)],
    );
    let mut board = TaskBoard::new(1);
    let granted = allocate(&mut board, &[AgentId(0), AgentId(1)], &requests);
    // Agent 0 wins the contended capture (equal priority, lower id) and
    // agent 1 falls through to its next-ranked request — never a duplicate.
    assert_eq!(granted.get(&AgentId(0)), Some(&capture_a));
    assert_eq!(granted.get(&AgentId(1)), Some(&capture_b));
    assert_eq!(board.assignees(&capture_a), &[AgentId(0)]);
    // Priority dominates agent id: agent 1's higher-priority request wins
    // the contested task even though agent 0 asked too.
    let mut requests = BTreeMap::new();
    requests.insert(AgentId(0), vec![request(capture_a, 5)]);
    requests.insert(AgentId(1), vec![request(capture_a, 9)]);
    let granted = allocate(&mut TaskBoard::new(1), &[AgentId(0), AgentId(1)], &requests);
    assert_eq!(granted.get(&AgentId(1)), Some(&capture_a));
    assert!(granted.get(&AgentId(0)).is_none());
    // Coordinated capacity flows through allocate as well.
    let strike = Task::Attack {
        portal: PortalId(3),
    };
    let mut requests = BTreeMap::new();
    requests.insert(AgentId(0), vec![coordinated(strike, 10, 2)]);
    requests.insert(AgentId(1), vec![coordinated(strike, 9, 2)]);
    let mut board = TaskBoard::new(1);
    let granted = allocate(&mut board, &[AgentId(0), AgentId(1)], &requests);
    assert_eq!(granted.get(&AgentId(0)), Some(&strike));
    assert_eq!(granted.get(&AgentId(1)), Some(&strike));
}

#[test]
fn reinforcements_allocate_through_the_same_path_with_no_fixed_roles() {
    // Three initial agents with settled requests; a reinforcement agent
    // (higher id, appended by the engine) joins with its own requests.
    // The initial trio's grants are unchanged — allocation has no
    // "first three build" special case keyed on AgentId.
    let build = (0..3)
        .map(|portal| {
            request(
                Task::Capture {
                    portal: PortalId(portal),
                },
                10,
            )
        })
        .collect::<Vec<_>>();
    let mut trio_requests = BTreeMap::new();
    for (agent, requests) in [
        (AgentId(0), vec![build[0]]),
        (AgentId(1), vec![build[1]]),
        (AgentId(2), vec![build[2]]),
    ] {
        trio_requests.insert(agent, requests);
    }
    let trio_agents = vec![AgentId(0), AgentId(1), AgentId(2)];
    let trio_grants = allocate(&mut TaskBoard::new(1), &trio_agents, &trio_requests);

    let mut with_reinforcement = trio_requests.clone();
    with_reinforcement.insert(
        AgentId(3),
        vec![request(
            Task::KeyFarm {
                portal: PortalId(9),
            },
            8,
        )],
    );
    let mut agents = trio_agents.clone();
    agents.push(AgentId(3));
    let grants = allocate(&mut TaskBoard::new(1), &agents, &with_reinforcement);
    for (agent, task) in trio_grants.iter() {
        assert_eq!(
            grants.get(agent),
            Some(task),
            "reinforcement joining must not change the existing grants"
        );
    }
    assert_eq!(
        grants.get(&AgentId(3)),
        Some(&Task::KeyFarm {
            portal: PortalId(9)
        })
    );
    // And a reinforcement CAN take a build task when it is the best
    // request — roles are dynamic, not id-keyed.
    let mut requests = BTreeMap::new();
    requests.insert(
        AgentId(3),
        vec![request(
            Task::Capture {
                portal: PortalId(7),
            },
            10,
        )],
    );
    let grants = allocate(&mut TaskBoard::new(1), &[AgentId(3)], &requests);
    assert_eq!(
        grants.get(&AgentId(3)),
        Some(&Task::Capture {
            portal: PortalId(7)
        })
    );
}

#[test]
fn allocation_is_a_pure_deterministic_function_of_its_inputs() {
    let capture = Task::Capture {
        portal: PortalId(0),
    };
    let mut requests = BTreeMap::new();
    requests.insert(AgentId(1), vec![request(capture, 7)]);
    requests.insert(AgentId(0), vec![request(capture, 7)]);
    let first = allocate(&mut TaskBoard::new(3), &[AgentId(0), AgentId(1)], &requests);
    let second = allocate(&mut TaskBoard::new(3), &[AgentId(0), AgentId(1)], &requests);
    assert_eq!(first, second);
    assert_eq!(first.len(), 1, "exactly one assignee for one-shot work");
    // Agent order in the slice never matters (requests carry the ids).
    let reordered = allocate(&mut TaskBoard::new(3), &[AgentId(1), AgentId(0)], &requests);
    assert_eq!(first, reordered);
}

// ---------------------------------------------------------------------
// Limited opponent model (plan 5.1/6.2): threat and enemy build zones
// from the LEGAL public event history only — obs.events — never enemy
// private state (unreachable from the Observation type by construction)
// ---------------------------------------------------------------------

use arena_bots::r2::opponent::{OpponentModel, OpponentParams};

fn link_created(seq: u64, round: u32, faction: Faction, a: u32, b: u32) -> Event {
    Event {
        seq,
        round,
        kind: EventKind::LinkCreated {
            faction,
            agent_id: AgentId(9),
            link: arena_core::types::LinkId::new(PortalId(a), PortalId(b)).expect("distinct"),
        },
    }
}

#[test]
fn opponent_model_reads_pressure_from_public_history_only() {
    let params = OpponentParams::default();
    let mut model = OpponentModel::new(params, Faction::Res);
    // Round 10 observation: the enemy hit P0 twice (rounds 9 and 10, both
    // inside the default window) and P1 once, long ago (round 1).
    let obs = ObsBuilder::new(10)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 60)
        .portal(1, Coord::new(16 * KM, 4 * KM), Owner::Enl, 90)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .event(enemy_attack(0, 9, 0, 80))
        .event(enemy_attack(1, 10, 0, 60))
        .event(enemy_attack(2, 1, 1, 90))
        .build();
    model.update(&obs);
    assert_eq!(model.pressure(&obs, PortalId(0)), 2, "two in-window hits");
    assert_eq!(
        model.pressure(&obs, PortalId(1)),
        0,
        "round-1 hit is outside"
    );
    assert_eq!(model.pressure(&obs, PortalId(2)), 0, "never attacked");
    assert_eq!(
        model.top_threat(&obs),
        Some((PortalId(0), 2)),
        "the hottest portal wins, ascending-id ties"
    );
    assert!(model.any_threat(&obs));
    // Determinism and idempotence: re-observing the same history changes
    // nothing (each event is consumed once by seq).
    model.update(&obs);
    assert_eq!(model.pressure(&obs, PortalId(0)), 2);
}

#[test]
fn a_calm_enemy_yields_zero_threat_and_zones() {
    let mut model = OpponentModel::new(OpponentParams::default(), Faction::Res);
    let obs = ObsBuilder::new(10)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 100)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .event(own_attack(0, 10, 0))
        .build();
    model.update(&obs);
    assert_eq!(
        model.pressure(&obs, PortalId(0)),
        0,
        "own attacks are not pressure"
    );
    assert!(!model.any_threat(&obs));
    assert_eq!(model.top_threat(&obs), None);
    assert_eq!(model.build_zone_heat(&obs, PortalId(0)), 0);
}

#[test]
fn opponent_model_tracks_enemy_build_zones_and_updates_incrementally() {
    let params = OpponentParams::default();
    let mut model = OpponentModel::new(params, Faction::Res);
    // Round 4: the enemy linked (P2, P3) this round and (P3, P4) two rounds
    // ago; both are inside the default zone window.
    let round4 = ObsBuilder::new(4)
        .portal(2, Coord::new(4 * KM, 16 * KM), Owner::Res, 100)
        .portal(3, Coord::new(36 * KM, 4 * KM), Owner::Res, 100)
        .portal(4, Coord::new(36 * KM, 16 * KM), Owner::Res, 100)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .event(link_created(0, 2, Faction::Res, 3, 4))
        .event(link_created(1, 4, Faction::Res, 2, 3))
        .build();
    model.update(&round4);
    assert_eq!(
        model.build_zone_heat(&round4, PortalId(3)),
        2,
        "shared endpoint"
    );
    assert_eq!(model.build_zone_heat(&round4, PortalId(2)), 1);
    // Own links never count.
    let with_own = ObsBuilder::new(4)
        .portal(5, Coord::new(24 * KM, 16 * KM), Owner::Enl, 100)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .event(link_created(2, 4, Faction::Enl, 0, 5))
        .build();
    model.update(&with_own);
    assert_eq!(model.build_zone_heat(&with_own, PortalId(5)), 0);
    // Both windows slide: at round 40 every round-4 fact is stale.
    let round40 = ObsBuilder::new(40)
        .portal(2, Coord::new(4 * KM, 16 * KM), Owner::Res, 100)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .build();
    model.update(&round40);
    assert_eq!(model.build_zone_heat(&round40, PortalId(3)), 0);
    assert_eq!(model.top_threat(&round40), None);
    // Repeated rounds with no new events stay deterministic.
    model.update(&round40);
    assert_eq!(model.build_zone_heat(&round40, PortalId(2)), 0);
}

#[test]
fn opponent_pressure_caps_at_the_recorded_bound_and_params_round_trip() {
    let mut params = OpponentParams::default();
    params.pressure_cap_per_portal = 3;
    let mut model = OpponentModel::new(params, Faction::Res);
    let mut builder = ObsBuilder::new(10)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 40)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20);
    for seq in 0..9u64 {
        builder = builder.event(enemy_attack(seq, 10, 0, 80));
    }
    let obs = builder.build();
    model.update(&obs);
    assert_eq!(model.pressure(&obs, PortalId(0)), 3, "saturated at the cap");
    // The recorded parameter set round-trips canonically.
    let default = OpponentParams::default();
    assert_eq!(OpponentParams::from_tag(&default.tag()), Some(default));
    let mut tuned = default;
    tuned.threat_window_rounds = 12;
    tuned.zone_window_rounds = 30;
    tuned.pressure_cap_per_portal = 7;
    assert_eq!(OpponentParams::from_tag(&tuned.tag()), Some(tuned));
    assert_ne!(tuned.tag(), default.tag());
    assert_eq!(OpponentParams::from_tag("window=8"), None);
    assert_eq!(OpponentParams::from_tag("bad"), None);
}

// ---------------------------------------------------------------------
// Coordinated builder core (plan 5.1/6.1): ranked candidates with seeded
// near-equal top pick, station-to-agent assignment with per-round
// reservation (no two agents on one one-shot station — the GreedyField
// 73.6% CaptureTargetNotNeutral root cause), FieldPlanner-semantics
// replanning, and claim-spread fallback
// ---------------------------------------------------------------------

use arena_bots::r2::build::{BuilderCore, CoreParams};
use arena_core::map::MapDefinition;
use arena_core::observe::Observation;
use arena_core::types::Command;

fn core_params() -> CoreParams {
    CoreParams::default()
}

fn spawned_core(map: &MapDefinition) -> BuilderCore {
    let cfg = support::fixture_cfg();
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x5EED_0000_0000_00B2,
        R2Stream::GreedyCoordinated,
        &cfg,
        core_params(),
    );
    let roster: Vec<AgentId> = (0..3).map(AgentId).collect();
    core.choose_spawns(&cfg, map, &roster);
    core
}

/// The first round after spawning on the two-triangle fixture map: the
/// three roster agents stand on the chosen triangle's vertices, and the
/// allocator grants three DISTINCT capture stations — one agent per
/// vertex, never two agents on one neutral portal.
#[test]
fn builder_core_assigns_distinct_stations_to_distinct_agents() {
    let map = support::fixture_map();
    let mut core = spawned_core(&map);
    let spawns = core.last_spawns().to_vec();
    let triangle = core.active().expect("spawn seeds the plan").vertices;
    let mut by_agent: Vec<(AgentId, Coord)> = spawns.clone();
    by_agent.sort_by_key(|(agent, _)| agent.0);
    assert_eq!(by_agent.len(), 3);
    for (_, coord) in &by_agent {
        // The station is a plan vertex, within the recorded jitter.
        assert!(
            triangle.iter().any(|v| {
                let position = map.portal(*v).expect("map portal").position;
                arena_core::geometry::dist2(position, *coord) <= 1_000i64 * 1_000
            }),
            "every spawn stands within the jitter of a plan vertex"
        );
    }
    // Round 1 observation: everyone at their spawn, everything neutral.
    let mut b = ObsBuilder::new(1);
    for portal in map.portals() {
        b = b.portal(portal.id.0, portal.position, portal.owner, portal.hp);
    }
    for (_index, (agent, coord)) in by_agent.iter().enumerate() {
        b = b.agent(agent.0, *coord, 20);
    }
    let obs = b.build();
    core.refresh(&obs);
    let agents: Vec<AgentId> = by_agent.iter().map(|(a, _)| *a).collect();
    let requests = core.build_requests(&obs);
    let grants = arena_bots::r2::tasks::allocate(
        &mut arena_bots::r2::tasks::TaskBoard::new(1),
        &agents,
        &requests,
    );
    let mut captured_portals: Vec<PortalId> = grants
        .values()
        .filter_map(|task| match task {
            Task::Capture { portal } => Some(*portal),
            _ => None,
        })
        .collect();
    captured_portals.sort_unstable();
    captured_portals.dedup();
    assert_eq!(captured_portals.len(), 3, "three distinct capture stations");
    assert_eq!(
        captured_portals,
        triangle.to_vec(),
        "the stations are exactly the plan vertices (sorted)"
    );
    // And the emitted commands are structurally legal: every agent at most
    // one Major Action and one Move, captures only when standing on the
    // neutral portal.
    let commands = core.execute(&obs, &grants);
    assert_eq!(commands.len(), 3);
    let mut built_by_agent: BTreeMap<AgentId, Vec<Command>> = BTreeMap::new();
    for (agent, list) in commands {
        let built = list.build();
        assert!(
            !built.is_empty(),
            "agent {} is never idle on round 1",
            agent.0
        );
        assert!(built.len() <= 10);
        built_by_agent.insert(agent, built);
    }
    // An agent standing at its own neutral station emits exactly one
    // Capture (a Major Action) and no Move.
    let me0 = &obs.own_agents[0];
    let station = obs
        .portals
        .iter()
        .find(|p| arena_core::geometry::dist2(p.position, me0.position) <= 5_000i64 * 5_000)
        .expect("spawned within operation range of a portal")
        .id;
    assert_eq!(
        built_by_agent.get(&me0.id).expect("agent 0 has commands"),
        &vec![Command::Capture { portal: station }],
        "standing on the neutral station: capture, no move"
    );
}

/// Replan semantics mirror the frozen FieldPlanner: a plan whose three own
/// links all exist is COMPLETE and the next-best viable triangle takes
/// over; a plan with an enemy-owned vertex is INVALID and likewise
/// replans. Triangle A completed -> triangle B becomes active.
#[test]
fn builder_core_replans_on_complete_and_invalid() {
    let map = support::fixture_map();
    let mut core = spawned_core(&map);
    let triangle = core.active().expect("spawn plan").vertices;
    let (a, b, c) = (triangle[0], triangle[1], triangle[2]);

    // COMPLETE: all three own links of the spawn triangle exist.
    let mut complete = ObsBuilder::new(5);
    for portal in map.portals() {
        let owner = if triangle.contains(&portal.id) {
            Owner::Enl
        } else {
            Owner::Neutral
        };
        complete = complete.portal(
            portal.id.0,
            portal.position,
            owner,
            if owner == Owner::Enl { 100 } else { 0 },
        );
    }
    complete = complete
        .link(a.0, b.0, Faction::Enl)
        .link(a.0, c.0, Faction::Enl)
        .link(b.0, c.0, Faction::Enl)
        .agent(0, map.portal(a).expect("map").position, 20)
        .agent(1, map.portal(b).expect("map").position, 20)
        .agent(2, map.portal(c).expect("map").position, 20);
    let obs = complete.build();
    core.refresh(&obs);
    let next = core.active().expect("replanned").vertices;
    assert_ne!(
        next, triangle,
        "a completed plan is replaced (its pairs are all linked, so the search cannot re-pick it)"
    );

    // INVALID: a vertex of the active plan belongs to the enemy.
    let mut invalid = ObsBuilder::new(6);
    for portal in map.portals() {
        let owner = if next.contains(&portal.id) {
            if portal.id == next[0] {
                Owner::Res
            } else {
                Owner::Enl
            }
        } else {
            Owner::Neutral
        };
        invalid = invalid.portal(
            portal.id.0,
            portal.position,
            owner,
            if owner == Owner::Neutral { 0 } else { 100 },
        );
    }
    invalid = invalid
        .agent(0, map.portal(next[1]).expect("map").position, 20)
        .agent(1, map.portal(next[2]).expect("map").position, 20)
        .agent(2, map.portal(next[0]).expect("map").position, 20);
    let obs = invalid.build();
    core.refresh(&obs);
    let after = core
        .active()
        .expect("invalid plans replan immediately")
        .vertices;
    assert_ne!(after, next, "the enemy-held triangle is abandoned");
}

/// Fallback (no viable triangle anywhere — collinear portals): the
/// allocator still spreads one-shot claims so agents work DIFFERENT
/// portals, and the request set stays deterministic.
#[test]
fn builder_core_fallback_spreads_claims_deterministically() {
    let collinear = MapDefinition::new(
        [0, 1, 2, 3]
            .map(|i| arena_core::types::Portal {
                id: PortalId(i),
                position: Coord::new(4_000 + i as i32 * 8_000, 4_000),
                owner: Owner::Neutral,
                hp: 0,
            })
            .to_vec(),
    );
    let cfg = support::fixture_cfg();
    let mut core = BuilderCore::new(
        Faction::Enl,
        0x00C0_FFEE_0000_0001,
        R2Stream::GreedyCoordinated,
        &cfg,
        core_params(),
    );
    let roster: Vec<AgentId> = (0..3).map(AgentId).collect();
    core.choose_spawns(&cfg, &collinear, &roster);
    assert!(core.active().is_none(), "no viable triangle on a line");

    let mut b = ObsBuilder::new(1);
    for portal in collinear.portals() {
        b = b.portal(portal.id.0, portal.position, portal.owner, portal.hp);
    }
    for (agent, (_, coord)) in roster.iter().zip(core.last_spawns().to_vec()) {
        b = b.agent(agent.0, coord, 20);
    }
    let obs = b.build();
    core.refresh(&obs);
    assert!(core.active().is_none());
    let requests = core.fallback_requests(&obs);
    let grants = arena_bots::r2::tasks::allocate(
        &mut arena_bots::r2::tasks::TaskBoard::new(1),
        &roster,
        &requests,
    );
    let mut stations: Vec<PortalId> = grants
        .values()
        .filter_map(|task| task.target_portal())
        .collect();
    stations.sort_unstable();
    stations.dedup();
    assert_eq!(
        stations.len(),
        grants.len().min(4),
        "claims spread over distinct portals"
    );
    // Determinism: an identical second pass grants the identical tasks.
    let again = arena_bots::r2::tasks::allocate(
        &mut arena_bots::r2::tasks::TaskBoard::new(1),
        &roster,
        &core.fallback_requests(&obs),
    );
    assert_eq!(grants, again);
    // Fallback stations emit commands too (never unintended idle).
    let commands = core.execute(&obs, &grants);
    assert_eq!(commands.len(), grants.len());
}

/// The core round (refresh -> requests -> allocate -> execute) is a
/// deterministic function of (observation, seed, faction, parameters), and
/// mirror factions draw DIFFERENT plans at spawn time (faction-keyed
/// near-equal sampling — plan 10.1 mirror gate at the decision level).
#[test]
fn builder_core_rounds_are_deterministic_and_mirror_factions_diverge() {
    let map = support::fixture_map();
    let round_obs = |core: &BuilderCore| -> Observation {
        let mut b = ObsBuilder::new(1);
        for portal in map.portals() {
            b = b.portal(portal.id.0, portal.position, portal.owner, portal.hp);
        }
        for (agent, coord) in [(0usize, AgentId(0)), (1, AgentId(1)), (2, AgentId(2))]
            .iter()
            .map(|(i, a)| (*a, core.last_spawns()[*i].1))
        {
            b = b.agent(agent.0, coord, 20);
        }
        b.build()
    };
    let run = |faction: Faction, seed: u64| -> Vec<Vec<Command>> {
        let cfg = support::fixture_cfg();
        let mut core = BuilderCore::new(
            faction,
            seed,
            R2Stream::GreedyCoordinated,
            &cfg,
            core_params(),
        );
        let roster: Vec<AgentId> = (0..3).map(AgentId).collect();
        core.choose_spawns(&cfg, &map, &roster);
        let obs = round_obs(&core);
        core.refresh(&obs);
        let requests = core.build_requests(&obs);
        let grants = arena_bots::r2::tasks::allocate(
            &mut arena_bots::r2::tasks::TaskBoard::new(1),
            &roster,
            &requests,
        );
        let mut commands = core.execute(&obs, &grants);
        roster
            .iter()
            .map(|agent| {
                commands
                    .remove(agent)
                    .map(|list| list.build())
                    .unwrap_or_default()
            })
            .collect()
    };
    assert_eq!(
        run(Faction::Enl, 42),
        run(Faction::Enl, 42),
        "deterministic"
    );
    assert_ne!(
        run(Faction::Enl, 7),
        run(Faction::Res, 7), /* same map, same seed */
        "faction-keyed"
    );

    // CoreParams round-trip and identity sensitivity of the record.
    let params = CoreParams::default();
    assert_eq!(CoreParams::from_tag(&params.tag()), Some(params));
    let mut tuned = params;
    tuned.near_equal_permille = 250;
    tuned.replan_retry_rounds = 40;
    tuned.fallback_priority = 5;
    assert_eq!(CoreParams::from_tag(&tuned.tag()), Some(tuned));
    assert_ne!(tuned.tag(), params.tag());
    assert_eq!(CoreParams::from_tag("nearequal=100"), None);
    assert_eq!(CoreParams::from_tag(""), None);
}

// ---------------------------------------------------------------------
// Seeded tie-break and near-equal candidate sampling (plan 5.1/6.3):
// deterministic given the stream, seed-sensitive across streams
// ---------------------------------------------------------------------

use arena_bots::r2::sample::{pick_near_equal, pick_uniform};

#[test]
fn sampling_is_deterministic_given_the_stream_position() {
    let items = [10i64, 10, 10, 9];
    let mut rng = R2Rng::new(0x5EED_0000_00B2, Faction::Enl, R2Stream::BlockerBuilder);
    let first = pick_near_equal(&items, |v| *v, 1000, &mut rng);
    // Rewinding is impossible, so determinism is pinned by re-keying the
    // identical stream: same key, same draw sequence, same pick.
    let mut replay = R2Rng::new(0x5EED_0000_00B2, Faction::Enl, R2Stream::BlockerBuilder);
    let second = pick_near_equal(&items, |v| *v, 1000, &mut replay);
    assert_eq!(first, second);
    // Uniform pick over the same stream state is deterministic too.
    let mut a = R2Rng::new(1, Faction::Enl, R2Stream::Idle);
    let mut b = R2Rng::new(1, Faction::Enl, R2Stream::Idle);
    assert_eq!(
        pick_uniform(&[7, 8, 9], &mut a),
        pick_uniform(&[7, 8, 9], &mut b)
    );
    assert_eq!(pick_uniform::<i64>(&[], &mut a), None);
    assert_eq!(pick_near_equal(&[], |v: &i64| *v, 1000, &mut a), None);
}

#[test]
fn near_equal_permille_bounds_the_eligible_band_exactly() {
    // Best value 1000; permille 50 admits 950 (floor = 1000 - 50), so both
    // items are eligible; permille 49 truncates to a floor of 951 and
    // excludes 950.
    let mut rng = R2Rng::new(2, Faction::Res, R2Stream::GreedyCoordinated);
    for _ in 0..64 {
        let pick = pick_near_equal(&[1000i64, 950], |v| *v, 50, &mut rng);
        assert!(
            pick == Some(&1000) || pick == Some(&950),
            "both band members are eligible at permille 50"
        );
    }
    let mut strict = R2Rng::new(2, Faction::Res, R2Stream::GreedyCoordinated);
    for _ in 0..64 {
        assert_eq!(
            pick_near_equal(&[1000i64, 950], |v| *v, 49, &mut strict),
            Some(&1000),
            "950 must be outside a 49-permille band of 1000"
        );
    }
    // permille 0 keeps only exact ties with the best.
    let mut exact = R2Rng::new(3, Faction::Enl, R2Stream::Idle);
    for _ in 0..64 {
        let pick = pick_near_equal(&[1000i64, 1000, 999], |v| *v, 0, &mut exact);
        assert!(pick == Some(&1000));
    }
    // A non-positive best value has no proportional band: only exact ties.
    let mut negative = R2Rng::new(4, Faction::Enl, R2Stream::Idle);
    for _ in 0..64 {
        let pick = pick_near_equal(&[-10i64, -10, -11], |v| *v, 1000, &mut negative);
        assert_ne!(pick, Some(&-11));
    }
}

#[test]
fn near_equal_sampling_is_seed_sensitive_across_streams() {
    // Two exactly equal candidates: across many streams and seeds both
    // must be picked sometimes — a single fixed pick would mean the draw
    // ignored its key. The candidates are equal by value, so the picked
    // ITEM is identified by address, not by value.
    let items = [100i64, 100];
    let mut picks_of_first = 0u32;
    let mut total = 0u32;
    for stream in R2Stream::ALL {
        for seed in 0..16u64 {
            let mut rng = R2Rng::new(seed, Faction::Enl, stream);
            let pick = pick_near_equal(&items, |v| *v, 1000, &mut rng).expect("non-empty");
            if std::ptr::eq(pick, &items[0]) {
                picks_of_first += 1;
            }
            total += 1;
        }
    }
    assert_eq!(total, 112);
    assert!(
        picks_of_first > 0 && picks_of_first < total,
        "both near-equal candidates must be reachable (first picked {picks_of_first}/{total})"
    );
    // A strictly better candidate dominates every stream when alone in
    // its band.
    for stream in R2Stream::ALL {
        for seed in 0..8u64 {
            let mut rng = R2Rng::new(seed, Faction::Res, stream);
            assert_eq!(
                pick_near_equal(&[100i64, 10], |v| *v, 100, &mut rng),
                Some(&100)
            );
        }
    }
}

// ---------------------------------------------------------------------
// Per-policy decision seams (plan 6.1): each policy's role overlay over
// the shared core, on hand-built observations — every output
// structurally legal (<= 1 Major Action, <= 1 Move, ledger-gated hacks)
// ---------------------------------------------------------------------

use arena_bots::policy::{PolicyId, make_policy_player};
use arena_bots::r2::adaptive_hybrid_r2::{AdaptiveHybridParams, AdaptiveHybridR2, RoleMix};
use arena_bots::r2::blocker_builder::{BlockerBuilder, BlockerBuilderParams};
use arena_bots::r2::builder_defender::{BuilderDefender, BuilderDefenderParams};
use arena_bots::r2::builder_disruptor::{BuilderDisruptor, BuilderDisruptorParams};
use arena_bots::r2::greedy_coordinated::GreedyCoordinated;
use arena_bots::r2::idle::IdlePolicy;
use arena_bots::r2::{PolicyPlayer, R2Policy};
use arena_core::harness::AgentSubmission;

/// Runs one policy for one round on `obs` and returns the per-agent
/// command vectors (agent id ascending; missing = explicit PASS).
fn round_of(policy: &mut dyn R2Policy, obs: &Observation) -> BTreeMap<AgentId, Vec<Command>> {
    let submissions: Vec<AgentSubmission> = policy.submit_round(obs);
    let mut out = BTreeMap::new();
    for submission in submissions {
        // Structural legality (plan 6.3 / D27): <= 10 commands, <= 1
        // Major Action, <= 1 Move, and the shape aux -> major -> move.
        assert!(submission.commands.len() <= 10);
        assert!(submission.commands.iter().filter(|c| c.is_major()).count() <= 1);
        assert!(
            submission
                .commands
                .iter()
                .filter(|c| matches!(c, Command::Move { .. }))
                .count()
                <= 1
        );
        let major_positions: Vec<usize> = submission
            .commands
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_major())
            .map(|(i, _)| i)
            .collect();
        let move_positions: Vec<usize> = submission
            .commands
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c, Command::Move { .. }))
            .map(|(i, _)| i)
            .collect();
        for &major_at in &major_positions {
            for aux_at in 0..major_at {
                assert!(!submission.commands[aux_at].is_major());
            }
        }
        if let (Some(&major_at), Some(&move_at)) = (major_positions.first(), move_positions.first())
        {
            assert!(major_at < move_at, "the Major Action precedes the Move");
        }
        out.insert(submission.agent_id, submission.commands);
    }
    out
}

/// builder-defender-r2 recharges exactly the value-selected anchor — an
/// urgent own anchor carrying a live field wins the allocator against
/// building — and never recharges when nothing is defense-urgent.
#[test]
fn defender_recharges_the_value_selected_anchor_only() {
    let cfg = support::fixture_cfg();
    let world = |p0_hp: u32| {
        ObsBuilder::new(1)
            .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, p0_hp)
            .portal(1, Coord::new(16 * KM, 4 * KM), Owner::Enl, 100)
            .portal(2, Coord::new(4 * KM, 16 * KM), Owner::Enl, 100)
            .portal(3, Coord::new(36 * KM, 4 * KM), Owner::Neutral, 0)
            .portal(4, Coord::new(36 * KM, 16 * KM), Owner::Neutral, 0)
            .portal(5, Coord::new(24 * KM, 16 * KM), Owner::Neutral, 0)
            .field([0, 1, 2], Faction::Enl, 1_000_000_000)
            .agent(0, Coord::new(4 * KM, 4 * KM), 20)
            .agent_keys(0, 0, 1)
            .agent(1, Coord::new(16 * KM, 4 * KM), 20)
            .agent_keys(1, 1, 1)
            .agent(2, Coord::new(4 * KM, 16 * KM), 20)
            .agent_keys(2, 2, 1)
            .build()
    };
    // Urgent anchor (hp 40 < the recorded 500 permille share) carrying a
    // huge live field: the agent standing on it recharges it, exactly
    // once, as its single Major Action.
    let mut defender = BuilderDefender::new(
        Faction::Enl,
        0x0DEF_0000_0000_0001,
        &cfg,
        BuilderDefenderParams::default(),
    );
    let commands = round_of(&mut defender, &world(40));
    assert_eq!(
        commands.get(&AgentId(0)).map(|c| c.as_slice()),
        Some(
            &[Command::Recharge {
                portal: PortalId(0)
            }][..]
        ),
        "the value-selected urgent anchor is recharged by its standing agent"
    );
    let recharges = commands
        .values()
        .flatten()
        .filter(|c| matches!(c, Command::Recharge { .. }))
        .count();
    assert_eq!(recharges, 1, "exactly one recharge (the defense cap)");
    // Healthy anchors: no defense value anywhere -> pure building.
    let mut calm = BuilderDefender::new(
        Faction::Enl,
        0x0DEF_0000_0000_0001,
        &cfg,
        BuilderDefenderParams::default(),
    );
    let commands = round_of(&mut calm, &world(100));
    assert!(
        commands
            .values()
            .flatten()
            .all(|c| !matches!(c, Command::Recharge { .. })),
        "value says build: no recharge"
    );
}

/// builder-disruptor-r2 sends ceil(hp / ATTACK_DAMAGE) attackers — the
/// coordinated-attack capacity — never "everyone", and no attack at all
/// when the target carries no value.
#[test]
fn disruptor_sends_exactly_the_needed_attackers() {
    let cfg = support::fixture_cfg();
    let world = |hp: u32| {
        let mut b = ObsBuilder::new(1)
            .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Res, hp)
            .portal(1, Coord::new(16 * KM, 4 * KM), Owner::Res, 100)
            .portal(2, Coord::new(4 * KM, 16 * KM), Owner::Res, 100)
            .field([0, 1, 2], Faction::Res, 100_000_000);
        for agent in 0..6u32 {
            b = b
                .agent(agent, Coord::new(4_500, 4_500), 20)
                .agent_keys(agent, 1, 1); // keys of an enemy portal are inert
        }
        b.build()
    };
    // hp 100 at ATTACK_DAMAGE 20 -> 5 attackers out of 6 agents.
    let mut disruptor = BuilderDisruptor::new(
        Faction::Enl,
        0x0155_0000_0000_0001,
        &cfg,
        BuilderDisruptorParams::default(),
    );
    let commands = round_of(&mut disruptor, &world(100));
    let attacks = commands
        .values()
        .flatten()
        .filter(|c| matches!(c, Command::Attack))
        .count();
    assert_eq!(attacks, 5, "ceil(100/20) attackers, not everyone");
    // hp 40 -> 2 attackers (also below the 600-permille share of six).
    let mut disruptor = BuilderDisruptor::new(
        Faction::Enl,
        0x0155_0000_0000_0001,
        &cfg,
        BuilderDisruptorParams::default(),
    );
    let commands = round_of(&mut disruptor, &world(40));
    let attacks = commands
        .values()
        .flatten()
        .filter(|c| matches!(c, Command::Attack))
        .count();
    assert_eq!(attacks, 2, "ceil(40/20) attackers");
    // A barren far enemy portal carries no attack value: no attack.
    let barren = ObsBuilder::new(1)
        .portal(9, Coord::new(36 * KM, 36 * KM), Owner::Res, 100)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .build();
    let mut disruptor = BuilderDisruptor::new(
        Faction::Enl,
        0x0155_0000_0000_0001,
        &cfg,
        BuilderDisruptorParams::default(),
    );
    let commands = round_of(&mut disruptor, &barren);
    assert!(
        commands
            .values()
            .flatten()
            .all(|c| !matches!(c, Command::Attack)),
        "no value -> no attack"
    );
}

/// logistics-builder-r2 routes a spare key (plus Supply headroom) to the
/// stationed linker with a drop-free Transfer: the receiver stands by,
/// the sender's payload respects TRANSFER_RANGE, its reserve and the
/// receiver's MAX_SUPPLY headroom.
#[test]
fn logistics_emits_a_drop_free_transfer_to_the_stationed_linker() {
    let cfg = support::fixture_cfg();
    // The only viable own candidate: (P0, P1, P2), all ENL, no links. Edge
    // (P0, P1): agent 1 stands 4 km from the source P0 without P1's key;
    // agent 0 (the holder) is 3 km from agent 1 but 7 km from P0 — out of
    // the source's operation range, so no direct link is possible and the
    // transport overlay is the only way to fire this edge this round.
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 100)
        .portal(1, Coord::new(20 * KM, 4 * KM), Owner::Enl, 100)
        .portal(2, Coord::new(4 * KM, 20 * KM), Owner::Enl, 100)
        .agent(0, Coord::new(11 * KM, 4 * KM), 20)
        .agent_keys(0, 1, 2) // holds TWO keys of P1
        .agent(1, Coord::new(8 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 20 * KM), 20)
        .agent_keys(2, 2, 1)
        .build();
    assert_eq!(cfg.transfer_range_cm, 5_000);
    assert_eq!(cfg.portal_operation_range_cm, 5_000);
    assert!(
        arena_core::geometry::dist2(Coord::new(11 * KM, 4 * KM), Coord::new(8 * KM, 4 * KM))
            <= 5_000i64 * 5_000
    );
    let mut logistics = arena_bots::r2::logistics_builder::LogisticsBuilder::new(
        Faction::Enl,
        0x106_0000_0000_0001,
        &cfg,
        arena_bots::r2::logistics_builder::LogisticsBuilderParams::default(),
    );
    let commands = round_of(&mut logistics, &obs);
    let transfer = commands
        .get(&AgentId(0))
        .expect("the holder delivers")
        .clone();
    assert_eq!(
        transfer,
        vec![Command::Transfer {
            target_agent: AgentId(1),
            // receiver supply 20, MAX_SUPPLY 100 -> headroom 80; sender
            // keeps the recorded reserve of 5 -> 15.
            supply: 15,
            keys: BTreeMap::from([(PortalId(1), 1)]),
        }]
    );
    // The receiver stands by (no Move) while the delivery is in flight.
    assert!(
        !commands.contains_key(&AgentId(1)),
        "the stationed linker waits for the key"
    );
}

/// blocker-builder-r2 picks the own link crossing the enemy's best
/// candidate edge — and only when the deny value clears the threshold.
#[test]
fn blocker_picks_the_link_crossing_the_enemy_candidate() {
    let cfg = support::fixture_cfg();
    let world = |keyed: bool| {
        let mut b = ObsBuilder::new(1)
            // The enemy candidate triangle: near-half of the map, so its
            // denied income dominates every own build option.
            .portal(0, Coord::new(2 * KM, 2 * KM), Owner::Res, 100)
            .portal(1, Coord::new(38 * KM, 2 * KM), Owner::Res, 100)
            .portal(2, Coord::new(2 * KM, 38 * KM), Owner::Res, 100)
            // Our crossing anchors: the vertical x = 8 segment cuts the
            // enemy pair P0-P1 (y = 30).
            .portal(6, Coord::new(8 * KM, 26 * KM), Owner::Enl, 100)
            .portal(7, Coord::new(8 * KM, 34 * KM), Owner::Enl, 100)
            // A small own scoring triangle, top-right.
            .portal(8, Coord::new(34 * KM, 34 * KM), Owner::Enl, 100)
            .portal(9, Coord::new(38 * KM, 34 * KM), Owner::Enl, 100)
            .portal(10, Coord::new(38 * KM, 38 * KM), Owner::Enl, 100)
            .agent(0, Coord::new(8 * KM, 26 * KM), 20)
            .agent(1, Coord::new(38 * KM, 38 * KM), 20)
            .agent(2, Coord::new(34 * KM, 34 * KM), 20);
        if keyed {
            b = b.agent_keys(0, 7, 1); // standing on P6 with P7's key
        }
        b.build()
    };
    let mut blocker = BlockerBuilder::new(
        Faction::Enl,
        0x8_0C_0000_0000_0001,
        &cfg,
        BlockerBuilderParams::default(),
    );
    let commands = round_of(&mut blocker, &world(true));
    assert_eq!(
        commands.get(&AgentId(0)).map(|c| c.as_slice()),
        Some(
            &[Command::Link {
                source_portal: PortalId(6),
                target_portal: PortalId(7),
            }][..]
        ),
        "the deny link crossing the enemy's best candidate edge fires"
    );
    // Without the key the same link is not emittable: no deny link.
    let mut blocker = BlockerBuilder::new(
        Faction::Enl,
        0x8_0C_0000_0000_0001,
        &cfg,
        BlockerBuilderParams::default(),
    );
    let commands = round_of(&mut blocker, &world(false));
    assert!(commands.get(&AgentId(0)).is_none_or(|c| {
        !c.iter()
            .any(|command| matches!(command, Command::Link { .. }))
    }));
}

/// adaptive-hybrid-r2 re-assigns roles by phase / public score / public
/// threat — never by AgentId: the same world flips between pure build,
/// defense and disruption as only those inputs change.
#[test]
fn adaptive_switches_roles_by_phase_score_and_threat() {
    let cfg = support::fixture_cfg();
    let world = |round: u32, threat: bool, enl_score: u128, res_score: u128| {
        let mut b = ObsBuilder::new(round)
            .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Enl, 40)
            .portal(1, Coord::new(16 * KM, 4 * KM), Owner::Enl, 100)
            .portal(2, Coord::new(4 * KM, 16 * KM), Owner::Enl, 100)
            .portal(3, Coord::new(36 * KM, 4 * KM), Owner::Res, 100)
            .portal(4, Coord::new(36 * KM, 16 * KM), Owner::Res, 100)
            .portal(5, Coord::new(24 * KM, 16 * KM), Owner::Res, 100)
            .field([0, 1, 2], Faction::Enl, 1_000_000_000)
            .field([3, 4, 5], Faction::Res, 100_000_000)
            .agent(0, Coord::new(4 * KM, 4 * KM), 20)
            .agent_keys(0, 0, 1)
            .agent(1, Coord::new(16 * KM, 4 * KM), 20)
            .agent_keys(1, 1, 1)
            .agent(2, Coord::new(4 * KM, 16 * KM), 20)
            .agent_keys(2, 2, 1);
        if threat {
            b = b.event(enemy_attack(0, round, 0, 40));
        }
        let mut obs = b.build();
        obs.scores.insert(Faction::Enl, enl_score);
        obs.scores.insert(Faction::Res, res_score);
        obs
    };
    // Each scenario drives a FRESH instance: the opponent model accumulates
    // public history by design, and these scenarios assert the switching
    // inputs in isolation.
    let mut fresh = || {
        AdaptiveHybridR2::new(
            Faction::Enl,
            0x0A0A_0000_0000_0001,
            &cfg,
            AdaptiveHybridParams::default(),
        )
    };
    // Phase input: round 1 of 500 (2 permille) is the opening.
    assert_eq!(fresh().role_mix(&world(1, false, 0, 0)), RoleMix::Opening);
    // Mid-game, tied public scores, no threat: pure build.
    assert_eq!(
        fresh().role_mix(&world(200, false, 100, 100)),
        RoleMix::MidBuild
    );
    // The same round under public threat switches to defense.
    assert_eq!(
        fresh().role_mix(&world(200, true, 100, 100)),
        RoleMix::MidDefend
    );
    // A recorded lead share switches to disruption.
    assert_eq!(
        fresh().role_mix(&world(200, false, 700, 300)),
        RoleMix::MidDisrupt
    );
    // The endgame returns to pure build.
    assert_eq!(
        fresh().role_mix(&world(430, false, 700, 300)),
        RoleMix::Endgame
    );

    // The decisions follow: threat -> the urgent anchor is recharged.
    let mut policy = AdaptiveHybridR2::new(
        Faction::Enl,
        0x0A0A_0000_0000_0001,
        &cfg,
        AdaptiveHybridParams::default(),
    );
    let commands = round_of(&mut policy, &world(200, true, 100, 100));
    assert!(
        commands
            .get(&AgentId(0))
            .expect("agent 0 acts")
            .contains(&Command::Recharge {
                portal: PortalId(0)
            })
    );
    // Tied, calm mid-game: building only (no recharge, no attack).
    let mut policy = AdaptiveHybridR2::new(
        Faction::Enl,
        0x0A0A_0000_0000_0001,
        &cfg,
        AdaptiveHybridParams::default(),
    );
    let commands = round_of(&mut policy, &world(200, false, 100, 100));
    assert!(
        commands
            .values()
            .flatten()
            .all(|c| { !matches!(c, Command::Recharge { .. }) && !matches!(c, Command::Attack) })
    );
    // Leading mid-game: the strike mass is staged at the value-selected
    // target (P3 — the enemy anchor with the largest capture gain, not
    // merely the nearest one) and fires.
    let mut strike_world = world(200, false, 700, 300);
    for (index, agent) in strike_world.own_agents.iter_mut().enumerate() {
        agent.position = Coord::new(36_500 + index as i32 * 10, 4_500);
    }
    let mut policy = AdaptiveHybridR2::new(
        Faction::Enl,
        0x0A0A_0000_0000_0001,
        &cfg,
        AdaptiveHybridParams::default(),
    );
    let commands = round_of(&mut policy, &strike_world);
    assert!(
        commands
            .values()
            .flatten()
            .any(|c| matches!(c, Command::Attack)),
        "leading mid-game disrupts the enemy anchor"
    );
}

/// greedy-coordinated-r2 through the era-agnostic PolicyPlayer: spawns
/// validate and rounds are structurally legal; the diagnostic idle
/// policy never submits a command.
#[test]
fn policy_player_builds_r2_policies_and_idle_passes() {
    let cfg = support::fixture_cfg();
    let map = support::fixture_map();
    let roster: Vec<AgentId> = (0..3).map(AgentId).collect();
    for spec in arena_bots::policy::r2_specs() {
        let mut player = make_policy_player(&spec, Faction::Enl, 0x0BB2_0000_0000_0001, &cfg, &map);
        let spawns = player.choose_spawns(&cfg, &map, &roster);
        support::validate_bot_spawns(&cfg, Faction::Enl, &map, &spawns);
        assert_eq!(spawns.len(), roster.len());
    }
    // The idle policy: legal spawns, explicit PASS every round.
    let mut idle = IdlePolicy;
    let spawns = idle.choose_spawns(&cfg, &map, &roster);
    support::validate_bot_spawns(&cfg, Faction::Enl, &map, &spawns);
    let obs = ObsBuilder::new(1)
        .portal(0, Coord::new(4 * KM, 4 * KM), Owner::Neutral, 0)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .build();
    assert!(idle.submit_round(&obs).is_empty());
    // The greedy-coordinated core round through the player adapter is the
    // same decision function as the policy type itself.
    let obs = two_triangle_world(1)
        .agent(0, Coord::new(4 * KM, 4 * KM), 20)
        .agent(1, Coord::new(16 * KM, 4 * KM), 20)
        .agent(2, Coord::new(4 * KM, 16 * KM), 20)
        .build();
    let mut via_player = make_policy_player(
        &PolicyId::GreedyCoordinatedR2.r2_spec(),
        Faction::Enl,
        7,
        &cfg,
        &map,
    );
    let mut direct = GreedyCoordinated::new(
        Faction::Enl,
        7,
        &cfg,
        arena_bots::r2::build::CoreParams::default(),
    );
    assert_eq!(
        via_player.submit_round(&obs),
        direct.submit_round(&obs),
        "PolicyPlayer delegates 1:1 to the R2 policy"
    );
    // The Player enum never leaks the Bot interface for R2 policies.
    let PolicyPlayer::R2(_) = make_policy_player(
        &PolicyId::AdaptiveHybridR2.r2_spec(),
        Faction::Res,
        1,
        &cfg,
        &map,
    ) else {
        panic!("R2 specs construct R2 players");
    };
}

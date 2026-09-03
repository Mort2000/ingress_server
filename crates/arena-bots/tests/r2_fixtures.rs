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

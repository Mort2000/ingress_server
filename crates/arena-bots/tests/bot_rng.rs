//! Bot RNG gates (checklist 1-2): domain separation, stream identity, a
//! golden vector, and the frozen uniform mapping shared with arena-core.

use arena_bots::bot::BotKind;
use arena_bots::rng::{BOT_RNG_VERSION, BotRng, bot_rng_key};
use arena_core::types::Faction;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The raw first words of one pinned key (independent of `uniform`).
fn raw_words(bot_seed: u64, faction: Faction, kind: BotKind, count: usize) -> Vec<u8> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&bot_rng_key(bot_seed, faction, kind));
    let mut reader = hasher.finalize_xof();
    let mut out = vec![0u8; count];
    reader.fill(&mut out);
    out
}

// Checklist 1: different (bot_seed, faction, kind) keys give different
// streams; the same key gives the identical stream.
#[test]
fn domain_separation_and_stream_identity() {
    let base = raw_words(42, Faction::Enl, BotKind::Random, 32);
    // Same key: identical stream.
    assert_eq!(raw_words(42, Faction::Enl, BotKind::Random, 32), base);
    // Every key component matters.
    assert_ne!(raw_words(43, Faction::Enl, BotKind::Random, 32), base);
    assert_ne!(raw_words(42, Faction::Res, BotKind::Random, 32), base);
    assert_ne!(raw_words(42, Faction::Enl, BotKind::Hybrid, 32), base);
    // And the two bots of one match never share a stream even with one
    // shared bot_seed: the faction tag differs.
    let enl = raw_words(7, Faction::Enl, BotKind::GreedyField, 32);
    let res = raw_words(7, Faction::Res, BotKind::GreedyField, 32);
    assert_ne!(enl, res);
}

// Checklist 1: golden vector pinning the first 8 words (32 bytes) of one
// pinned key — bumps to BOT_RNG_VERSION or the key layout must be conscious
// rule-version changes.
#[test]
fn golden_vector_pins_first_eight_words() {
    assert_eq!(BOT_RNG_VERSION, 1);
    let golden = raw_words(
        0x0123_4567_89AB_CDEF,
        Faction::Res,
        BotKind::NearestPortal,
        32,
    );
    assert_eq!(
        hex(&golden),
        "59a42032c0a6cc7dd222fd8a6465ac6aafea8ed1dfab6e758aeaabad3db9d785"
    );
}

// Checklist 2: uniform(n) stays in [0, n) over long draws of every range
// shape used by the bots.
#[test]
fn uniform_stays_in_range() {
    let mut rng = BotRng::new(1, Faction::Enl, BotKind::Random);
    for n in [1u32, 2, 3, 4, 5, 7, 8, 120, 600_001] {
        for _ in 0..200 {
            assert!(rng.uniform(n) < n);
        }
    }
}

// Checklist 2: n == 1 always answers 0 and consumes exactly one word (the
// frozen loop accepts every word for n = 1); a rejection would show up as
// desynchronized stream positions between the two assertions below.
#[test]
fn uniform_n1_consumes_exactly_one_word() {
    let mut rng = BotRng::new(9, Faction::Res, BotKind::Random);
    let raw = raw_words(9, Faction::Res, BotKind::Random, 20);
    assert_eq!(rng.uniform(1), 0);
    assert_eq!(rng.uniform(1), 0);
    // After two one-word draws, the stream must sit at byte offset 8.
    let mut next = [0u8; 4];
    rng.fill(&mut next);
    assert_eq!(next.to_vec(), raw[8..12].to_vec());
}

// Checklist 2: n = 2 divides 2^32 exactly, so the frozen mapping accepts
// EVERY word without rejection — the whole stream stays word-synchronized
// (a biased `% n`-style change or a rejection here would break the offset).
#[test]
fn uniform_n2_never_rejects_and_uses_every_word() {
    let mut rng = BotRng::new(1234, Faction::Enl, BotKind::Hybrid);
    let raw = raw_words(1234, Faction::Enl, BotKind::Hybrid, 16);
    let d0 = rng.uniform(2);
    let d1 = rng.uniform(2);
    let d2 = rng.uniform(2);
    let d3 = rng.uniform(2);
    let word = |offset: usize| u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
    assert_eq!(d0, word(0) % 2);
    assert_eq!(d1, word(4) % 2);
    assert_eq!(d2, word(8) % 2);
    assert_eq!(d3, word(12) % 2);
}

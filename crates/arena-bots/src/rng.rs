//! Bot randomness (D53): each faction-bot owns one keyed BLAKE3 XOF stream,
//! mirroring the AMAP pre-match-tooling pattern (D12). The stream is keyed
//! with the match's `bot_seed` (derived by the runner, recorded in the
//! MatchRecord), the faction, and the bot kind, so the two bots of a match
//! and the same bot across matches never share draws. Words are drawn
//! sequentially through the FROZEN rejection-sampling loop shared with
//! arena-core (`uniform_from_words`) — never `RandomOracle`, never a wall
//! clock. Draw ORDER (spawns first, then round draws in submission order) is
//! part of each bot's frozen, fixed-version definition: with the match setup
//! (seeds, config, map, both bot algorithms) fixed, a bot is deterministic.

use arena_core::rng::uniform_from_words;
use arena_core::types::Faction;

use crate::bot::BotKind;

/// Version of the bot RNG key encoding (D53); bumping it changes every bot
/// draw and therefore every bot's frozen behavior.
pub const BOT_RNG_VERSION: u32 = 1;

/// Domain label of the bot RNG stream (ASCII `ABOT`).
pub const BOT_RNG_DOMAIN: &[u8; 4] = b"ABOT";

/// Builds the canonical key of one faction-bot's stream:
/// `b"ABOT" || u32 BOT_RNG_VERSION || u64 bot_seed || u8 faction_tag ||
/// u8 kind_tag` (fixed-width LE integers, concatenated without padding — D2
/// conventions; faction/kind tags follow the D51 0=Enl/1=Res style).
pub fn bot_rng_key(bot_seed: u64, faction: Faction, kind: BotKind) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 4 + 8 + 1 + 1);
    key.extend_from_slice(BOT_RNG_DOMAIN);
    key.extend_from_slice(&BOT_RNG_VERSION.to_le_bytes());
    key.extend_from_slice(&bot_seed.to_le_bytes());
    key.push(faction_tag(faction));
    key.push(kind_tag(kind));
    key
}

/// Faction tag of the bot RNG key (0=Enl, 1=Res; D51 convention).
pub fn faction_tag(faction: Faction) -> u8 {
    match faction {
        Faction::Enl => 0,
        Faction::Res => 1,
    }
}

/// Kind tag of the bot RNG key, frozen in `BotKind` declaration order
/// starting at 1 (a 0 tag would extend the domain label).
pub fn kind_tag(kind: BotKind) -> u8 {
    match kind {
        BotKind::Random => 1,
        BotKind::NearestPortal => 2,
        BotKind::GreedyField => 3,
        BotKind::DestroyAnchor => 4,
        BotKind::Hybrid => 5,
    }
}

/// One faction-bot's sequential uniform source. Constructed once per bot;
/// `uniform` draws successive 4-byte LE words through the frozen rejection
/// loop, so rejections advance the stream exactly like the runtime oracle's.
#[derive(Debug)]
pub struct BotRng {
    reader: blake3::OutputReader,
}

impl BotRng {
    /// Keys the stream for `(bot_seed, faction, kind)` (D53).
    pub fn new(bot_seed: u64, faction: Faction, kind: BotKind) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&bot_rng_key(bot_seed, faction, kind));
        BotRng {
            reader: hasher.finalize_xof(),
        }
    }

    /// Unbiased uniform integer in `[0, n_exclusive)` from this bot's stream.
    ///
    /// # Panics
    ///
    /// `n_exclusive == 0` is a programmer error and panics (D11 contract of
    /// the shared frozen loop); `n_exclusive == 1` consumes one word and
    /// returns 0.
    pub fn uniform(&mut self, n_exclusive: u32) -> u32 {
        uniform_from_words(&mut self.reader, n_exclusive)
    }

    /// Reads the next `out.len()` raw XOF bytes of the stream (test
    /// visibility into the exact word sequence; draws only through
    /// [`BotRng::uniform`] in bot logic).
    pub fn fill(&mut self, out: &mut [u8]) {
        self.reader.fill(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_layout_is_the_frozen_canonical_shape() {
        // seed 0x0102030405060708, RES (tag 1), GreedyField (tag 3).
        let key = bot_rng_key(0x0102_0304_0506_0708, Faction::Res, BotKind::GreedyField);
        assert_eq!(
            key,
            vec![
                b'A', b'B', b'O', b'T', // domain
                1, 0, 0, 0, // BOT_RNG_VERSION
                8, 7, 6, 5, 4, 3, 2, 1, // bot_seed LE
                1, // faction: Res
                3, // kind: GreedyField
            ]
        );
        assert_eq!(key.len(), 18);
    }

    #[test]
    fn kind_tags_are_distinct_and_nonzero() {
        let kinds = [
            BotKind::Random,
            BotKind::NearestPortal,
            BotKind::GreedyField,
            BotKind::DestroyAnchor,
            BotKind::Hybrid,
        ];
        let tags: Vec<u8> = kinds.iter().map(|k| kind_tag(*k)).collect();
        let mut sorted = tags.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(tags.len(), sorted.len(), "tags are distinct");
        assert!(tags.iter().all(|t| *t > 0), "no tag extends the domain");
    }
}

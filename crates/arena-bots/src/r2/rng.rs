//! R2 policy RNG namespace (B2, `docs/ai-balance-plan.md` sections 5.1 and
//! 6.3; ADR-0015). R2 policies draw their tie-break randomness through
//! the SAME frozen D53 key shape as the m6 baselines —
//! `b"ABOT" || u32 BOT_RNG_VERSION || u64 bot_seed || u8 faction_tag ||
//! u8 kind_tag`, fixed-width LE integers — but with FRESH stream kind-tag
//! bytes 6..=12, so every R2 stream is disjoint from the frozen m6 kind
//! streams (bytes 1..=5) and from every other R2 policy's stream. The
//! faction tag keys the two mirror factions to different streams, which is
//! exactly what makes two instances of one policy on a symmetric map
//! choose different spawn sets (plan section 10.1, mirror gate).
//!
//! The kind-tag namespace here is FROZEN at its first use: byte 6 belongs
//! permanently to `greedy-coordinated-r2`, …, 11 to `adaptive-hybrid-r2`
//! (plan section 6.1 declaration order), 12 to the diagnostic idle policy.
//! Future policies append new bytes >= 13; existing bytes are never
//! reassigned (version semantics are additive, ADR-0015).

use arena_core::rng::uniform_from_words;
use arena_core::types::Faction;

use crate::rng::{BOT_RNG_DOMAIN, BOT_RNG_VERSION, faction_tag};

/// One R2 policy's RNG stream identity: the six plan-6.1 policies, the
/// diagnostic idle opponent, and the B3 canonical ablation arm
/// (ADR-0017). Declaration order freezes the kind-tag bytes 6..=13.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum R2Stream {
    /// `greedy-coordinated-r2` (kind tag 6).
    GreedyCoordinated,
    /// `builder-defender-r2` (kind tag 7).
    BuilderDefender,
    /// `builder-disruptor-r2` (kind tag 8).
    BuilderDisruptor,
    /// `logistics-builder-r2` (kind tag 9).
    LogisticsBuilder,
    /// `blocker-builder-r2` (kind tag 10).
    BlockerBuilder,
    /// `adaptive-hybrid-r2` (kind tag 11).
    AdaptiveHybrid,
    /// The diagnostic idle policy (kind tag 12) — a first-class recorded
    /// opponent for the plan-10.1 idle gate, never a competitive policy.
    Idle,
    /// The diagnostic `greedy-coordinated-canonical` ablation arm (kind
    /// tag 13, ADR-0017) — the r2 core configured for canonical
    /// deterministic spawns; the first B3 stream under the "append bytes
    /// >= 13" freeze rule.
    GreedyCoordinatedCanonical,
}

impl R2Stream {
    /// The full registry, kind-tag declaration order.
    pub const ALL: [R2Stream; 8] = [
        R2Stream::GreedyCoordinated,
        R2Stream::BuilderDefender,
        R2Stream::BuilderDisruptor,
        R2Stream::LogisticsBuilder,
        R2Stream::BlockerBuilder,
        R2Stream::AdaptiveHybrid,
        R2Stream::Idle,
        R2Stream::GreedyCoordinatedCanonical,
    ];

    /// Stable lowercase tag — the plan section 6.1 policy tag of the policy
    /// drawing from this stream.
    pub fn tag(self) -> &'static str {
        match self {
            R2Stream::GreedyCoordinated => "greedy-coordinated-r2",
            R2Stream::BuilderDefender => "builder-defender-r2",
            R2Stream::BuilderDisruptor => "builder-disruptor-r2",
            R2Stream::LogisticsBuilder => "logistics-builder-r2",
            R2Stream::BlockerBuilder => "blocker-builder-r2",
            R2Stream::AdaptiveHybrid => "adaptive-hybrid-r2",
            R2Stream::Idle => "idle",
            R2Stream::GreedyCoordinatedCanonical => "greedy-coordinated-canonical",
        }
    }

    /// Parses [`R2Stream::tag`] back.
    pub fn from_tag(tag: &str) -> Option<R2Stream> {
        match tag {
            "greedy-coordinated-r2" => Some(R2Stream::GreedyCoordinated),
            "builder-defender-r2" => Some(R2Stream::BuilderDefender),
            "builder-disruptor-r2" => Some(R2Stream::BuilderDisruptor),
            "logistics-builder-r2" => Some(R2Stream::LogisticsBuilder),
            "blocker-builder-r2" => Some(R2Stream::BlockerBuilder),
            "adaptive-hybrid-r2" => Some(R2Stream::AdaptiveHybrid),
            "idle" => Some(R2Stream::Idle),
            "greedy-coordinated-canonical" => Some(R2Stream::GreedyCoordinatedCanonical),
            _ => None,
        }
    }

    /// The stream's kind-tag byte in the frozen D53 key shape (6..=13,
    /// frozen in declaration order).
    pub fn kind_tag(self) -> u8 {
        match self {
            R2Stream::GreedyCoordinated => 6,
            R2Stream::BuilderDefender => 7,
            R2Stream::BuilderDisruptor => 8,
            R2Stream::LogisticsBuilder => 9,
            R2Stream::BlockerBuilder => 10,
            R2Stream::AdaptiveHybrid => 11,
            R2Stream::Idle => 12,
            R2Stream::GreedyCoordinatedCanonical => 13,
        }
    }
}

/// Builds the canonical key of one faction's R2 policy stream: the frozen
/// D53 shape `b"ABOT" || u32 BOT_RNG_VERSION || u64 bot_seed || u8
/// faction_tag || u8 stream kind_tag` — the SAME domain, version and
/// faction bytes as the m6 keys (reused verbatim from [`crate::rng`]),
/// with the stream byte selecting the disjoint R2 namespace.
pub fn r2_rng_key(bot_seed: u64, faction: Faction, stream: R2Stream) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 4 + 8 + 1 + 1);
    key.extend_from_slice(BOT_RNG_DOMAIN);
    key.extend_from_slice(&BOT_RNG_VERSION.to_le_bytes());
    key.extend_from_slice(&bot_seed.to_le_bytes());
    key.push(faction_tag(faction));
    key.push(stream.kind_tag());
    key
}

/// One faction-policy's sequential uniform source (the R2 counterpart of
/// the frozen [`crate::rng::BotRng`]). Constructed once per policy
/// instance; `uniform` draws successive 4-byte LE words through the frozen
/// rejection-sampling loop shared with arena-core, so rejections advance
/// the stream exactly like every other draw site.
#[derive(Debug)]
pub struct R2Rng {
    reader: blake3::OutputReader,
}

impl R2Rng {
    /// Keys the stream for `(bot_seed, faction, stream)`.
    pub fn new(bot_seed: u64, faction: Faction, stream: R2Stream) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&r2_rng_key(bot_seed, faction, stream));
        R2Rng {
            reader: hasher.finalize_xof(),
        }
    }

    /// Unbiased uniform integer in `[0, n_exclusive)` from this policy's
    /// stream.
    ///
    /// # Panics
    ///
    /// `n_exclusive == 0` is a programmer error and panics (D11 contract
    /// of the shared frozen loop); `n_exclusive == 1` consumes one word and
    /// returns 0.
    pub fn uniform(&mut self, n_exclusive: u32) -> u32 {
        uniform_from_words(&mut self.reader, n_exclusive)
    }

    /// Reads the next `out.len()` raw XOF bytes of the stream (test
    /// visibility into the exact word sequence; policy logic draws only
    /// through [`R2Rng::uniform`]).
    pub fn fill(&mut self, out: &mut [u8]) {
        self.reader.fill(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_tags_round_trip_and_carry_the_plan_61_names() {
        for stream in R2Stream::ALL {
            assert_eq!(R2Stream::from_tag(stream.tag()), Some(stream));
        }
        assert_eq!(R2Stream::from_tag("greedy-coordinated"), None);
        // Declaration order is the kind-tag order 6..=13 (the B3 canonical
        // ablation arm appended byte 13, ADR-0017).
        let tags: Vec<u8> = R2Stream::ALL.iter().map(|s| s.kind_tag()).collect();
        assert_eq!(tags, vec![6, 7, 8, 9, 10, 11, 12, 13]);
    }
}

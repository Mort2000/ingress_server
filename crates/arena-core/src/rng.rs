//! Domain-separated BLAKE3 random oracle (R-RNG-01 / ADR-0011).
//!
//! The oracle has NO mutable cursor: every query re-hashes its address from
//! scratch, so the order and number of other queries can never influence a
//! result. The address encoding, rejection sampling, and golden vectors are
//! FROZEN as part of the rule version.
//!
//! Canonical address encoding (all little-endian, no padding):
//! `"ARNG"` (4 ASCII bytes) || u32 rng_version || u64 game_seed ||
//! u32 domain_tag || u32 index_count || index_count × u64 index.

use crate::types::{AgentId, Coord, PortalId};

/// Version of the runtime RNG address encoding; bumping it changes every
/// derived random value and therefore the rule version.
pub const RNG_VERSION: u32 = 1;

/// Random-usage domains with stable tags.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RngDomain {
    Hack,
    Reinforcement,
}

impl RngDomain {
    /// Frozen domain tag used in the canonical address encoding.
    pub fn tag(self) -> u32 {
        match self {
            RngDomain::Hack => 1,
            RngDomain::Reinforcement => 2,
        }
    }
}

/// Reinforcement coordinate axis; discriminants are frozen stable indices.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X = 0,
    Y = 1,
}

/// Immutable random oracle over the match's `game_seed`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RandomOracle {
    game_seed: u64,
}

impl RandomOracle {
    pub fn new(game_seed: u64) -> Self {
        RandomOracle { game_seed }
    }

    pub fn game_seed(&self) -> u64 {
        self.game_seed
    }

    /// Builds the canonical address for `domain` over the given stable
    /// indices. Index order is fixed per domain by the typed helpers below.
    pub fn address(&self, domain: RngDomain, indices: &[u64]) -> Vec<u8> {
        let mut out = Vec::with_capacity(24 + 8 * indices.len());
        out.extend_from_slice(b"ARNG");
        out.extend_from_slice(&RNG_VERSION.to_le_bytes());
        out.extend_from_slice(&self.game_seed.to_le_bytes());
        out.extend_from_slice(&domain.tag().to_le_bytes());
        out.extend_from_slice(&(indices.len() as u32).to_le_bytes());
        for &index in indices {
            out.extend_from_slice(&index.to_le_bytes());
        }
        out
    }

    /// Hack address: stable indices `(round, agent_id, portal_id, hack_seq)`
    /// where `hack_seq` counts only SUCCESSFUL hacks (failed hacks never
    /// sample).
    pub fn hack_address(
        &self,
        round: u32,
        agent: AgentId,
        portal: PortalId,
        hack_seq: u32,
    ) -> Vec<u8> {
        self.address(
            RngDomain::Hack,
            &[
                round as u64,
                agent.0 as u64,
                portal.0 as u64,
                hack_seq as u64,
            ],
        )
    }

    /// Reinforcement address: stable indices `(epoch, axis)`.
    pub fn reinforcement_address(&self, epoch: u32, axis: Axis) -> Vec<u8> {
        self.address(RngDomain::Reinforcement, &[epoch as u64, axis as u64])
    }

    /// Shared reinforcement spawn position for reinforcement interval
    /// `index` (R-SPAWN-02): one rejection-sampled uniform per axis over the
    /// closed map range `[0, dim_cm]` — i.e. `n = dim_cm + 1` values per
    /// axis, so the drawn coordinate is in bounds by construction. Both
    /// factions spawn at this one position.
    pub fn reinforcement_position(&self, index: u32, width_n: u32, height_n: u32) -> Coord {
        let x = self.uniform(&self.reinforcement_address(index, Axis::X), width_n);
        let y = self.uniform(&self.reinforcement_address(index, Axis::Y), height_n);
        Coord::new(x as i32, y as i32)
    }

    /// Fresh XOF bytes for `address`; every call derives from scratch.
    pub fn xof_bytes(&self, address: &[u8], out_len: usize) -> Vec<u8> {
        let mut reader = Self::xof_reader(address);
        let mut out = vec![0u8; out_len];
        reader.fill(&mut out);
        out
    }

    /// Unbiased uniform integer in `[0, n_exclusive)` via FROZEN rejection
    /// sampling over the address's XOF stream.
    ///
    /// # Panics
    ///
    /// `n_exclusive == 0` is a programmer error and panics. `n_exclusive == 1`
    /// naturally returns 0 (the threshold `2^32` accepts every word).
    pub fn uniform(&self, address: &[u8], n_exclusive: u32) -> u32 {
        assert!(
            n_exclusive > 0,
            "RandomOracle::uniform with n_exclusive == 0 is a programmer error"
        );
        let mut reader = Self::xof_reader(address);
        uniform_from_words(&mut reader, n_exclusive)
    }

    fn xof_reader(address: &[u8]) -> blake3::OutputReader {
        let mut hasher = blake3::Hasher::new();
        hasher.update(address);
        hasher.finalize_xof()
    }
}

/// The FROZEN rejection loop shared by every uniform mapping of XOF output to
/// a finite range. Reads successive 4-byte LE words starting at offset 0;
/// each rejected word advances the stream by 4 bytes. `t` is the largest
/// multiple of `n` not exceeding `2^32`, so `x % n` is unbiased for accepted
/// words. Never plain `x % n` without rejection.
///
/// Public since M6: local bots (arena-bots) draw their own stream randomness
/// through this same frozen mapping (D53) — visibility only, the mapping is
/// part of the rule version and never changes.
pub fn uniform_from_words<R: WordSource>(words: &mut R, n_exclusive: u32) -> u32 {
    let n = n_exclusive as u64;
    let threshold = ((1u64 << 32) / n) * n;
    loop {
        let x = words.next_u32() as u64;
        if x < threshold {
            return (x % n) as u32;
        }
    }
}

/// Source of successive 4-byte LE words for [`uniform_from_words`]. Public
/// since M6 (D53) so external frozen-mapping consumers can reuse the loop.
pub trait WordSource {
    fn next_u32(&mut self) -> u32;
}

impl WordSource for blake3::OutputReader {
    fn next_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        self.fill(&mut buf);
        u32::from_le_bytes(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Deterministic word source pinning the FROZEN rejection loop of
    /// [`uniform_from_words`] on synthetic sequences (the golden vectors'
    /// first words are all accepted, so they cannot exercise this branch).
    struct FakeWords {
        words: Vec<u32>,
        draws: usize,
    }

    impl WordSource for FakeWords {
        fn next_u32(&mut self) -> u32 {
            let word = self.words[self.draws];
            self.draws += 1;
            word
        }
    }

    /// Draws one uniform over a fixed word sequence; returns
    /// `(result, words_consumed)` so tests can also pin stream advancement.
    fn draw_uniform(words: &[u32], n_exclusive: u32) -> (u32, usize) {
        let mut source = FakeWords {
            words: words.to_vec(),
            draws: 0,
        };
        let value = uniform_from_words(&mut source, n_exclusive);
        (value, source.draws)
    }

    #[test]
    fn blake3_empty_hash_golden() {
        // Upstream BLAKE3 vector for the empty input; verifies the pinned
        // crate's baseline before trusting any derived golden below.
        let hasher = blake3::Hasher::new();
        let hash = hasher.finalize();
        assert_eq!(
            hex(hash.as_bytes()),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    #[test]
    fn golden_hack_address_encoding() {
        let oracle = RandomOracle::new(0x0123_4567_89AB_CDEF);
        let address = oracle.hack_address(10, AgentId(7), PortalId(17), 1);
        assert_eq!(
            hex(&address),
            "41524e4701000000efcdab89674523010100000004000000\
             0a00000000000000070000000000000011000000000000000100000000000000"
        );
    }

    #[test]
    fn golden_hack_xof_32_bytes() {
        let oracle = RandomOracle::new(0x0123_4567_89AB_CDEF);
        let address = oracle.hack_address(10, AgentId(7), PortalId(17), 1);
        assert_eq!(
            hex(&oracle.xof_bytes(&address, 32)),
            "840722cf15434243fd5450c427b9d83ba9232425dc289d9f9834a4e60f696c8b"
        );
    }

    #[test]
    fn golden_reinforcement_address_and_xof() {
        let oracle = RandomOracle::new(42);
        let address = oracle.reinforcement_address(5, Axis::Y);
        assert_eq!(
            hex(&address),
            "41524e47010000002a000000000000000200000002000000\
             05000000000000000100000000000000"
        );
        assert_eq!(
            hex(&oracle.xof_bytes(&address, 32)),
            "be08913496e86ff442845df02cab11eb3d2629d66117cb081f99f8b3162bf266"
        );
    }

    #[test]
    fn golden_uniform_hack_n3_returns_2() {
        let oracle = RandomOracle::new(0x0123_4567_89AB_CDEF);
        let address = oracle.hack_address(10, AgentId(7), PortalId(17), 1);
        assert_eq!(oracle.uniform(&address, 3), 2);
    }

    #[test]
    fn golden_uniform_hack_n7_returns_1() {
        let oracle = RandomOracle::new(0x0123_4567_89AB_CDEF);
        let address = oracle.hack_address(10, AgentId(7), PortalId(17), 1);
        assert_eq!(oracle.uniform(&address, 7), 1);
    }

    #[test]
    fn golden_uniform_reinforcement_n600001_returns_518721() {
        let oracle = RandomOracle::new(42);
        let address = oracle.reinforcement_address(5, Axis::Y);
        assert_eq!(oracle.uniform(&address, 600_001), 518_721);
    }

    #[test]
    fn rejection_first_word_at_threshold_rejected_second_used() {
        // n = 3: threshold = floor(2^32 / 3) * 3 = 4_294_967_295 — exactly one
        // word value (the threshold itself) must be rejected.
        assert_eq!(((1u64 << 32) / 3) * 3, 4_294_967_295);
        // First word equals the threshold: rejected, and the SECOND word
        // answers (7 % 3 == 1) after exactly two draws.
        assert_eq!(draw_uniform(&[4_294_967_295, 7], 3), (1, 2));
        // A naive `x % n` over the first word would answer 4_294_967_295 % 3
        // == 0, so this pins the rejection branch, not just the modulo
        // mapping.
        assert_ne!(4_294_967_295u64 % 3, 7u64 % 3);
        // The largest ACCEPTED word (threshold - 1) is consumed alone.
        assert_eq!(draw_uniform(&[4_294_967_294], 3), (2, 1));
        // Word 0 is accepted and maps to 0.
        assert_eq!(draw_uniform(&[0], 3), (0, 1));
    }

    #[test]
    fn rejection_consecutive_rejected_words_advance_stream() {
        // n = 7: threshold = 4_294_967_292, so the four word values
        // 4_294_967_292..=4_294_967_295 are rejected. Three consecutive
        // rejections each advance the stream by exactly one word before 10
        // is accepted (10 % 7 == 3).
        assert_eq!(((1u64 << 32) / 7) * 7, 4_294_967_292);
        assert_eq!(
            draw_uniform(&[4_294_967_292, 4_294_967_293, 4_294_967_295, 10], 7),
            (3, 4)
        );
        // Largest accepted word (threshold - 1): 4_294_967_291 % 7 == 6.
        assert_eq!(draw_uniform(&[4_294_967_291], 7), (6, 1));
    }

    #[test]
    fn rejection_threshold_never_rejects_when_range_divides_2_pow_32() {
        // n = 1: threshold = 2^32 exceeds every u32 word, so even u32::MAX is
        // accepted and maps to 0 (D11).
        assert_eq!(draw_uniform(&[u32::MAX], 1), (0, 1));
        // n = 2 divides 2^32 exactly: threshold = 2^32, the full word range
        // is unbiased without any rejection.
        assert_eq!(draw_uniform(&[u32::MAX], 2), (1, 1));
    }

    #[test]
    fn rejection_branch_hit_through_real_xof_stream() {
        // n = 2^31 + 1 has threshold == n, so roughly half of all first words
        // are rejected: the branch also fires on REAL oracle streams, and the
        // walk must advance 4 bytes per rejection through the OutputReader.
        // The expected value is derived independently from xof_bytes chunks.
        let n: u32 = 0x8000_0001;
        let oracle = RandomOracle::new(0xC0FF_EE0);
        let mut found = None;
        for seq in 0..64u32 {
            let address = oracle.hack_address(2, AgentId(1), PortalId(1), seq);
            let bytes = oracle.xof_bytes(&address, 8);
            let w0 = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
            let w1 = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
            if w0 >= n && w1 < n {
                found = Some((address, w0, w1));
                break;
            }
        }
        let (address, w0, w1) = found.expect("a rejected first word within 64 draws");
        // The rejected and accepted words map to different answers, so a
        // naive first-word `% n` cannot pass this assertion.
        assert_ne!(w0 % n, w1 % n);
        assert_eq!(oracle.uniform(&address, n), w1 % n);
    }

    #[test]
    fn uniform_n1_always_zero() {
        let oracle = RandomOracle::new(7);
        for seq in 0..100u32 {
            let address = oracle.hack_address(1, AgentId(0), PortalId(0), seq);
            assert_eq!(oracle.uniform(&address, 1), 0);
        }
    }

    #[test]
    #[should_panic(expected = "programmer error")]
    fn uniform_n0_panics() {
        let oracle = RandomOracle::new(1);
        let address = oracle.hack_address(1, AgentId(0), PortalId(0), 0);
        oracle.uniform(&address, 0);
    }

    #[test]
    fn uniform_results_within_bound() {
        let oracle = RandomOracle::new(0xDEAD_BEEF);
        for n in 1u32..=64 {
            for seq in 0..100u32 {
                let address = oracle.hack_address(3, AgentId(seq.min(99)), PortalId(seq), n);
                assert!(oracle.uniform(&address, n) < n);
            }
        }
    }

    #[test]
    fn uniform_distribution_sanity_deterministic() {
        // 3000 hack addresses, n = 3 (the key-yield range): every bucket lands
        // in [27%, 37%] of the samples (810..=1110). Exact counts are
        // deterministic for a fixed seed; integer bounds only, no floats.
        let oracle = RandomOracle::new(0x0123_4567_89AB_CDEF);
        let mut counts = [0usize; 3];
        for seq in 0..3000u32 {
            let address = oracle.hack_address(10, AgentId(7), PortalId(17), seq);
            counts[oracle.uniform(&address, 3) as usize] += 1;
        }
        for (i, &count) in counts.iter().enumerate() {
            assert!(
                (810..=1110).contains(&count),
                "bucket {i} count {count} out of [810, 1110] (counts {:?})",
                counts
            );
        }
    }

    #[test]
    fn addresses_stable_and_input_sensitive() {
        let oracle = RandomOracle::new(100);
        let base = oracle.hack_address(4, AgentId(2), PortalId(9), 3);
        assert_eq!(
            base.clone(),
            oracle.hack_address(4, AgentId(2), PortalId(9), 3)
        );

        let differs = |other: Vec<u8>| assert_ne!(base, other, "address must change");
        differs(oracle.hack_address(4, AgentId(2), PortalId(9), 4)); // index value
        differs(oracle.hack_address(5, AgentId(2), PortalId(9), 3));
        differs(RandomOracle::new(101).hack_address(4, AgentId(2), PortalId(9), 3)); // seed
        differs(oracle.address(RngDomain::Hack, &[4, 2, 9])); // index count
        differs(oracle.address(RngDomain::Hack, &[]));
    }

    #[test]
    fn domain_separation_hack_vs_reinforcement() {
        let oracle = RandomOracle::new(9);
        // Identical index payload (1, 2) under both domains.
        let hack = oracle.address(RngDomain::Hack, &[1, 2]);
        let reinf = oracle.address(RngDomain::Reinforcement, &[1, 2]);
        assert_ne!(hack, reinf);
        assert_ne!(oracle.xof_bytes(&hack, 16), oracle.xof_bytes(&reinf, 16));
    }

    #[test]
    fn no_cursor_state_interleaved() {
        let oracle = RandomOracle::new(1234);
        let a = oracle.hack_address(1, AgentId(1), PortalId(1), 1);
        let b = oracle.hack_address(1, AgentId(1), PortalId(1), 2);
        let first = oracle.xof_bytes(&a, 32);
        let _ = oracle.xof_bytes(&b, 48);
        let third = oracle.xof_bytes(&a, 32);
        assert_eq!(first, third);
    }

    #[test]
    fn query_independence() {
        let oracle = RandomOracle::new(77);
        let target = oracle.hack_address(9, AgentId(4), PortalId(12), 5);
        let expected = oracle.uniform(&target, 1000);

        // Fresh oracle queries other addresses first, including k-1 and
        // failed-hack sequences; the target result must not move.
        let oracle2 = RandomOracle::new(77);
        for seq in 0..5u32 {
            let _ = oracle2.uniform(&oracle2.hack_address(9, AgentId(4), PortalId(12), seq), 7);
            let _ = oracle2.uniform(&oracle2.reinforcement_address(3, Axis::X), 600_001);
        }
        assert_eq!(oracle2.uniform(&target, 1000), expected);
    }

    #[test]
    fn game_seed_accessors() {
        let oracle = RandomOracle::new(0xABCD_1234_5678_0001);
        assert_eq!(oracle.game_seed(), 0xABCD_1234_5678_0001);
    }
}

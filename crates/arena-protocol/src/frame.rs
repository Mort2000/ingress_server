//! Frame framing (P3-RPC, D62): every message is a `u32` BIG-ENDIAN
//! payload-length prefix followed by exactly that many payload bytes (one
//! FlatBuffers `ProtocolEnvelope`). The length excludes the prefix.
//!
//! `MAX_FRAME_PAYLOAD_BYTES` is the LEGAL maximum (65,535): ADR-0008 states
//! the hard limit as 65,536 bytes, and a prefix of exactly 65,536 already
//! declares an over-limit payload, so the legal maximum is one less. The
//! limit is checked BEFORE any payload allocation: [`FrameReader`] never
//! pre-allocates from the declared length — it only accumulates bytes that
//! actually arrived, so a hostile 4 GiB prefix can never size an
//! allocation. An over-limit prefix poisons the reader (every later poll
//! returns `OverLimit`); the server closes the connection without draining
//! (R-RESOLVE-01), which after the match has started produces
//! `CLIENT_FORFEIT`.

/// Legal maximum payload size in bytes (D62; the 65,536 of ADR-0008 is the
/// exclusive ceiling, not a legal size).
pub const MAX_FRAME_PAYLOAD_BYTES: usize = 65_535;

/// Framing error kinds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameError {
    /// [`encode_frame`] payload exceeds [`MAX_FRAME_PAYLOAD_BYTES`].
    PayloadTooLarge { len: usize },
}

impl core::fmt::Display for FrameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FrameError::PayloadTooLarge { len } => write!(
                f,
                "frame payload of {len} bytes exceeds the legal maximum {MAX_FRAME_PAYLOAD_BYTES}"
            ),
        }
    }
}

impl std::error::Error for FrameError {}

/// Appends one complete frame (prefix + payload) to `out`.
pub fn encode_frame(payload: &[u8], out: &mut Vec<u8>) -> Result<(), FrameError> {
    if payload.len() > MAX_FRAME_PAYLOAD_BYTES {
        return Err(FrameError::PayloadTooLarge { len: payload.len() });
    }
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

/// One complete frame, encoded into a fresh buffer.
pub fn encode_frame_vec(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    let mut out = Vec::with_capacity(payload.len() + 4);
    encode_frame(payload, &mut out)?;
    Ok(out)
}

/// Result of polling [`FrameReader::next_frame`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FramePoll {
    /// More bytes are needed before the next frame is complete.
    NeedMore,
    /// One complete, in-limit payload.
    Frame(Vec<u8>),
    /// The next frame's declared length exceeds the legal maximum. The
    /// reader is poisoned: every subsequent poll returns `OverLimit` again
    /// with the same `declared` value. Callers must stop feeding bytes and
    /// close the connection without draining.
    OverLimit { declared: u32 },
}

/// Incremental frame reassembler over an ordered byte stream.
///
/// Bytes are pushed as they arrive ([`FrameReader::push_bytes`]); complete
/// frames are pulled one at a time. The declared prefix is validated
/// against [`MAX_FRAME_PAYLOAD_BYTES`] before the payload can influence
/// any allocation, and the internal buffer never holds more than one legal
/// frame plus one prefix.
#[derive(Debug)]
pub struct FrameReader {
    /// Received, not-yet-consumed bytes (at most one partial frame).
    buf: Vec<u8>,
    /// Offset of the first unconsumed byte in `buf`.
    start: usize,
    /// Set on the first over-limit prefix; every poll reports it again.
    poison: Option<u32>,
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameReader {
    pub fn new() -> Self {
        FrameReader {
            buf: Vec::new(),
            start: 0,
            poison: None,
        }
    }

    /// Feeds newly received bytes. Cheaper than it looks: the buffer holds
    /// at most one incomplete frame.
    pub fn push_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if self.start == self.buf.len() {
            self.buf.clear();
            self.start = 0;
        }
        self.buf.extend_from_slice(bytes);
    }

    /// Tries to take the next complete frame.
    pub fn next_frame(&mut self) -> FramePoll {
        if let Some(declared) = self.poison {
            return FramePoll::OverLimit { declared };
        }
        let available = self.buf.len() - self.start;
        if available < 4 {
            self.compact();
            return FramePoll::NeedMore;
        }
        let declared = u32::from_be_bytes([
            self.buf[self.start],
            self.buf[self.start + 1],
            self.buf[self.start + 2],
            self.buf[self.start + 3],
        ]);
        // Check BEFORE any use of `declared` for allocation or slicing:
        // an over-limit prefix never allocates and poisons the reader.
        if declared as usize > MAX_FRAME_PAYLOAD_BYTES {
            self.poison = Some(declared);
            return FramePoll::OverLimit { declared };
        }
        let end = self.start + 4 + declared as usize;
        if self.buf.len() < end {
            self.compact();
            return FramePoll::NeedMore;
        }
        let payload = self.buf[self.start + 4..end].to_vec();
        self.start = end;
        if self.start == self.buf.len() {
            self.buf.clear();
            self.start = 0;
        }
        FramePoll::Frame(payload)
    }

    /// Drops consumed bytes so the buffer stays one-frame sized.
    fn compact(&mut self) {
        if self.start > 0 {
            self.buf.drain(..self.start);
            self.start = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(payload: &[u8]) -> Vec<u8> {
        encode_frame_vec(payload).expect("in-limit payload")
    }

    #[test]
    fn legal_maximum_is_one_below_the_adr_ceiling() {
        assert_eq!(MAX_FRAME_PAYLOAD_BYTES, 65_535);
    }

    #[test]
    fn encode_rejects_over_limit_payloads() {
        let mut out = Vec::new();
        assert_eq!(
            encode_frame(&[0u8; 65_536], &mut out),
            Err(FrameError::PayloadTooLarge { len: 65_536 })
        );
        // The exact legal maximum encodes fine.
        assert_eq!(encode_frame(&[7u8; 65_535], &mut out), Ok(()));
        assert_eq!(out.len(), 4 + 65_535);
    }

    #[test]
    fn full_frame_in_one_write_decodes_immediately() {
        let mut reader = FrameReader::new();
        reader.push_bytes(&frame(b"hello"));
        assert_eq!(reader.next_frame(), FramePoll::Frame(b"hello".to_vec()));
        assert_eq!(reader.next_frame(), FramePoll::NeedMore);
    }

    #[test]
    fn byte_by_byte_feed_reassembles_exactly() {
        let mut reader = FrameReader::new();
        let bytes = frame(&[0, 1, 2, 3, 4, 5]);
        // Only the final byte completes the frame.
        for (index, byte) in bytes.iter().enumerate() {
            reader.push_bytes(&[*byte]);
            if index + 1 < bytes.len() {
                assert_eq!(reader.next_frame(), FramePoll::NeedMore);
            }
        }
        assert_eq!(
            reader.next_frame(),
            FramePoll::Frame(vec![0, 1, 2, 3, 4, 5])
        );
        assert_eq!(reader.next_frame(), FramePoll::NeedMore);
    }

    #[test]
    fn random_chunk_sizes_reassemble_two_frames_in_order() {
        let both = [frame(b"first"), frame(b"second")].concat();
        // Deterministic pseudo-random chunking (no RNG dependency).
        let mut reader = FrameReader::new();
        let mut position = 0;
        let mut chunk = 1;
        let mut decoded = Vec::new();
        while position < both.len() {
            let end = (position + chunk).min(both.len());
            reader.push_bytes(&both[position..end]);
            loop {
                match reader.next_frame() {
                    FramePoll::Frame(payload) => decoded.push(payload),
                    FramePoll::NeedMore => break,
                    FramePoll::OverLimit { .. } => panic!("in-limit stream"),
                }
            }
            position = end;
            chunk = chunk * 7 % 13 + 1;
        }
        assert_eq!(decoded, vec![b"first".to_vec(), b"second".to_vec()]);
    }

    #[test]
    fn empty_payload_is_a_legal_frame() {
        let mut reader = FrameReader::new();
        reader.push_bytes(&frame(b""));
        assert_eq!(reader.next_frame(), FramePoll::Frame(Vec::new()));
    }

    #[test]
    fn over_limit_prefix_poisons_before_allocation() {
        // 65,536 and 65,537 prefixes (D62 / P3-GATE boundary triple).
        for declared in [65_536u32, 65_537, u32::MAX] {
            let mut fresh = FrameReader::new();
            fresh.push_bytes(&declared.to_be_bytes());
            assert_eq!(
                fresh.next_frame(),
                FramePoll::OverLimit { declared },
                "prefix {declared}"
            );
            // Poisoned: a later well-formed frame still reports OverLimit.
            fresh.push_bytes(&frame(b"anything"));
            assert_eq!(fresh.next_frame(), FramePoll::OverLimit { declared });
        }
        // The exact legal prefix is NOT over-limit even with zero payload
        // bytes arrived yet.
        let mut fresh = FrameReader::new();
        fresh.push_bytes(&65_535u32.to_be_bytes());
        assert_eq!(fresh.next_frame(), FramePoll::NeedMore);
        // And it completes once the payload arrives.
        fresh.push_bytes(&[0u8; 65_535]);
        assert!(matches!(fresh.next_frame(), FramePoll::Frame(_)));
    }

    #[test]
    fn partial_prefix_then_over_limit_detected_before_payload() {
        let mut reader = FrameReader::new();
        reader.push_bytes(&[0x00]); // 1 of 4 prefix bytes
        assert_eq!(reader.next_frame(), FramePoll::NeedMore);
        reader.push_bytes(&[0x01, 0x00, 0x00]); // completes 0x00010000 = 65,536
        assert_eq!(
            reader.next_frame(),
            FramePoll::OverLimit { declared: 65_536 }
        );
    }
}

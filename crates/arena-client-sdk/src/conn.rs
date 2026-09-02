//! The client side of the single ordered UDS stream (ADR-0008): serial
//! whole-frame writes and framed reads. One connection per client for the
//! whole match; no concurrent submissions (only the client loop thread
//! writes).

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use arena_protocol::build::{envelope_bytes, envelope_frame};
use arena_protocol::decode::{Envelope, decode_envelope};
use arena_protocol::frame::{FramePoll, FrameReader};

use crate::SdkError;

/// The client's connection: connect, HELLO, then the message loop.
#[derive(Debug)]
pub struct Connection {
    stream: UnixStream,
    reader: FrameReader,
    /// Server frames received (diagnostics).
    pub frames_received: u64,
    pub frames_sent: u64,
}

impl Connection {
    pub fn connect(socket: &Path) -> Result<Self, SdkError> {
        let stream = UnixStream::connect(socket)?;
        Ok(Connection {
            stream,
            reader: FrameReader::new(),
            frames_received: 0,
            frames_sent: 0,
        })
    }

    /// Sends one envelope as one complete frame (serial, whole-frame).
    pub fn send(&mut self, envelope: &Envelope) -> Result<(), SdkError> {
        let frame = envelope_frame(envelope);
        self.stream.write_all(&frame)?;
        self.stream.flush()?;
        self.frames_sent += 1;
        Ok(())
    }

    /// Sends one envelope payload WITHOUT the frame prefix already
    /// written by the caller — used only by the probe client's split-write
    /// misbehavior; the official path is [`Connection::send`].
    pub fn send_raw(&mut self, bytes: &[u8]) -> Result<(), SdkError> {
        self.stream.write_all(bytes)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Encodes an envelope to bytes without sending (probe support).
    pub fn envelope_bytes(envelope: &Envelope) -> Vec<u8> {
        envelope_bytes(envelope)
    }

    /// Waits for the next server envelope. Drains already-buffered
    /// complete frames BEFORE blocking on the socket — the server writes
    /// pipelined frames (ROUND_CLOSED immediately followed by the next
    /// ROUND_START_SIGNAL), and blocking first would stall a buffered
    /// frame until the next unrelated write arrives.
    pub fn next_envelope(&mut self, timeout: Duration) -> Result<Option<Envelope>, SdkError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match self.reader.next_frame() {
                FramePoll::Frame(payload) => {
                    self.frames_received += 1;
                    let envelope =
                        decode_envelope(&payload).map_err(|e| SdkError::Protocol(e.to_string()))?;
                    return Ok(Some(envelope));
                }
                FramePoll::OverLimit { declared } => {
                    // A server sending over-limit frames is broken
                    // infrastructure; the client stops.
                    return Err(SdkError::Unexpected(format!(
                        "server frame over limit (declared {declared})"
                    )));
                }
                FramePoll::NeedMore => {}
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            self.stream.set_read_timeout(Some(remaining))?;
            let mut buf = [0u8; 16 * 1024];
            match self.stream.read(&mut buf) {
                Ok(0) => {
                    return Err(SdkError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "server closed the connection",
                    )));
                }
                Ok(n) => self.reader.push_bytes(&buf[..n]),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(SdkError::Io(error)),
            }
        }
    }

    /// Waits indefinitely (bounded by `timeout` per read attempt) until an
    /// envelope arrives; `None` only on timeout.
    pub fn expect_envelope(&mut self, timeout: Duration) -> Result<Envelope, SdkError> {
        self.next_envelope(timeout)?
            .ok_or_else(|| SdkError::Unexpected("timed out waiting for a server frame".into()))
    }
}

//! Test/diagnostic utilities: an in-process scripted RAW client speaking
//! the real framing protocol over a real UDS connection. Used by the
//! arena-server integration tests (transport matrix) — not part of the
//! server itself.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use arena_protocol::build::envelope_bytes;
use arena_protocol::decode::{Envelope, decode_envelope};
use arena_protocol::frame::{FramePoll, FrameReader, encode_frame};

/// A raw scriptable client: connects, sends arbitrary byte chunks and
/// envelopes, and decodes server frames with a timeout.
pub struct RawClient {
    stream: UnixStream,
    reader: FrameReader,
    name: String,
    last_decode_failed: bool,
}

impl RawClient {
    /// Connects to the server socket and immediately sends a version-valid
    /// CLIENT_HELLO.
    pub fn hello(socket: &std::path::Path, name: &str) -> std::io::Result<Self> {
        let mut client = RawClient::connect(socket, name)?;
        client.send_envelope(&Envelope::ClientHello {
            client_name: name.to_string(),
            client_version: "test".into(),
            bot_version: String::new(),
            sdk_version: String::new(),
            protocol_version: arena_protocol::PROTOCOL_VERSION,
            public_snapshot_version: arena_protocol::PUBLIC_SNAPSHOT_VERSION,
            private_snapshot_version: arena_protocol::PRIVATE_SNAPSHOT_VERSION,
        });
        Ok(client)
    }

    /// Connects with a CLIENT_HELLO whose version fields are shifted by
    /// `bump` (0 = valid; 1..3 bump one of the three versions).
    pub fn hello_with_version_bump(
        socket: &std::path::Path,
        name: &str,
        bump: u32,
    ) -> std::io::Result<Self> {
        let mut client = RawClient::connect(socket, name)?;
        let mut triple = [
            arena_protocol::PROTOCOL_VERSION,
            arena_protocol::PUBLIC_SNAPSHOT_VERSION,
            arena_protocol::PRIVATE_SNAPSHOT_VERSION,
        ];
        let index = (bump as usize - 1) % 3;
        triple[index] += 1;
        client.send_envelope(&Envelope::ClientHello {
            client_name: name.to_string(),
            client_version: "test".into(),
            bot_version: String::new(),
            sdk_version: String::new(),
            protocol_version: triple[0],
            public_snapshot_version: triple[1],
            private_snapshot_version: triple[2],
        });
        Ok(client)
    }

    /// Connects without sending anything yet.
    pub fn connect(socket: &std::path::Path, name: &str) -> std::io::Result<Self> {
        let stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        Ok(RawClient {
            stream,
            reader: FrameReader::new(),
            name: name.to_string(),
            last_decode_failed: false,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sends one IN-LIMIT frame carrying the given payload bytes verbatim
    /// (corrupt/truncated/empty payloads for the envelope-reject tests).
    pub fn send_envelope_bytes(&mut self, payload: &[u8]) {
        let mut frame = Vec::with_capacity(payload.len() + 4);
        arena_protocol::frame::encode_frame(payload, &mut frame).expect("in limit");
        self.send_raw(&frame);
    }

    /// Sends raw bytes exactly as given (partial frames, junk, anything).
    pub fn send_raw(&mut self, bytes: &[u8]) {
        if let Err(error) = self.stream.write_all(bytes) {
            let _ = error;
        }
        let _ = self.stream.flush();
    }

    /// Sends one complete framed envelope.
    pub fn send_envelope(&mut self, envelope: &Envelope) {
        let payload = envelope_bytes(envelope);
        let mut frame = Vec::with_capacity(payload.len() + 4);
        encode_frame(&payload, &mut frame).expect("test frames are in limit");
        self.send_raw(&frame);
    }

    /// Sends the first `split` bytes of one framed envelope now, holding
    /// the remainder for [`RawClient::send_pending_tail`].
    pub fn send_partial_envelope(&mut self, envelope: &Envelope, split: usize) -> Vec<u8> {
        let payload = envelope_bytes(envelope);
        let mut frame = Vec::with_capacity(payload.len() + 4);
        encode_frame(&payload, &mut frame).expect("test frames are in limit");
        let tail = frame.split_off(split.min(frame.len()));
        self.send_raw(&frame);
        tail
    }

    /// Waits for the next envelope from the server (None on timeout, EOF,
    /// or an undecodable frame — the caller distinguishes via
    /// [`RawClient::last_decode_failed`]).
    pub fn next_envelope(&mut self, timeout: Duration) -> Option<Envelope> {
        self.last_decode_failed = false;
        let deadline = std::time::Instant::now() + timeout;
        loop {
            // Drain buffered frames before blocking (pipelined writes).
            match self.reader.next_frame() {
                FramePoll::Frame(payload) => match decode_envelope(&payload) {
                    Ok(envelope) => return Some(envelope),
                    Err(_) => {
                        self.last_decode_failed = true;
                        return None;
                    }
                },
                FramePoll::OverLimit { .. } => return None,
                FramePoll::NeedMore => {}
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            self.stream.set_read_timeout(Some(remaining)).ok()?;
            let mut buf = [0u8; 8192];
            match self.stream.read(&mut buf) {
                Ok(0) => return None,
                Ok(n) => self.reader.push_bytes(&buf[..n]),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return None;
                }
                Err(_) => return None,
            }
        }
    }

    /// Whether the last [`RawClient::next_envelope`] None was caused by an
    /// undecodable (corrupt) frame rather than a timeout/EOF.
    pub fn last_decode_failed(&self) -> bool {
        self.last_decode_failed
    }
}

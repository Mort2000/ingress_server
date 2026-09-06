//! Connection layer (P3-RPC, D65): one accept thread plus exactly one
//! reader thread per accepted connection. The driver thread owns the write
//! halves, `GameState`, the resolver, the replay writer, and the SHM
//! publishers; reader threads only frame, timestamp, decode, and forward.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use arena_protocol::decode::{Envelope, ProtocolError, decode_envelope};
use arena_protocol::frame::FramePoll;
use arena_protocol::frame::FrameReader;

/// What a reader thread (or the accept thread) reports to the driver.
#[derive(Debug)]
pub enum ConnEvent {
    /// A connection was accepted; the driver owns the write half from now
    /// on (the accept thread starts its reader after publishing this event).
    Connected { conn: u8, stream: UnixStream },
    /// A surplus (third or later) connection was rejected with
    /// SERVER_HELLO status=2 and closed by the accept thread.
    SurplusRejected,
    /// The handshake window closed before two version-matching HELLOs
    /// arrived (unscored setup error).
    HandshakeIncomplete,
    /// One complete, in-limit frame.
    Frame {
        /// Connection slot (0 or 1).
        conn: u8,
        /// Monotonic stamp of the LAST payload byte's arrival (P3-RPC).
        received_at: Instant,
        /// Strictly increasing per-connection frame counter (never reset
        /// across the match; one connection == one faction, so this is the
        /// Faction Receive Sequence).
        receive_seq: u32,
        /// Eager full-decode result: `Err` = envelope reject (recoverable,
        /// freezes nothing).
        envelope: Result<Envelope, ProtocolError>,
    },
    /// An over-limit length prefix: the connection is dead (closed without
    /// draining, D62); after the match starts this is CLIENT_FORFEIT.
    OverLimit { conn: u8, declared: u32 },
    /// EOF or unrecoverable read error.
    Disconnected { conn: u8 },
}

/// Bounds a single `read` so a slow trickle cannot pin the thread forever
/// between polls; large enough to be noise-free.
const READ_CHUNK: usize = 16 * 1024;

/// Write timeout armed on every server-side stream (P3-TIMEOUT: a slow
/// client must never block the server; P4-GATE: a client timeout or crash
/// must never wedge it). A client that stays connected but stops READING
/// never generates an EOF — the socket's send buffer simply stops
/// draining — and after it fills, a blocking `write_all` would pin the
/// driver thread forever: no deadline could fire, no CLIENT_FORFEIT could
/// be produced, `serve_match` would never return. This timeout turns that
/// hang into a write error (treated as a disconnect: the faction
/// forfeits). Generously above any legitimate drain — server frames are
/// at most ~65 KiB and healthy clients read continuously.
pub const SERVER_WRITE_TIMEOUT: Duration = Duration::from_millis(2_000);

/// Arms [`SERVER_WRITE_TIMEOUT`] on an accepted stream (the driver's
/// write half; the fd setting covers the socket, the reader only reads).
pub fn arm_write_timeout(stream: &UnixStream) {
    let _ = stream.set_write_timeout(Some(SERVER_WRITE_TIMEOUT));
}

/// Spawns one reader thread for an accepted connection. `conn` is the slot
/// index; the thread exits when the stream EOFs, errors, poisons
/// (over-limit prefix), or the driver goes away (channel send fails).
pub fn spawn_reader(conn: u8, mut stream: UnixStream, tx: Sender<ConnEvent>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name(format!("conn-{conn}-reader"))
        .spawn(move || {
            let mut reader = FrameReader::new();
            let mut receive_seq: u32 = 0;
            let mut buf = [0u8; READ_CHUNK];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => {
                        let _ = tx.send(ConnEvent::Disconnected { conn });
                        break;
                    }
                    Ok(n) => {
                        reader.push_bytes(&buf[..n]);
                        loop {
                            match reader.next_frame() {
                                FramePoll::NeedMore => break,
                                FramePoll::Frame(payload) => {
                                    // Timestamp the COMPLETE frame's arrival.
                                    let received_at = Instant::now();
                                    let envelope = decode_envelope(&payload);
                                    if tx
                                        .send(ConnEvent::Frame {
                                            conn,
                                            received_at,
                                            receive_seq,
                                            envelope,
                                        })
                                        .is_err()
                                    {
                                        return;
                                    }
                                    receive_seq = receive_seq.wrapping_add(1);
                                }
                                FramePoll::OverLimit { declared } => {
                                    // Poisoned: stop draining, report, die.
                                    let _ = tx.send(ConnEvent::OverLimit { conn, declared });
                                    return;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        if error.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        let _ = tx.send(ConnEvent::Disconnected { conn });
                        break;
                    }
                }
            }
        })
        .expect("reader thread spawns")
}

/// Writes one whole frame to a connection (server -> client direction is
/// serial by construction: only the driver thread writes).
pub fn write_frame(stream: &mut UnixStream, frame: &[u8]) -> std::io::Result<()> {
    stream.write_all(frame)?;
    stream.flush()
}

/// Closes a connection for reading and writing (used for over-limit and
/// duplicate-connection termination).
pub fn hard_close(stream: &mut UnixStream) {
    let _ = stream.shutdown(Shutdown::Both);
}

/// Deadline helper: remaining duration clamped at zero.
pub fn remaining_until(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

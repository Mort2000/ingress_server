//! arena-client-sdk: the official participant API for Ingress Arena
//! clients (M7, P3/P4). Audited Clients reach the system boundary ONLY
//! through this crate: the UDS framing connection, the read-only SHM
//! observation views, the phase machine, and the round context.
//!
//! * Zero-copy observations are lent as [`round::RoundContext`] ->
//!   [`view::ObservationView`]; the view borrows the session, so it cannot
//!   escape `on_round`, be stored in bot state, or reach a detached thread
//!   (ADR-0006; enforced by lifetimes and pinned by compile-fail tests).
//! * The phase machine refuses every SHM read between ROUND_CLOSED and
//!   the next ROUND_START_SIGNAL (Settlement Phase, P3-RPC) and the round
//!   context closes on last-submit / explicit commit / deadline.
//! * Unsafe policy: `deny(unsafe_code)` crate-wide with ONE sanctioned
//!   module — `shm.rs` (read-only POSIX SHM mappings via `extern "C"`,
//!   no libc crate) carrying a scoped, documented allow.

#![deny(unsafe_code)]
// The sole sanctioned unsafe module in this crate (see above).
#[allow(unsafe_code)]
pub mod shm;

pub mod bot;
pub mod client;
pub mod conn;
pub mod phase;
pub mod round;
pub mod view;

pub use bot::ClientBot;
pub use client::{ConnectOptions, RunSummary, run};
pub use phase::{Phase, PhaseGate};
pub use round::{RoundContext, SpawnContext, SubmissionAck};
pub use view::ObservationView;

/// Every SDK failure mode surfaced to participant code.
#[derive(Debug)]
pub enum SdkError {
    Io(std::io::Error),
    /// Shared-memory open/read failure.
    Shm(String),
    /// A frame payload failed the eager decode gate.
    Protocol(String),
    /// An SHM read or context construction was attempted outside the
    /// licensed phase (Settlement Phase, P3-RPC).
    PhaseViolation,
    /// The round context already closed (last submit, commit, deadline).
    ContextClosed,
    /// SERVER_HELLO rejected the version triple.
    VersionRejected(String),
    /// The faction's Initial Spawn Set was refused (SPAWN_FORFEIT); the
    /// match is over for this client — no replacement, no retry.
    SpawnForfeit(String),
    /// The server ended the match early (technical outcome).
    ServerAborted(String),
    /// Anything the SDK did not expect from the server.
    Unexpected(String),
}

impl core::fmt::Display for SdkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SdkError::Io(e) => write!(f, "io error: {e}"),
            SdkError::Shm(e) => write!(f, "shared memory error: {e}"),
            SdkError::Protocol(e) => write!(f, "protocol error: {e}"),
            SdkError::PhaseViolation => write!(
                f,
                "SHM access refused: Settlement Phase (between ROUND_CLOSED and ROUND_START_SIGNAL)"
            ),
            SdkError::ContextClosed => {
                write!(f, "round context closed (last submit, commit, or deadline)")
            }
            SdkError::VersionRejected(d) => write!(f, "server rejected versions: {d}"),
            SdkError::SpawnForfeit(d) => write!(f, "initial spawn set refused: {d}"),
            SdkError::ServerAborted(d) => write!(f, "server ended the match: {d}"),
            SdkError::Unexpected(d) => write!(f, "unexpected server behavior: {d}"),
        }
    }
}

impl std::error::Error for SdkError {}

impl From<std::io::Error> for SdkError {
    fn from(value: std::io::Error) -> Self {
        SdkError::Io(value)
    }
}

impl From<crate::shm::ShmError> for SdkError {
    fn from(value: crate::shm::ShmError) -> Self {
        SdkError::Shm(value.to_string())
    }
}

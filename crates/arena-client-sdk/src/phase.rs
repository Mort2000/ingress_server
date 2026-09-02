//! The participant phase machine (P3-RPC): SHM reads are licensed only in
//! the Spawn phase (pre-match map view) and while a round is open. From
//! ROUND_CLOSED to the next ROUND_START_SIGNAL every read is a
//! [`crate::SdkError::PhaseViolation`] — "从 ROUND_CLOSED 到下一次
//! ROUND_START_SIGNAL，Client 读取任何 SHM 都属于作弊". The MVP does not
//! physically block the mapping (that arrives with untrusted clients,
//! P4); the OFFICIAL SDK refuses, and audited clients cannot reach the
//! mapping except through it.

use crate::SdkError;

/// Client-side phases.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Not connected yet.
    Connecting,
    /// After INITIAL_SPAWN_START: the pre-match map view is readable.
    Spawn,
    /// After ROUND_START_SIGNAL: the round's observations are readable.
    RoundOpen,
    /// After ROUND_CLOSED: no SHM read is licensed.
    Settlement,
}

/// The phase transition gate. Owned by the [`crate::round::Session`];
/// atomic-interior so a shared session reference can check the phase from
/// scoped worker threads too.
#[derive(Debug)]
pub struct PhaseGate {
    phase: std::sync::atomic::AtomicU8,
}

const PHASE_CONNECTING: u8 = 0;
const PHASE_SPAWN: u8 = 1;
const PHASE_ROUND_OPEN: u8 = 2;
const PHASE_SETTLEMENT: u8 = 3;

impl Phase {
    fn code(self) -> u8 {
        match self {
            Phase::Connecting => PHASE_CONNECTING,
            Phase::Spawn => PHASE_SPAWN,
            Phase::RoundOpen => PHASE_ROUND_OPEN,
            Phase::Settlement => PHASE_SETTLEMENT,
        }
    }

    fn from_code(code: u8) -> Phase {
        match code {
            PHASE_SPAWN => Phase::Spawn,
            PHASE_ROUND_OPEN => Phase::RoundOpen,
            PHASE_SETTLEMENT => Phase::Settlement,
            _ => Phase::Connecting,
        }
    }
}

impl PhaseGate {
    pub fn new() -> Self {
        PhaseGate {
            phase: std::sync::atomic::AtomicU8::new(PHASE_CONNECTING),
        }
    }

    pub fn phase(&self) -> Phase {
        Phase::from_code(self.phase.load(std::sync::atomic::Ordering::Acquire))
    }

    pub fn set(&self, phase: Phase) {
        self.phase
            .store(phase.code(), std::sync::atomic::Ordering::Release);
    }

    /// Transitions on INITIAL_SPAWN_START.
    pub fn on_initial_spawn_start(&self) {
        self.set(Phase::Spawn);
    }

    /// Transitions on ROUND_START_SIGNAL.
    pub fn on_round_start(&self) {
        self.set(Phase::RoundOpen);
    }

    /// Transitions on ROUND_CLOSED.
    pub fn on_round_closed(&self) {
        self.set(Phase::Settlement);
    }

    /// Whether an SHM read is licensed right now.
    pub fn read_licensed(&self) -> bool {
        matches!(self.phase(), Phase::Spawn | Phase::RoundOpen)
    }

    /// Errors unless a read is licensed (the Settlement Phase refusal).
    pub fn assert_read_licensed(&self) -> Result<(), SdkError> {
        if self.read_licensed() {
            Ok(())
        } else {
            Err(SdkError::PhaseViolation)
        }
    }
}

impl Default for PhaseGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settlement_phase_refuses_reads() {
        let gate = PhaseGate::new();
        assert_eq!(gate.phase(), Phase::Connecting);
        assert!(gate.assert_read_licensed().is_err());

        gate.on_initial_spawn_start();
        assert_eq!(gate.phase(), Phase::Spawn);
        assert!(gate.assert_read_licensed().is_ok());

        gate.on_round_start();
        assert_eq!(gate.phase(), Phase::RoundOpen);
        assert!(gate.assert_read_licensed().is_ok());

        gate.on_round_closed();
        assert_eq!(gate.phase(), Phase::Settlement);
        assert!(matches!(
            gate.assert_read_licensed(),
            Err(SdkError::PhaseViolation)
        ));

        // The next round re-licenses reads.
        gate.on_round_start();
        assert!(gate.assert_read_licensed().is_ok());
    }
}

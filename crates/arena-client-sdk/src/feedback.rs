//! Transport feedback is not an execution verdict. Local timing measures
//! elapsed wall time, not CPU time or the authoritative Priority Bucket.
use arena_core::types::AgentId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionStatus {
    Accepted,
    RejectedStructural,
    IgnoredDuplicate,
    Unlocatable,
    LateDrop,
    Unknown(u8),
}

impl From<u8> for SubmissionStatus {
    fn from(value: u8) -> Self {
        use arena_protocol::decode::*;
        match value {
            SUBMIT_ACCEPTED => Self::Accepted,
            SUBMIT_REJECTED_STRUCTURAL => Self::RejectedStructural,
            SUBMIT_IGNORED_DUPLICATE => Self::IgnoredDuplicate,
            SUBMIT_UNLOCATABLE => Self::Unlocatable,
            SUBMIT_LATE_DROP => Self::LateDrop,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionFeedback {
    pub round: u32,
    pub agent: AgentId,
    pub status: SubmissionStatus,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubmissionTiming {
    pub agent: AgentId,
    /// Local wall time from context creation through successful frame write.
    pub sent_after_us: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundTiming {
    pub round: u32,
    /// Includes context preparation, planning, blocking, scheduling and writes;
    /// NOT CPU time. The measurement starts before context construction.
    pub callback_elapsed_us: u64,
    pub submissions: Vec<SubmissionTiming>,
}

impl RoundTiming {
    /// Optional structured log payload. Runner treats it as client-declared
    /// diagnostics, never authenticated CPU or server receive measurements.
    pub fn to_json(&self) -> String {
        let submissions = self
            .submissions
            .iter()
            .map(|s| {
                format!(
                    "{{\"agent\":{},\"sent_after_us\":{}}}",
                    s.agent.0, s.sent_after_us
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"arena-client-timing-v1\",\"round\":{},\"callback_elapsed_us\":{},\"submissions\":[{submissions}]}}",
            self.round, self.callback_elapsed_us
        )
    }
}

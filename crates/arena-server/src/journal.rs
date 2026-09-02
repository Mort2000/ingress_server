//! Key=value match journal: the operator-readable trace of every phase
//! transition, transport drop, and forfeit. Written to
//! `<out>/journal.txt` at match end; also returned in memory for tests
//! (the parity test reads ROUND_START_SIGNAL send order out of it).

use std::time::Instant;

/// Accumulates `key=value` lines with a monotonic match clock
/// (milliseconds since journal creation — diagnostic only, never a rule
/// input).
#[derive(Debug, Default)]
pub struct Journal {
    lines: Vec<String>,
    started: Option<Instant>,
}

impl Journal {
    pub fn new() -> Self {
        Journal {
            lines: Vec::new(),
            started: None,
        }
    }

    /// Starts the match clock.
    pub fn start_clock(&mut self) {
        if self.started.is_none() {
            self.started = Some(Instant::now());
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.started
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    /// Records one event line: `at_ms=<clock> key=value...`.
    pub fn event(&mut self, key: &str, value: &str) {
        let line = format!("at_ms={} {}={}", self.elapsed_ms(), key, value);
        self.lines.push(line);
    }

    /// Records a key=value pair whose value may contain spaces (reasons).
    pub fn note(&mut self, key: &str, value: &str) {
        self.event(key, &format!("{value:?}"));
    }

    /// Round-start journal entry: parity order is the P3 requirement
    /// ("odd rounds RES first, even rounds ENL first", tested over >=6
    /// consecutive rounds).
    pub fn round_start(&mut self, round: u32, notified_first: Faction) {
        let first = match notified_first {
            Faction::Enl => "enl",
            Faction::Res => "res",
        };
        self.event("round_start", &format!("{} notified_first={first}", round));
    }

    pub fn round_closed(&mut self, round: u32, duration_ms: u64, submissions: usize) {
        self.event(
            "round_closed",
            &format!("{round} duration_ms={duration_ms} submissions={submissions}"),
        );
    }

    /// Transport-level late drop: a frame completing at/after the round
    /// deadline (P3-TIMEOUT); it never enters a slot.
    pub fn late_drop(&mut self, round: u32, agent: u32) {
        self.event("late_drop", &format!("round={round} agent={agent}"));
    }

    /// Envelope reject: an in-limit frame whose payload failed the
    /// Verifier gate. Recoverable; nothing frozen.
    pub fn envelope_reject(&mut self, conn: u8) {
        self.event("envelope_reject", &format!("conn={conn}"));
    }

    /// An envelope-valid but unlocatable submission (wrong round, unknown
    /// agent, faction mismatch): forwarded to the Resolver, nothing
    /// frozen.
    pub fn unlocatable(&mut self, round: u32, agent: u32) {
        self.event("unlocatable", &format!("round={round} agent={agent}"));
    }

    pub fn spawn_set(&mut self, faction: Faction, accepted: bool, detail: &str) {
        let tag = match faction {
            Faction::Enl => "enl",
            Faction::Res => "res",
        };
        self.event(
            "spawn_set",
            &format!("{tag} accepted={accepted} detail={detail:?}"),
        );
    }

    pub fn outcome(&mut self, outcome_tag: &str) {
        self.event("outcome", outcome_tag);
    }

    /// The whole journal as `key=value` lines (one per line).
    pub fn text(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            out.push_str(line);
            out.push('\n');
        }
        out
    }
}

use arena_core::types::Faction;

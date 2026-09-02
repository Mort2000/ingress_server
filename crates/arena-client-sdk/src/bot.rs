//! The participant interface (P-ARCH-04): a `ClientBot` is called with
//! contexts it may use and nothing else. Spawns are chosen once from the
//! complete frozen map; every round hands the bot a
//! [`crate::RoundContext`] whose observation borrow cannot escape the
//! call.

use arena_core::types::{AgentId, Coord};

use crate::round::{RoundContext, SpawnContext};

/// An Audited Client's participant logic (safe Rust; the SDK owns every
/// system boundary).
pub trait ClientBot: Send {
    /// The pre-match spawn choice: the complete `agent_id -> coordinate`
    /// set, validated as a whole by the server (R-SPAWN-01). One shot; a
    /// rejected set is a SPAWN_FORFEIT with no replacement.
    fn choose_spawns(&mut self, ctx: &SpawnContext<'_>) -> Vec<(AgentId, Coord)>;

    /// One round of participation. Submit inside the context (first
    /// submission of the last agent, `commit`, or the deadline closes it).
    fn on_round(&mut self, ctx: RoundContext<'_>);
}

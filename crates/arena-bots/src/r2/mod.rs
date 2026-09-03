//! R2 shared capability layer (`docs/ai-balance-plan.md` sections 5.1,
//! 6.2 and 6.3): the deep, individually testable modules the six R2
//! policies compose, instead of per-policy re-implementation.
//!
//! - [`rng`]: the R2 policy RNG namespace — the frozen D53 key shape with
//!   FRESH stream kind-tag bytes 6..=12, disjoint from the frozen m6
//!   kind tags 1..=5 (ADR-0015);
//! - [`value`]: the interpretable INTEGER value model of plan 6.2 (field /
//!   attack / recharge-defense / transfer / block value and field
//!   candidate generation), pure over the legal `Observation` and public
//!   event history only;
//! - [`tasks`]: the agent-task allocator with per-round target reservation
//!   and the explicit coordinated-attack exception (plan 6.3);
//! - [`sample`]: seeded tie-break and near-equal candidate sampling,
//!   deterministic given `(observation, seed, faction, version)`.
//!
//! This layer holds no policy identity of its own: identities, families,
//! versions and the serialized parameter records live in
//! [`crate::policy`] (ADR-0015). The fixed version tag every B2 policy
//! carries is [`R2_VERSION`].

/// Fixed version tag of every B2 R2 policy (plan section 14 row B2; the
/// policies are registered in `crate::policy` with exactly this version).
pub const R2_VERSION: &str = "r2.0";

pub mod rng;
pub mod sample;
pub mod tasks;
pub mod value;

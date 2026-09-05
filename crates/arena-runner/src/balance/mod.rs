//! The balance experiment layer (B1, `docs/balance/ai-balance-plan.md` section
//! 5.2): plan -> schedule -> execute -> pairs -> archive over the existing
//! in-process match runner. The frozen m6 `BatchConfig` path is untouched
//! and keeps serving the frozen M6 batches. `pairs.tsv` carries integer
//! pair deltas only; the plan-section-9 offline statistics live in
//! `analysis` (B3, ADR-0017) and never feed back into any match, Game
//! Result, Replay Hash or settlement.

#![forbid(unsafe_code)]

pub mod ablation;
pub mod analysis;
pub mod archive;
pub mod effects;
pub mod execute;
pub mod pairs;
pub mod plan;
pub mod readiness;
pub mod record;
pub mod schedule;
pub mod sensitivity;
pub mod smoke;

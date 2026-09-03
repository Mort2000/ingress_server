//! The balance experiment layer (B1, `docs/ai-balance-plan.md` section
//! 5.2): plan -> schedule -> execute -> pairs -> archive over the existing
//! in-process match runner. The frozen m6 `BatchConfig` path is untouched
//! and keeps serving the frozen M6 batches; statistical analysis is B3/B4
//! and is deliberately absent here — `pairs.tsv` carries integer pair
//! deltas only.

#![forbid(unsafe_code)]

pub mod plan;

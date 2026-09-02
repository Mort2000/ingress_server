//! arena-core: authoritative game model and deterministic settlement for
//! Ingress Arena v0.1. Rule semantics come from `.scratch/ingress_arena_v0.1/rules/`;
//! this crate must not contain a second copy of the rules.
//!
//! Milestone M1 modules: types (domain model), config (R-CONFIG-01),
//! geometry (P0-GEOMETRY integer math), rng (R-RNG-01 random oracle),
//! map (R-MAP-02 frozen definition + generator), spawn (R-SPAWN-01 gate).
//! Milestone M2 adds: event (R-EVENT-01 broadcast log), state (GameState),
//! resolve (R-RESOLVE intake/ordering/execution), observe (R-VIS faction
//! views), harness (in-process scripted matches).

#![forbid(unsafe_code)]

pub mod config;
pub mod event;
pub mod geometry;
pub mod harness;
pub mod map;
pub mod observe;
pub mod resolve;
pub mod rng;
pub mod spawn;
pub mod state;
pub mod types;

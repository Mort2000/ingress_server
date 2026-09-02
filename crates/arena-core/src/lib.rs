//! arena-core: authoritative game model and deterministic settlement for
//! Ingress Arena v0.1. Rule semantics come from `.scratch/ingress_arena_v0.1/rules/`;
//! this crate must not contain a second copy of the rules.
//!
//! Milestone M1 modules: types (domain model), config (R-CONFIG-01),
//! geometry (P0-GEOMETRY integer math), rng (R-RNG-01 random oracle),
//! map (R-MAP-02 frozen definition + generator), spawn (R-SPAWN-01 gate).

#![forbid(unsafe_code)]

pub mod config;
pub mod geometry;
pub mod map;
pub mod rng;
pub mod spawn;
pub mod types;

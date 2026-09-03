//! Baseline bots for self-play (P2, M6). Five fixed-version, interpretable
//! measuring tools — not strong AI — that act ONLY on their faction
//! `Observation` (R-VIS enforced by types) plus `GameConfig` constants and
//! their own memory of their own past submissions. Runtime randomness comes
//! from each bot's own keyed BLAKE3 XOF stream (D53), never the runtime
//! `RandomOracle` and never a wall clock.

#![forbid(unsafe_code)]

pub mod bot;
pub mod common;
pub mod destroy_anchor;
pub mod greedy_field;
pub mod hybrid;
pub mod nearest;
pub mod policy;
pub mod r2;
pub mod random;
pub mod rng;

pub use bot::{BOT_VERSION, Bot, BotKind, BotStrategy, make_bot};
pub use policy::{
    PolicyFamily, PolicyId, PolicyParameters, PolicySpec, frozen_m6_specs, make_policy_bot,
};
pub use rng::BotRng;

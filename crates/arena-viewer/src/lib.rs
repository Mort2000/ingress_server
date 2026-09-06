//! Verified replay data for the Tactical Replay browser application.
//! Decodes and verifies IARP, then materializes round snapshots and telemetry.
//! Scores remain decimal strings to preserve integer precision in JavaScript.

#![forbid(unsafe_code)]

pub mod data;
pub mod json;
mod telemetry;

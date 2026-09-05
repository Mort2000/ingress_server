//! replay-export (M10, D87): the P6 replay-viewer export pipeline.
//!
//! The binary `replay-export` turns a verified IARP replay into ONE
//! self-contained HTML file (inline CSS + JS, embedded replay data) that
//! opens locally from `file://` with zero external requests. The viewer
//! source assets live at the repository root `viewer/` (P-ARCH-03 — outside
//! the Rust crates) and are `include_str!`-ed here; the EMITTED artifact is
//! the single HTML page.
//!
//! Pipeline (D88): decode → `arena_replay::replay` (full D46/D50
//! verification of every hash/delta/event/intake fact + trailer — an
//! unverified IARP is REFUSED, no HTML is produced) → a second
//! materialization pass re-running the same `Resolver::with_seed` +
//! `run_round`/`advance_round` loop, snapshotting
//! `Observation::for_faction(state, f)` per faction at each D45 round-close
//! point, ending with a final-hash equality assert against the verified
//! outcome (divergence tripwire). Snapshotting via `Observation` keeps the
//! embedded agent data on the one R-VIS construction path.
//!
//! Data schema `ingress-viewer-v1` (D89): per-round FULLY materialized
//! post-round state; jumping to round i is O(1) array access (IARP
//! checkpoints are embedded as display-only slider ticks, not seek
//! machinery). `score2`/`delta2`/`final_score2` travel as decimal STRINGS
//! (u128-exact, D91); every other embedded number is a plain integer
//! (< 2^53 by the D10 bounds). The viewer may compute with floats and
//! BigInts freely — it is display layer — but the embedded DATA stays
//! integer.
//!
//! R-VIS freeze (D90): by default (`--dev 0`, requires `--faction enl|res`)
//! the embedded `agents` object contains EXACTLY that faction's private
//! agents — enemy private data does not exist in the file (R-VIS by
//! construction; the other faction-view label renders disabled; enemy
//! knowledge = public events only). `--dev 1` embeds both factions,
//! activates all four view labels, sets `header.dev = true`, and shows an
//! on-screen debug caveat. In BOTH modes a Faction View never draws enemy
//! agents; Public View draws no agents at all.

#![forbid(unsafe_code)]

pub mod data;
pub mod json;
pub mod views;

mod telemetry;

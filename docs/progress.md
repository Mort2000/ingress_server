# Implementation Progress

Handoff doc between milestone implementation rounds. The orchestrator updates it
after each milestone; every agent should read it before starting work.

## Environment constraints (read first)

- **Network works through a configured cargo mirror** (crates.io is reachable via
  cargo itself; direct curl to the default CDN times out — probe with cargo, not curl).
- **Dependency policy (ADR-0013)**: official `blake3` only (version pinned by the
  committed `Cargo.lock`); M7 adds the official `flatbuffers` runtime + pure-Rust
  `planus` compiler (no `flatc` binary available). Everything else stays std-only.
  Never add other dependencies without recording it in an ADR and here.
- Toolchain: rustc/cargo 1.98.0 (system, at /usr/bin), 12 cores, 7.7 GB RAM.

## Authoritative specs

- Rules: `.scratch/ingress_arena_v0.1/rules/` (only source of rule semantics)
- Plan: `.scratch/ingress_arena_v0.1/mvp/` (milestones M1–M10, gates)
- Terms: `CONTEXT.md` · Decisions: `docs/adr/`

## Conventions for milestone agents

- Never `git commit` (orchestrator commits per milestone); keep `Cargo.lock` current when deps change.
- English comments; identifiers use CONTEXT.md terminology (Faction/Agent/Portal/Link/Field/Score).
- `arena-core` stays `#![forbid(unsafe_code)]`.
- Authoritative math: integers only, i32 coords promoted to i64 before subtraction,
  checked arithmetic, `Area2(u64)`, `Score2(u128)`; no floats in authoritative code.
- Handoff state: `cargo fmt --check` clean, `cargo test` green.

## Milestone log

| Milestone | Status | Commit | Notes |
|---|---|---|---|
| M0 skeleton | done | c0a1495 | workspace, ADR-0012; corrected by ADR-0013 (cargo mirror, official blake3) |
| M1 core geometry/RNG | done | (this commit) | 97 tests green; types/config/geometry/rng/map/spawn; decisions D1–D15 below |

## Frozen design decisions (M1, must not silently change)

- D1: `blake3("")` hex starts `af1349b9f5f9a1a6` (verified against pinned blake3 1.8.7; the `…a1a7` form is wrong).
- D2: canonical encodings use fixed-width unsigned little-endian integers, concatenated without padding.
- D3: `HACK_KEY_YIELD DiscreteUniform{1,2,3}` = `1 + uniform(addr, 3)`; config stores min/max (1,3).
- D4: `CLUSTERED_PORTAL_RATIO` stored as permille u32 (default 700); clustered count = floor(count·permille/1000).
- D5: `PORTAL_CLUSTER_COUNT` is one config u32, default 6.
- D6: cluster spread = `4 × portal_min_distance`; per-axis gaussian-like = clamped Irwin–Hall of 3 uniforms.
- D7: generated portals start Neutral with hp 0.
- D8: `segments_intersect` is the pure CLOSED predicate; R-LINK-02 endpoint/touch exceptions are composed by link rules, not geometry.
- D9: promote i32→i64 before any subtraction/square/cross; checked ops with `expect("bounded by GameConfig::validate")`; no floats/wrapping/saturating.
- D10: config validation caps (dims ≤ 2^20 cm, distance params ≤ i32::MAX, portal_count 2..=10_000, min distance ≥ 1) guarantee runtime checked ops cannot overflow.
- D11: `uniform(_, 0)` panics (documented); `n == 1` naturally returns 0.
- D12: map generation is pre-match tooling on its own `"AMAP"` blake3 stream — never `RandomOracle`, never replayed.
- D13: spawn-set errors: per-entry submission order (unknown → duplicate → out-of-bounds), then missing ids sorted; no coordinate-coincidence checks.
- D14: `Command` enum currently carries only `Move`; later milestones add the rest.
- D15: `game_seed: u64` is match-setup input (Replay Header material from M5); RandomOracle is the only runtime randomness source.

## Known minor debt (from M1 review — fix when touched, or in final review)

- `geometry.rs` public fns (`dist2`, `within_closed_range`, `cross`) rely on the documented GameConfig bounds precondition; callers MUST validate `in_bounds` (etc.) before calling so illegal inputs become DROPs, not panics.
- `InitialSpawns::apply` silently skips unknown agent ids if applied to a different roster than validated against.
- One spawn test (`invalid_set_applies_nothing`) cannot fail as written.
- `RandomOracle::uniform` panics on `n == 0` by contract (documented; acceptable).
- Error enums lack `std::error::Error`/`Display`; `MapError::InvalidConfig` conflates config vs definition defects; comments reference decision IDs D1–D15 now recorded here.

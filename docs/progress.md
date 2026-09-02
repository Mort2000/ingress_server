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

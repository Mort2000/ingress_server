# Ingress Arena

Code-arena where AI clients control Agents on a 2D integer grid, contest Portals,
build non-crossing Link graphs, and score Fields by area × time.

- Rules (source of truth): [`.scratch/ingress_arena_v0.1/rules/`](./.scratch/ingress_arena_v0.1/rules/README.md)
- Terminology: [`CONTEXT.md`](./CONTEXT.md)
- MVP plan and milestones M1–M10: [`.scratch/ingress_arena_v0.1/mvp/`](./.scratch/ingress_arena_v0.1/mvp/README.md)
- Decisions: [`docs/adr/`](./docs/adr/README.md)

## Workspace

| Crate | Responsibility |
|---|---|
| `crates/arena-core` | Authoritative game model, Resolver, deterministic settlement |
| `crates/arena-bots` | Local baseline bots |
| `crates/arena-protocol` | FlatBuffers schema subset, framing, versioned messages |
| `crates/arena-server` | Runtime: process, SHM, RPC, timers |
| `crates/arena-client-sdk` | Participant API, zero-copy observation views |
| `crates/arena-runner` | Batch matches, map sets, statistics, ranking |
| `crates/arena-replay` | Replay encode/decode, checkpoints, canonical state hash |
| `viewer/` | Replay UI |
| `bots/` | Standalone client examples |
| `tools/` | Replay CLI, audit and statistics tools |

## Build & test

```sh
cargo build
cargo test
```

Dependencies follow [ADR-0013](./docs/adr/0013-official-crates-via-cargo-mirror.md):
the official `blake3` crate (pinned through the committed `Cargo.lock`), and from
M7 `planus` (pure-Rust FlatBuffers-compatible compiler + runtime) for protocol
schema codegen; generated code is committed. Everything else is std-only; replay and canonical-state encodings
are custom deterministic formats. The environment reaches crates.io through a
configured cargo mirror.

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
| `crates/arena-blake3` | In-house BLAKE3 (hash / keyed / derive_key / XOF), no dependencies |
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
cargo build --offline
cargo test --offline
```

The repository is **dependency-free** (std only) by decision
[ADR-0012](./docs/adr/0012-offline-stdlib-only-dependency-closure.md): the build
environment has no network access, and every algorithm the rules require
(BLAKE3, the FlatBuffers wire subset) is implemented and tested in-tree.

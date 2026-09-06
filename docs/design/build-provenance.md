# Build provenance: audited builds (M8 / P4-BUILD)

> Alpha 布局更新：参赛源码位于独立 bot 仓，通过 `audit --root <server> --participant <bot>` 审核；官方 path 依赖目标为 server 的 `crates/*`／`tools/*`，并检查选手自身的 Cargo.lock。原 `probe-client` 已移至 `tools/probe-client`。旧版本路径只表示历史布局，当前接入见 [Alpha 记录](alpha-reference.md)。

Status: implemented (M8; dependency resolution reversed by D94 — see
§2). Every match product must be traceable to the exact audited inputs:
source tree, lockfile, toolchain identity, and binary hashes (ADR-0005,
P4-BUILD). This document records the mechanism, the closure, the
integrity basis, and the operator runbook.

## 1. Dependency policy and the frozen closure

Policy (mirror adoption, amended M7): the sanctioned external
set is exactly **blake3 + planus**; everything else is std-only. The
committed `Cargo.lock` pins precisely this closure — 15 external
crates, no more:

| Crate | Version | Why it is in the closure | Native findings (audit R3) |
|---|---|---|---|
| blake3 | 1.8.7 | sanctioned: hashing / RNG oracle | **build.rs** (C/asm intrinsics selection) — SANCTIONED |
| cc | 1.4.4 | build-dep of blake3 | compiles the C/asm intrinsics — SANCTIONED (build-dep) |
| arrayvec | 0.7.8 | blake3 dep | none |
| cfg-if | 1.0.4 | blake3 dep | none |
| constant_time_eq | 0.4.2 | blake3 dep | none |
| cpufeatures | 0.3.1 | blake3 dep | none (runtime CPUID, no build.rs) |
| libc | 0.2.189 | cpufeatures dep | **build.rs** — SANCTIONED |
| planus | 1.3.0 | sanctioned: FlatBuffers-compatible runtime | none — **pure Rust, no build.rs, no proc-macros** |
| array-init-cursor | 0.2.1 | planus dep | none |
| hashbrown | 0.16.1 | planus dep | none |
| allocator-api2 | 0.2.21 | hashbrown dep | none |
| equivalent | 1.0.2 | hashbrown dep | none |
| foldhash | 0.2.0 | hashbrown dep | none |
| find-msvc-tools | 0.1.11 | cc dep (Windows targets) | none |
| shlex | 2.0.1 | cc dep | none |

The audit's R3 report derives native capability from the Cargo.lock
external set on every run (and from a `vendor/` source scan whenever a
vendored closure is present — dormant since D94); any native capability
NOT in the frozen sanctioned pair (blake3 + cc, libc) fails the audit.
The frozen allowlist is pinned equal to the actual lockfile closure by
a test (`frozen_allowlist_matches_the_lockfile_closure`) — they cannot
drift.

## 2. Dependency resolution and integrity (D94)

The M8 design froze the closure in-tree with `cargo vendor` and
redirected crates.io via `.cargo/config.toml` (offline-first builds).
D94 reversed that: the original requirements never asked for
mirror-independent or self-contained builds, and this environment has
in fact always resolved through the configured cargo mirror. Since D94:

* Dependencies resolve through the cargo mirror per the committed
  `Cargo.lock`; `vendor/` and `.cargo/config.toml` are removed and
  offline resolvability is no longer a property of this repository.
* **Integrity is unchanged**: every one of the 15 lockfile entries
  carries a SHA-256 checksum, and `--locked` builds verify each
  downloaded `.crate` against it — a hostile mirror cannot substitute
  bytes without a SHA-256 preimage. Lock + mirror gives the same
  integrity guarantee the vendored closure gave; what is given up is
  availability-independence and self-containment, neither of which is
  a requirement.
* Builds use `cargo build --release --locked`; adding or updating a
  dependency still requires recording it (ADR gate) and updates the
  lockfile, which changes `lock_hash` and `tree_hash`.

## 3. Build evidence (history and current practice)

The M8–M-final era used a frozen offline-proof recipe
(`scripts/offline-build-proof.sh`: fresh CARGO_HOME + target, `env -i`,
`--release --locked --offline`), recorded per milestone handoff under
`runs/builds/<id>/`. Recorded demonstration run (tree at proof time):
`runs/builds/20260903-0011-34ac97841d9d` — tree hash
`34ac97841d9da5c90792ede13223c43952eff43412c5099c45f3823b72632801`,
919 files hashed, lock hash
`e721b8419ffeaf9f64248f542408cc00a8dd1cacbc6098053b8124209a79075c`,
exit 0. Those manifests remain valid history: they attest the trees
they hashed (pre-D94 trees included `vendor/`; tree_hash values are
NOT comparable across the D94 removal — the hashed set shrank).

Current practice: build with `cargo build --release --locked`, then
record the manifest with `target/release/provenance --root .` (see the
runbook); verify any tree the same way and compare `tree-hash`.

## 4. Toolchain pinning

* `rust-toolchain.toml` pins `channel = "1.98.0"` (+
  `components = ["rustfmt"]`). The rustup proxies at `/usr/bin/cargo`
  and `/usr/bin/rustc` HONOR the file, so every cargo command inside
  the repository resolves 1.98.0 — verified (`cargo -V` → 1.98.0
  under the pin; `stable` on this host happens to be the same
  1.98.0/88d9e12ae build).
* **Hazard**: a pin without the toolchain installed breaks EVERY cargo
  command in the repository (rustup downloads on demand only when a
  network is reachable). Provisioning is the one out-of-band network
  step — rustup itself is not resolved through the cargo mirror:

  ```bash
  rustup toolchain install 1.98.0 --profile minimal --component rustfmt
  ```
* The pinned rustfmt component is required: without it `cargo fmt`
  (used at every milestone handoff) fails under the pin even though
  builds succeed.

## 5. D71 canonicalization (frozen, verbatim)

* **tree_hash** = blake3 keyed with
  `derive_key("ingress-arena-tree-v0.1")` (the official KDF
  construction; the pinned blake3 1.8.7 predates the `new_der_key`
  convenience wrapper for it) over the stream: for each regular file
  in ascending order **sorted by the raw path bytes** (POSIX
  `/`-relative UTF-8 paths):
  `u32le(path_len) ‖ path ‖ u64le(content_len) ‖ content ‖ u8(exec_bit)`;
  terminated by `u64le(file_count)`. Walk root = the repository. The
  frozen exclusion set prunes any directory named `.git`, `target`,
  `runs`, or `.scratch` (build outputs, evidence dirs, planning
  scratch). Everything else in the tree participates — whatever the
  tree contains is the audited input (since D94 there is no `vendor/`;
  dependencies resolve via mirror per the lockfile). Symlinks are an
  error (never followed).
  `exec_bit = (mode & 0o111 != 0) as u8` — git's only tracked mode
  bit.
* **lock_hash** = same construction under
  `"ingress-arena-lock-v0.1"` over the raw `Cargo.lock` bytes.
* **toolchain_hash** = `"ingress-arena-toolchain-v0.1"` over
  `rustc -Vv` stdout — an identity proxy, not a full toolchain content
  hash (stated honestly; the channel pin + the recorded `rustc -Vv`
  text is what the operator verifies).
* **binary hash** = `"ingress-arena-bin-v0.1"` over the file bytes,
  recorded as `name ‖ size ‖ hex`.

The byte-exact stream is pinned by the golden test
`tree_hash_matches_the_hand_assembled_canonical_stream`, which
hand-assembles the encoding for a known fixture and feeds it to the
same domain.

## 6. Manifest schema

`runs/builds/<build-id>/manifest.json`, `<build-id> = YYYYMMDD-HHMM-<tree_hash[0..12]>`
(hand-rolled ordered JSON, std-only, no serde):

```json
{
  "schema": "ingress-provenance-v1",
  "tree":     { "hash": ..., "file_count": ..., "excluded_dirs": [...] },
  "lock":     { "hash": ... },
  "toolchain":{ "channel": ..., "commit_hash": ..., "rustc_vv": ..., "hash": ... },
  "binaries": [ { "name": ..., "size": ..., "blake3": ... } ]
}
```

Fixed key order; **no wall-clock fields in the body** — two
computations over the same tree are byte-identical (pinned by
`double_compute_is_deterministic`); the timestamp lives only in the
build id/directory name. Recorded products: `arena-server`, the `probe-client` protocol fixture, and the tools binaries (`provenance`, `audit`,
`sandbox-runner`, the five sb-fixtures).

## 7. Reproducibility and mutation evidence

* Determinism: two `compute()` runs → identical hashes and manifest
  bodies.
* Mutation sensitivity: one appended byte in one file changes
  `tree_hash`; so do a rename, a move between directories, and an
  exec-bit flip.
* Exclusion correctness: changes under `target/`, `.git/`, `runs/`,
  `.scratch/` never change `tree_hash`; any non-excluded content
  change does.
* Path order: ascending raw bytes, not locale collation — pinned by
  fixtures whose adjacent paths differ above ASCII
  (`paths_sort_by_raw_bytes_not_locale`).
* Symlinks: refused, never followed.

## 8. Operator runbook (one match product, end to end)

```bash
# 0) provision the pinned toolchain (once, out-of-band network step)
rustup toolchain install 1.98.0 --profile minimal --component rustfmt

# 1) white-box audit the participants (exit 0 = PASS)
cargo run -p audit                      # or: target/debug/audit --root .

# 2) audited build (mirror + committed lockfile) + evidence record
cargo build --release --locked
target/release/provenance --root .      # runs/builds/<id>/manifest.json

# 3) run matches from the RECORDED products only
target/release/arena-server --socket ... --match-id <fresh-id> ...
# clients are executed under the sandbox runner (docs/design/sandbox.md):
target/release/sandbox-runner --rw <scratch> -- target/release/<client> <socket>
```

The audit gate (step 1) covers the full dependency graph, not just the
top-level crate: participant manifests (every dependency spelling —
dotted `[dependencies.x]` tables and `target.*.dependencies` sections
included; `path =` deps must resolve to workspace member crates
`crates/*`/`bots/*`/`tools/*`),
`Cargo.lock` (every `source =` entry — registry, git, or sparse — must
be one of the frozen 15 names), and every `.rs` file under each
`bots/*` crate directory (not only `src/`; `#[path]`/`include!`
inclusion forms are flagged for operator review since they can name
files outside the crate directory). Match play uses the binaries whose
hashes the manifest recorded; any other artifact is out of policy.

# Sandbox: capability boundaries and resource isolation (M8 / P4)

> Alpha 布局更新：参赛源码位于独立 bot 仓，通过 `audit --root <server> --participant <bot>` 审核；官方 path 依赖目标为 server 的 `crates/*`／`tools/*`，并检查选手自身的 Cargo.lock。原 `probe-client` 已移至 `tools/probe-client`。旧版本路径只表示历史布局，当前接入见 [Alpha 记录](alpha-reference.md)。

Status: implemented (M8). This document is the honest enforcement map:
what is technically enforced, what is audit-enforced, and what is
explicitly NOT claimed.

## 1. Threat model

v0.1 runs **audited safe-Rust participants built by the operator from
source** (`bots/*`: every crate is `#![forbid(unsafe_code)]`, verified
by the white-box audit, `tools/audit`). The operator builds the whole
closure per the committed `Cargo.lock` (via the cargo mirror, D94) with
a pinned toolchain and records every input and product hash
(`docs/design/build-provenance.md`).

**v0.1 does NOT claim safe execution of arbitrary untrusted code.**
Verbatim from the P4 gate: 「支持任意不可信 Client 不属于 v0.1 Gate」
(supporting arbitrary untrusted clients is out of scope for the v0.1
gate). Container/cgroup/rlimit/landlock isolation limits resources and
removes accident surfaces; it is not a security boundary against a
hostile adversary with a compiler. A non-compliant same-UID process
retains the powers of its UID (see §2 "not claimed").

## 2. Enforcement matrix

| Boundary | Class | Mechanism | Evidence |
|---|---|---|---|
| No `unsafe` in participant crates | audit (R1) | lexical strip + token scan over EVERY `.rs` file under each participant crate directory (not only `src/` — `#[path]`/`include!` can pull source from anywhere in the crate); crate root must carry `#![forbid(unsafe_code)]`; the `#[path]`/`include*!` inclusion forms themselves FAIL as operator-review findings (they can name files OUTSIDE the crate directory, where no scan runs) | `tools/audit` tests: `clean_fixture_passes_every_rule`, `unsafe_block_and_missing_forbid_fail_r1`, `path_included_module_outside_src_fails_r1_r4`, `include_macro_fails_r1_as_operator_review`, `audit_passes_on_real_participants` |
| Sanctioned deps only | audit (R2) | manifest parser: path deps CONTAINED to workspace member crates (`crates/*`/`bots/*`/`tools/*`) or the frozen 15-name set; `git =`/registry/renamed sources FAIL (key-based, spacing-tolerant); dotted `[dependencies.x]` tables and `target.*.dependencies` sections (cfg OR triple) parse like plain sections; the lockfile backstop counts every `source =` entry (git/sparse included), not only registry | `bad_dep_and_git_dep_fail_r2`, `renamed_and_registry_sources_fail_r2_even_with_allowed_names`, `dotted_and_triple_target_dep_sections_fail_r2`, `path_dep_escaping_the_workspace_fails_r2`, `git_sourced_lock_entries_count_as_external`, `frozen_allowlist_matches_the_lockfile_closure` |
| No native code from participants | audit (R3) | participant `build.rs`/build-deps/proc-macro/`links =`/cdylib FAIL; closure natives reported with verdicts (blake3 + cc C intrinsics, libc build.rs = SANCTIONED per) | `participant_build_rs_fails_r3`, `closure_native_report_flags_unknown_and_sanctions_the_frozen_pair`, `real_closure_has_no_unknown_native_capability` |
| No networking tokens in participants | audit (R4) | `std::net`/Tcp*/Udp*/SocketAddr/lookup_host absent after strip | `net_import_fails_r4` |
| No raw SHM/FFI in participants | audit (R5) | `extern`/`shm_open`/`mmap`/`MAP_SHARED`/`/dev/shm`/`syscall` absent; SHM access only via the official SDK | `shm_ffi_fixture_fails_r5`, `operator_crates_are_the_documented_exemption` |
| CPU limit | technical | `RLIMIT_CPU` (soft=hard) → SIGXCPU | `cpu_limit_kills_the_spin_loop` (sb-spin, 1 s cap, `cpu_limit`) |
| Memory limit | technical | `RLIMIT_AS` (default 2 GiB) → allocation-failure abort | `address_space_limit_aborts_the_malloc_bomb`, `default_two_gib_limit_enforced` |
| Wall clock | technical | watchdog SIGKILLs the child's whole process GROUP at the deadline; surviving members killed again after the reap (containment); the output drain is deadline-bounded (wall + 1 s grace) — a lingering grandchild cannot wedge the runner | `wall_clock_watchdog_sigkills` (400 ms, `wall_timeout`), `orphaned_grandchild_cannot_wedge_the_runner` |
| Threads/forks | technical (caveat) | `RLIMIT_NPROC` = 256 (uid-wide; see §3) — the frozen constructor default is the ONLY default (the CLI overrides it only when `--nproc` is passed) | covered indirectly by every sandboxed run; fork-heavy tests deliberately fork-free; `cli_has_no_own_nproc_default` |
| File size / fds / core | technical | `RLIMIT_FSIZE` 64 MiB, `RLIMIT_NOFILE` 256, `RLIMIT_CORE` 0 | `scratch_writes_fit_under_the_file_size_cap` |
| Filesystem writes | technical | landlock (ABI 7 live here): read+exec on RO roots only, read+write on ONE scratch dir | `landlock_fs_and_no_new_privs_enforced` (`RO_WRITE=denied`, `RW_WRITE=ok`) |
| TCP bind/connect | technical | landlock network layer (ABI ≥ 4) denies all TCP | `landlock_denies_tcp_connect_to_a_live_listener` (kernel `PermissionDenied` vs unsandboxed control `ok`) |
| UDP | **not kernel-enforced** | not landlock-coverable (D76) — covered by the audit chain (R1 forbids the `unsafe` needed for raw sockets; R4 bans `std::net` tokens; R2 keeps net crates out of the closure) | audit tests above |
| Privilege escalation | technical | `PR_SET_NO_NEW_PRIVS` before exec (required by landlock) | `NOPRIVS=1` in `landlock_fs_and_no_new_privs_enforced` |
| Env scrub + cwd | technical | `env_clear` + `ARENA_*`/`PROBE_*` allowlist, cwd = scratch dir | `environment_scrubbed_and_cwd_is_the_scratch_dir` |
| SHM write by clients | technical + construction | clients map `PROT_READ` only (SDK twin); publisher side is the server | `session_maps_only_own_partitions_and_every_mapping_is_read_only` (`/proc/self/maps` shows `r--`, never `rw`, for SDK readers) |
| Enemy private state via SDK | technical | `Session` opens exactly public + OWN partition; no API path opens the enemy name; raw-mapping escape hatch absent | same test (no `-res` mapping for an ENL session) + compile-fail `raw_mapping_unreachable.rs` (E0599) |
| Enemy private state via same-UID open | **audit boundary (D77)** | a non-compliant same-UID process CAN `shm_open` the enemy region name in v0.1 (unguessable per-match id, 0600, unlinked at drop — but same UID wins). This is exactly what R1/R5 + the operator-built closure exist to prevent | documented live in `session_maps_only_own_partitions_and_every_mapping_is_read_only` (the open succeeds and is immediately dropped; recorded, not asserted away) |
| Settlement-phase SHM reads | SDK + audit | `PhaseGate` refuses reads between ROUND_CLOSED and the next ROUND_START (R-RESOLVE-01); audit confirms no bypass | M7 evidence (D66): `settlement_phase_refuses_reads`, `lent_views_refuse_reads_after_round_closed`; compile-fail harness (E0106/E0521/lifetime) + M8 `raw_mapping_unreachable.rs` |
| Regions before publish / after match | technical | created `O_EXCL` at publish, unlinked at drop; fresh match id → no `/dev/shm` files | `no_region_files_exist_before_first_publish`, `regions_are_unlinked_when_the_match_end` |
| Server survives client death under the sandbox | technical | EOF → CLIENT_FORFEIT; server untouched | `sandbox_killed_client_forfeits_and_server_finishes_cleanly` (8 MiB AS kill at first SHM mapping; exit 6/7; journal; abnormal IARP trailer) |
| Side channels (timing, CPU cache, allocator), cross-match leakage via leftovers of OTHER runs, physical container escape | **not claimed** | — | — |

## 3. Resource limits (defaults, D74) and measured facts

| Resource | Default | Enforcement | Live measurement (this host) |
|---|---|---|---|
| Address space | 2 GiB | `RLIMIT_AS` | malloc bomb (VA-only growth) aborts ≈0.2 s under a 64 MiB cap; classified `mem_limit` |
| CPU | 2 × wall allowance | `RLIMIT_CPU` | sb-spin killed at 1.00 s (SIGXCPU), `cpu_limit` |
| Wall clock | per-run (watchdog) | SIGKILL of the child's process group | 400 ms → kill at 404 ms, `wall_timeout`; a grandchild holding the pipe past its parent (`sh -c 'sleep 20 & exec /bin/true'`) used to wedge the runner for the orphan's lifetime (measured 20.0 s on a 1.5 s budget; `sleep infinity` hung) — group kill + containment + bounded drain fix it (measured 21 ms) |
| NPROC | **256** (not the planned 64) | `RLIMIT_NPROC` (uid-wide) | measured live: a 64 cap broke even an `sh` pipeline fork on this host (ambient per-uid count ≈ 56+ threads/processes under a parallel test load); 256 keeps a bounded cap with headroom |
| FSIZE | 64 MiB | `RLIMIT_FSIZE` | scratch writes legal under 1 MiB cap (`RW_WRITE=ok`) |
| NOFILE | 256 | `RLIMIT_NOFILE` | every sandboxed run |
| CORE | 0 | `RLIMIT_CORE` | — |

Spawn order (frozen, D74): rlimits → cwd + env scrub (on the
`Command`) → `PR_SET_NO_NEW_PRIVS` → landlock rules →
`restrict_self` → `exec`. The pre-exec callback performs direct
syscalls only (async-signal-safe).

## 4. cgroup v2: probed, degraded, never fatal (D75)

Measured fact on this host: `/sys/fs/cgroup` is mounted rw but NOT
delegated — `mkdir /sys/fs/cgroup/...` → `EACCES` for the user. The
runner therefore:

* probes the configurable root, attempts to create
  `ingress-sandbox-<pid>/` with `cpu.max` = 2 CPUs, `memory.max` =
  2 GiB, `pids.max` = 64, and attaches the CHILD pid;
* on failure records `"cgroup not enforced (...)"` in the result JSON
  and continues under rlimits + landlock — visible in every run's
  `degraded` array (`real_cgroup_root_eacces_is_a_recorded_degradation`);
* the writer logic itself is fully tested against a temp FAKE root
  (`cgroup_writer_against_a_fake_root`: files, contents, pid attach,
  removal on drop).

Production shape (spec P4-CAPABILITY table): each client runs in its
own container with a per-client cgroup (2 CPU / 2 GiB / bounded pids /
no external net / read-only fs + temp dir). The rlimit+landlock runner
is the same policy rendered without a container, so M8 tests exercise
the enforcement on developer hardware; deployment wraps it in the
container per the spec table. The server uses a separate, stable
quota (not shared with clients).

## 5. Landlock details (D76)

* ABI probed at runtime (`landlock_create_ruleset(NULL, 0, VERSION)`);
  this host: **ABI 7** (WSL2 6.18). The probe result is recorded in
  every result JSON.
* FS layer at ABI ≥ 1: handled set grows with the ABI (REFER at 2,
  TRUNCATE at 3, IOCTL_DEV at 5); RO roots = `/usr`, `/lib`, `/lib64`,
  `/etc`, `/proc`, `/dev`, `/dev/shm` (read: clients must read SHM
  regions) plus the child binary's own directory; the single rw dir
  gets the full write set.
* NET layer at ABI ≥ 4: `bind_tcp` + `connect_tcp` handled and granted
  nowhere → all TCP denied. On a pre-4 kernel the runner records the
  degradation and the audit chain (R1/R2/R4) is the boundary.
* PR_SET_NO_NEW_PRIVS on this host's headers is **38** (verified live:
  `prctl(38, 1)` sets `NoNewPrivs: 1`; the commonly cited 39 returns
  `EINVAL` here).

## 6. M7 debt disposition (D78)

Both `shm.rs` twins (server publisher, SDK reader) were hardened —
behavior-preserving:

1. **payload_len bound**: the SHM-derived active-buffer length is
   checked against the immutable capacity BEFORE any slice is built
   (`ShmError::Corrupt`); previously the single-writer invariant was
   the only guard. Pinned by
   `oversize_shm_derived_payload_len_is_refused_as_corrupt` (server)
   and `sdk_reader_refuses_oversize_shm_derived_payload_len` (SDK,
   cross-crate on server-published regions) — both corrupt the length
   through the raw `/dev/shm` file from outside the mapping and watch
   the refusal, then repair and read again.
2. **read-side `fence(Acquire)`** between the payload/metadata
   snapshot and the confirming seq load. The publisher was already
   Release-ordered and x86-TSO made the missing fence unobservable;
   the fence closes the formal gap (weaker orderings, compiler
   reordering).

## 7. Test evidence index

Every sandbox/isolation claim above is pinned in the default
`cargo test` run:

* `tools/provenance` (10) — D71 canonicalization, determinism,
  mutation sensitivity, exclusions, byte-order sort, manifest shape.
* `tools/audit` (28 = 24 integration + 4 lexer unit) — lexer pins,
  manifest parser (incl. dotted `[dependencies.x]` tables, triple
  target sections, spacing-tolerant key extraction), fixtures (clean +
  8 mutants, incl. the `#[path]`-included-module bypass),
  real-participant pass, allowlist == lockfile (every `source =` entry
  counts), closure native report, bin exit codes.
* `tools/sandbox-runner` (10) — cgroup fake root, EACCES degradation,
  spawn errors, JSON shape, default limit set, CLI carries no own
  nproc default, handled-set growth REFER 2 / TRUNCATE 3 / IOCTL_DEV 5
  (handled-not-granted), exactly-one-unsafe-module source shape.
* `tools/sb-fixtures` (11) — AS/CPU/wall enforcement, landlock fs +
  NoNewPrivs, TCP deny with control, env scrub + cwd, fsize,
  orphaned-grandchild drain bound.
* `crates/arena-server`: `isolation.rs` (3) — pre-publish absence,
  unlink at end, name grammar; `clients.rs`
  `sandbox_killed_client_forfeits_and_server_finishes_cleanly` —
  sandbox→server end to end; `shm.rs` D78 unit test.
* `crates/arena-client-sdk`: `isolation.rs` (2) — maps-based `r--` +
  own-partition-only + same-UID residual recorded; `runtime.rs` D78
  twin test; `compile_fail/raw_mapping_unreachable.rs` — no
  raw-mapping escape on participant-facing types.

Flake note: the full-workspace suite runs 53 test binaries in
parallel; the M7 timing tests (40 ms decision windows) can transiently
fail under extreme cross-binary load (observed once for
`split_and_pipelined_writes_reassemble_in_order` during a run that
coincided with the compile-fail harness's nested cargo builds; three
isolated reruns and two full-suite reruns green). This is the
documented M7 stability caveat, unchanged by M8.

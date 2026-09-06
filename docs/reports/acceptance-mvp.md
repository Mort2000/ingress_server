# MVP Acceptance Record (M-final)

One section per `A-*` row of `docs/spec/mvp/06-roadmap-acceptance.md`
("MVP Done"). Verdicts are **MET**, **MET-VIA-ESCAPE-HATCH** (met through a
documented, plan-sanctioned deviation), or **PARTIAL** (with the reason and
what remains). The roadmap bookkeeping line applies to every row:
*Game Config · Seed Set · Server/Bot build version · test-report location*.

Shared bookkeeping (referenced by every row below rather than repeated):

| Item | Value |
|---|---|
| Code base | commit `1bec84c` + this milestone's changes (the orchestrator records the milestone commit; every experiment below names its own base commit) |
| Rules-engine (server) version | `arena_replay::format::SERVER_VERSION = 1` |
| Wire protocol version | `arena_protocol::PROTOCOL_VERSION = 1` — equal to the IARP header constant by the `protocol_version_tie` test (M-final) |
| RNG / replay encoding | `RNG_VERSION = 1`, `STATE_ENCODING_VERSION = 1` |
| Bot build | arena-bots `BOT_VERSION = "m6.0"` (fixed version, D53/D54); external clients are the same frozen algorithms behind the SDK (`nearest-client`, `hybrid-client`, `probe-client`) |
| Default config | `GameConfig::default()` (R-CONFIG-01), identity hash `0x9ead052219fa7ff3` (blake3 over the canonical 31-word encoding); per-experiment deviations are named in each row |
| Toolchain | rustc/cargo 1.98.0 pinned by `rust-toolchain.toml`; offline closure committed under `vendor/` (15 crates, D72; reversed by D94); build-provenance manifests recorded per milestone handoff under `runs/builds/` |
| Suite | `cargo test --workspace` green in debug AND release at handoff (see the handoff note in `docs/progress.md`); `cargo fmt --check` clean |

---

## A-CORE — Game Core — **MET**

*Supports all v0.1 rules; a full default match (500 rounds) completes
stably; handles at least 200 portals; no obvious unnecessary >O(N²)
bottleneck.*

- **All v0.1 rules**: MOVE / ATTACK / CAPTURE / RECHARGE (M2), HACK /
  TRANSFER + epochs (M3), LINK / FIELD / layered fields / scoring /
  reinforcement (M4), each with its rules-reference cross-checks in
  `crates/arena-core/src/{state,resolve,observe,linkgraph}.rs` tests
  (239 tests at M7; the module test suites are the rule-by-rule evidence).
- **500 rounds stable**: `arena_core::harness::tests::gate_m4_500_round_match_completes`
  (120 portals, default config, fields built on both sides, 500/500 rounds,
  counter at 501); determinism twin `gate_m4_full_match_determinism`
  (identical `Vec<RoundReport>` + `GameState`).
- **≥ 200 portals (M-final)**: `arena_core::harness::tests::gate_final_300_portal_500_round_match_completes`
  — a 300-portal map from the FROZEN generator (map 1,000,000 cm square,
  clustered ratio 300‰ so the generator stays feasible), a full
  default-length 500-round match in-process with the same scripted
  FieldBot, asserting completion, 300 portals live through the match,
  both fields built, scores > 0 with R-SCORE-01 delta telescoping, the
  plain score-comparison result, and the four reinforcement boundaries.
  **Measured 20 ms in a debug build** (map generation and triangle search
  included) on the 12-core dev box; the assert bounds it at 60 s to catch
  complexity regressions without pinning hardware.
- **No unnecessary >O(N²) bottleneck**: authoritative structures are
  incremental — `linkgraph.rs` maintains links/adjacency/field cache per
  mutation (`gate_m4_field_cache_equals_full_recompute_at_match_end` pins
  the cache equals the from-scratch recompute on the real 500-round final
  state; the O(N³) recompute is a diagnostic, never on the hot path);
  ordering is a linear merge over the round's streams (D19). The known
  super-linear spots are enumerated under Residual risk and none is
  accidental.
- Config: `GameConfig::default()` (300-portal stress: 1,000,000 cm map,
  clustered ratio 300‰, everything else default). Seeds: stress map seed
  `0x0055_0000_0300`, game seed `0xBEEF_0004` (pinned in the test).
  Reports: the test itself + this file.

## A-DETERMINISM — Determinism — **MET**

- **Golden vectors**: the M1 RNG golden tests pin `blake3` oracle draws
  (`arena_core::rng` tests, D1); canonical-encoding golden bytes + hashes
  for a fresh and a mid-match state (`arena_replay::canon` tests, D44);
  Elo and map-codec golden vectors in arena-runner.
- **Same inputs ⇒ identical canonical state hash and final state**: the
  1,000-match replay gate `arena_replay::tests::gate_1000` (D50) — every
  match is recorded, serialized, decoded, re-executed from the header
  alone, and compared on EVERY per-round state hash, score delta, event
  slice, and intake fact, then re-encoded byte-exact. Evidence (debug,
  12 threads): matches=1000 skips=0 elapsed≈14.2 s, ≈282 KB/match.
  Seed set: game seeds `LE-u64(blake3("ingress-arena-m5-game" || m_le32))`,
  m = 0..1000, distinct map seeds from the `…-m5-map…` domain; strategy
  mix m%4 ∈ {FF, FA, AF, AA}. Worker-count independence (12 vs 1) and
  sub-batch reproducibility are pinned by arena-replay/arena-runner tests.
- Cross-engine agreement: the M7 live match (external client binaries)
  reproduces EVERY per-round state hash by independent in-process
  re-execution of the recorded submissions (see A-RUNTIME).
- Config: `GameConfig::default()`; report location: the gate test +
  `docs/progress.md` D50 entry.

## A-RUNTIME — Runtime — **MET**

*Two clients run independently; zero-copy client-side Observation;
deadlines fire; a slow client never blocks; a client crash never crashes
the server.*

- **Two independent external clients**: `arena-server` live-match tests —
  a 500-round match with the REAL `nearest-client`/`hybrid-client`
  binaries over a Unix socket in ~3 s (debug), both clients receiving
  identical GAME_END payloads, the archived IARP replaying via
  `arena_replay::replay`, and an independent in-process re-execution
  reproducing every per-round hash (M7 evidence; `ARENA_SKIP_LIVE=1`
  skips for tight iteration).
- **Zero-copy Observation**: the SDK reads the SHM ACTIVE buffer in place
  and hands out lifetime-bounded `ObservationView`s (M66; compile-fail
  harness pins that views cannot be stored, escaped, or detached).
- **Deadlines fire / slow client never blocks**: the transport matrix
  (14+ tests) — per-round `recv_timeout` to the active deadline, a slow
  client (1000 vs 0 permille window) settles at its window, late and
  cross-deadline frames are transport-dropped (D64), and every server
  stream carries a 2 s write timeout (`SERVER_WRITE_TIMEOUT`, M7.1) so a
  client that stops reading forfeits (`write stalled`) instead of wedging
  the driver.
- **Client crash ≠ server crash**: a client killed mid-match (including
  by the sandbox) yields CLIENT_FORFEIT, a clean server exit (6/7), the
  journal, and the abnormal IARP trailer while the peer completes
  normally (arena-server clients.rs + isolation tests; M8 evidence).
- Config: `arena_server::fast_config(100)` for the tournament live runs;
  default config for the 500-round live match. Build: server version 1,
  bot binaries `m6.0`. Reports: arena-server test suite,
  `docs/progress.md` M7/M7.1 entries.

## A-LATENCY — Latency Competition — **MET** (conflict clause realized
offline, M-final — D70's deferral is now closed)

*Trackable submission latency, bucket distribution, and conflict results;
strategy speed benefit distinguishable from scheduling noise.*

- **Submission latency + bucket distribution + timeout rate**: per-faction
  metrics in every external match's `result.txt` since M7 (D70) —
  latency histogram bins, priority-bucket distribution from the recorded
  execution order, missed-deadline rate with prompt COMMITs counted
  separately (`committed_passes`), notified-first counts. Archived for all
  14 r1 matches under `runs/tournaments/r1/matches/m*/server/result.txt`.
- **Conflict results / conflict win rate (M-final)**: offline replay
  analysis `arena_replay::analysis` — a conflict is two opposing-faction
  streams in the same round targeting the same object with opposing
  intent (double CAPTURE, geometrically crossing LINKs, ATTACK vs RECHARGE
  on the same portal; TRANSFER never counts, R-RESOLVE-04). The pass
  VERIFIES the archive first (`arena_replay::replay`, P6-GATE reference),
  re-executes to recover per-command outcomes, names the winner (effect
  executed; the loser DROPped or acted on the pre-emptied state), and
  aggregates by bucket position. Operator entry point:
  `cargo run --release -p arena-runner -- conflict-stats runs`.
- **Numbers (this machine, debug-verified archives, release runner)** —
  frozen self-play batch `runs/conflict-r1/` (8 matches: Hybrid×Hybrid ×4,
  GreedyField×DestroyAnchor ×4, `GameConfig::default()` = 500 rounds,
  Flat(0) buckets, master seed `0x006d3a00000000c1`) plus every archived
  tournament IARP (r1 external 14 × 100 rounds, inproc-r1 4 × 60):
  **26 replays, 5,640 rounds, 62 decisive conflicts, 227 indecisive;
  winner-in-earlier-bucket 0 (0‰), winner-same-bucket-first-arrival 62
  (1000‰); the loser dropped on the winner's pre-emptied state in 62/62
  (1000‰)**. By kind: double-capture 52, crossing-links 0,
  attack-vs-recharge 10. Interpretation: under the default `Flat(0)`
  policy every bot states bucket 0, so the BUCKET layer never decides a
  conflict and the within-bucket order (faction alternation from the
  round parity, then arrival) decides 100% of them — exactly the
  "scheduling noise vs strategy speed" split the acceptance row asks to
  make visible: the cross-bucket experiment (`runs/m6-r1-crossbucket`,
  120 H×H matches under `Factions{enl:0,res:1}`, D56) shows bucket order
  dominating parity when buckets DO differ, and the analysis attributes
  each win to its deciding layer. Crossing-links conflicts are zero in
  the bot archives (both link-building planners avoid crossings by
  construction); the class is pinned by the hand-constructed unit tests.
- Unit tests: `arena_replay::tests::conflict_analysis` — hand-constructed
  scripted matches with known winners (double-capture same-bucket and
  split-bucket, crossing links, attack-vs-recharge decisive + indecisive,
  quiet HACK/TRANSFER/MOVE rounds produce none, unverified input refused).
  Config/seed set: per-test pinned seeds in the test file; batch seeds
  derived from `0x006d3a00000000c1` under the `ingress-arena-conflict-game`
  / `-map` / `-bot` domains. Reports: `runs/conflict-r1/summary.txt`
  (+ the per-replay lines it aggregates).

## A-ISOLATION — Isolation — **MET**

*Only operator-approved, toolchain-locked, offline-built safe-Rust
clients run; system access only via the official SDK; no enemy private
state, no SHM writes, no network.*

- **Safe Rust + frozen dependency closure + offline build**: audit R1–R5
  (`tools/audit`, 28 tests) — forbid-unsafe lexical scan over EVERY `.rs`
  under the participant directory (incl. `#[path]`/`include!` inclusion
  forms), dependency allowlist = workspace path deps + the frozen 15-name
  external set (lockfile backstop counts every `source =` entry),
  participant build.rs/proc-macro/native-code rejection, network-token
  absence, `extern`/shm-token absence. Offline proof:
  `scripts/offline-build-proof.sh` (fresh CARGO_HOME + target, `env -i`,
  `--release --locked --offline`), evidence manifests recorded per
  milestone handoff under `runs/builds/`. *(Post-acceptance note,
  2026-09-05: the vendored closure and the offline proof were later
  reversed by D94 — dependencies resolve via the cargo mirror per the
  committed `Cargo.lock`; this record describes the state at acceptance
  time.)*
- **Resource + filesystem isolation**: `tools/sandbox-runner` (rlimits,
  landlock ABI 7 with handled-not-granted semantics incl. REFER/TRUNCATE/
  IOCTL_DEV, NO_NEW_PRIVS, TCP bind/connect kernel-denied — verified
  against a LIVE listener, wall-clock watchdog, group kill) + live child
  fixtures (`tools/sb-fixtures`, 11 tests). cgroup v2 probe records the
  un-delegated-host degradation honestly (D75).
- **Information isolation**: SDK mappings are `r--` and own-partition-only
  (pinned via `/proc/self/maps`), enemy regions absent before publish and
  after match end, a sandbox-killed client ⇒ CLIENT_FORFEIT + abnormal
  IARP. The same-UID `shm_open` of an enemy region name remains possible
  for non-compliant v0.1 code — recorded live, dropped immediately, and
  defended by audit R1/R5 (D77 states the exact boundary; NOT claimed:
  side channels, physical container escape, arbitrary untrusted code).
- Config: frozen r1 sandbox settings (2 GiB AS, 20 s wall clamped inside
  the per-match budget). Reports: `docs/design/sandbox.md`,
  `docs/design/build-provenance.md`, the audit/sandbox test suites.

## A-PHASE-INTEGRITY — Phase Integrity — **MET**

- **SDK refuses Settlement-Phase SHM reads**: the `PhaseGate` makes every
  `ObservationView`/`MapView` accessor re-check the phase — a lent view
  refuses reads between ROUND_CLOSED and the next ROUND_START_SIGNAL
  (pinned by arena-client-sdk tests, M7.1 P3-RPC fix), and context
  construction itself is a `PhaseViolation` there.
- **White-box audit**: R1/R4/R5 scan the participant crates for the
  tokens a bypass would need (`extern`, `mmap`, `shm_open`, net APIs);
  the SDK audited as a participant trips R5 — the documented operator
  exemption, pinned from both sides. The compile-fail harness pins that
  views expose no raw-mapping escape (E0599).
- Reports: arena-client-sdk `runtime.rs`/`isolation.rs` tests +
  `tools/audit` suite; `docs/design/sandbox.md` evidence map (D77).

## A-BALANCE — Balance — **MET-VIA-ESCAPE-HATCH** (documented and
scheduled; the tuning round has NOT been run)

- The criterion: no known simple strategy crushes other REASONABLE
  baselines at >90% win rate on most random maps. The 10,000-match
  frozen batch (`runs/m6-r1/`, `GameConfig::default()`, master seed
  `0x006d360000000031`, arena-bots `m6.0`) found one violation candidate:
  **Hybrid beats GreedyField 1750/1750 (100%)**; GreedyField beats
  NearestPortal (97.8%, 1711/1750) but NearestPortal is not a
  "reasonable baseline" — it is the pure nearest-expansion probe that
  structurally never LINKs and so cannot out-score any builder (balance
  report §5, root cause A). Everything else sits at or below
  50% with the mirror exactly balanced (H×H 1126/1124).
- The finding is RECORDED, root-caused, and scheduled (M6 milestone
  log + `docs/balance/balance-report-r1.md` §5): the plan's balance loop expects
  the first analysis round to surface exactly such dominances and to
  answer them with rule/parameter tuning in a later round — which has
  not happened yet. Related recorded biases: the Hybrid-mirror ENL bias
  0.768 (both factions compute the same seed triangle; the odd-round
  first-settler wins the round-1 triple capture race — now also visible
  as conflict-layer evidence in A-LATENCY) and GreedyField's 19.6% drop
  rate under active disruption.
- **Not claimed**: a tuned, dominance-free ruleset. Verdict per the
  roadmap's own escape: measured, reported, scheduled.

## A-OBSERVABILITY — Observability — **MET**

*The replay locates every action and its illegal reason, portal changes,
links, fields, and per-round scores.*

- **Per-action outcomes**: the IARP RoundRecord stores every submission
  with receive metadata, the unlocatable/ignored-duplicate lists, the
  final execution order, and per-slot statuses (D46); the replay engine
  re-derives all of them; the DROP taxonomy is one variant per distinct
  rule failure (D24), so every illegal action carries its rule reason —
  the conflict analysis (A-LATENCY) consumes exactly these per-command
  outcomes offline.
- **Portal/link/field/score per round**: Tactical Replay 展示逐轮状态与整数精确得分；实现、视角语义及测试见 `docs/design/viewer.md`。
- Reports: `arena-replay` 与 `arena-viewer` 测试，以及 `viewer/app/` 浏览器测试。

---

## Residual risk (known debt that remains at MVP handoff)

1. **Balance tuning round not run** (A-BALANCE): Hybrid's 100% over
   GreedyField and the Hybrid-mirror ENL bias 0.768 are measured,
   root-caused, and scheduled, not fixed.
2. **cgroup v2 not delegated on this host** (A-ISOLATION): the sandbox
   records the degradation in every JSON report; landlock + rlimits +
   audit carry the isolation claims; production shape is one container
   per client (D74/D75).
3. **UDP is not kernel-blocked** (A-ISOLATION): landlock covers TCP only;
   the audit token scan (R4) is the UDP boundary.
4. **M7 timing caveat**: one transient failure of the 40 ms-window
   transport test was observed once under extreme cross-binary load;
   isolated and full-suite reruns green (documented in the M8 log).
   ARENA_SKIP_LIVE=1 exists for tight iteration.
5. **Known minor code debt** (progress.md, "Known minor debt" sections):
   `MapError::InvalidConfig` conflates config vs definition defects;
   arena-core error enums still lack `Display`/`std::error::Error`
   (arena-runner's `BatchError`/`TournamentError` and arena-replay's
   `ReplayError` gained them; the M10 `ParseError`/`DepthError` have
   them); `merge_order` panics on `bucket_ms == 0` by contract;
   `remove_links_incident` scans the whole links map (perf only);
   `InitialSpawns::apply` silently skips unknown ids when misapplied.
6. **Viewer**: 当前 Tactical Replay 的自动化覆盖见 `viewer/app/README.md`。
7. **runs/ is local evidence**: every experiment artifact under `runs/`
   is untracked; the tracked stand-ins are the committed docs
   (`balance-report-r1.md`, `tournament-r1.md`, `sandbox.md`) and the tests that re-verify the artifacts
   wherever they exist (M9 skip-on-fresh-clone convention).

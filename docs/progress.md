# Implementation Progress

Handoff doc between milestone implementation rounds. The orchestrator updates it
after each milestone; every agent should read it before starting work.

## Environment constraints (read first)

- **Network works through a configured cargo mirror** (crates.io is reachable via
  cargo itself; direct curl to the default CDN times out — probe with cargo, not curl).
- **Dependency policy (ADR-0013, amended for M7)**: official `blake3` only until
  M7; M7 adds `planus` (pure-Rust FlatBuffers-compatible compiler + runtime — the
  wire format is FlatBuffers; planus generates planus-runtime code, so the
  official flatbuffers crate is NOT used; generated code is committed so builds
  do not require planus). Everything else stays std-only.
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
| M1 core geometry/RNG | done | 6f9966b | 97 tests green; types/config/geometry/rng/map/spawn; decisions D1–D15 |
| M2 basic combat | done | e530047 | 156 tests green; state/event/resolve/observe/harness; decisions D16–D29; reviewer major (unchecked u64 in virtual receive) fixed |
| M3 economy | done | d8fa100 | 180 tests green; HACK/TRANSFER in types/state/resolve/harness; decisions D30–D35; golden vectors pinned to the M1 RNG address; economy + determinism acceptance gates |
| M4 field game | done | 17d1c07 | 229 tests green; linkgraph/state/event/resolve/observe extended; first playable: full 500-round match in-process (0.08s); decisions D36–D43 |
| M5 deterministic replay | done | 581c873 | 266 tests green; new crate `arena-replay` (canon/format/recorder/replay, `#![forbid(unsafe_code)]`, blake3 + arena-core only); D47 run_round guard closes the M4 debt; 1000-match gate: every match replays to identical per-round + final hashes, 0 skips, ~14–15 s on 12 threads; decisions D44–D52 |
| M6 baseline AI / self-play | done | (this commit) | 308 tests green; new crates `arena-bots` (5 fixed-version P2 bots + BotRng, `#![forbid(unsafe_code)]`, arena-core + blake3) and `arena-runner` (batch/schedule/policy/run/metrics/report + binary, `#![forbid(unsafe_code)]`); frozen 10,000-match batch `runs/m6-r1` + 120-match cross-bucket batch: 76.6 s release on 12 threads, 0 crashes, subset-reproducible + replay-spot-checked; first balance analysis `docs/balance-report-r1.md` (A-BALANCE: Hybrid>Greedy 100% and Greedy>Nearest 97.8% recorded + scheduled; P(win\|leading@R100) 1.000/0.921 in two rows; mirror-only ENL bias 0.768); decisions D53–D59 |

## Frozen design decisions (must not silently change)

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
- D14: `Command` enum currently carries only `Move`; later milestones add the rest. (Superseded in M2: Move/Attack/Capture/Recharge.)
- D15: `game_seed: u64` is match-setup input (Replay Header material from M5); RandomOracle is the only runtime randomness source.

M2 (Basic Combat) additions:

- D16: modules — `state.rs` (GameState + round trackers + mutations), `event.rs` (broadcast events), `resolve.rs` (intake, ordering, execution, per-command results), `observe.rs` (faction views), `harness.rs` (in-process loop, scripted strategies). DROP/outcome types live in `resolve.rs`.
- D17: roster — `GameState::new` derives agents ENL `AgentId(0..n-1)`, RES `AgentId(n..2n-1)`, `n = initial_agents_per_faction`; `initial_roster(cfg)` feeds `validate_initial_spawn_set`; the constructor rejects spawn sets not exactly covering each roster.
- D18: virtual receive — strategies state a bucket directly; the harness sets `received_at_ms = bucket * priority_bucket_ms`, per-faction `receive_seq` = collection order. Public pure `receive_bucket(at_ms, bucket_ms) -> u64` (floor div), boundary-tested. Default bucket 0 (ADR-0002).
- D19: merge — per bucket (ascending); each faction's queue sorted by `(received_at_ms, receive_seq)`; alternation starts at the round's start faction and restarts fresh in every bucket; one queue empty ⇒ drain the other in order.
- D20: start faction from parity only — odd round ⇒ ENL (RES notified first), even ⇒ RES. No separate signal-order input in M2.
- D21: locatability (core envelope analog) — wrong `round`, unknown agent, or faction ≠ agent's faction ⇒ recorded unlocatable, slot NOT frozen (R-RESOLVE-01 "envelope reject doesn't freeze"). All other transport concerns are M7.
- D22: round close — agents with no submission when input closes are frozen PASS (deadline analog). No COMMIT_ROUND in-process in M2.
- D23: zero-target ATTACK ⇒ DROP `AttackNoEnemyPortalsInRange` (state-identical to a no-op success; chosen for per-command observability). The major quota is consumed either way.
- D24: DROP taxonomy — one variant per distinct rule failure, no catch-all (see `DropReason` in `resolve.rs`).
- D25: attack ordering — enemy portals in closed range are damaged in ascending `PortalId`; per portal emit `PORTAL_ATTACKED` then `PORTAL_NEUTRALIZED` if hp reaches 0. `GameState::neutralize_portal` is the documented M4 link/field-cascade extension point (today: `owner=Neutral, hp=0` only).
- D26: integer clamps — hp floor via `checked_sub(damage).unwrap_or(0)` (game clamp, not an overflow escape); recharge sum in u64 then `min(portal_max_hp)`; `event_seq` is global, 0-based u64.
- D27: structural vs execution — structural = commands ≤ `max_commands_per_agent_round` AND majors ≤ 1 (count-only, world-blind). Unknown `PortalId` is execution-time `PortalNotFound`, never structural.
- D28: major-on-drop — `major_used` is set even when the major DROPs; intake caps majors at 1 per stream so double-spend is unreachable — the tracker fact is for spec compliance, later milestones, and tests.
- D29: placeholders — scores stay `Score2(0)`; no links/fields until M4; `Observation` still exposes scores (R-VIS-01).

M3 (Economy) additions:

- D30: `Command` grows `Hack { portal }` and `Transfer { target_agent, supply, keys: BTreeMap<PortalId, u32> }` (BTreeMap = canonical ascending order + free dedup); both auxiliary. The payload names no sender — the stream owner is the sender by construction (R-TRANSFER-01). `Command` drops `Copy` (keeps `Clone`).
- D31: hack RNG stable index = per-`(agent, portal)` LIFETIME count of successful hacks (0-based, never reset by epochs); address = frozen `RandomOracle::hack_address(round, agent, portal, ordinal)`, `RNG_VERSION` stays 1 — epoch-local ordinals would shift every draw after an `EPOCH_LENGTH` retune.
- D32: `GameState::epoch_index(round, epoch_length) = (round - 1) / epoch_length` (0-based; rounds are 1-based, floor div). Epoch reset is EAGER inside `advance_round`: on a boundary, every hack tracker resets `last_hack_round`/`hack_count_epoch` while `hack_ordinal` survives — "Round 100 hack must not block Round 101" is a plain state fact.
- D33: `Resolver` owns a by-value `RandomOracle` — `Resolver::with_seed(game_seed)` is the authoritative constructor; `Resolver::new()`/`Default` stay seed-0 for M2 fixtures. `MatchHarness::new(game_seed, cfg, map, spawns, enl, res)` takes the seed first; `GameState` never holds seed or oracle (D15).
- D34: HACK validation order is contractual: portal exists → closed `PORTAL_OPERATION_RANGE` from the current position (NO owner check) → cooldown → epoch limit. A passing hack always succeeds and draws exactly one sample (`min + uniform(addr, max-min+1)`); supply grants CLAMP at `MAX_SUPPLY` in u64 (a clamp is not a failure; keys never clamp; tracker still advances). Dropped hacks mutate nothing and draw nothing. Range failures reuse `OutOfOperationRange`; when cooldown AND epoch limit both fail, cooldown reports.
- D35: TRANSFER validation order is contractual: target exists → same faction → closed `TRANSFER_RANGE` from CURRENT positions → sender supply → sender key balance per portal ascending `PortalId` (zero-count entries inert, first shortfall reported) → receiver `supply + transfer <= MAX_SUPPLY` (u64 sum; `capacity` in the DROP reason is the receiver's REMAINING headroom). Any failure ⇒ DROP with zero mutation; sender key entries depleted to 0 are removed (canonical inventory for M5 hashing); self-transfer and zero-quantity transfers are legal no-ops that still pass target/faction/range checks.

## Known minor debt (from M1 review — fix when touched, or in final review)

- `geometry.rs` public fns (`dist2`, `within_closed_range`, `cross`) rely on the documented GameConfig bounds precondition; callers MUST validate `in_bounds` (etc.) before calling so illegal inputs become DROPs, not panics.
- `InitialSpawns::apply` silently skips unknown agent ids if applied to a different roster than validated against.
- One spawn test (`invalid_set_applies_nothing`) cannot fail as written.
- `RandomOracle::uniform` panics on `n == 0` by contract (documented; acceptable).
- Error enums lack `std::error::Error`/`Display`; `MapError::InvalidConfig` conflates config vs definition defects; comments reference decision IDs D1–D15 now recorded here.

From M2 review (minor):

- `MapDefinition::validate` does not check hp/owner consistency (e.g. Neutral with hp>0) though `MapDefinition::new` invites hand-built maps.
- `merge_order` panics on `bucket_ms == 0` without a documented contract like `receive_bucket` has.
- Two event tests are struct-shape tautologies (construct a value, assert its fields).
- No resolver-level test that a MOVE to negative coordinates DROPs (behavior correct per reviewer probes).

From M3 review (minor):

- Self-transfer exceeding own headroom DROPs (`TransferExceedsReceiverSupply`) though its net effect is identity; uniform rule, documented in D35 note.
- Comment/test mismatch in the golden-vector test (says agent spawns at boundary, spawns elsewhere).
- No test pins that `HACK_LIMIT_PER_EPOCH` is per (agent, portal) across DIFFERENT pairs (same-pair saturation is tested).
- Public `epoch_index` panics on round == 0 (documented 1-based precondition, but public surface).
- `command_hack_and_transfer_payload_shapes` is a compile-time tautology test.

M4 (Basic Combat → Field Game) additions:

- D36 storage — `linkgraph.rs`: `links: BTreeMap<LinkId, Link{faction}>`, adjacency `BTreeMap<PortalId, BTreeSet<PortalId>>`, `fields: BTreeMap<FieldId, Field{faction, area2}>` (area2 computed once at derivation); `FieldId` canonical `a<b<c`.
- D37 field cache — `insert_link` derives fields from `adj(A) ∩ adj(B)` before the edge exists; `remove_links_incident` unions dead fields before removal (layered fields over two removed edges counted once); `fields_full_recompute` is the diagnostic cross-check (each triple once).
- D38 LINK check order — source exists → target exists → self-loop → source owned → target owned → closed source range → target key → max-link-distance (squared i64) → duplicate canonical LinkId → portal-on-closed-segment (first offender ascending) → illegal intersection (first offender ascending).
- D39 intersection composition — illegal iff `segments_intersect(new, existing) && !shared_portal_endpoint` (D8; collinear overlap counts as intersect); check 10 precedes 11; field interiors never block links.
- D40 neutralize cascade — PORTAL_NEUTRALIZED → incident LINK_DESTROYED ascending → dead FIELD_DESTROYED ascending; a triangle's links share the endpoints' owner (faction invariant); D25 hook now returns the cascade inventory.
- D41 score settlement — `settle_scores` at the end of `run_round` sums cached area2 into `Score2(u128)` and returns the delta map; `final_result` compares scores only, equal ⇒ `MatchResult::Draw`; first field of the acceptance fixture scores at round 14.
- D42 reinforcement — boundary `round % interval == 0 && round < total`; stable index = interval index (1,2,3,4 default), NOT the hack epoch; both factions share `RandomOracle::reinforcement_position(index, w, h)`; newcomer AgentIds are consecutive ENL then RES blocks from `next_agent_id` (init 2n); they join after the boundary round's slots/scoring.
- D43 observation — links/fields exposed as ascending public vectors identical for both factions; scores real (D29 retired).

From M4 review (minor; the first two matter for M5):

- **`run_round` has no same-round double-call guard; since M4 a double call observably corrupts scores (settle_scores runs again). M5's replay engine must settle exactly once per round — add the guard.** (RESOLVED in M5 — D47.)
- Reinforcement event carries index/position/count/first_agent_id but no faction/agent_id field (R-EVENT-01 wording wants them on every event).
- `reinforcement_position` panics on zero width/height (documented contract, public surface).
- `remove_links_incident` scans the whole links map instead of using adjacency (perf only).
- Rule-8 comment overclaims T-touch pre-rejection; late-DROP key-consumption not pinned by a test; every cascade test kills ALL fields of a portal (no partial-survival pin).
- Tautology tests: `match_result_covers_three_variants` and two similar; `next_agent_id` init expect is vacuous while the real hazard is the unchecked append increment.

M5 (Deterministic Replay) additions:

- D44 canonical state layout — 8 sections, all fixed-width LE (D2), u32 length prefixes, ascending-id iteration: (1) config identity as 31 u32 words in `GameConfig` declaration order (shared `arena_core::codec`, 124 B); (2) round u32; (3) portals dense-by-id (owner u8, hp u32, x u32, y u32 — the shared portal codec, 13 B); (4) agents ascending (id, faction u8, x, y, supply, key count, (portal,count) ascending); (5) hack trackers ascending (agent, portal, last_hack_round u32 with 0=None, hack_count_epoch, hack_ordinal — the D31 lifetime ordinal); (6) links ascending LinkId (min, max, faction u8); (7) scores enl/res u128; (8) next_agent_id u32. EXCLUDED (ADR-0010): events + event_seq (replay compares them as records), round trackers (reset by advance_round before any next-round command), link-graph field/adjacency caches (pure functions of links × map), the D47 settle guard, game_seed/oracle (D15 — Header material). Golden vectors pin bytes + hash of a fresh and a mid-match state; insertion-order and exclusion tests hold.
- D45 state hash — `BLAKE3-256` over `b"ingress-arena-state-v0.1" || canonical_state_bytes` (domain label, not a prefix field); hash POINT frozen: immediately after round r fully closed (`run_round` incl. settle+reinforcement AND `advance_round` incl. the epoch sweep), so `state.round() == r+1` and the hash covers exactly what round r+1 reads. First recorded hash = round 1; final = after `total_rounds`.
- D46 IARP v1 file format — magic `b"IARP"`; format/server/protocol (placeholder until M7)/rng (must equal `arena_core::rng::RNG_VERSION`)/state-encoding (must equal `canon::STATE_ENCODING_VERSION`) versions; game_seed u64; total_rounds u32 as a validated echo of the config; config + map (shared codecs); spawns ENL then RES ascending. Tagged blocks: RoundRecord (round, notified_first stored AND verified against the D20 parity, submissions echo incl. unlocatable + ignored duplicates with receive metadata, unlocatable list, ignored-duplicate list, execution order (agent ids), slot statuses, events, score2_delta, state_hash), Checkpoint, EndOfMatch (normal: rounds_completed + final hash + result tag 0/1/2; abnormal: last_completed_round + last_valid_hash + outcome tag ≥ 3, exercised by hand-crafted tests only — in-process M5 always ends normal). Decode validates magic/versions/round sequencing/notified-first parity/checkpoint placement + hash agreement + canonical-bytes integrity/trailer counts, never panics, and rejects unknown tags; length prefixes are guarded so absurd counts cannot pre-allocate. NOTE (spec-driven extension of the plan sketch): the RoundRecord carries unlocatable/ignored_duplicates/execution_order/slots because P1-REPLAY requires the ignored duplicates, final execution order, and validation results as replay content and the M5 checklist requires the replay to re-derive them.
- D47 run_round double-call guard (closes the M4 debt) — `GameState.settled_round: Option<u32>` (private, excluded from the hash); `Resolver::run_round` begins with check-and-set and PANICS on a same-round double call (`"run_round already settled round {N}; call advance_round first"`); `advance_round` clears it. Panic (not error) keeps the signature and matches the programmer-error convention; `settle_scores` stays a direct-callable primitive (state tests call it twice legitimately). Six pre-existing resolve.rs tests that legitimately re-ran scenarios on one state now `advance_round` between them. `GameState::advance_round` became `pub` (visibility only) — the replay engine owns the same run_round → advance_round sequence as the harness.
- D48 recorder — `ReplayWriter` is a thin public-API wrapper over `MatchHarness` output (harness untouched): header from setup inputs, per-round record from `RoundReport` + the post-advance state (events sliced from the append-only log via a written-marker, hash at the D45 point), `finish()` writes the trailer and self-decodes as a bug tripwire. `RoundReport.submissions: Vec<SubmittedCommandStream>` (additive arena-core field) echoes the EXACT input slice in submission order — the replay file stores it verbatim and the engine re-executes it. `GameState::hack_trackers()` accessor added for canonical enumeration.
- D49 checkpoints — written after every round `% 50 == 0` (round, hash — must equal that round record's hash, u64 byte length, canonical state bytes); decode enforces placement, agreement, and that the canonical bytes hash to their own hash. M5 replay does NOT resume from checkpoints (header replay ≈ 15 ms/match measured); seek/resume is M10 viewer material.
- D50 acceptance gate — 1000 matches, `GameConfig::default()`; seeds `LE-u64(blake3("ingress-arena-m5-game" || m_le32))` and `…-map…`, m = 0..1000 (distinct game AND map seeds); strategy mix `m % 4` = FF/FA/AF/AA where A = the new deterministic AggroBot (pure f(Observation), integer-only `ceil_integer_sqrt` stepping; nearest enemy portal, fallback neutral, ATTACK/CAPTURE/HACK in operation range) and F = the FieldBot ported from the M4 harness tests; skip rule `map_seed(m + 1000·k)` when the triangle-pair search fails (0 skips occurred in 750 search matches); per match: record → serialize → decode → replay (verifies every per-round hash, delta, event, and intake fact) → re-encode byte-exact. `std::thread::scope` with `available_parallelism().min(12)` workers over disjoint m-ranges; serial spot-reruns pin worker-count independence. EVIDENCE (debug build, 12 threads): matches=1000 skips=0 distinct_maps=1000 elapsed≈14.2 s (≈70 matches/s, budget 180 s) avg_replay_bytes≈282 KB; aggregate links=1500 fields=500 hacks=17956 events=1,227,598; mixes FF/FA/AF/AA all present. `ARENA_REPLAY_GATE_COUNT` lowers the count for iteration; the default is 1000.
- D51 tag/width conventions — Faction 0=Enl/1=Res; Owner 0=Neutral/1=Enl/2=Res; command tags 1..7 in `Command` declaration order; event tags 1..8 in `EventKind` declaration order; `Option<u32>` rounds encode as u32 with 0=None (rounds are 1-based); i32 coords as two's-complement u32; u128 = 16 LE bytes; every collection u32-counted and ascending.
- D52 arena-replay crate — `#![forbid(unsafe_code)]`, dependencies exactly `blake3` (official, pinned by Cargo.lock) + `arena-core` path dep (ADR-0013 policy holds); modules canon/format/recorder/replay + tests (support bots, replay_flow integration, 1000-match gate). `ReplayError` implements `Display` + `std::error::Error` (contrast: arena-core enums still lack it, see M1 debt).

M6 (Baseline AI / Self-play) additions:

- D53 bot RNG — every faction-bot owns one keyed BLAKE3 XOF stream: key = `b"ABOT" || u32 BOT_RNG_VERSION=1 || u64 bot_seed || u8 faction_tag(0=Enl/1=Res) || u8 kind_tag(1..=5 in BotKind declaration order)`, one `OutputReader` drawn sequentially through the FROZEN `arena_core::rng::uniform_from_words` rejection loop (made `pub`, visibility only — shared frozen mapping, never `% n`). Spawn draws come first, then round draws in submission order; the whole match setup (seeds, config, map, both fixed-version algorithms) makes a bot deterministic. NEVER the runtime `RandomOracle`, never a wall clock. `bot_seed` is per-match, runner-derived (D58). Golden vector pins the first 8 words of one pinned key.
- D54 Bot integration — `trait Bot: Send { kind, version("m6.0"), choose_spawns(cfg, map, roster), submit_round(obs) }`; `BotStrategy(Box<dyn Bot>)` adapts to `RoundStrategy`. Bots act ONLY on their faction `Observation` (R-VIS enforced by types) + `GameConfig` constants + own memory (own past submissions; the hack ledger of (agent,portal)→(last round, epoch count) with the D32 eager epoch sweep mirrors the unreadable engine trackers and keeps Hack drops near zero). The frozen `MapDefinition` is visible only during the spawn phase (spec: spawn strategy based on the complete map). The SAME bot instance chooses spawns and then plays (one RNG stream); a spawn set rejected by `validate_initial_spawn_set` is a bot bug → recorded `Outcome::Crash`, never silently skipped. Reinforcement agents appear via `obs.own_agents` and are handled generically.
- D55 schedule — one schedule seed per batch: `LE-u64(blake3("ingress-arena-m6-sched" || master_seed_le64))`; per matchup over its M (even) matches, want-bit = `blake3(b"m6-sched" || sched_le64 || tag || j_le32)[0] & 1`, honored while `cap = M/2` slots of the wanted faction remain, else flipped — guaranteeing EXACTLY M/2 ENL and M/2 RES per bot (|ENL−RES| = 0, stricter than the spec's "≤1"), randomized order, one match per item, no auto swap, no rematch. Deterministic in (sched, tag, j); changes with the seed.
- D56 bucket policy — `enum BucketPolicy { Flat(u64), Factions { enl, res } }` in arena-runner; `PolicyStrategy` wraps each faction's strategy and rewrites every returned `AgentSubmission.bucket` AFTER the bot returns (bots always state 0; D18 receive-time derivation stays in the harness). Balance default `Flat(0)` (arrival = collection order, parity alternation D19/D20). Cross-bucket experiment: 120 H×H matches under `Factions{enl:0,res:1}` — the runner test pins that every round's execution order puts ALL ENL streams before ALL RES streams (vs parity alternation under Flat(0)), end-to-end evidence that bucket order dominates parity.
- D57 records & artifacts — `MatchDescriptor` (index, matchup tag/j, both bot kinds + versions, schedule decision, game/map/bot seeds, map index, bucket policy, config hash) expands from `BatchConfig`; each match maps to a `MatchRecord` = descriptor echo + `Outcome{EnlWin,ResWin,Draw,Crash}` (panics caught per match via `catch_unwind(AssertUnwindSafe)`, crashes stay in the result set) + final scores, `canon::state_hash`, per-faction `FactionMatchStats`, elapsed_ms (METADATA ONLY — never a seed, policy input, or result). `MatchFold` folds stats incrementally per round from `RoundReport` execution_order effects (in execution order, so live-field sets drive layering depth and anchor-attack share exactly as each command saw the world) + the round's new `PortalCaptured` events (turnover) + telescoped `score2_delta` snapshots at R50/100/200/300; the 500 reports are never retained. Artifacts are hand-rolled std-only text: `runs/<batch_id>/manifest.txt` (key=value setup + run metadata), `matches.tsv` (one row per match; parsing reproduces the records exactly), `summary.txt` (per-matchup aggregates, integer fixed-point). Replays are NOT written for the 10k run; 8 recorded M6 matches replay to identical per-round + final hashes (M5 property extended).
- D58 seed derivations & frozen experiment — all `derive(label, bytes) = LE-u64(blake3(label || bytes))` with domains `"ingress-arena-m6-map" || k_le32` (500-map pool, shared across matchups so map variance cancels; map index = j % map_count per matchup), `…-game`/`…-bot` `|| tag || j_le32` (one bot seed per match; the two bots' streams stay disjoint via the D53 faction/kind tags), `…-sched` `|| master_seed_le64`. Frozen batch m6-r1: RR 1000, NN 1500, NG 1750, GD 1750, GH 1750, HH 2250 (all even), master seed `0x006d360000000031`, `GameConfig::default()` (identity hash `0x9ead052219fa7ff3`), 12 threads. No map skip rule: bots degrade gracefully on every map; `MapError::Infeasible` aborts the batch (config defect). Evidence: 10,000 matches in 76.6 s release (≈130 matches/s), 0 crashes; 32-match sub-batch + single matches reproduce identical records/final hashes; worker-count independence (12 vs 1) pinned.
- D59 M6 crates — `arena-bots` (bot/rng/common/random/nearest/greedy_field/destroy_anchor/hybrid) and `arena-runner` (batch/schedule/policy/run/metrics/report + `arena-runner` binary), both `#![forbid(unsafe_code)]`; deps per ADR-0013 (arena-core path, blake3; arena-runner additionally takes the arena-bots + arena-replay path deps). Bots stay simple, interpretable, fixed-version (P2: measuring tools). Bot algorithm freezes: RandomBot = legality-weighted menu, k = 1+u(3) draws without replacement, Move always available, non-move commands before the move; NearestPortalBot = nearest portal that is not ours or is ours without a key from this agent (move/hack/capture loop), Capture/Attack/Hack in closed operation range; GreedyFieldBot = cached max-area triangle (candidate edges = own-or-neutral pairs within closed MAX_LINK_DISTANCE, no link on the pair, segment clear of portals, no crossing without shared endpoint; rank area2 desc → all-own preferred → ascending ids) + six shared tour stations (3 capture+key stops, 3 edges), all agents offset 0 (the M4 FieldBot pattern), replan on complete/invalid (vertex enemy-owned, foreign link on a planned pair, foreign crossing), FAILED searches re-tried at most every 25 rounds, NearestPortal fallback otherwise — never idle, never skip a map; DestroyAnchorBot = `anchor_value(p) = Σ area2 of enemy fields containing p`, all agents converge on argmax (ties ascending id), claim-spread expansion (agents ascending each claim the nearest unclaimed enemy portal, else neutral) when all-zero, lowest-id agent in range of a damaged own portal recharges in passing, supply < 10 hacks first; HybridBot = first 3 agents build the GreedyField plan with per-agent station offsets, everyone else (reinforcements included) attacks via the DestroyAnchor body (claim-spread, supply < 3 hack), and when the top own anchor was attacked below 50% max hp last round the nearest ATTACKER recharges/moves to it.

M5 deviations from the architect plan (spec wins / noted):

- RoundRecord extended beyond the plan §4 field list with the intake facts (see D46 note) — P1-REPLAY lists them as required replay content and the plan's own checklist item 20 demands the comparison.
- `GameState::advance_round` changed `pub(crate)` → `pub`: the plan's replay loop calls it directly, which was impossible before (visibility-only change, zero behavior change).
- `replay()` re-checks header versions itself (defense in depth for in-memory `ReplayFile`s, not only decoded bytes).
- Gate AA-matches never LINK/HACK (AggroBot holds no keys by design); links/fields/hacks non-vacuity is satisfied by the 750 F-participating matches, per the aggregate assert.

M6 deviations from the architect plan (spec wins / noted):

- `arena_core::rng::uniform_from_words`/`WordSource` made `pub` (the plan sanctioned this visibility-only change) so the bot RNG shares the frozen rejection loop.
- GreedyField's re-search throttle applies ONLY to FAILED searches; a successful search resets it, so completed/invalidated plans replan immediately. The plan sketch implied a general retry cadence; without the reset, an invalidation could idle the faction in fallback for up to 25 rounds (found by the fixture replan test).
- NearestPortalBot's target is the nearest portal that is (not ours) OR (ours without a key held by this agent). The plan text's "nearest portal not owned by us … else Hack" is self-contradictory (the else-branch is unreachable under a strict non-own target); the extension matches the P2 table's "move, Hack, Capture" and the plan's own test wording "hacks own in range".
- DestroyAnchor expansion uses deterministic claim-spreading (ascending agents each claim a distinct enemy/neutral portal) instead of per-agent independent "nearest": with the centroid spawn every agent would pick the identical target and its own CAPTUREs would race (the <5% drop-rate gate). Anchor mode still converges every agent on the argmax anchor as planned.
- Hybrid's defense dispatch selects the nearest ATTACKER ("instead of attacking" selects among agents that would attack); builders never divert from the field plan.
- MatchRecord bot versions are owned `String`s (a `&'static str` cannot round-trip through `matches.tsv`); `same_ignoring_elapsed` supports the reproducibility comparison because `elapsed_ms` is metadata.
- Metric sources: captures from `PortalCaptured` events (authoritative broadcast), field lifecycle/layering/anchor share from executed `Linked`/`Attacked` effects in execution order (the event log alone loses intra-round ordering vs attacks).
- The binary additionally runs the plan §7's "explicit, small" 120-match cross-bucket H×H experiment (`runs/m6-r1-crossbucket`) alongside the frozen 10k; the ordering semantics themselves are pinned by the runner tests.
- The 10k run's manifest cannot embed a git commit hash (std-only binary, no build script); `docs/balance-report-r1.md` states the base commit (`581c873`) and the reproduce command instead.

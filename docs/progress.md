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
| M1 core geometry/RNG | done | 6f9966b | 97 tests green; types/config/geometry/rng/map/spawn; decisions D1–D15 |
| M2 basic combat | done | e530047 | 156 tests green; state/event/resolve/observe/harness; decisions D16–D29; reviewer major (unchecked u64 in virtual receive) fixed |
| M3 economy | done | d8fa100 | 180 tests green; HACK/TRANSFER in types/state/resolve/harness; decisions D30–D35; golden vectors pinned to the M1 RNG address; economy + determinism acceptance gates |
| M4 field game | done | (this commit) | 229 tests green; linkgraph/state/event/resolve/observe extended; first playable: full 500-round match in-process (0.08s); decisions D36–D43 |

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

- **`run_round` has no same-round double-call guard; since M4 a double call observably corrupts scores (settle_scores runs again). M5's replay engine must settle exactly once per round — add the guard.**
- Reinforcement event carries index/position/count/first_agent_id but no faction/agent_id field (R-EVENT-01 wording wants them on every event).
- `reinforcement_position` panics on zero width/height (documented contract, public surface).
- `remove_links_incident` scans the whole links map instead of using adjacency (perf only).
- Rule-8 comment overclaims T-touch pre-rejection; late-DROP key-consumption not pinned by a test; every cascade test kills ALL fields of a portal (no partial-survival pin).
- Tautology tests: `match_result_covers_three_variants` and two similar; `next_agent_id` init expect is vacuous while the real hazard is the unchecked append increment.

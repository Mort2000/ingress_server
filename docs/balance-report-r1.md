# Balance Report R1 — Baseline Self-play (M6, P2.5)

First-round balance analysis from the frozen 10,000-match self-play batch.
All numbers come from the actual run artifacts in `runs/m6-r1/`
(`manifest.txt`, `matches.tsv`, `summary.txt`) and the explicit
cross-bucket batch in `runs/m6-r1-crossbucket/`.

## 1. Methodology

| Item | Value |
|---|---|
| Batch | `m6-r1`, 10,000 matches, 0 crashes, 0 skipped maps |
| Config | `GameConfig::default()` (R-CONFIG-01; identity hash `0x9ead052219fa7ff3` = blake3 of the canonical 31-word config encoding) |
| Maps | 500 distinct generated maps, shared pool across matchups; each map used 2.0–4.5x per matchup (1000/500 … 2250/500), map index = `j % 500` per matchup |
| Master seed | `0x006d360000000031` (frozen, D58) |
| Schedule seed | `LE-u64(blake3("ingress-arena-m6-sched" \|\| master_seed))` = `0x95f21f71edd55927`; want-bit = `blake3(b"m6-sched" \|\| sched \|\| tag \|\| j)[0] & 1`; exactly M/2 ENL per bot per matchup (D55) |
| Match seeds | `game_seed = LE-u64(blake3("ingress-arena-m6-game" \|\| tag \|\| j_le32))`; `bot_seed = LE-u64(blake3("ingress-arena-m6-bot" \|\| tag \|\| j_le32))`; `map_seed = LE-u64(blake3("ingress-arena-m6-map" \|\| k_le32))` — distinct domains per stream (D58) |
| Bots | arena-bots `m6.0` (fixed version, D53/D54): `random`, `nearest-portal`, `greedy-field`, `destroy-anchor`, `hybrid` |
| Bucket policy | `Flat(0)` — arrival = collection order; parity alternation (odd round ENL settles first, even RES, D19/D20) |
| Executor | 12 threads (std::thread::scope, stride partition), release build, `catch_unwind` per match (Outcome::Crash would stay in the result set) |
| Runtime | 76.6 s for 10,000 matches (≈130.5 matches/s); cross-bucket batch 2.0 s for 120 matches |
| Code | M6 milestone worktree (base commit `581c873`; the orchestrator records the milestone commit). Reproduce: `cargo run --release -p arena-runner` |
| Reproducibility | 32-match sub-batch re-runs bit-identical `MatchRecord`s and final state hashes; 12-thread vs 1-thread runs identical (worker-count independence); 8 recorded matches replay to identical per-round and final hashes (M5 property extended to M6 bots; all pinned by the arena-runner test suite) |

Matchup table (all counts even):

| Matchup | Count | | Matchup | Count |
|---|---:|---|---|---:|
| Random x Random | 1000 | | Greedy x DestroyAnchor | 1750 |
| Nearest x Nearest | 1500 | | Greedy x Hybrid | 1750 |
| Nearest x Greedy | 1750 | | Hybrid x Hybrid | 2250 |

## 2. Results per matchup

Win outcomes (bot-perspective; ENL/RES exposure is exactly 1:1 per bot):

| Matchup | Bot A wins | Bot B wins | Draws | Crashes | ENL win rate | RES win rate |
|---|---:|---:|---:|---:|---:|---:|
| Random x Random | 273 (27.3%) | 305 (30.5%) | 422 | 0 | 0.286 | 0.292 |
| Nearest x Nearest | 0 | 0 | 1500 | 0 | 0.000 | 0.000 |
| Nearest x Greedy | 0 | **1711 (97.8%)** | 39 | 0 | 0.485 | 0.493 |
| Greedy x DestroyAnchor | 225 (12.9%) | 0 | 1525 | 0 | 0.066 | 0.063 |
| Greedy x Hybrid | 0 | **1750 (100%)** | 0 | 0 | 0.500 | 0.500 |
| Hybrid x Hybrid | 1126 (50.0%) | 1124 (50.0%) | 0 | 0 | **0.768** | 0.232 |

Mean per-match metrics by bot inside each matchup (per match, 500 rounds):

| Matchup / bot | cmds | drop rate | hacks | attacks | recharges | captures | links | fields | first field | mean field area2 (cm^2 x2) | layering | supply left |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| RR random | 2581 | 0.004% | 42.9 | 21.9 | 27.2 | 9.6 | 7.7 | 2.41 | 246.8 | 8.93e8 | 1.32 | 155.7 |
| NN nearest | 2500 | 4.5% | 71.4 | 1509.5 | 0 | 328.2 | 0 | 0 | — | — | 0 | 211.4 |
| NG greedy | 2500 | 12.7% | 153.1 | 251.8 | 0 | 66.3 | 23.6 | 5.41 | 133.1 | 1.00e10 | 2.04 | 293.1 |
| NG nearest | 2500 | 6.0% | 568.1 | 313.5 | 0 | 165.0 | 0 | 0 | — | — | 0 | 602.9 |
| GD greedy | 2498 | 19.6% | 115.8 | 245.9 | 0 | 101.9 | 14.1 | **0.22** | 342.7 | 9.87e8 | 0.15 | 255.8 |
| GD destroy | 1862 | 1.1% | 2.3 | 500.9 | 8.4 | 163.9 | 0 | 0 | — | — | 0 | 133.8 |
| GH greedy | 2500 | 7.0% | 119.2 | 36.6 | 0 | 41.7 | 9.7 | 1.69 | 86.7 | 2.90e10 | 0.99 | 259.2 |
| GH hybrid | 2351 | 0.5% | 53.1 | 201.3 | 0.1 | 123.3 | 28.8 | **9.45** | **53.2** | 2.61e10 | 3.13 | 193.0 |
| HH hybrid | 2301 | 0.5% | 34.1 | 338.1 | 3.0 | 92.1 | 19.7 | 3.66 | 60.3 | 3.01e10 | 1.25 | 171.1 |

("mean field area2" is per field; the per-match area sums behind them are
e.g. GH greedy 4.90e10 over 1.69 fields and GH hybrid 2.47e11 over 9.45.)

Field quality per matchup (both factions pooled):

| Matchup | fields created (total) | mean area2 | mean lifetime, destroyed (rounds) | mean lifetime, survivors | mean lifetime, all |
|---|---:|---:|---:|---:|---:|
| Random x Random | 4815 | 8.93e8 | 108.7 | 122.6 | 115.8 |
| Nearest x Nearest | 0 | — | — | — | — |
| Nearest x Greedy | 9468 | 1.00e10 | 37.2 | 116.4 | 39.7 |
| Greedy x DestroyAnchor | 380 | 9.87e8 | 4.5 | 0.0 (none survive) | 4.5 |
| Greedy x Hybrid | 19501 | 2.65e10 | 87.7 | 221.2 | 192.8 |
| Hybrid x Hybrid | 16491 | 3.01e10 | 31.8 | 6.6 | 31.3 |

Resource and control metrics (per match, pooled factions):

| Matchup | anchor-attack share | key utilization | reinforcement link share | portal turnover (captures/portal/match) |
|---|---:|---:|---:|---:|
| Random x Random | 31.6% | 8.9% | 28.0% | 0.16 |
| Nearest x Nearest | 0.0% | 0% | — (no links) | 5.47 |
| Nearest x Greedy | 3.6% | 1.6% | 12.9% | 1.93 |
| Greedy x DestroyAnchor | 0.1% | 5.9% | 5.6% | 2.22 |
| Greedy x Hybrid | 4.6% | 11.2% | 1.5% | 1.37 |
| Hybrid x Hybrid | 4.8% | 28.9% | 0.0% | 1.53 |

(Turnover is both factions' captures per portal per match — it can exceed
1 because recaptures count: under Nearest x Nearest every portal changes
hands ~5.5 times per match, while Random x Random settles at ~0.16. The
per-faction values in `summary.txt` sum to these, e.g. NN 2.811 + 2.658.)

Drop-rate note (illegal-action rate): the aggregate drop rate is 0–7% for
every bot except `greedy-field` under active disruption (19.6% vs
DestroyAnchor). A sampled Greedy x DestroyAnchor match (4,496 commands,
490 drops) breaks down as 353 `CaptureTargetNotNeutral` (internal capture
races — all GreedyField agents walk the same station in the same order),
60 `LinkPortalNotOwned` (vertex neutralized between observation and
execution), 40 `LinkAlreadyExists` (two own agents link the same pair in
one round), 36 `AttackNoEnemyPortalsInRange` (expansion attack raced by a
teammate). Against a passive opponent the same bot drops < 5% (pinned by
the arena-bots legality gate), so these are same-round races under
pressure, not illegal-by-design commands.

## 3. Faction-bias check (ENL vs RES)

| Matchup | ENL wins | RES wins | ENL rate | Verdict |
|---|---:|---:|---:|---|
| Random x Random | 286 | 292 | 0.286 | balanced |
| Nearest x Nearest | 0 | 0 | — | all draws |
| Nearest x Greedy | 848 | 863 | 0.485 | balanced |
| Greedy x DestroyAnchor | 115 | 110 | 0.066 | balanced |
| Greedy x Hybrid | 875 | 875 | 0.500 | balanced |
| Hybrid x Hybrid | 1729 | 521 | **0.768** | **ENL-biased** |
| Pooled excluding HxH | 2124 | 2140 | 0.498 | balanced (±0.4%) |

The only faction bias lives in the Hybrid mirror. Root cause: both
Hybrids compute the identical seed triangle from the same public map, so
both trios spawn on the same three portals; on round 1 (odd round, ENL
settles first) ENL wins the triple double-CAPTURE race (R-RESOLVE-04),
takes the largest triangle, and keeps the better field position for the
rest of the match (ENL mean first field 48.9 vs RES 71.7). The explicit
cross-bucket batch (120 HxH matches, `Factions { enl: 0, res: 1 }` — ENL
executes first in every round) gives ENL 0.750, i.e. no stronger than
Flat(0)'s 0.768: the bias is the round-1 race, not bucket ordering. The
runner tests separately pin that bucket policy does dominate the merge
order (all ENL streams before all RES streams in every round under
`Factions{0,1}`, parity alternation under `Flat(0)` — D19/D20/D56).

Consequence for the "no faction swap needed" hypothesis: it holds for
competitive (non-mirror) pairings — the schedule's exact 1:1 exposure
removes faction bias from every measured pairing (pooled ±0.4%). Mirror
matchups carry an inherent ENL edge via the round-1 race; M9 tournament
design should avoid letting a mirror final hinge on that coin flip (or
accept it as first-mover advantage; no auto rematch per P2.5).

## 4. Snowball: P(win | leading at round 50/100/200/300)

Ties at the checkpoint are excluded and their rate reported (matches whose
leader is decided are the denominator):

| Matchup | R50 | R100 | R200 | R300 |
|---|---:|---:|---:|---:|
| Random x Random | 0.833 (24 decided, 97.6% ties) | 0.730 (111, 88.9%) | 0.780 (273, 72.7%) | 0.859 (403, 59.7%) |
| Nearest x Nearest | — (0 decided) | — (0) | — (0) | — (0) |
| Nearest x Greedy | — (0, 100% ties) | **1.000** (679, 61.2%) | **1.000** (1403, 19.8%) | **1.000** (1664, 4.9%) |
| Greedy x DestroyAnchor | — (0, 100% ties) | — (0) | 1.000 (5) | 1.000 (86, 95.1% ties) |
| Greedy x Hybrid | **1.000** (1116, 36.2% ties) | **0.921** (1748, 0.1%) | **0.999** (1750, 0%) | **1.000** (1750, 0%) |
| Hybrid x Hybrid | 0.811 (2174, 3.4% ties) | **0.817** (2250, 0%) | 0.896 (2250) | **0.928** (2250) |
| Pooled | 0.875 | 0.879 | 0.948 | 0.965 |

Reading: `P(win | leading@R100)` exceeds the 90% action threshold
(R-BALANCE-02) in `Nearest x Greedy` (1.000) and `Greedy x Hybrid`
(0.921) — in both rows because the eventual scorer's first field is
effectively unrecoverable for the non-scoring/overmatched opponent. The
healthy reference points are Random x Random (0.730 at R100 — real
comebacks) and Hybrid x Hybrid (0.817 at R100, still below threshold; it
creeps to 0.928 by R300). Per the spec's checklist for this exact
symptom, the next round should look at `MAX_LINK_DISTANCE` (single early
fields are worth 2.7e10–3.0e10 area2 and decide the match), portal
density, attack/recharge strength, blocking strength, and field-area
weighting — concretely ordered in section 6.

## 5. A-BALANCE verdict

**Not clean on the first round; recorded and scheduled (the gate's own
condition is met via the documented-and-scheduled clause).**

- Two rows exceed 90% win rate over other reasonable baselines:
  `Hybrid > Greedy 100%` (1750/1750) and `Greedy > NearestPortal 97.8%`
  (1711/1750, 39 draws).
- Root cause A (role asymmetry, not tuning): `nearest-portal` and
  `destroy-anchor` structurally never LINK, so they can never out-score
  any builder; their matchups can only end in builder-wins or draws.
  This is by design of the P2 measuring tools — the rows measure "can
  pure expansion / pure disruption beat any scorer?", answered "no"
  (0 wins) and "almost never" (87.1% draws vs Greedy: the Disruptor role
  hard-counters the pure Field Builder, holding it to 0.22 fields/match
  vs 5.41 when unopposed).
- Root cause B (genuine balance signal): `Hybrid` dominates `Greedy`
  100%. Mechanism, from the per-bot metrics: Hybrid's offset-staggered
  trio fields 5.6x more often (9.45 vs 1.69 fields/match), first field
  53 vs 87, 3.1x layered depth, 5x field area, while its four
  reinforcement attackers simultaneously suppress Greedy (Greedy's
  fields/match drop from 5.41 vs Nearest to 1.69 vs Hybrid). Part of the
  gap is bot-quality (GreedyField's all-agents-one-order tour bunches —
  353 internal capture-race drops in the sampled GD match — so the pure
  builder is weaker than its plan deserves), but a fixed-version simple
  strategy beating every other reasonable baseline 100% is exactly the
  pattern A-BALANCE looks for, and it compounds with the R100 snowball
  threshold above.
- Also watched (below 90% now): Hybrid mirror ENL bias 0.768 (section 3)
  and HxH snowball 0.928 at R300.

The follow-ups are scheduled in section 6 under the R-CONFIG-02
discipline (one variable per round, no new mechanics). If after the
tuning rounds Hybrid still exceeds 90% over Greedy, the next lever is a
bot-definition round (re-version GreedyField to `m6.1` with per-agent
station offsets — a measuring-tool fix, explicitly not a rules change).

## 6. Tuning recommendations (P2.5-TUNING order, one variable per round)

1. **`MOVE_BUDGET` (200 m)** — first, cheapest lever on snowball pacing.
   Builder tour time is the dominant term of first-field round (53–133
   across matchups; DestroyAnchor pushes Greedy's to 343). Raising it
   accelerates everyone's fields (more snowball); lowering it flattens
   the whole table. Run one batch at 160 m and one at 240 m.
2. **`MAX_LINK_DISTANCE` (2 km)** — directly caps single-field area
   (current means 2.7e10–3.0e10 area2; the R100 leaders are effectively
   unbeatable). Shorter links mean smaller fields, more of them, more
   contention. This is the spec's first-listed suspect for
   `P(win|leading@R100) > 90%`.
3. **`ATTACK_DAMAGE` (20)** — five attacks neutralize a portal and the
   Disruptor currently hard-counters the pure Builder (87.1% draws in
   GD). Lower damage weakens single-point collapse; higher makes
   disruption (and Hybrid's attackers) even stronger.
4. **`RECHARGE_AMOUNT` / `RECHARGE_COST` (15 / 1)** — defense is nearly
   unused (0.1–3 recharges/match except DestroyAnchor's in-passing 8.4);
   cheaper/stronger recharge is the counter-lever to attack-heavy
   strategies before touching supply.
5. **`INITIAL_SUPPLY` (20)** — only after the above; NN shows hack-farmed
   supply piles to 600+ per match, so the binding constraint is not
   supply today.
6. **Portal density (`PORTAL_COUNT` 120)** — last; reshapes the map
   geometry all of the above act on.

Per R-CONFIG-02: change one parameter per experiment round, record
config/seeds/versions alongside results (the manifest already does), and
do not add mechanics to mask balance problems.

## 7. What to watch (next rounds)

- Hybrid's title defense: GH and HH rows are the balance canary; if a
  `MAX_LINK_DISTANCE` or `MOVE_BUDGET` change does not move GH off 100%,
  the domination is bot-structural, not parameter-structural (then plan
  the GreedyField `m6.1` station-offset re-version).
- The Hybrid-mirror ENL bias (0.768) and its round-1 mechanism — any
  spawn-rule or first-round change should re-measure this row first.
- Draw incidence: NN is 100% draws by construction and GD 87.1% —
  expected for non-scoring baselines, but the draw rate is the metric
  that should drop if attack/retake dynamics get more decisive.
- Key utilization is low everywhere (1.6%–28.9%): keys are abundant
  relative to link opportunities; revisit only after link distance
  changes.

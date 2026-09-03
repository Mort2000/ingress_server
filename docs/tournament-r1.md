# Tournament R1 — r1 (M9, P5)

First ranked tournament over the frozen Benchmark Seed Set: external
client binaries through the real server/protocol stack, one match per
scheduling item (no auto faction-swap rematch), integer Elo with draw =
0.5, technical outcomes excluded from ratings. Every number below comes
from the actual run artifacts in `runs/tournaments/r1/`.

## 1. Method

| Item | Value |
|---|---|
| Tournament | `r1` | 
| Entrants | `nearest`, `hybrid`, `probe` | 
| Matches | 14 (nearest-hybrid 6, nearest-probe 4, hybrid-probe 4) | 
| Master seed | `0x006d390000000032` -> schedule seed `0x5c88429ad6e6f8e3` | 
| Benchmark Seed Set | 4 maps, `map_seed = LE-u64(blake3("ingress-arena-m9-benchmap" || k_le32 || master_le64))`, map index = j % 4 | 
| Config | rounds/match 100, windows 400 ms spawn / 40 ms decision, identity `0x082d067e2e851c45` | 
| Elo | K = 32, initial = 1500, Q32.32 integer fixed point, replayed in item order over scored matches only | 
| Execution | 3 workers, sandbox sandbox-runner, per-match budget 20000 ms | 
| Runtime | 307 ms total (metadata only) | 
| Reproduce | `cargo run --release -p arena-runner -- tournament-r1` | 

Entrant provenance (binary hashes computed by the runner; tree/lock/
toolchain/manifest ids recorded from `tools/provenance` output):

| Entrant | Kind | Binary blake3 (`ingress-arena-m9-binary`) | Tree | Lock | Toolchain | Manifest |
|---|---|---|---|---|---|---|
| nearest | external | `8ad6e6b50e4c3a0cc12db038576c6e29957a482ce15525b44a718567b16a31f7` | `578243c3d20fe1bcdc83c7e3e691dc4b7a7d459c950eec3b0c8abd105af4b990` | `2537aced826a2784a2f6d44ed8834c9874e8d607c571b3935ccac7a7c9ad6452` | `fb9cd866c2fe90173345886771913dd64eed147810b2a72dfc64a1061f431eeb` | `runs/builds/20260903-0221-578243c3d20f` |
| hybrid | external | `88c26c09c93d74b3c3ec097fda7ed7f80867c860295808e8e322288d643f16ae` | `578243c3d20fe1bcdc83c7e3e691dc4b7a7d459c950eec3b0c8abd105af4b990` | `2537aced826a2784a2f6d44ed8834c9874e8d607c571b3935ccac7a7c9ad6452` | `fb9cd866c2fe90173345886771913dd64eed147810b2a72dfc64a1061f431eeb` | `runs/builds/20260903-0221-578243c3d20f` |
| probe | external | `1fc92e13c89dcda472198cb52979ea952118a5796c72b88f34d241240873ee1a` | `578243c3d20fe1bcdc83c7e3e691dc4b7a7d459c950eec3b0c8abd105af4b990` | `2537aced826a2784a2f6d44ed8834c9874e8d607c571b3935ccac7a7c9ad6452` | `fb9cd866c2fe90173345886771913dd64eed147810b2a72dfc64a1061f431eeb` | `runs/builds/20260903-0221-578243c3d20f` |

## 2. Standings

| Rank | Entrant | Rating | W-D-L | ffW | unscored |
|---:|---|---:|---|---:|---:|
| 1 | hybrid | 1596 | 8-2-0 | 0 | 0 |
| 2 | nearest | 1458 | 0-6-4 | 0 | 0 |
| 3 | probe | 1446 | 0-4-4 | 0 | 0 |

Faction splits over scored matches (the no-swap hypothesis check;
scheduling gives each entrant exactly M/2 ENL and M/2 RES slots per matchup):

| Entrant | ENL W-D-L | RES W-D-L | ENL mean dScore2 | RES mean dScore2 |
|---|---|---|---:|---:|
| nearest | 0-4-1 | 0-2-3 | -130316696136.000 | -428394394991.600 |
| hybrid | 5-0-0 | 3-2-0 | 1209013143369.600 | 859192463429.000 |
| probe | 0-2-2 | 0-2-2 | -911094709116.250 | -975773435472.500 |

## 3. Matches

| # | Matchup | Entrant A | Entrant B | A faction | Map | Outcome | Winner | Rounds | Replay |
|---:|---|---|---|---|---|---|---|---:|---|
| 0 | nearest-hybrid | nearest | hybrid | ENL | 0 | res_win | hybrid | 100 | server/replay.iarp |
| 1 | nearest-hybrid | nearest | hybrid | ENL | 1 | draw | - | 100 | server/replay.iarp |
| 2 | nearest-hybrid | nearest | hybrid | RES | 2 | enl_win | hybrid | 100 | server/replay.iarp |
| 3 | nearest-hybrid | nearest | hybrid | RES | 3 | enl_win | hybrid | 100 | server/replay.iarp |
| 4 | nearest-hybrid | nearest | hybrid | RES | 0 | enl_win | hybrid | 100 | server/replay.iarp |
| 5 | nearest-hybrid | nearest | hybrid | ENL | 1 | draw | - | 100 | server/replay.iarp |
| 6 | nearest-probe | nearest | probe | ENL | 0 | draw | - | 100 | server/replay.iarp |
| 7 | nearest-probe | nearest | probe | ENL | 1 | draw | - | 100 | server/replay.iarp |
| 8 | nearest-probe | nearest | probe | RES | 2 | draw | - | 100 | server/replay.iarp |
| 9 | nearest-probe | nearest | probe | RES | 3 | draw | - | 100 | server/replay.iarp |
| 10 | hybrid-probe | hybrid | probe | RES | 0 | res_win | hybrid | 100 | server/replay.iarp |
| 11 | hybrid-probe | hybrid | probe | ENL | 1 | enl_win | hybrid | 100 | server/replay.iarp |
| 12 | hybrid-probe | hybrid | probe | ENL | 2 | enl_win | hybrid | 100 | server/replay.iarp |
| 13 | hybrid-probe | hybrid | probe | RES | 3 | res_win | hybrid | 100 | server/replay.iarp |

## 4. Technical exclusions

None — every match completed normally.

## 5. Reproducibility

The schedule, seeds, map hashes, and both entrants' provenance are frozen in
`runs/tournaments/r1/tournament.json`; every match row (seeds, factions,
map hash, config identity, provenance ids, replay path) is in `index.tsv`;
per-match Elo pre/post ratings are in `ratings.tsv` and reproduce the
standings exactly. Each `matches/m<index>/` directory carries `meta.json`
plus the server's `journal.txt`/`result.txt` and — for every match that
entered Round 1 — a `replay.iarp` that decodes and replays to the recorded
final state hash.

Code: M9 milestone worktree (base commit `24029ce`, the M8 handoff; the
orchestrator records the milestone commit). The cited build manifest
cannot hash the sentence citing it (the M8 limitation): verify any tree
with `target/release/provenance --root .` — the entrant binaries above
match the manifest hashes exactly.

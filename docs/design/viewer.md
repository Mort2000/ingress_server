# Replay Viewer (M10, P6)

One self-contained HTML page per archived replay: open it locally
(`file://`), press Play, step or jump to any round, and read the same
state under the three R-VIS views. The page carries inline CSS + JS and
the fully materialized replay data — zero external requests, no network,
no framework (P-ARCH-03: viewer source assets live in `viewer/` at the
repository root, outside the Rust crates; the export tool is
`tools/replay-export`).

## Exporting a replay

```text
replay-export <replay.iarp> [-o out.html] [--meta <meta.json>] [--dev 0|1] [--faction enl|res]
```

Built and run from the repository root (any archived `.iarp` works; the
`runs/` evidence exists wherever it was produced):

```sh
cargo run --release -p replay-export -- \
  runs/tournaments/r1/matches/m11/server/replay.iarp \
  -o out.html \
  --meta runs/tournaments/r1/matches/m11/meta.json \
  --dev 1
```

The tool first VERIFIES the IARP with `arena_replay::replay` — every
per-round state hash, score delta, event slice, and intake fact plus the
trailer (the P6-GATE reference). An unverified replay is an error and NO
HTML is written (D88). On success it prints one summary line (rounds,
result, final hash, final score2) taken from the SAME verified outcome
embedded into the page, then the output path. Exit codes: 0 ok, 1
usage/IO, 2 unverified input.

Modes (D90):

| Mode | Command | Embedded agents | View labels |
|---|---|---|---|
| Faction export (player-facing) | `--dev 0 --faction enl` (or `res`) | exactly that faction's private agents | other faction label disabled |
| Dev export (debug/balance) | `--dev 1` | both factions | all four active, on-screen caveat |

## Opening the viewer

Open the emitted `.html` file directly in a browser (`file://`). Everything
renders from the embedded data; the page makes no requests (a test scans
every emitted byte for `http://`, `https://`, `src=`, `href=`, `fetch(`,
`XMLHttpRequest`, `WebSocket`).

## Controls and shortcuts

| Input | Action |
|---|---|
| Play / Pause button, `Space` | play / pause (loops rounds at the set speed) |
| `←` / `→` | step one round back / forward |
| `Home` / `End` | jump to round 0 / the last round |
| `+` / `−` | speed up / slow down (1000 / 500 / 250 / 125 ms per round) |
| `V` | cycle view (Faction ENL → Faction RES → Public → God) |
| slider | jump to any round — direct O(1) array access (D89) |

Round 0 is the pre-match moment (neutral map + spawn dots). The gold
ticks on the slider mark IARP checkpoint rounds (display only — they
prove archive integrity; seeking never needs them because every round is
fully materialized).

## View semantics (R-VIS)

| Section | Faction ENL view | Faction RES view | Public View | God View |
|---|---|---|---|---|
| Portals (position, owner, hp) | yes | yes | yes | yes |
| Links / Fields | yes | yes | yes | yes |
| Scores (both factions) | yes | yes | yes | yes |
| Public event history | yes | yes | yes | yes |
| Own agents (position, supply) | yes | yes | — | yes |
| Enemy agents | **never** | **never** | — | only in dev exports |

- **Public View draws no agents at all** (R-VIS-02: agent positions are
  private).
- **A Faction View never draws enemy agents.** In a faction export this
  is a construction guarantee (D90): the enemy's private rows and even
  its initial spawn coordinates do not exist in the file — enemy
  knowledge is public events only. In a dev export both factions are
  embedded and the predicate still selects only the view's own faction.
- **God View** exists for debugging/balance work; on a faction export it
  degrades to the embedded faction only.
- Enemy agent IDs may appear inside public events (R-EVENT-01 broadcasts
  them); that is public information, not private state.

The predicate lives in ONE place per environment: `viewer/filter.js`
inside the page, mirrored by the Rust twins in
`tools/replay-export/src/views.rs`, and re-asserted in-page by selftest.

## Embedded data layout (`ingress-viewer-v1`)

```json
{ "format": "ingress-viewer-v1",
  "generated": { "tool": "replay-export", "version": "…", "iarp_format": 1 },
  "header": { "game_seed": "0x…", "total_rounds": N, "rounds_completed": N,
              "versions": {…}, "config": {…31 identity fields…},
              "map": { "portals": [[id,x_cm,y_cm],…] },
              "spawns": { "enl": [[agent,x,y],…], "res": […] },
              "outcome": {…}, "checkpoints": [50,100],
              "dev": false, "factions_embedded": ["enl"], "meta": {…|null} },
  "rounds":  [ { "r": i, "score2": [s,s], "delta2": [s,s],
                 "portals": [[id,owner,hp],…], "links": [[a,b,f],…],
                 "fields": [[a,b,c,f,area2],…], "events": […], "hash": "…" } ],
  "agents":  { "enl": [[[id,x,y,supply],…]…] } }
```

- `rounds[i-1]` is the FULLY materialized state after round `i` closed
  (the D45 point). Jumping is O(1) array access.
- Owner tags 0=Neutral/1=ENL/2=RES; link/field `f` 0=ENL/1=RES (D51).
  `config` is the 31 identity fields in D44 codec order, name → integer.
- Event objects: `s` seq, `r` round, `k` kind tag (1 CAPTURE, 2 ATTACK,
  3 NEUTRALIZE, 4 LINK+, 5 LINK−, 6 FIELD+, 7 FIELD−, 8 REINFORCE),
  `f` faction tag, `a` agent; extras per kind — Attacked `d` damage /
  `h` hp_after, Link* `l [a,b]`, Field* `t [a,b,c]` + `ar` area2
  (recomputed from the frozen map with the same exact `geometry::area2`
  the link graph used), Reinforcement `i`, `pos [x,y]`, `n`, `fa`
  (both-faction: no `f`/`a`).
- **`score2` / `delta2` / `final_score2` are decimal STRINGS** (u128-exact;
  JS `Number` cannot hold values above 2^53 — D91). The viewer folds and
  converts them with `BigInt`. Display conversion is presentation-only:
  exact halving to cm² (the `.5` is visible when the integer is odd),
  then an exact decimal shift by 10^-10 to km². All other embedded
  numbers are plain integers bounded by the D10 limits.

## Selftest mode

Append `?selftest=1` to the page URL (works from `file://`):

```text
file:///…/<emitted-page>.html?selftest=1
```

The page re-runs the pure-data invariants in-place and renders PASS/FAIL
rows plus a title suffix: delta fold == cumulative score2 for every
round, round indexing, final-score equality, per-round view filters,
event-filter predicate, jump determinism, and the exact score display
conversion.

## Machine-tested vs human-verified (D93)

Machine-tested (`cargo test -p replay-export`, runs in the default
workspace suite): every data-level invariant — embedded JSON parses with
the strict integer-only parser; score accumulation is u128-exact for the
normal and abnormal fixtures and equals
the IARP records; final result/hash/scores equal `arena_replay::replay`
and the trailer and the printed summary line; round counts and the
abnormal shape; R-VIS faction-export filtering; the self-containment
scan; the three-view predicate via the Rust twins; mini-JSON round trip
and `</script>` inertness; unverified-input refusal; selftest-mode
presence; the docs gate.

Human-verified visually (no browser automation exists): canvas rendering
(portals/links/fields/agents), keyboard feel, animation timing, and the
`?selftest=1` display. The in-page selftest re-runs the same pure-data
assertions as a human-triggered bonus.

## Committed sample (withdrawn, D95)

The M10 milestone committed a dev-mode sample page at
`viewer/sample.html` (the r1 `m11` replay export); D95 withdrew it —
the zero-build demo entry is no longer wanted and no generated page is
tracked; you can regenerate one with the command at the top of this document
whenever one is needed.

// filter.js — THE view predicate (single source of truth inside the page).
//
//   God View     = public state + every EMBEDDED faction's agents
//   Faction F    = public state + agents[F]            (and nothing else)
//   Public View  = public state only — no agents at all
//
// Mirrored by the Rust twins in tools/replay-export/src/views.rs and
// asserted in-page by ?selftest=1 (checklist item 7, P6-GATE(c)). R-VIS:
// agent positions/supply are PRIVATE — a faction view never draws enemy
// agents; in a --dev 0 export the enemy's data is not even in the file
// (D90 construction guarantee), and in a --dev 1 export this predicate
// still selects only the view's own faction.
const Filter = {
  embedded: [], // e.g. ["enl"] or ["enl", "res"]

  init() {
    this.embedded = D.data.header.factions_embedded.slice();
  },

  // Which factions' agent arrays a view may read.
  visibleFactions(view) {
    if (view === "public") return [];
    if (view === "god") return this.embedded.slice();
    return this.embedded.includes(view) ? [view] : [];
  },

  // The agents `view` shows at round i (0..N). Round 0 derives the
  // pre-match spawn dots from header.spawns for the same factions.
  visibleAgents(view, i) {
    const out = [];
    for (const key of this.visibleFactions(view)) {
      if (i === 0) {
        const rows = D.data.header.spawns[key] || [];
        for (const row of rows) {
          out.push({ faction: key, id: row[0], x: row[1], y: row[2], supply: -1 });
        }
      } else {
        const rows = (D.data.agents[key] || [])[i - 1] || [];
        for (const row of rows) {
          out.push({ faction: key, id: row[0], x: row[1], y: row[2], supply: row[3] });
        }
      }
    }
    return out;
  },

  // Event-log predicate: kind checked AND faction checked. REINFORCEMENT
  // (k=8) carries no faction (it is both-faction), so it is visible under
  // any faction filter as long as its kind is checked.
  eventVisible(ev, kinds, factions) {
    return kinds.has(ev.k) && (ev.f === undefined || factions.has(ev.f));
  },
};

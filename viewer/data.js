// data.js — data layer. The exporter embedded `const VIEWER_DATA = {...}`
// in its own script block above this one; everything here reads that
// constant and hands out normalized round state. Embedded DATA stays
// integer: score2/delta2/final_score2 are decimal STRINGS (u128-exact,
// D91) and are converted with BigInt only at the display boundary.
// Jumping is O(1): rounds[i-1] is the FULLY materialized post-round state
// (D89) — checkpoints are display ticks, not seek machinery.
const D = {
  data: VIEWER_DATA,

  get header() { return this.data.header; },
  get rounds() { return this.data.rounds; },
  get roundCount() { return this.data.rounds.length; },

  // Map id -> [x, y] for the static portal positions (cm integers).
  portalPos: null,

  init() {
    this.portalPos = new Map(
      this.data.header.map.portals.map((p) => [p[0], [p[1], p[2]]])
    );
  },

  // Normalized state at round i (i in 0..N). Round 0 is the pre-match
  // moment: the frozen neutral map plus spawn dots (derived here, not
  // embedded — D89 note).
  stateAt(i) {
    if (i === 0) {
      return {
        r: 0,
        portals: this.data.header.map.portals.map((p) => [p[0], 0, 0]),
        links: [],
        fields: [],
        events: [],
        score2: ["0", "0"],
        delta2: ["0", "0"],
        hash: null,
      };
    }
    return this.rounds[i - 1];
  },

  // Cumulative score2 at round i as BigInts (exact).
  score2At(i) {
    const s = this.stateAt(i).score2;
    return [BigInt(s[0]), BigInt(s[1])];
  },

  // The events of rounds 1..i, flattened (round 0 has none).
  eventsThrough(i) {
    const out = [];
    for (let r = 1; r <= i; r += 1) {
      for (const ev of this.rounds[r - 1].events) out.push(ev);
    }
    return out;
  },
};

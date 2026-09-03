// selftest.js — ?selftest=1 re-runs the pure-data invariants of the
// embedded payload inside the page (works from file://). Human-triggered
// bonus mode (D93): no browser automation exists — the machine-tested
// split is documented in docs/viewer.md; this mode makes the same class
// of checks observable by hand in the real rendering environment.
(function runSelfTest() {
  const search = location.search || "";
  if (!/(^|[?&])selftest=1(&|$)/.test(search)) return;

  const checks = [];
  const add = (name, fn) => checks.push({ name, fn });
  const deepEq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

  // 1. Integer fold: cumulative delta2 == score2 for EVERY round.
  add("delta2 fold == score2 for every round (BigInt)", () => {
    for (let i = 0; i < D.rounds.length; i += 1) {
      const round = D.rounds[i];
      const expected = [
        (i > 0 ? BigInt(D.rounds[i - 1].score2[0]) : 0n) + BigInt(round.delta2[0]),
        (i > 0 ? BigInt(D.rounds[i - 1].score2[1]) : 0n) + BigInt(round.delta2[1]),
      ];
      if (BigInt(round.score2[0]) !== expected[0] || BigInt(round.score2[1]) !== expected[1]) {
        return "round " + round.r + ": " + round.score2 + " != fold " + expected;
      }
    }
    return null;
  });

  // 2. Indexing: rounds[i].r == i+1 and len == header.rounds_completed.
  add("round indexing (r == i+1, len == rounds_completed)", () => {
    if (D.rounds.length !== D.header.rounds_completed) {
      return D.rounds.length + " rounds vs header " + D.header.rounds_completed;
    }
    for (let i = 0; i < D.rounds.length; i += 1) {
      if (D.rounds[i].r !== i + 1) return "rounds[" + i + "].r == " + D.rounds[i].r;
    }
    return null;
  });

  // 3. Last cumulative == outcome.final_score2.
  add("last cumulative score2 == outcome final_score2", () => {
    const last = D.rounds[D.rounds.length - 1];
    const finalScore = D.header.outcome.final_score2;
    if (last.score2[0] !== finalScore[0] || last.score2[1] !== finalScore[1]) {
      return last.score2 + " != " + finalScore;
    }
    return null;
  });

  // 4. View filters: Public has no agents anywhere; a faction view never
  //    contains enemy agents; God is a superset of each faction view.
  add("per-round view filters (public=none, faction=own only, god superset)", () => {
    for (let i = 1; i <= D.roundCount; i += 1) {
      if (Filter.visibleAgents("public", i).length !== 0) {
        return "public view shows agents at round " + i;
      }
      for (const key of ["enl", "res"]) {
        const agents = Filter.visibleAgents(key, i);
        if (agents.some((a) => a.faction !== key)) {
          return "faction view " + key + " leaks another faction at round " + i;
        }
        const god = Filter.visibleAgents("god", i);
        for (const a of agents) {
          if (!god.some((g) => g.faction === a.faction && g.id === a.id && g.x === a.x && g.y === a.y)) {
            return "god view lacks " + key + " agent " + a.id + " at round " + i;
          }
        }
      }
    }
    if (Filter.visibleAgents("public", 0).length !== 0) return "public view shows spawn dots";
    return null;
  });

  // 5. Event-filter predicate derivation.
  add("event filter predicate (kinds x factions, reinforcement both)", () => {
    const allKinds = new Set([1, 2, 3, 4, 5, 6, 7, 8]);
    // Synthetic predicate probes (hold even for a vacuous archive).
    if (!Filter.eventVisible({ k: 8 }, allKinds, new Set())) {
      return "synthetic reinforcement hidden by a faction filter";
    }
    if (Filter.eventVisible({ k: 1, f: 0 }, new Set(), new Set([0, 1]))) {
      return "synthetic kind shown with no kinds checked";
    }
    const events = D.eventsThrough(D.roundCount);
    for (const ev of events) {
      if (!Filter.eventVisible(ev, allKinds, new Set([0, 1]))) {
        return "all-filters-on hides event " + ev.s;
      }
      if (Filter.eventVisible(ev, new Set(), new Set([0, 1]))) {
        return "no-kinds shows event " + ev.s;
      }
      if (ev.k === 8) {
        if (ev.f !== undefined) return "reinforcement carries a faction tag";
        if (!Filter.eventVisible(ev, allKinds, new Set())) {
          return "both-faction reinforcement hidden by a faction filter";
        }
      } else if (ev.f === undefined) {
        return "kind " + ev.k + " event misses its faction tag";
      }
    }
    return null;
  });

  // 6. Jump determinism: the same round renders the same data twice.
  add("jump determinism (same round, same data)", () => {
    for (const i of [0, 1, Math.floor(D.roundCount / 2), D.roundCount]) {
      const a = D.stateAt(i);
      const b = D.stateAt(i);
      if (!deepEq(a, b)) return "stateAt(" + i + ") not stable";
      if (!deepEq(Filter.visibleAgents("god", i), Filter.visibleAgents("god", i))) {
        return "visibleAgents(" + i + ") not stable";
      }
    }
    return null;
  });

  // 7. Exact score display conversion (D91): cm2 is the exact half (one
  //    fractional digit at most) and km2 is that times 10^-10, verified as
  //    an exact integer relation (digits * 2 * 10^10 == n * 10^places).
  add("score2 display conversion exact (cm2 halving, km2 shift)", () => {
    for (const s of [
      "0",
      "1",
      "2",
      "3",
      "1000000000001",
      D.header.outcome.final_score2[0],
      D.header.outcome.final_score2[1],
    ]) {
      const n = BigInt(s);
      const cm2 = Panels.cm2FromScore2(s);
      const [int, frac = ""] = cm2.split(".");
      if (frac !== "" && frac !== "5") return "cm2 fraction of " + s + ": " + frac;
      if (BigInt(int) * 2n + (frac === "5" ? 1n : 0n) !== n) return "cm2 of " + s;
      const km2 = Panels.km2FromScore2(s);
      const kmDigits = km2.replace(".", "");
      const places = km2.includes(".") ? km2.length - km2.indexOf(".") - 1 : 0;
      if (BigInt(kmDigits) * 2n * 10n ** 10n !== n * 10n ** BigInt(places)) {
        return "km2 of " + s + " -> " + km2;
      }
    }
    return null;
  });

  const rows = [];
  let passed = 0;
  for (const check of checks) {
    let failure = null;
    try {
      failure = check.fn();
    } catch (error) {
      failure = String(error);
    }
    const ok = failure === null;
    if (ok) passed += 1;
    rows.push(
      "<tr><td class='" + (ok ? "pass" : "fail") + "'>" + (ok ? "PASS" : "FAIL") +
      "</td><td>" + check.name + "</td>" +
      (ok ? "" : "<td class='why'>" + String(failure).replace(/&/g, "&amp;").replace(/</g, "&lt;") + "</td>") +
      "</tr>"
    );
  }
  const all = passed === checks.length;
  document.getElementById("selftest-result").innerHTML =
    "<div class='" + (all ? "pass" : "fail") + "'>SELFTEST " + passed + "/" + checks.length +
    (all ? " — PASS" : " — FAIL") + "</div><table>" + rows.join("") + "</table>";
  document.title += all ? " — SELFTEST PASS" : " — SELFTEST FAIL";
})();

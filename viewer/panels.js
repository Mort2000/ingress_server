// panels.js — the header block, the score curve, and the event log.
// Score display is presentation-only (rules/04: score2 is twice-squared
// cm; the exact integer is always shown, the km2 conversion below is an
// EXACT decimal shift, never a float).
const Panels = {
  kindNames: {
    1: "CAPTURE",
    2: "ATTACK",
    3: "NEUTRALIZE",
    4: "LINK+",
    5: "LINK-",
    6: "FIELD+",
    7: "FIELD-",
    8: "REINFORCE",
  },
  kindFilters: new Set([1, 2, 3, 4, 5, 6, 7, 8]),
  factionFilters: new Set([0, 1]), // 0=ENL 1=RES (both on by default)
  curveCanvas: null,
  curveCtx: null,

  init() {
    this.curveCanvas = document.getElementById("curve");
    this.curveCtx = this.curveCanvas.getContext("2d");
    this.renderHeader();
    this.buildEventFilters();
  },

  // ---- header block (always visible) ---------------------------------
  renderHeader() {
    const h = D.header;
    const meta = h.meta || {};
    const entrants =
      meta.entrant_a !== undefined
        ? meta.entrant_a + " vs " + meta.entrant_b +
          "  (" + (meta.tag || "?") + " j" + (meta.j !== undefined ? meta.j : "?") +
          ", map " + (meta.map_index !== undefined ? meta.map_index : "?") + ")"
        : "(no meta.json embedded)";
    const outcome = h.outcome;
    let outcomeLine;
    if (outcome.kind === "normal") {
      outcomeLine = "<b>" + outcome.result + "</b> after " + h.rounds_completed +
        " / " + h.total_rounds + " rounds";
    } else {
      outcomeLine = "<b>ABNORMAL END</b> outcome_tag=" + outcome.outcome_tag +
        " at last completed round " + outcome.last_round + " / " + h.total_rounds;
    }
    const finalScore = outcome.final_score2;
    const parts = [];
    parts.push('<div class="title">Ingress Arena Replay</div>');
    parts.push('<div class="grid">');
    parts.push("<span>" + this.esc(entrants) + "</span>");
    parts.push("<span>game_seed <b class='hash'>" + this.esc(h.game_seed) + "</b></span>");
    if (meta.map_seed !== undefined) {
      parts.push("<span>map_seed <b class='hash'>" + this.esc(meta.map_seed) + "</b></span>");
    }
    parts.push(
      "<span>portals <b>" + h.map.portals.length + "</b></span>" +
      "<span>rounds <b>" + h.rounds_completed + "/" + h.total_rounds + "</b></span>" +
      "<span>checkpoints <b>[" + h.checkpoints.join(",") + "]</b></span>" +
      "<span>versions <b>server " + h.versions.server + " · protocol " + h.versions.protocol +
      " · rng " + h.versions.rng + " · enc " + h.versions.state_encoding + "</b></span>"
    );
    parts.push("</div>");
    parts.push('<div class="grid">');
    parts.push("<span>outcome " + outcomeLine + "</span>");
    parts.push(
      "<span>final_score2 <b class='hash'>enl " + this.esc(finalScore[0]) +
      " · res " + this.esc(finalScore[1]) + "</b></span>"
    );
    parts.push(
      "<span>" + (outcome.kind === "normal" ? "final_hash" : "last_hash") +
      " <b class='hash'>" + this.esc(outcome.kind === "normal" ? outcome.final_hash : outcome.last_hash) + "</b></span>"
    );
    parts.push("</div>");
    if (outcome.kind === "abnormal") {
      parts.push(
        '<div class="banner abnormal">ABNORMAL TERMINATION — the match ended technically ' +
        "(outcome_tag " + outcome.outcome_tag + " = client forfeit for tags &gt;= 4, spawn forfeit = 3); " +
        "rounds after " + outcome.last_round + " were never played.</div>"
      );
    }
    if (h.dev) {
      parts.push(
        '<div class="banner dev">DEV EXPORT — BOTH factions\' private agents are embedded ' +
        "(header.dev=true): a debug/balance view. Player-facing exports embed one faction only.</div>"
      );
    } else {
      const embedded = h.factions_embedded.join(", ");
      parts.push(
        '<div class="banner rvis">R-VIS EXPORT — embedded agents: ' + embedded +
        ". The other faction\'s private data does not exist in this file; enemy knowledge " +
        "= public events only.</div>"
      );
    }
    document.getElementById("hdr").innerHTML = parts.join("");
  },

  // ---- exact score display (D91) ---------------------------------------
  // score2 -> cm2 by exact halving ("...5" when odd), then cm2 -> km2 by
  // an exact decimal shift of the digit string (floats never touch this).
  cm2FromScore2(s) {
    const n = BigInt(s);
    const whole = n / 2n;
    return whole.toString() + (n % 2n === 1n ? ".5" : "");
  },

  shiftRight(digits, places) {
    let padded = digits;
    while (padded.length <= places) padded = "0" + padded;
    let out = padded.slice(0, padded.length - places) + "." + padded.slice(padded.length - places);
    out = out.replace(/0+$/, "");
    if (out.endsWith(".")) out = out.slice(0, -1);
    return out;
  },

  km2FromScore2(s) {
    const n = BigInt(s);
    const odd = n % 2n === 1n;
    // cm2 = I(.5); as tenth-of-cm2 digits that is str(I) + "5", so km2
    // shifts the decimal point 11 places (10 for the cm2->km2 step plus
    // the extra tenth).
    const digits = (n / 2n).toString() + (odd ? "5" : "");
    return this.shiftRight(digits, odd ? 11 : 10);
  },

  // ---- score curve + readout -------------------------------------------
  updateCurve(i) {
    const ctx = this.curveCtx;
    const w = this.curveCanvas.width;
    const h = this.curveCanvas.height;
    ctx.clearRect(0, 0, w, h);
    const n = D.roundCount;

    // Max cumulative as a float ONLY for pixel scaling (display layer).
    let max = 1;
    for (const r of D.rounds) {
      max = Math.max(max, Number(r.score2[0]), Number(r.score2[1]));
    }
    const xOf = (r) => (n === 0 ? 0 : 30 + (r / n) * (w - 44));
    const yOf = (v) => h - 22 - (Number(v) / max) * (h - 38);

    // gridlines
    ctx.strokeStyle = "rgba(140,150,160,0.18)";
    ctx.lineWidth = 1;
    ctx.font = "10px ui-monospace, Consolas, monospace";
    ctx.fillStyle = "#77828f";
    ctx.textAlign = "right";
    for (const frac of [0, 0.5, 1]) {
      const y = h - 22 - frac * (h - 38);
      ctx.beginPath();
      ctx.moveTo(30, y);
      ctx.lineTo(w - 8, y);
      ctx.stroke();
      ctx.fillText(this.axisLabel(max * frac), 26, y + 3);
    }
    ctx.textAlign = "center";
    ctx.fillText("km2 (score2/2)", 60, 12);

    // dual cumulative polylines
    const series = [
      { color: Render.COLOR_ENL, idx: 0 },
      { color: Render.COLOR_RES, idx: 1 },
    ];
    for (const s of series) {
      ctx.strokeStyle = s.color;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(xOf(0), yOf("0"));
      for (let r = 1; r <= n; r += 1) {
        ctx.lineTo(xOf(r), yOf(D.rounds[r - 1].score2[s.idx]));
      }
      ctx.stroke();
    }

    // current-round marker
    if (i > 0) {
      const state = D.stateAt(i);
      const x = xOf(i);
      ctx.strokeStyle = "#e8b64c";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(x, 6);
      ctx.lineTo(x, h - 22);
      ctx.stroke();
      for (const s of series) {
        ctx.fillStyle = s.color;
        ctx.beginPath();
        ctx.arc(x, yOf(state.score2[s.idx]), 3.5, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // readout with the exact integers
    const state = D.stateAt(i);
    const row = (label, cls, cum, delta) =>
      '<div class="row ' + cls + '"><span>' + label + "</span>" +
      '<span class="num">score2 ' + cum + " (&gt;" + delta + ") · " +
      this.cm2FromScore2(cum) + " cm2 = " + this.km2FromScore2(cum) + " km2</span></div>";
    document.getElementById("score-readout").innerHTML =
      row("ENL", "enl", state.score2[0], state.delta2[0]) +
      row("RES", "res", state.score2[1], state.delta2[1]);
  },

  axisLabel(v) {
    // Curve axis labels are pure display: km2 with 2 significant decimals.
    const km2 = Number(v) / 2 / 1e10;
    return km2.toPrecision(3);
  },

  // ---- event log --------------------------------------------------------
  buildEventFilters() {
    const box = document.getElementById("event-filters");
    const parts = [];
    for (let k = 1; k <= 8; k += 1) {
      parts.push(
        '<label><input type="checkbox" data-kind="' + k + '" checked> ' + this.kindNames[k] + "</label>"
      );
    }
    parts.push('<span class="group"></span>');
    parts.push('<label><input type="checkbox" data-faction="0" checked> ENL</label>');
    parts.push('<label><input type="checkbox" data-faction="1" checked> RES</label>');
    box.innerHTML = parts.join("");
    box.addEventListener("change", (event) => {
      const target = event.target;
      if (target.dataset.kind !== undefined) {
        const k = Number(target.dataset.kind);
        if (target.checked) this.kindFilters.add(k);
        else this.kindFilters.delete(k);
      }
      if (target.dataset.faction !== undefined) {
        const f = Number(target.dataset.faction);
        if (target.checked) this.factionFilters.add(f);
        else this.factionFilters.delete(f);
      }
      Controls.refresh();
    });
  },

  updateEvents(i) {
    const list = document.getElementById("events");
    const parts = [];
    for (let r = 1; r <= i; r += 1) {
      for (const ev of D.rounds[r - 1].events) {
        if (!Filter.eventVisible(ev, this.kindFilters, this.factionFilters)) continue;
        parts.push(this.eventLine(ev, r === i));
      }
    }
    list.innerHTML = parts.join("");
    if (list.lastElementChild) {
      list.lastElementChild.scrollIntoView({ block: "nearest" });
    }
  },

  eventLine(ev, current) {
    const factionClass = ev.f === undefined ? "both" : ev.f === 0 ? "enl" : "res";
    const factionText = ev.f === undefined ? "BOTH" : ev.f === 0 ? "ENL" : "RES";
    let detail = "";
    if (ev.p !== undefined) detail = "p" + ev.p;
    else if (ev.l !== undefined) detail = "l" + ev.l[0] + "-" + ev.l[1];
    else if (ev.t !== undefined) detail = "t" + ev.t[0] + "-" + ev.t[1] + "-" + ev.t[2];
    else if (ev.pos !== undefined) detail = "@" + ev.pos[0] + "," + ev.pos[1];
    let extra = "";
    if (ev.d !== undefined) extra += " d" + ev.d + " hp" + ev.h;
    if (ev.ar !== undefined && ev.ar > 0) extra += " ar" + ev.ar;
    if (ev.n !== undefined) extra += " n" + ev.n;
    const agent = ev.a !== undefined ? " a" + ev.a : "";
    return (
      '<li class="' + (current ? "current " : "") + factionClass + '">' +
      '<span class="dim">R' + String(ev.r).padStart(3, " ") + " #" + String(ev.s).padStart(5, " ") + "</span> " +
      this.kindNames[ev.k] + " " + detail + agent + extra +
      ' <span class="dim">' + factionText + "</span></li>"
    );
  },

  esc(text) {
    return String(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  },
};

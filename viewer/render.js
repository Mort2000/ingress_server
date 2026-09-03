// render.js — the map canvas (display layer: floats are fine here; the
// data feeding it stays integer). One canvas; cm -> px transform fits the
// frozen map into the element with a margin and a flipped y axis.
const Render = {
  canvas: null,
  ctx: null,
  margin: 26,
  scale: 1,

  COLOR_NEUTRAL: "#8a8f98",
  COLOR_ENL: "#1fb79b",
  COLOR_RES: "#3f8cf9",
  VIEW_LABEL: {
    enl: "Faction View ENL",
    res: "Faction View RES",
    public: "Public View",
    god: "God View",
  },

  init() {
    this.canvas = document.getElementById("map");
    this.ctx = this.canvas.getContext("2d");
    const w = D.header.config.map_width_cm;
    const h = D.header.config.map_height_cm;
    this.scale = Math.min(
      (this.canvas.width - 2 * this.margin) / w,
      (this.canvas.height - 2 * this.margin) / h
    );
  },

  toPx(x, y) {
    return [
      this.margin + x * this.scale,
      this.canvas.height - this.margin - y * this.scale,
    ];
  },

  portalColor(owner) {
    if (owner === 1) return this.COLOR_ENL;
    if (owner === 2) return this.COLOR_RES;
    return this.COLOR_NEUTRAL;
  },

  factionColor(tagOrKey) {
    if (tagOrKey === 0 || tagOrKey === "enl") return this.COLOR_ENL;
    if (tagOrKey === 1 || tagOrKey === "res") return this.COLOR_RES;
    return this.COLOR_NEUTRAL;
  },

  // Draw the fully materialized state of round `i` under `view`.
  draw(i, view) {
    const ctx = this.ctx;
    const state = D.stateAt(i);
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);

    // Fields first (background layer): semi-transparent triangles.
    ctx.globalAlpha = 0.18;
    for (const f of state.fields) {
      const pa = D.portalPos.get(f[0]);
      const pb = D.portalPos.get(f[1]);
      const pc = D.portalPos.get(f[2]);
      if (!pa || !pb || !pc) continue;
      ctx.fillStyle = this.factionColor(f[3]);
      ctx.beginPath();
      ctx.moveTo(...this.toPx(pa[0], pa[1]));
      ctx.lineTo(...this.toPx(pb[0], pb[1]));
      ctx.lineTo(...this.toPx(pc[0], pc[1]));
      ctx.closePath();
      ctx.fill();
    }
    ctx.globalAlpha = 1;

    // Links: 2px faction lines.
    ctx.lineWidth = 2;
    for (const l of state.links) {
      const pa = D.portalPos.get(l[0]);
      const pb = D.portalPos.get(l[1]);
      if (!pa || !pb) continue;
      ctx.strokeStyle = this.factionColor(l[2]);
      ctx.beginPath();
      ctx.moveTo(...this.toPx(pa[0], pa[1]));
      ctx.lineTo(...this.toPx(pb[0], pb[1]));
      ctx.stroke();
    }

    // Portals: filled circles with an hp ring arc.
    const maxHp = D.header.config.portal_max_hp;
    for (const p of state.portals) {
      const pos = D.portalPos.get(p[0]);
      if (!pos) continue;
      const [px, py] = this.toPx(pos[0], pos[1]);
      const color = this.portalColor(p[1]);
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(px, py, 5, 0, Math.PI * 2);
      ctx.fill();
      if (p[1] !== 0 && p[2] > 0 && maxHp > 0) {
        // hp ring: fraction of the full circle, damaged portion dimmed.
        const frac = p[2] / maxHp;
        ctx.strokeStyle = color;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(px, py, 9, -Math.PI / 2, -Math.PI / 2 + frac * Math.PI * 2);
        ctx.stroke();
        ctx.strokeStyle = "rgba(140,150,160,0.35)";
        ctx.beginPath();
        ctx.arc(px, py, 9, -Math.PI / 2 + frac * Math.PI * 2, -Math.PI / 2 + Math.PI * 2);
        ctx.stroke();
      }
      // id label
      ctx.fillStyle = "#6f7a86";
      ctx.font = "9px ui-monospace, Consolas, monospace";
      ctx.textAlign = "center";
      ctx.fillText(String(p[0]), px, py + 20);
    }

    // Agents: dots only where the active view allows (R-VIS).
    for (const a of Filter.visibleAgents(view, i)) {
      const [px, py] = this.toPx(a.x, a.y);
      ctx.fillStyle = this.factionColor(a.faction);
      ctx.beginPath();
      ctx.arc(px, py, 4, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = "#0b0e12";
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.fillStyle = "#c9d3dd";
      ctx.font = "10px ui-monospace, Consolas, monospace";
      ctx.textAlign = "left";
      ctx.fillText(
        "a" + a.id + (i > 0 && a.supply >= 0 ? " (" + a.supply + ")" : ""),
        px + 7,
        py - 6
      );
    }

    // View label + round badge.
    ctx.fillStyle = "#8a97a8";
    ctx.font = "12px 'Segoe UI', system-ui, sans-serif";
    ctx.textAlign = "left";
    ctx.fillText(this.VIEW_LABEL[view] + "  ·  round " + i + " / " + D.roundCount, 12, 18);
    ctx.fillStyle = "#5d6772";
    ctx.font = "11px 'Segoe UI', system-ui, sans-serif";
    ctx.fillText(
      "ENL " + this.COLOR_ENL + "   RES " + this.COLOR_RES + "   Neutral #8a8f98",
      12,
      this.canvas.height - 10
    );
  },
};

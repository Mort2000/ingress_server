import { colors, type Point, ReplayModel } from "./model";
interface Camera {
  x: number;
  y: number;
  width: number;
  height: number;
}
const ns = "http://www.w3.org/2000/svg";
const line = (a: Point, b: Point) => `M${a.join(",")}L${b.join(",")}`;
export class TacticalMap {
  private svg = document.createElementNS(ns, "svg");
  private camera: Camera = { x: 0, y: 0, width: 1, height: 1 };
  private drag?: { point: Point; camera: Camera; box: boolean; moved: boolean };
  private selection = document.createElementNS(ns, "rect");
  private boxMode = false;
  private resizeObserver = new ResizeObserver(() =>
    this.refreshCameraDrawing(),
  );
  constructor(
    private host: HTMLElement,
    private model: ReplayModel,
    private changed: () => void,
    private hovered: () => void,
  ) {
    this.svg.id = "map";
    this.svg.setAttribute(
      "aria-label",
      "战术地图：滚轮缩放，拖动平移，Shift 拖动框选放大",
    );
    this.svg.setAttribute("tabindex", "0");
    this.selection.setAttribute("class", "zoom-selection");
    host.prepend(this.svg);
    this.reset();
    this.resizeObserver.observe(host);
    this.svg.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        this.zoom(e.deltaY < 0 ? 1.25 : 0.8, this.point(e));
      },
      { passive: false },
    );
    this.svg.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      this.drag = {
        point: this.point(e),
        camera: { ...this.camera },
        box: e.shiftKey || this.boxMode,
        moved: false,
      };
      this.svg.setPointerCapture(e.pointerId);
    });
    this.svg.addEventListener("pointermove", (e) => {
      if (this.drag) {
        const p = this.point(e),
          d = this.drag;
        if (
          Math.hypot(p[0] - d.point[0], p[1] - d.point[1]) >
          this.camera.width / 250
        )
          d.moved = true;
        if (!d.moved) return;
        if (d.box) {
          this.selection.setAttribute("x", String(Math.min(p[0], d.point[0])));
          this.selection.setAttribute("y", String(Math.min(p[1], d.point[1])));
          this.selection.setAttribute(
            "width",
            String(Math.abs(p[0] - d.point[0])),
          );
          this.selection.setAttribute(
            "height",
            String(Math.abs(p[1] - d.point[1])),
          );
          this.svg.append(this.selection);
        } else {
          this.camera.x += d.point[0] - p[0];
          this.camera.y += d.point[1] - p[1];
          this.applyCamera();
        }
      } else {
        const portal = (e.target as Element)
          .closest("[data-portal]")
          ?.getAttribute("data-portal");
        this.setHover(portal == null ? null : Number(portal));
      }
    });
    this.svg.addEventListener("pointerup", (e) => {
      const d = this.drag;
      if (!d) return;
      if (d.box && d.moved) {
        const p = this.point(e),
          width = Math.abs(p[0] - d.point[0]),
          height = Math.abs(p[1] - d.point[1]);
        if (width > d.camera.width / 100 && height > d.camera.height / 100) {
          this.camera = {
            x: Math.min(p[0], d.point[0]),
            y: Math.min(p[1], d.point[1]),
            width,
            height,
          };
          this.applyCamera();
        }
      } else if (!d.moved) {
        const target = document
          .elementFromPoint(e.clientX, e.clientY)
          ?.closest("[data-agent]");
        if (target) {
          this.model.selected = Number(target.getAttribute("data-agent"));
          this.changed();
        }
      }
      this.drag = undefined;
      this.selection.remove();
      this.svg.releasePointerCapture(e.pointerId);
    });
    this.svg.addEventListener("pointercancel", () => {
      this.drag = undefined;
      this.selection.remove();
    });
    this.svg.addEventListener("pointerleave", () => {
      if (!this.drag) this.setHover(null);
    });
    this.svg.addEventListener("keydown", (e) => {
      const target = (e.target as Element).closest("[data-agent]");
      if (target && (e.key === "Enter" || e.key === " ")) {
        e.preventDefault();
        e.stopPropagation();
        this.model.selected = Number(target.getAttribute("data-agent"));
        this.changed();
      }
    });
    this.svg.addEventListener("focusin", (e) => {
      const id = (e.target as Element)
        .closest("[data-portal]")
        ?.getAttribute("data-portal");
      this.setHover(id == null ? null : +id);
    });
    this.svg.addEventListener("focusout", () => this.setHover(null));
  }
  destroy() {
    this.resizeObserver.disconnect();
    this.svg.remove();
  }
  setBoxMode(enabled: boolean) {
    this.boxMode = enabled;
    this.svg.classList.toggle("box-mode", enabled);
  }
  private setHover(id: number | null) {
    if (this.model.hoverPortal !== id) {
      this.model.hoverPortal = id;
      this.hovered();
    }
  }
  private point(e: MouseEvent): Point {
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(
      this.svg.getScreenCTM()!.inverse(),
    );
    return [p.x, p.y];
  }
  private applyCamera() {
    this.svg.setAttribute(
      "viewBox",
      `${this.camera.x} ${this.camera.y} ${this.camera.width} ${this.camera.height}`,
    );
    const factor =
      this.model.data.header.config.map_width_cm / this.camera.width;
    this.host.querySelector("#zoom-level")!.textContent =
      `${factor.toFixed(1)}×`;
    this.refreshCameraDrawing();
  }
  private refreshCameraDrawing() {
    this.render();
    this.highlight();
  }
  reset() {
    const c = this.model.data.header.config,
      margin = c.map_width_cm * 0.035;
    this.camera = {
      x: -margin,
      y: -margin,
      width: c.map_width_cm + margin * 2,
      height: c.map_height_cm + margin * 2,
    };
    this.applyCamera();
  }
  zoom(
    factor: number,
    center: Point = [
      this.camera.x + this.camera.width / 2,
      this.camera.y + this.camera.height / 2,
    ],
  ) {
    const width = this.camera.width / factor;
    if (
      width < this.model.data.header.config.map_width_cm / 100 ||
      width > this.model.data.header.config.map_width_cm * 4
    )
      return;
    this.camera = {
      x: center[0] - (center[0] - this.camera.x) / factor,
      y: center[1] - (center[1] - this.camera.y) / factor,
      width,
      height: this.camera.height / factor,
    };
    this.applyCamera();
  }
  focusSelected() {
    const a = this.model.agent;
    if (!a) return;
    const size = this.model.data.header.config.map_width_cm / 5;
    this.camera = {
      x: a.row[1] - size / 2,
      y: this.model.data.header.config.map_height_cm - a.row[2] - size / 2,
      width: size,
      height: size,
    };
    this.applyCamera();
  }
  render() {
    const m = this.model,
      c = m.data.header.config,
      snapshot = m.snapshot;
    const screen = ([x, y]: Point): Point => [x, c.map_height_cm - y];
    const positions = new Map(
      m.data.header.map.portals.map(([id, x, y]) => [id, screen([x, y])]),
    );
    // Markers and labels remain readable at every zoom; game geometry,
    // including the operation radius, remains in authoritative world units.
    const radius = 5.5 / (this.svg.getScreenCTM()?.a ?? 1);
    const factionColor = (tag: number) => (tag === 0 ? colors.enl : colors.res);
    let content = `<defs><pattern id="grid" width="${c.map_width_cm / 12}" height="${c.map_height_cm / 12}" patternUnits="userSpaceOnUse"><path d="M${c.map_width_cm / 12},0H0V${c.map_height_cm / 12}" fill="none" stroke="#112333" vector-effect="non-scaling-stroke"/></pattern></defs><rect width="${c.map_width_cm}" height="${c.map_height_cm}" fill="url(#grid)" stroke="#25435b" vector-effect="non-scaling-stroke"/>`;
    for (const [a, b, d, fac] of snapshot.fields)
      content += `<polygon points="${[a, b, d].map((id) => positions.get(id)!.join(",")).join(" ")}" fill="${factionColor(fac)}" fill-opacity=".09"/>`;
    for (const [a, b, fac] of snapshot.links)
      content += `<path d="${line(positions.get(a)!, positions.get(b)!)}" stroke="${factionColor(fac)}" opacity=".65" fill="none" stroke-width="1.5" vector-effect="non-scaling-stroke"/>`;
    if (m.agent) {
      const segments = m.movements(m.agent);
      const [ax, ay] = screen([m.agent.row[1], m.agent.row[2]]);
      content += `<circle id="operation-range" cx="${ax}" cy="${ay}" r="${c.portal_operation_range_cm}" fill="#00e5ff08" stroke="#00e5ff77" stroke-dasharray="4 5" vector-effect="non-scaling-stroke" pointer-events="none"/>`;
      const roundEnds = new Map(segments.map((s) => [s.round, s.to]));
      for (const [r, point] of roundEnds) {
        if (r === m.round || r % 5 !== 0) continue;
        const [x, y] = screen(point);
        content += `<g class="trail-label" pointer-events="none"><circle cx="${x}" cy="${y}" r="${radius * 0.3}" fill="${colors[m.agent.faction]}"/><text x="${x}" y="${y - radius}" font-size="${radius * 1.3}" fill="#b1c4d577">R${r}</text></g>`;
      }
      for (const cmd of m.stream(m.agent)?.cmds ?? []) {
        if (!cmd.from || !cmd.to || cmd.status !== "ok") continue;
        const [x, y] = screen([
          (cmd.from[0] + cmd.to[0]) / 2,
          (cmd.from[1] + cmd.to[1]) / 2,
        ]);
        content += `<text class="move-label" x="${x}" y="${y - radius}" font-size="${radius * 1.5}" fill="#00e5ff" pointer-events="none">${(cmd.cost_cm ?? 0) / 100} m</text>`;
      }
      for (const current of [false, true]) {
        const d = segments
          .filter((s) => (s.round === m.round) === current)
          .map((s) => line(screen(s.from), screen(s.to)))
          .join("");
        content += `<path class="trajectory ${current ? "current" : "history"}" data-segments="${segments.filter((s) => (s.round === m.round) === current).length}" d="${d}" fill="none" stroke="${current ? "#ffffff" : colors[m.agent.faction]}" stroke-opacity="${current ? 1 : 0.4}" stroke-width="${current ? 3.5 : 1.5}" vector-effect="non-scaling-stroke"/>`;
      }
    }
    for (const [id, owner, hp] of snapshot.portals) {
      const [x, y] = positions.get(id)!;
      const color =
        owner === 0 ? colors.neutral : owner === 1 ? colors.enl : colors.res;
      content += `<g class="portal" data-portal="${id}" transform="translate(${x} ${y})" tabindex="0" role="button" aria-label="Portal ${id}, HP ${hp}"><circle class="key-ring" r="${radius * 2.5}"/><circle class="portal-hp" r="${radius * 1.6}" fill="none" stroke="${color}" stroke-width="1.5" pathLength="100" stroke-dasharray="${(hp / c.portal_max_hp) * 100} 100" transform="rotate(-90)" vector-effect="non-scaling-stroke"/><circle r="${radius}" fill="${color}" fill-opacity=".2" stroke="${color}" stroke-width="1.4" vector-effect="non-scaling-stroke"/><path d="M${-radius * 0.5},0H${radius * 0.5}M0,${-radius * 0.5}V${radius * 0.5}" stroke="${color}" vector-effect="non-scaling-stroke"/><text x="${radius * 1.5}" y="${-radius}" font-size="${radius * 1.8}" fill="${color}">P${id}</text></g>`;
    }
    for (const event of snapshot.events) {
      if (event.k !== 8 || !event.pos) continue;
      const [x, y] = screen(event.pos);
      content += `<g class="reinforcement" transform="translate(${x} ${y})" pointer-events="none"><circle r="${radius * 4}" fill="none" stroke="#b98cff" vector-effect="non-scaling-stroke"/><text y="${-radius * 5}" font-size="${radius * 1.5}" fill="#b98cff">REINFORCE +${event.n}/FACTION</text></g>`;
    }
    for (const agent of m.agents) {
      const [id, x, y] = agent.row,
        color = colors[agent.faction],
        selected = m.selected === id;
      content += `<g class="map-agent ${selected ? "selected" : ""}" data-agent="${id}" transform="translate(${x} ${c.map_height_cm - y})" tabindex="0" role="button" aria-label="Agent ${id}"><circle class="key-ring" r="${radius * 3}"/><circle class="agent-halo" r="${radius * 2}" fill="none" stroke="${color}" opacity="${selected ? ".65" : "0"}" vector-effect="non-scaling-stroke"/><path d="M0,${-radius * 1.5}L${radius},${radius}L0,${radius * 0.55}L${-radius},${radius}Z" fill="${color}" stroke="#04060c" vector-effect="non-scaling-stroke"/><text x="${radius * 1.8}" y="${radius * 2}" font-size="${radius * 2}" fill="${color}">A${id}</text></g>`;
    }
    this.svg.innerHTML = content;
  }
  highlight() {
    const portal = this.model.hoverPortal;
    this.svg
      .querySelectorAll("[data-portal]")
      .forEach((node) =>
        node.classList.toggle(
          "key-highlight",
          Number(node.getAttribute("data-portal")) === portal,
        ),
      );
    const ids = new Set(
      portal == null ? [] : this.model.holders(portal).map((a) => a.row[0]),
    );
    this.svg
      .querySelectorAll("[data-agent]")
      .forEach((node) =>
        node.classList.toggle(
          "key-highlight",
          ids.has(Number(node.getAttribute("data-agent"))),
        ),
      );
  }
}

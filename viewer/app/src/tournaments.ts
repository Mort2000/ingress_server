import "./tournaments.css";
import { TournamentOverview } from "./tournament-overview";

export interface Entrant {
  id: number;
  registered_id: string;
  name: string;
  version: string | null;
}
interface Attempt {
  number: string;
  state: string;
  attempt_reason: string | null;
  error: string | null;
  replay: boolean;
  outcome?: string;
  stage?: string;
  reason?: string;
  winner?: number;
  score_enl?: string;
  score_res?: string;
  rounds?: number;
  elapsed_ms?: number;
  finished_unix_ms: number | null;
  factions_verified?: boolean;
}
interface Match {
  index: number;
  map: number;
  map_hash: string;
  enl: number;
  res: number;
  state: string;
  attempts: Attempt[];
}
interface Summary {
  id: string;
  label: string;
  name?: string;
  error: string | null;
  total?: number;
  completed?: number;
}
interface Catalog {
  schema: string;
  scanned_unix_ms: number;
  events: Summary[];
  errors: string[];
}
interface Standing {
  entrant: number;
  rank: number;
  rating: number;
  wins: number;
  draws: number;
  losses: number;
  forfeit_wins: number;
  forfeit_losses: number;
  unscored: number;
}
export interface Matchup {
  a: number;
  b: number;
  total: number;
  a_wins: number;
  b_wins: number;
  draws: number;
  a_technical_wins: number;
  b_technical_wins: number;
  pending: number;
  faults: number;
  unscored: number;
}
export interface Event extends Summary {
  schema?: string;
  scanned_unix_ms: number;
  counts: Record<string, number>;
  entrants: Entrant[];
  matches: Match[];
  standings: Standing[];
  matched: number;
  offset: number;
  limit: number;
  matchups?: Matchup[];
}
const states: Record<string, string> = {
  pending: "待执行",
  recorded_running: "记录为运行中（存活未确认）",
  publishing: "归档发布中",
  completed: "已完成",
  infrastructure_fault: "基础设施故障",
  interrupted: "已记录中断",
  corrupt: "归档损坏",
};
const outcomes: Record<string, string> = {
  draw: "平局",
  enl_win: "ENL 胜",
  res_win: "RES 胜",
  technical_win: "技术判胜",
  unscored: "无评分",
};
function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text = "",
  className = "",
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.textContent = text;
  node.className = className;
  return node;
}
const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;

/** UI talks only to its same-origin host. No archive paths or runner endpoints. */
export class TournamentPanel {
  private readonly panel = element("section");
  private token = "";
  private timer: ReturnType<typeof setInterval>;
  private refreshRequest?: AbortController;
  private replayRequest?: AbortController;
  private generation = 0;
  private selected = "";
  private offset = 0;
  private data?: Event;
  private expanded = new Set<number>();
  private overview: TournamentOverview;
  private filters = new URLSearchParams();
  private rowsSignature = "";

  constructor(
    private openReplay: (file: File) => void,
    private pauseReplay: () => void,
  ) {
    this.readAuthorization();
    const button = element("button", "赛事", "btn");
    button.id = "open-tournaments";
    button.onclick = () => this.show();
    $("topright").prepend(button);
    this.panel.id = "tournament-panel";
    this.panel.hidden = true;
    this.panel.setAttribute("aria-label", "赛事监控");
    this.panel.innerHTML = `
      <header class="tournament-heading"><div><p class="eyebrow">TOURNAMENT ARCHIVE</p><h1>赛事监控</h1></div><button class="btn" id="close-tournaments">返回回放</button></header>
      <p id="tournament-status" role="status" aria-live="polite">尚未连接</p>
      <div class="tournament-tools"><label>赛事 <select id="event-select" aria-label="赛事选择"></select></label><button class="btn" id="refresh-tournaments">立即刷新</button></div>
      <p id="event-progress"></p><p id="event-warning"></p>
      <nav class="tournament-tabs" role="tablist" aria-label="赛事视图">
        <button id="tab-overview" role="tab" aria-controls="panel-overview" aria-selected="true">赛事概览</button>
        <button id="tab-matches" role="tab" aria-controls="panel-matches" aria-selected="false" tabindex="-1">比赛详情与回放</button>
      </nav>
      <section id="panel-overview" role="tabpanel" aria-labelledby="tab-overview"></section>
      <section id="panel-matches" role="tabpanel" aria-labelledby="tab-matches" hidden>
      <h2>比赛详情与回放</h2>
      <form id="event-filters" class="tournament-tools">
        <label>选手<select id="event-entrant" aria-label="对决选手"><option value="">全部选手</option></select></label>
        <label>对手<select id="event-opponent" aria-label="对决对手"><option value="">全部对手</option></select></label>
        <label>选手或版本<input id="event-search" aria-label="选手或版本" maxlength="256" placeholder="名称、注册 ID、版本"></label>
        <label>地图<input id="event-map" aria-label="地图编号" type="number" min="0" placeholder="全部"></label>
        <label>状态<select id="event-state" aria-label="状态筛选"><option value="">全部</option>${Object.entries(
          states,
        )
          .map(([k, v]) => `<option value="${k}">${v}</option>`)
          .join("")}</select></label>
        <label>结果<select id="event-outcome" aria-label="结果筛选"><option value="">全部</option>${Object.entries(
          outcomes,
        )
          .map(([k, v]) => `<option value="${k}">${v}</option>`)
          .join("")}</select></label>
        <label>排序<select id="event-order" aria-label="比赛排序"><option value="latest">最新完成优先</option><option value="schedule">冻结赛程顺序</option></select></label>
        <button class="btn" type="submit">应用筛选</button>
        <button class="btn" type="button" id="clear-event-filters">清除筛选</button>
      </form>
      <p id="active-match-filter" class="tournament-note" aria-live="polite"></p>
      <div id="match-list"></div>
      <nav class="tournament-tools"><button class="btn" id="matches-prev">上一页</button><span id="matches-page"></span><button class="btn" id="matches-next">下一页</button></nav>
      </section>
      <p class="tournament-note">后台只读归档，不启动／恢复／补赛。运行标记不证明进程存活；回放包含双方赛后私有信息，不是参赛程序可使用的输入。</p>`;
    document.body.append(this.panel);
    $<HTMLButtonElement>("matches-prev").disabled = true;
    $<HTMLButtonElement>("matches-next").disabled = true;
    this.overview = new TournamentOverview($("panel-overview"), (a, b) => {
      $<HTMLFormElement>("event-filters").reset();
      $<HTMLSelectElement>("event-entrant").value = String(a);
      $<HTMLSelectElement>("event-opponent").value = String(b);
      this.setTab("matches", true);
      this.applyFilters();
    });
    const tabs = ["overview", "matches"] as const;
    for (const [index, tab] of tabs.entries()) {
      $(`tab-${tab}`).onclick = () => this.setTab(tab);
      $(`tab-${tab}`).onkeydown = (e) => {
        let next: number;
        if (e.key === "Home") next = 0;
        else if (e.key === "End") next = tabs.length - 1;
        else if (e.key === "ArrowRight") next = (index + 1) % tabs.length;
        else if (e.key === "ArrowLeft")
          next = (index + tabs.length - 1) % tabs.length;
        else return;
        e.preventDefault();
        this.setTab(tabs[next], true);
      };
    }
    $("close-tournaments").onclick = () => this.close();
    $("refresh-tournaments").onclick = () => void this.refresh(true);
    $("event-select").onchange = () => {
      this.selected = $<HTMLSelectElement>("event-select").value;
      this.offset = 0;
      this.expanded.clear();
      this.filters = new URLSearchParams();
      $<HTMLFormElement>("event-filters").reset();
      this.setTab("overview");
      this.clear();
      void this.refresh(true);
    };
    $("event-filters").onsubmit = (e) => {
      e.preventDefault();
      this.applyFilters();
    };
    $("event-filters").oninput = () =>
      $<HTMLSelectElement>("event-opponent").setCustomValidity("");
    $("clear-event-filters").onclick = () => {
      $<HTMLFormElement>("event-filters").reset();
      this.applyFilters();
    };
    $("matches-prev").onclick = () => {
      this.offset = Math.max(0, this.offset - 50);
      void this.refresh(true);
    };
    $("matches-next").onclick = () => {
      this.offset += 50;
      void this.refresh(true);
    };
    this.timer = setInterval(() => {
      if (!this.panel.hidden) void this.refresh();
    }, 2000);
    window.addEventListener("pagehide", () => {
      clearInterval(this.timer);
      this.close();
    });
    if (this.token) this.show();
    window.addEventListener("hashchange", () => {
      if (this.readAuthorization()) this.show();
    });
  }
  private setTab(tab: "overview" | "matches", focus = false) {
    for (const name of ["overview", "matches"]) {
      const selected = name === tab;
      $(`tab-${name}`).setAttribute("aria-selected", String(selected));
      $(`tab-${name}`).tabIndex = selected ? 0 : -1;
      $(`panel-${name}`).hidden = !selected;
    }
    if (focus) $(`tab-${tab}`).focus();
  }
  private applyFilters() {
    const entrant = $<HTMLSelectElement>("event-entrant").value;
    const opponent = $<HTMLSelectElement>("event-opponent");
    opponent.setCustomValidity(
      entrant && entrant === opponent.value ? "请选择不同的对手" : "",
    );
    if (!$<HTMLFormElement>("event-filters").reportValidity()) return;
    this.filters = new URLSearchParams();
    for (const [key, id] of [
      ["entrant", "event-entrant"],
      ["opponent", "event-opponent"],
      ["q", "event-search"],
      ["map", "event-map"],
      ["state", "event-state"],
      ["outcome", "event-outcome"],
      ["order", "event-order"],
    ])
      this.filters.set(key, $<HTMLInputElement>(id).value);
    this.offset = 0;
    this.rowsSignature = "";
    this.expanded.clear();
    $("match-list").replaceChildren(
      element("p", "正在查询比赛…", "tournament-note"),
    );
    this.updateFilterLabel();
    void this.refresh(true);
  }
  private updateFilterLabel() {
    const a = this.filters.get("entrant"),
      b = this.filters.get("opponent");
    $("active-match-filter").textContent =
      a || b
        ? `当前对决：${a ? this.participant(Number(a)) : "全部选手"} ↔ ${b ? this.participant(Number(b)) : "全部对手"}（包含双方阵营）`
        : "全部对决 · 可从赛事概览的对决表点击进入";
  }
  private readAuthorization() {
    const raw = new URLSearchParams(location.hash.slice(1)).get("token");
    const token = raw && /^[a-f0-9]{32}$/.test(raw) ? raw : null;
    try {
      if (token) sessionStorage.setItem("arena-viewer-token", token);
      this.token = token || sessionStorage.getItem("arena-viewer-token") || "";
    } catch {
      if (token) this.token = token;
    }
    if (raw)
      history.replaceState(null, "", location.pathname + location.search);
    return token !== null;
  }
  get visible() {
    return !this.panel.hidden;
  }
  show() {
    this.pauseReplay();
    this.panel.hidden = false;
    $("app").inert = true;
    this.panel
      .querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')
      ?.focus();
    void this.refresh(true);
  }
  close() {
    this.panel.hidden = true;
    $("app").inert = false;
    $("open-tournaments").focus({ preventScroll: true });
    this.generation++;
    this.refreshRequest?.abort();
    this.refreshRequest = undefined;
    this.replayRequest?.abort();
  }
  private clear() {
    this.data = undefined;
    this.rowsSignature = "";
    this.overview.clear();
    for (const id of ["event-entrant", "event-opponent"]) {
      const option = element(
        "option",
        id === "event-entrant" ? "全部选手" : "全部对手",
      );
      option.value = "";
      $(id).replaceChildren(option);
    }
    $<HTMLButtonElement>("matches-prev").disabled = true;
    $<HTMLButtonElement>("matches-next").disabled = true;
    for (const id of [
      "event-progress",
      "event-warning",
      "match-list",
      "matches-page",
      "active-match-filter",
    ])
      $(id).replaceChildren();
  }
  private async fetch(path: string, signal: AbortSignal) {
    const response = await fetch(path, {
      headers: { Authorization: `Bearer ${this.token}` },
      signal,
      cache: "no-store",
    });
    if (!response.ok)
      throw new Error(`${response.status}: ${await response.text()}`);
    return response;
  }
  private async refresh(force = false) {
    if (this.refreshRequest && !force) return;
    if (!this.token) {
      $("tournament-status").textContent =
        "请启动独立 arena-viewer 后台，并打开终端输出的授权链接。手动打开 .iarp 不需要后台。";
      return;
    }
    this.refreshRequest?.abort();
    const controller = new AbortController();
    this.refreshRequest = controller;
    const generation = ++this.generation;
    const timeout = setTimeout(() => controller.abort(), 8000);
    try {
      const catalog: Catalog = await (
        await this.fetch("./api/events", controller.signal)
      ).json();
      if (catalog.schema !== "arena-catalog-v1")
        throw new Error("不兼容的赛事目录格式");
      if (generation !== this.generation) return;
      const select = $<HTMLSelectElement>("event-select");
      const options = catalog.events.map((e) =>
        element(
          "option",
          `${e.name || e.label} · ${e.label}${e.error ? "（未就绪）" : ""}`,
        ),
      );
      options.forEach((o, i) => (o.value = catalog.events[i].id));
      if (
        select.options.length !== options.length ||
        options.some(
          (o, i) =>
            o.value !== select.options[i]?.value ||
            o.text !== select.options[i]?.text,
        )
      )
        select.replaceChildren(...options);
      if (!catalog.events.some((e) => e.id === this.selected)) {
        this.selected = catalog.events[0]?.id || "";
        this.offset = 0;
        this.expanded.clear();
        this.filters = new URLSearchParams();
        $<HTMLFormElement>("event-filters").reset();
        this.setTab("overview");
        this.clear();
      }
      select.value = this.selected;
      if (this.selected) {
        const query = new URLSearchParams(this.filters);
        query.set("offset", String(this.offset));
        query.set("limit", "50");
        const data: Event = await (
          await this.fetch(
            `./api/events/${this.selected}?${query}`,
            controller.signal,
          )
        ).json();
        if (generation !== this.generation) return;
        if (data.schema && data.schema !== "arena-monitor-v1")
          throw new Error("不兼容的赛事格式");
        this.data = data;
        this.render(data);
      } else {
        $("event-progress").textContent =
          "尚未发现赛事；后台会自动发现配置目录下新发布的计划。";
      }
      $("tournament-status").textContent =
        `已连接 · 后台扫描时间 ${catalog.scanned_unix_ms ? new Date(catalog.scanned_unix_ms).toLocaleTimeString() : "首次扫描中"}${catalog.errors.length ? ` · 扫描异常：${catalog.errors.join("；")}` : ""}`;
    } catch (e) {
      if (generation === this.generation)
        $("tournament-status").textContent =
          `连接中断或查询失败；保留上次数据，可能已过期。${String(e)}（后台重启后请重新打开授权链接）`;
    } finally {
      clearTimeout(timeout);
      if (this.refreshRequest === controller) this.refreshRequest = undefined;
    }
  }
  private participant(id: number) {
    const p = this.data?.entrants?.find((e) => e.id === id);
    return p ? `${p.name} · ${p.version || "版本未知"}` : "选手未知";
  }
  private render(data: Event) {
    $("event-warning").textContent = data.error
      ? `归档不可用；以下可能是旧快照，回放入口已停用：${data.error}`
      : "";
    if (!data.matches) {
      this.clear();
      $("event-warning").textContent =
        `赛事归档尚未就绪：${data.error || "没有可用快照"}`;
      return;
    }
    this.overview.render(data);
    for (const id of ["event-entrant", "event-opponent"]) {
      const select = $<HTMLSelectElement>(id);
      const entries = data.entrants.map((e) => ({
        value: String(e.id),
        label: `#${e.id} ${this.participant(e.id)}`,
      }));
      if (
        select.options.length !== entries.length + 1 ||
        entries.some(
          (e, i) =>
            select.options[i + 1]?.value !== e.value ||
            select.options[i + 1]?.text !== e.label,
        )
      ) {
        const value = select.value;
        const all = element(
          "option",
          id === "event-entrant" ? "全部选手" : "全部对手",
        );
        all.value = "";
        select.replaceChildren(
          all,
          ...entries.map((e) => {
            const option = element("option", e.label);
            option.value = e.value;
            return option;
          }),
        );
        select.value = entries.some((e) => e.value === value) ? value : "";
      }
    }
    this.updateFilterLabel();
    $("event-progress").textContent = [
      `已完成 ${data.completed} / ${data.total}`,
      ...Object.entries(data.counts)
        .filter(([s, n]) => s !== "completed" && n > 0)
        .map(([s, n]) => `${states[s] || s} ${n}`),
      `归档扫描 ${new Date(data.scanned_unix_ms).toLocaleTimeString()}`,
    ].join(" · ");
    const signature = JSON.stringify([
      data.matches,
      data.entrants,
      data.error,
      data.matched,
      data.offset,
    ]);
    if (signature === this.rowsSignature) return;
    this.rowsSignature = signature;
    const focused =
      document.activeElement instanceof HTMLElement
        ? document.activeElement.dataset.matchFocus
        : undefined;
    const rows = data.matches.map((m) => {
      const row = element("article", "", "match-row");
      row.append(element("h3", `M${m.index} · 地图 ${m.map}`));
      row.title = `地图哈希 ${m.map_hash}`;
      row.append(
        element(
          "p",
          `ENL ${this.participant(m.enl)} ↔ RES ${this.participant(m.res)}`,
        ),
      );
      const latest = m.attempts.at(-1);
      row.append(
        element(
          "p",
          `${states[m.state] || m.state}${latest ? ` · ${this.result(latest)}` : ""}`,
          "match-result",
        ),
      );
      if (latest?.replay && !data.error)
        row.append(this.replayButton(m, latest, "打开回放"));
      else
        row.append(
          element(
            "span",
            m.state === "completed" ? "无可用回放" : "回放未就绪",
            "tournament-note",
          ),
        );
      if (m.attempts.length) {
        const history = element("details");
        history.open = this.expanded.has(m.index);
        const summary = element("summary", `尝试记录（${m.attempts.length}）`);
        summary.dataset.matchFocus = `history-${m.index}`;
        history.append(summary);
        for (const a of m.attempts) {
          const info = element("div", "", "attempt-row");
          info.append(
            element(
              "p",
              `#${a.number} · ${states[a.state] || a.state} · ${a.attempt_reason || "原因未记录"}`,
            ),
          );
          info.append(element("p", this.result(a)));
          if (a.replay && !data.error)
            info.append(this.replayButton(m, a, `打开 #${a.number} 回放`));
          history.append(info);
        }
        history.ontoggle = () => {
          if (history.isConnected) {
            if (history.open) this.expanded.add(m.index);
            else this.expanded.delete(m.index);
          }
        };
        row.append(history);
      }
      return row;
    });
    $("match-list").replaceChildren(
      ...(rows.length
        ? rows
        : [element("p", "没有符合筛选条件的比赛。", "tournament-note")]),
    );
    if (focused) {
      for (const node of $("match-list").querySelectorAll<HTMLElement>(
        "[data-match-focus]",
      ))
        if (node.dataset.matchFocus === focused)
          node.focus({ preventScroll: true });
    }
    $("matches-page").textContent =
      `${data.matched} 场匹配 · 第 ${Math.floor(this.offset / 50) + 1} 页`;
    $<HTMLButtonElement>("matches-prev").disabled = this.offset === 0;
    $<HTMLButtonElement>("matches-next").disabled =
      this.offset + 50 >= data.matched;
  }
  private result(a: Attempt) {
    const parts = [
      a.outcome ? outcomes[a.outcome] || a.outcome : "",
      a.stage || "",
      a.reason || a.error || "",
    ];
    if (a.score_enl !== undefined)
      parts.push(`score2 ${a.score_enl} : ${a.score_res} · ${a.rounds} 轮`);
    if (a.winner !== undefined)
      parts.push(`胜者 ${this.participant(a.winner)}`);
    if (a.elapsed_ms !== undefined) parts.push(`${a.elapsed_ms} ms`);
    if (a.finished_unix_ms)
      parts.push(new Date(a.finished_unix_ms).toLocaleString());
    if (a.factions_verified === false) parts.push("阵营绑定未核验");
    return parts.filter(Boolean).join(" · ");
  }
  private replayButton(m: Match, a: Attempt, title: string) {
    const button = element("button", title, "btn");
    button.dataset.matchFocus = `replay-${m.index}-${a.number}-${title}`;
    const event = this.selected;
    button.onclick = async () => {
      this.replayRequest?.abort();
      const controller = new AbortController();
      this.replayRequest = controller;
      const timeout = setTimeout(() => controller.abort(), 60_000);
      $("tournament-status").textContent =
        `正在读取 M${m.index} / #${a.number} …`;
      try {
        const response = await this.fetch(
          `./api/events/${event}/matches/${m.index}/attempts/${a.number}/replay`,
          controller.signal,
        );
        const bytes = await response.arrayBuffer();
        if (
          controller.signal.aborted ||
          event !== this.selected ||
          this.replayRequest !== controller
        )
          return;
        this.close();
        this.openReplay(new File([bytes], `M${m.index}-${a.number}.iarp`));
      } catch (e) {
        if (!controller.signal.aborted)
          $("tournament-status").textContent = `回放读取失败：${String(e)}`;
      } finally {
        clearTimeout(timeout);
      }
    };
    return button;
  }
}

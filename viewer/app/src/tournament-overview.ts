import type { Entrant, Event, Matchup } from "./tournaments";

function node<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text = "",
  className = "",
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  el.textContent = text;
  el.className = className;
  return el;
}
const name = (e: Entrant) => `${e.name} · ${e.version || "版本未知"}`;
interface Segment {
  label: string;
  count: number;
  color: string;
}
function segments(pair: Matchup, row: number): Segment[] {
  const first = row === pair.a;
  return [
    { label: "胜", count: first ? pair.a_wins : pair.b_wins, color: "win" },
    {
      label: "技术胜",
      count: first ? pair.a_technical_wins : pair.b_technical_wins,
      color: "win",
    },
    { label: "平", count: pair.draws, color: "draw" },
    { label: "负", count: first ? pair.b_wins : pair.a_wins, color: "loss" },
    {
      label: "技术负",
      count: first ? pair.b_technical_wins : pair.a_technical_wins,
      color: "loss",
    },
    { label: "未决", count: pair.pending, color: "pending" },
    { label: "异常", count: pair.faults, color: "fault" },
    { label: "无评分", count: pair.unscored, color: "unscored" },
  ];
}

/** A complete snapshot projection, independent of the paginated match list. */
export class TournamentOverview {
  private signature = "";
  constructor(
    private host: HTMLElement,
    private selectPair: (row: number, column: number) => void,
  ) {
    this.clear();
  }
  clear() {
    this.signature = "";
    this.host.replaceChildren(
      node("p", "选择赛事后查看排名与对决情况。", "tournament-note"),
    );
  }
  render(data: Event) {
    const signature = JSON.stringify([
      data.entrants,
      data.standings,
      data.matchups,
      data.counts,
      data.total,
      data.completed,
    ]);
    if (signature === this.signature) return;
    this.signature = signature;
    const focus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement.dataset.pair
        : undefined;
    const scroll = this.host.querySelector(".matchup-scroll")?.scrollLeft || 0;
    const entrants = new Map(data.entrants.map((e) => [e.id, e]));
    const ordered = data.standings
      .map((s) => entrants.get(s.entrant))
      .filter((e): e is Entrant => !!e);
    const ranked = new Set(ordered.map((e) => e.id));
    ordered.push(...data.entrants.filter((e) => !ranked.has(e.id)));
    const stats = node("div", "", "event-metrics");
    const faults = ["corrupt", "interrupted", "infrastructure_fault"].reduce(
      (sum, state) => sum + (data.counts[state] || 0),
      0,
    );
    for (const [label, value, hint] of [
      ["参赛选手", data.entrants.length, "按注册选手区分版本"],
      [
        "已完成 / 总场次",
        `${data.completed || 0} / ${data.total || 0}`,
        "覆盖全部地图与阵营",
      ],
      [
        "未决比赛",
        Math.max(0, (data.total || 0) - (data.completed || 0) - faults),
        "待执行、运行记录及发布中",
      ],
      ["异常比赛", faults, "故障、中断或归档损坏"],
    ]) {
      const card = node("div", "", "event-metric");
      card.append(
        node("span", String(label)),
        node("strong", String(value)),
        node("small", String(hint)),
      );
      stats.append(card);
    }
    const ranking = node("section");
    ranking.id = "event-standings";
    ranking.append(
      node("h2", "参赛选手 · 暂定排名"),
      node(
        "p",
        "全赛事已验证结果，按冻结赛程顺序计算；技术结果单列，不计入普通胜平负及 Elo。",
        "tournament-note",
      ),
    );
    const table = node("table");
    const head = node("thead"),
      tr = node("tr");
    for (const label of [
      "名次",
      "选手 / 版本",
      "Elo",
      "胜 / 平 / 负",
      "技术胜 / 负",
      "无评分",
    ]) {
      const th = node("th", label);
      th.scope = "col";
      tr.append(th);
    }
    head.append(tr);
    table.append(head);
    const body = node("tbody");
    for (const s of data.standings) {
      const row = node("tr");
      row.append(node("td", String(s.rank), "rank-number"));
      const participant = node("th");
      participant.scope = "row";
      const e = entrants.get(s.entrant);
      participant.append(
        node("span", e?.name || "选手未知", "ranking-name"),
        node(
          "small",
          `#${s.entrant} · ${e?.version || "版本未知"}`,
          "ranking-version",
        ),
      );
      row.append(participant);
      for (const value of [
        s.rating,
        `${s.wins} / ${s.draws} / ${s.losses}`,
        `${s.forfeit_wins} / ${s.forfeit_losses}`,
        s.unscored,
      ])
        row.append(node("td", String(value)));
      body.append(row);
    }
    table.append(body);
    const rankingScroll = node("div", "", "standings-scroll");
    rankingScroll.tabIndex = 0;
    rankingScroll.setAttribute("role", "region");
    rankingScroll.setAttribute("aria-label", "参赛选手排名，可横向滚动");
    rankingScroll.append(table);
    ranking.append(rankingScroll);
    if (!data.standings.length)
      ranking.append(node("p", "暂无排名。", "tournament-note"));
    const matrix = node("section");
    matrix.id = "event-matchups";
    matrix.append(
      node("h2", "N × N 对决表"),
      node(
        "p",
        "从行选手视角读取胜负；每格合并全部地图和双方阵营。色带按场次占比分段，点击进入对应比赛。",
        "tournament-note",
      ),
    );
    const legend = node("div", "", "matchup-legend");
    for (const [label, color] of [
      ["胜 / 技术胜", "win"],
      ["负 / 技术负", "loss"],
      ["平", "draw"],
      ["未决", "pending"],
      ["异常", "fault"],
      ["无评分", "unscored"],
    ]) {
      const item = node("span", label);
      item.style.setProperty("--status-color", `var(--match-${color})`);
      legend.append(item);
    }
    matrix.append(legend);
    if (!data.matchups)
      matrix.append(
        node(
          "p",
          "后台尚未提供全赛事对决汇总，请更新并重启赛事后台。",
          "tournament-note",
        ),
      );
    else if (!ordered.length)
      matrix.append(node("p", "暂无参赛选手。", "tournament-note"));
    else {
      const pairs = new Map(data.matchups.map((p) => [`${p.a}:${p.b}`, p]));
      const scroll = node("div", "", "matchup-scroll");
      scroll.tabIndex = 0;
      scroll.setAttribute("role", "region");
      scroll.setAttribute("aria-label", "选手对决表，可横向滚动");
      const grid = node("table", "", "matchup-table");
      const caption = node("caption", "行选手 × 列对手 · 胜 / 平 / 负");
      grid.append(caption);
      const thead = node("thead"),
        header = node("tr");
      header.append(node("th", "选手 / 对手", "matchup-corner"));
      for (const entrant of ordered)
        header.append(this.heading(entrant, "col"));
      thead.append(header);
      grid.append(thead);
      const tbody = node("tbody");
      for (const a of ordered) {
        const row = node("tr");
        row.append(this.heading(a, "row"));
        for (const b of ordered) {
          const cell = node("td");
          if (a.id === b.id) {
            cell.className = "matchup-self";
            cell.textContent = "—";
            cell.setAttribute("aria-label", "同一选手");
          } else {
            const pair = pairs.get(
              `${Math.min(a.id, b.id)}:${Math.max(a.id, b.id)}`,
            );
            if (!pair || !pair.total) {
              cell.className = "matchup-empty";
              cell.textContent = "无赛程";
            } else cell.append(this.cell(a, b, pair));
          }
          row.append(cell);
        }
        tbody.append(row);
      }
      grid.append(tbody);
      scroll.append(grid);
      matrix.append(scroll);
    }
    const tables = node("div", "", "overview-tables");
    tables.append(ranking, matrix);
    this.host.replaceChildren(stats, tables);
    const scroller = this.host.querySelector(".matchup-scroll");
    if (scroller) scroller.scrollLeft = scroll;
    if (focus)
      for (const button of this.host.querySelectorAll<HTMLButtonElement>(
        "[data-pair]",
      ))
        if (button.dataset.pair === focus)
          button.focus({ preventScroll: true });
  }
  private heading(e: Entrant, scope: "row" | "col") {
    const th = node("th", "", "matchup-heading");
    th.scope = scope;
    th.title = `#${e.id} ${name(e)}`;
    th.append(
      node("small", `#${e.id}`),
      node("span", e.name),
      node("small", e.version || "版本未知"),
    );
    return th;
  }
  private cell(a: Entrant, b: Entrant, pair: Matchup) {
    const parts = segments(pair, a.id);
    const wins = parts[0].count,
      losses = parts[3].count;
    const description = parts
      .filter((p) => p.count)
      .map((p) => `${p.count} ${p.label}`)
      .join(" · ");
    const button = node("button", "", "matchup-cell");
    button.type = "button";
    button.dataset.pair = `${a.id}:${b.id}`;
    button.title = `行：#${a.id} ${name(a)}\n列：#${b.id} ${name(b)}\n${pair.total} 场 · ${description}\n点击查看双方比赛与回放`;
    button.setAttribute(
      "aria-label",
      `#${a.id} ${name(a)} 对 #${b.id} ${name(b)}：${description}，共 ${pair.total} 场，查看比赛`,
    );
    let offset = 0;
    const gradient = parts
      .filter((p) => p.count)
      .map((p) => {
        const start = offset;
        offset += (p.count / pair.total) * 100;
        return `var(--match-${p.color}) ${start}% ${offset}%`;
      });
    button.style.setProperty(
      "--match-gradient",
      `linear-gradient(90deg, ${gradient.join(", ")})`,
    );
    button.append(
      node("strong", `${wins} / ${pair.draws} / ${losses}`),
      node("small", `共 ${pair.total} 场`),
    );
    const extra = parts
      .filter((p) => p.count && !["胜", "平", "负"].includes(p.label))
      .map((p) => `${p.count}${p.label}`)
      .join(" · ");
    if (extra) button.append(node("small", extra, "matchup-extra"));
    button.onclick = () => this.selectPair(a.id, b.id);
    return button;
  }
}

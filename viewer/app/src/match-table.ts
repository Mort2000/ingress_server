import type { Attempt, Event, Match, PlayerStatistics } from "./tournaments";
import { score } from "./model";

function node<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  text = "",
  className = "",
) {
  const value = document.createElement(tag);
  value.textContent = text;
  value.className = className;
  return value;
}

export function winner(m: Match, a?: Attempt): number | undefined {
  if (a?.outcome === "technical_win") return a.winner;
  if (a?.outcome === "enl_win") return m.enl;
  if (a?.outcome === "res_win") return m.res;
  return undefined;
}

function ratio(value: bigint, total: bigint) {
  return total > 0n ? Number((value * 1000n + total / 2n) / total) / 10 : 0;
}
function meter(label: string, percent: number, kind = "") {
  const bar = node("div", label, `stat-meter ${kind}`);
  bar.style.setProperty("--fill", `${Math.min(100, Math.max(0, percent))}%`);
  return bar;
}
function badge(label: string, kind: string) {
  return node("span", label, `result-badge ${kind}`);
}
function statistic(a: Attempt | undefined, faction: "enl" | "res") {
  return a?.statistics?.[faction];
}

interface TableOptions {
  entrant?: number;
  state: (state: string) => string;
  participant: (id: number) => string;
  result: (m: Match, a: Attempt) => string;
  replay: (m: Match, a: Attempt, title: string) => HTMLButtonElement;
  expanded: Set<number>;
}

export function matchTable(data: Event, options: TableOptions): HTMLElement {
  const scroll = node("div", "", "match-table-scroll");
  scroll.tabIndex = 0;
  scroll.setAttribute("role", "region");
  scroll.setAttribute("aria-label", "比赛技术统计，可横向滚动");
  const table = node("table", "", "match-table");
  const caption = node(
    "caption",
    "每场两行，统计与选手逐行对应 · 优先显示所选选手，未选择时胜者在前",
  );
  table.append(caption);
  const head = node("thead");
  const headings = node("tr");
  for (const label of [
    "比赛 / 地图",
    "选手",
    "胜负",
    "总得分",
    "Portal",
    "Link",
    "Field",
    "空过",
    "关注提示",
    "回放 / 尝试",
  ]) {
    const th = node("th", label);
    th.scope = "col";
    if (["Portal", "Link", "Field"].includes(label))
      th.append(
        node("small", label === "Portal" ? "占领 / 摧毁" : "建立 / 摧毁"),
      );
    if (label === "空过") th.append(node("small", "全队轮次 · Agent 比例"));
    headings.append(th);
  }
  head.append(headings);
  table.append(head);
  for (const m of data.matches) {
    const a = m.attempts.at(-1);
    const won = winner(m, a);
    const first =
      options.entrant !== undefined && [m.enl, m.res].includes(options.entrant)
        ? options.entrant
        : (won ?? Math.min(m.enl, m.res));
    const sides =
      first === m.enl ? (["enl", "res"] as const) : (["res", "enl"] as const);
    const body = node("tbody", "", "match-row");
    body.dataset.match = String(m.index);
    const rounds = a?.statistics?.rounds ?? a?.rounds;
    for (const [i, faction] of sides.entries()) {
      const id = m[faction];
      const stats = statistic(a, faction);
      const other = statistic(a, faction === "enl" ? "res" : "enl");
      const rawScore =
        stats?.score2 ?? (faction === "enl" ? a?.score_enl : a?.score_res);
      const otherScore =
        other?.score2 ?? (faction === "enl" ? a?.score_res : a?.score_enl);
      const tr = node("tr");
      if (i === 0) {
        const match = node("td", "", "match-identity");
        match.rowSpan = 2;
        match.append(
          node("strong", `M${m.index}`),
          node("span", `地图 ${m.map}`),
          node("small", options.state(m.state)),
        );
        match.title = `地图哈希 ${m.map_hash}`;
        match.append(
          node("small", rounds === undefined ? "轮次 —" : `${rounds} 轮`),
        );
        if (a?.elapsed_ms !== undefined)
          match.append(node("small", `${(a.elapsed_ms / 1000).toFixed(2)} s`));
        if (a?.finished_unix_ms)
          match.append(
            node("small", new Date(a.finished_unix_ms).toLocaleString()),
          );
        tr.append(match);
      }
      const person = node("th", "", "match-player");
      person.scope = "row";
      const entrant = data.entrants.find((e) => e.id === id);
      person.append(
        node("strong", entrant?.name ?? "选手未知"),
        node(
          "small",
          `#${id} · ${entrant?.version || "版本未知"} · ${faction.toUpperCase()}`,
        ),
      );
      person.title = options.participant(id);
      tr.append(person);
      const result = node("td");
      const kind =
        won !== undefined
          ? won === id
            ? "win"
            : "loss"
          : a?.outcome === "draw"
            ? "draw"
            : "pending";
      const label =
        won !== undefined
          ? `${a?.outcome === "technical_win" ? "技术" : ""}${won === id ? "胜" : "负"}`
          : a?.outcome === "draw"
            ? "平"
            : a?.outcome === "unscored"
              ? "无评分"
              : "—";
      result.append(badge(label, kind));
      if (a) result.title = options.result(m, a);
      tr.append(result);
      const scoreCell = node("td", "", "match-score");
      if (rawScore !== undefined && otherScore !== undefined) {
        const total = BigInt(rawScore) + BigInt(otherScore);
        const share = ratio(BigInt(rawScore), total);
        scoreCell.append(
          meter(score(rawScore), share, kind),
          node("small", total > 0n ? `占比 ${share.toFixed(1)}%` : "双方零分"),
        );
        scoreCell.title = `总得分 = score2 ÷ 2；原始 score2 ${rawScore}`;
      } else scoreCell.textContent = "—";
      tr.append(scoreCell);
      const metrics: [keyof PlayerStatistics, keyof PlayerStatistics][] = [
        ["portals_captured", "portals_destroyed"],
        ["links_created", "links_destroyed"],
        ["fields_created", "fields_destroyed"],
      ];
      for (const [created, destroyed] of metrics) {
        const cell = node("td", "", "match-structures");
        if (stats && other) {
          const a = Number(stats[created]),
            b = Number(stats[destroyed]);
          const max = Math.max(
            a,
            b,
            Number(other[created]),
            Number(other[destroyed]),
            1,
          );
          cell.append(
            meter(`+${a}`, (a / max) * 100, "created"),
            meter(`−${b}`, (b / max) * 100, "destroyed"),
          );
          cell.title =
            "上：占领／建立；下：摧毁对方。数据条按本场同类四个计数的最大值缩放。";
        } else cell.textContent = "—";
        tr.append(cell);
      }
      const idle = node("td", "", "match-idle");
      const alerts = node("td", "", "match-alerts");
      if (stats) {
        const empty = stats.pass_slots + stats.not_submitted_slots;
        const fraction = stats.agent_rounds ? empty / stats.agent_rounds : 0;
        idle.append(
          node("span", `${stats.idle_rounds} / ${rounds} 轮`),
          meter(
            stats.agent_rounds ? `${(fraction * 100).toFixed(1)}%` : "—",
            fraction * 100,
            "idle",
          ),
        );
        idle.title = `全队空过：全部活跃 Agent 均 PASS 或未提交；Agent 空过 ${empty} / ${stats.agent_rounds}，其中主动 PASS ${stats.pass_slots}、未提交 ${stats.not_submitted_slots}；拒绝 ${stats.rejected_slots}（不计空过）。`;
        if (stats.agent_rounds > 0 && fraction >= 0.5)
          alerts.append(badge("高空过", "attention"));
        if (stats.not_submitted_slots)
          alerts.append(
            badge(`未提交 ${stats.not_submitted_slots}`, "attention"),
          );
        if (stats.rejected_slots)
          alerts.append(badge(`拒绝 ${stats.rejected_slots}`, "attention"));
      } else {
        idle.textContent = "—";
        alerts.append(node("small", "统计不可用"));
      }
      if (
        rawScore !== undefined &&
        BigInt(rawScore) === 0n &&
        (rounds ?? 0) > 0
      )
        alerts.append(badge("零分", "attention"));
      if (
        rawScore !== undefined &&
        otherScore !== undefined &&
        a?.outcome !== "technical_win"
      ) {
        const x = BigInt(rawScore),
          y = BigInt(otherScore),
          sum = x + y;
        const gap = x > y ? x - y : y - x;
        if (sum > 0n && gap * 100n <= sum * 5n)
          alerts.append(badge("胶着 ≤5%", "draw"));
        if (sum > 0n && gap * 100n >= sum * 90n)
          alerts.append(badge("悬殊 ≥90%", "attention"));
      }
      if (a?.outcome === "technical_win")
        alerts.append(badge("技术判定", "attention"));
      if (a?.factions_verified === false)
        alerts.append(badge("绑定未核验", "attention"));
      if (!alerts.childNodes.length) alerts.append(node("small", "—"));
      tr.append(idle, alerts);
      if (i === 0) {
        const actions = node("td", "", "match-actions");
        actions.rowSpan = 2;
        if (a?.replay && !data.error)
          actions.append(options.replay(m, a, "打开回放"));
        else
          actions.append(
            node(
              "small",
              m.state === "completed" ? "无可用回放" : "回放未就绪",
            ),
          );
        if (m.attempts.length) {
          const history = node("details");
          history.open = options.expanded.has(m.index);
          const summary = node("summary", `尝试记录（${m.attempts.length}）`);
          summary.dataset.matchFocus = `history-${m.index}`;
          history.append(summary);
          for (const attempt of m.attempts) {
            const info = node("div", "", "attempt-row");
            info.append(
              node(
                "p",
                `#${attempt.number} · ${options.state(attempt.state)} · ${attempt.attempt_reason || "原因未记录"}`,
              ),
              node("p", options.result(m, attempt)),
            );
            if (attempt.replay && !data.error)
              info.append(
                options.replay(m, attempt, `打开 #${attempt.number} 回放`),
              );
            history.append(info);
          }
          history.ontoggle = () => {
            if (!history.isConnected) return;
            if (history.open) options.expanded.add(m.index);
            else options.expanded.delete(m.index);
          };
          actions.append(history);
        }
        tr.append(actions);
      }
      body.append(tr);
    }
    table.append(body);
  }
  scroll.append(table);
  return scroll;
}

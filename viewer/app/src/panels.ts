import {
  colors,
  factions,
  ReplayModel,
  score,
  type ReplayEvent,
} from "./model";
export const escape = (value: unknown) =>
  String(value).replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ]!,
  );
const names = [
  "",
  "CAPTURE",
  "ATTACK",
  "NEUTRALIZE",
  "LINK CREATED",
  "LINK DESTROYED",
  "FIELD CREATED",
  "FIELD DESTROYED",
  "REINFORCEMENT",
];
export function eventText(e: ReplayEvent) {
  return `${names[e.k]} ${e.p != null ? `P${e.p}` : (e.l ?? e.t ?? []).map((p) => `P${p}`).join(" / ")}${e.d != null ? ` · −${e.d} HP → ${e.h}` : ""}${e.n != null ? ` · +${e.n} / faction` : ""}`;
}
export function roster(m: ReplayModel) {
  return (
    '<div class="mlabel">Agent roster <b>在线名册</b></div>' +
    m.visibleFactions
      .map(
        (f) =>
          `<section class="facgroup ${f}"><div class="fg-head"><i class="fg-dot"></i><span class="fg-name">${f.toUpperCase()}</span><span class="fg-bot">${m.agents.filter((a) => a.faction === f).length} AGENTS</span></div>${m.agents
            .filter((a) => a.faction === f)
            .map((a) => {
              const count = Object.values(m.keys(a)).reduce((s, n) => s + n, 0);
              return `<button class="chip ${f} ${m.selected === a.row[0] ? "sel" : ""}" data-select="${a.row[0]}" aria-pressed="${m.selected === a.row[0]}"><span class="cid">A${a.row[0]}<small>SUP ${a.row[3]}</small></span><span class="cbar"><i style="width:${(100 * a.row[3]) / m.data.header.config.max_supply}%"></i></span><span class="ckey ${count ? "" : "none"}">⌑ ${count}</span></button>`;
            })
            .join("")}</section>`,
      )
      .join("") +
    (m.view === "public"
      ? '<p class="empty-note">公开视角仅显示 Portal、Link、Field 和公共事件。</p>'
      : '<p class="empty-note">选择 Agent 查看行动与库存<br>悬停 Portal 查找 Key 持有人</p>')
  );
}
export function dossier(m: ReplayModel) {
  const a = m.agent;
  if (!a)
    return (
      '<div class="mlabel">Agent dossier</div><p class="empty-note">选择可见的 Agent 查看档案。<br>当前回合尚未出现的增援没有私有状态。</p>' +
      events(m)
    );
  const [id, x, y, supply] = a.row,
    cfg = m.data.header.config,
    keys = Object.entries(m.keys(a)).filter(([, n]) => n > 0);
  const previous =
    m.round > 1
      ? m.data.agents[a.faction][m.round - 2].find((row) => row[0] === id)?.[3]
      : m.data.header.spawns[a.faction].some((row) => row[0] === id)
        ? cfg.initial_supply
        : undefined;
  const delta = previous == null ? null : supply - previous;
  const stream = m.stream(a),
    status = stream?.slot ?? "initial";
  const supplies = [
    m.data.header.spawns[a.faction].some((row) => row[0] === id)
      ? cfg.initial_supply
      : null,
    ...m.data.agents[a.faction]
      .slice(0, m.round)
      .map((rows) => rows.find((row) => row[0] === id)?.[3] ?? null),
  ];
  const points = supplies
    .flatMap((n, i) =>
      n == null
        ? []
        : [
            `${(i / Math.max(1, m.round)) * 300},${42 - (n / cfg.max_supply) * 36}`,
          ],
    )
    .join(" ");
  return `<div class="mlabel">Agent dossier <b>个体档案</b></div>
    <div id="d-head"><div id="d-hex" class="${a.faction}">${id}</div><div id="d-title"><div id="d-name">AGENT ${id}</div><div id="d-sub">${a.faction.toUpperCase()} · ROUND <b>${m.round}</b></div></div><span id="d-status" class="${status === "rejected" ? "bad" : ""}">${status.toUpperCase()}</span></div>
    <section class="dsec"><div class="mlabel">Position <b>当前位置</b></div><div id="d-pos"><div class="cell"><div class="k">X / m</div><div class="v">${(x / 100).toFixed(2)}</div></div><div class="cell"><div class="k">Y / m</div><div class="v">${(y / 100).toFixed(2)}</div></div></div><div id="pos-moved">MOVED <b>${(m.movedCm / 100).toFixed(2)} m</b> / ${cfg.move_budget_cm / 100} m 本轮预算</div><button class="btn" id="focus-agent">定位并放大 Agent</button></section>
    <section class="dsec"><div class="mlabel">Supply <b>携带补给</b></div><div id="gauge-wrap"><span id="su-num">${supply}</span><span id="su-max">/ ${cfg.max_supply}</span><span id="su-delta" class="${delta != null && delta >= 0 ? "up" : "dn"}">${delta == null ? "NEW" : `${delta >= 0 ? "+" : ""}${delta}`} THIS ROUND</span></div><div class="gauge" style="--gc:${colors[a.faction]}">${Array.from({ length: 24 }, (_, i) => `<i class="${i < (supply / cfg.max_supply) * 24 ? "on" : ""}"></i>`).join("")}</div><svg id="spark" viewBox="0 0 300 46" preserveAspectRatio="none" aria-label="Supply 历史"><polyline points="${points}" stroke="${colors[a.faction]}" stroke-width="1.5" fill="none"/></svg></section>
    <section class="dsec"><div class="mlabel">Key inventory <b>${keys.reduce((n, [, v]) => n + v, 0)} KEYS</b></div><div id="keylist">${keys.map(([portal, count]) => `<button class="key" data-key="${portal}" aria-label="Key P${portal}, ${count} 把">P${portal} <b>×${count}</b></button>`).join("") || '<span class="keynone">NO KEYS · 暂无持有</span>'}</div></section>
    <section class="dsec"><div class="mlabel">Command stream <b>当轮命令</b></div><div id="slotline" class="${status === "rejected" ? "bad" : "ok"}">${escape(status.toUpperCase())}${stream?.order != null ? ` · ORDER ${stream.order} · BUCKET ${stream.bucket} · SEQ ${stream.seq}` : ""}${stream?.reason ? ` · ${escape(stream.reason)}` : ""}</div><div id="cmdlist">${stream?.cmds.map((cmd, i) => `<article class="cmd ${cmd.status === "drop" ? "drop" : ""}"><div class="cmd-head"><span class="cidx">${String(i + 1).padStart(2, "0")}</span><span class="cname">${escape(cmd.name)}</span><span class="ctag ${cmd.major ? "major" : ""}">${cmd.major ? "MAJOR" : "AUX"}</span></div><p class="cmd-params">${escape(cmd.params)}</p><p class="cmd-out">${cmd.status.toUpperCase()} · ${escape(cmd.detail)}</p></article>`).join("") || '<p class="passnote">本轮没有执行命令</p>'}</div></section>${events(m)}`;
}
function events(m: ReplayModel) {
  return `<section class="dsec"><div class="mlabel">Round events <b>${m.snapshot.events.length} EVENTS</b></div><div id="evlist">${m.snapshot.events.map((e) => `<div class="ev"><i class="dot" style="--c:${e.f == null ? "#b98cff" : colors[factions[e.f]]}"></i><span class="et">${escape(eventText(e))}</span><span class="ea">${e.a == null ? "ALL" : `A${e.a}`}</span></div>`).join("") || '<p class="evnone">本轮无公共事件</p>'}</div></section>`;
}
export function timeline(m: ReplayModel) {
  const total = m.data.rounds.length,
    max = Math.max(1, ...m.data.rounds.flatMap((r) => r.score2.map(Number)));
  return `<svg id="rail" viewBox="0 0 1000 65" preserveAspectRatio="none" aria-label="累计得分曲线，点击跳转"><path d="M0,60H1000" stroke="#214155"/>${factions.map((f, i) => `<polyline points="0,60 ${m.data.rounds.map((r, n) => `${((n + 1) / Math.max(1, total)) * 1000},${60 - (Number(r.score2[i]) / max) * 52}`).join(" ")}" fill="none" stroke="${colors[f]}" stroke-width="1.5"/>`).join("")}${m.data.header.checkpoints.map((r) => `<path d="M${(r / Math.max(1, total)) * 1000},56v9" stroke="#b98cff"/>`).join("")}<path id="rail-cursor" d="M${(m.round / Math.max(1, total)) * 1000},0v65" stroke="#00e5ff"/></svg>`;
}
export function updateScores(m: ReplayModel) {
  factions.forEach((f, i) => {
    document.getElementById(`score-${f}`)!.textContent = score(
      m.snapshot.score2[i],
    );
  });
}

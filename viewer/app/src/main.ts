import "./style.css";
import { ReplayModel, type Replay, type View } from "./model";
import { TacticalMap } from "./map";
import { dossier, roster, timeline, updateScores } from "./panels";
import { TournamentPanel } from "./tournaments";
const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;
$("app").innerHTML = `
<header id="topbar"><div id="brand"><div id="logo">⬡</div><div><div class="t1">INGRESS ARENA <em>//</em> TACTICAL REPLAY</div><div class="t2">战术回放终端 · REPLAY CONSOLE</div></div></div><div id="matchmeta"><div class="row">SEED <b id="m-seed">—</b></div><div class="row">MAP <b id="m-map">—</b> · PROTO <b id="m-proto">—</b></div></div><div id="versus"><div class="fscore enl"><span class="fname">ENLIGHTENED</span><span class="fval" id="score-enl">0</span></div><div id="vsmid"><span id="vsbadge">VS</span><span id="resultpill">STANDBY</span></div><div class="fscore res"><span class="fname">RESISTANCE</span><span class="fval" id="score-res">0</span></div></div><div id="topright"><span id="verified">LOCAL REPLAY</span><button id="open-file" class="btn">打开 .iarp</button><input type="file" id="file-input" accept=".iarp" hidden aria-label="打开 IARP 回放文件"></div></header>
<div id="load-status" role="status" aria-live="polite" hidden></div>
<main id="stage"><aside id="roster" class="hud"><div class="mlabel">Agent roster</div><p class="empty-note">载入回放后显示阵营名册</p></aside><section id="mapwrap" class="hud"><div id="roundghost">ROUND 000</div><div id="map-tools"><div class="seg" id="views" aria-label="视角"><button data-view="god" class="on">GOD</button><button data-view="enl">ENL</button><button data-view="res">RES</button><button data-view="public">PUBLIC</button></div><div class="zoom-tools"><button class="btn" id="zoom-in" aria-label="放大地图">+</button><button class="btn" id="zoom-out" aria-label="缩小地图">−</button><button class="btn" id="zoom-box" aria-pressed="false">框选放大</button><button class="btn" id="zoom-reset">全图</button><span id="zoom-level">1.0×</span></div></div><div id="welcome"><div class="welcome-icon">⬡</div><p class="eyebrow">INGRESS ARENA / REPLAY ARCHIVE</p><h1>重返战场，洞察每一步。</h1><p>拖入 .iarp 回放文件，或点击右上角打开。<br>文件在浏览器本地解析与校验，无需上传。</p><button class="btn" id="choose-file">选择回放文件</button><small>滚轮缩放 · 拖动平移 · Shift + 拖动局部放大</small></div><div id="portal-info" aria-live="polite" hidden></div><div id="legend"><span><i style="background:#41ff7e"></i>ENL</span><span><i style="background:#3fb6ff"></i>RES</span><span>虚淡线：历史轨迹</span><span>亮白线：当轮移动</span><span>虚线圈：操作范围</span></div></section><aside id="dossier" class="hud"><div class="mlabel">Agent dossier</div><p class="empty-note">Agent 行动轨迹、Supply 与 Key 库存将在此显示。</p></aside></main>
<footer id="chrono"><div id="transport"><button class="btn" id="btn-first" aria-label="第一轮">|◀</button><button class="btn" id="btn-prev" aria-label="上一轮">◀</button><button class="btn" id="btn-play" aria-label="播放">▶</button><button class="btn" id="btn-next" aria-label="下一轮">▶</button><button class="btn" id="btn-last" aria-label="最后一轮">▶|</button><button class="btn" id="speedbtn" aria-label="播放速度">1×</button></div><div id="railwrap"><div id="chart"></div><input type="range" id="seek" min="0" max="0" value="0" aria-label="跳转回合"></div><div id="readout"><div id="ro-round">000 <small>/ 000</small></div><div id="ro-sub">尚未载入回放</div><div class="round-jump"><label for="jump">ROUND</label><input id="jump" type="number" min="0" value="0" aria-label="回合编号"></div><div id="ro-keys">SPACE 播放 · ← → 步进 · [ ] Agent · G/E/R 视角</div></div></footer>`;
let model: ReplayModel | undefined,
  map: TacticalMap | undefined,
  worker: Worker | undefined;
let tournaments: TournamentPanel | undefined;
for (const faction of ["enl", "res"]) {
  const area = document.querySelector(`.fscore.${faction}`)!;
  area.querySelector(".fname")!.textContent = faction.toUpperCase();
  const name = document.createElement("span");
  name.id = `identity-${faction}`;
  name.className = "participant-name";
  name.textContent = "选手未知";
  const version = document.createElement("span");
  version.id = `version-${faction}`;
  version.className = "participant-version";
  version.textContent = "版本未知";
  area.insertBefore(name, area.querySelector(".fval"));
  area.insertBefore(version, area.querySelector(".fval"));
}
const identityDetails = document.createElement("details");
identityDetails.id = "identity-details";
identityDetails.innerHTML = "<summary>选手详情</summary><pre></pre>";
$("topbar").after(identityDetails);
function renderIdentity(replay: Replay) {
  const details = [
    "身份来源仅作追溯；回放校验不是构建签名。",
    `执行实例：${replay.header.identity?.execution_id || "未知"}`,
  ];
  for (const [index, faction] of ["enl", "res"].entries()) {
    const p = replay.header.identity?.participants[index];
    const name = p?.display_name || p?.client_name || "选手未知";
    const version = p?.registered_version || p?.bot_version || "版本未知";
    $(`identity-${faction}`).textContent = name;
    $(`identity-${faction}`).title = name;
    $(`version-${faction}`).textContent = version;
    $(`version-${faction}`).title = version;
    details.push(
      `\n${faction.toUpperCase()} · ${name} · ${version}`,
      `注册 ID：${p?.registered_id || "未知"}`,
      `注册名称：${p?.display_name || "未知"}`,
      `注册 Bot 版本：${p?.registered_version || "版本未知"}`,
      `自报名称：${p?.client_name || "未知"}`,
      `自报 Bot 版本：${p?.bot_version || "版本未知"}`,
      `客户端版本：${p?.client_version || "未知"}`,
      `SDK 版本：${p?.sdk_version || "未知"}`,
      `构建哈希：${p?.binary_hash || "未知"}`,
      `阵营绑定：${p?.binding_verified ? "运行器已核验" : "未核验"}`,
    );
  }
  identityDetails.querySelector("pre")!.textContent = details.join("\n");
}
let loading = false,
  generation = 0,
  playing = false,
  timer: ReturnType<typeof setInterval> | undefined,
  speedIndex = 1;
const speeds = [0.5, 1, 2, 4, 8];
function status(message: string, error = false) {
  $("load-status").hidden = !message;
  $("load-status").textContent = message;
  $("load-status").classList.toggle("error", error);
}
function setPlaying(value: boolean) {
  playing = value && !!model && model.data.rounds.length > 0 && !loading;
  clearInterval(timer);
  $("btn-play").textContent = playing ? "Ⅱ" : "▶";
  $("btn-play").setAttribute("aria-label", playing ? "暂停" : "播放");
  if (playing)
    timer = setInterval(() => {
      if (!model || model.round >= model.data.rounds.length) setPlaying(false);
      else seek(model.round + 1);
    }, 650 / speeds[speedIndex]);
}
function seek(round: number) {
  if (!model || !Number.isFinite(round)) return;
  model.seek(round);
  render();
  if (model.round === model.data.rounds.length) setPlaying(false);
}
function render() {
  if (!model) return;
  $("roster").innerHTML = roster(model);
  $("dossier").innerHTML = dossier(model);
  updateScores(model);
  map!.render();
  highlight();
  const r = model.round,
    total = model.data.rounds.length;
  $("roundghost").textContent = `ROUND ${String(r).padStart(3, "0")}`;
  $("ro-round").innerHTML =
    `${String(r).padStart(3, "0")} <small>/ ${total}</small>`;
  $("ro-sub").textContent =
    `EPOCH ${Math.floor(r / model.data.header.config.epoch_length)} · ${model.snapshot.events.length} EVENTS`;
  $<HTMLInputElement>("seek").value = String(r);
  $<HTMLInputElement>("jump").value = String(r);
  $("seek").style.setProperty("--p", `${(r / Math.max(1, total)) * 100}%`);
  $("rail-cursor").setAttribute(
    "d",
    `M${(r / Math.max(1, total)) * 1000},0v65`,
  );
  $("verified").textContent = "✓ VERIFIED";
  $("verified").title =
    model.snapshot.hash ||
    model.data.header.outcome.final_hash ||
    model.data.header.outcome.last_hash ||
    "";
  $("views")
    .querySelectorAll<HTMLButtonElement>("button")
    .forEach((button) => {
      const on = button.dataset.view === model!.view;
      button.classList.toggle("on", on);
      button.setAttribute("aria-pressed", String(on));
    });
}
function highlight() {
  if (!model) return;
  map!.highlight();
  const p = model.hoverPortal,
    holders = p == null ? [] : model.holders(p),
    ids = new Set(holders.map((a) => a.row[0]));
  $("roster")
    .querySelectorAll<HTMLElement>("[data-select]")
    .forEach((node) =>
      node.classList.toggle(
        "key-highlight",
        ids.has(Number(node.dataset.select)),
      ),
    );
  $("dossier")
    .querySelectorAll<HTMLElement>("[data-key]")
    .forEach((node) =>
      node.classList.toggle("key-highlight", Number(node.dataset.key) === p),
    );
  $("portal-info").hidden = p == null;
  if (p != null) {
    const portal = model.snapshot.portals.find((row) => row[0] === p)!;
    $("portal-info").textContent =
      `P${p} · ${["NEUTRAL", "ENL", "RES"][portal[1]]} · HP ${portal[2]} · Key 持有人${model.view === "god" ? "" : "（当前视角）"}：${holders.map((a) => `A${a.row[0]} ×${model!.keys(a)[p]}`).join(" / ") || "无"}`;
  }
}
async function load(file: File) {
  tournaments?.close();
  const request = ++generation;
  worker?.terminate();
  worker = undefined;
  setPlaying(false);
  loading = true;
  status(`正在读取并校验 ${file.name} …`);
  try {
    const bytes = await file.arrayBuffer();
    if (request !== generation) return;
    worker = new Worker(new URL("./replay.worker.ts", import.meta.url), {
      type: "module",
    });
    const current = worker;
    const fail = (message: string) => {
      if (request !== generation) return;
      current.terminate();
      worker = undefined;
      loading = false;
      status(`无法打开回放：${message}`, true);
    };
    current.onerror = (e) => fail(e.message || "回放 Worker 启动失败");
    current.onmessage = (
      e: MessageEvent<{ replay?: Replay; error?: string }>,
    ) => {
      if (request !== generation) return;
      if (e.data.error || !e.data.replay) {
        fail(e.data.error || "无回放数据");
        return;
      }
      current.terminate();
      worker = undefined;
      loading = false;
      model = new ReplayModel(e.data.replay);
      map?.destroy();
      map = new TacticalMap($("mapwrap"), model, render, highlight);
      $("welcome").hidden = true;
      $("zoom-box").setAttribute("aria-pressed", "false");
      $("chart").innerHTML = timeline(model);
      $("m-seed").textContent = model.data.header.game_seed;
      $("m-map").textContent =
        `${model.data.header.map.portals.length} PORTALS`;
      $("m-proto").textContent = String(model.data.header.versions.protocol);
      renderIdentity(model.data);
      const outcome = model.data.header.outcome;
      $("resultpill").textContent =
        outcome.kind === "normal"
          ? (outcome.result ?? "END").replaceAll("_", " ").toUpperCase()
          : `ABNORMAL ${outcome.outcome_tag}`;
      $("resultpill").classList.toggle("live", outcome.kind !== "normal");
      $<HTMLInputElement>("seek").max = $<HTMLInputElement>("jump").max =
        String(model.data.rounds.length);
      status(
        `${file.name} · ${model.data.rounds.length} 轮已校验 · 本地分析视角`,
      );
      render();
    };
    current.postMessage(bytes, [bytes]);
  } catch (error) {
    if (request === generation) {
      loading = false;
      status(`无法读取文件：${String(error)}`, true);
    }
  }
}
$("open-file").onclick = $("choose-file").onclick = () =>
  $<HTMLInputElement>("file-input").click();
$("file-input").onchange = () => {
  const input = $<HTMLInputElement>("file-input");
  if (input.files?.[0]) void load(input.files[0]);
  input.value = "";
};
document.addEventListener("dragover", (e) => {
  e.preventDefault();
  document.body.classList.add("drag-over");
});
document.addEventListener("dragleave", (e) => {
  if (!e.relatedTarget) document.body.classList.remove("drag-over");
});
document.addEventListener("drop", (e) => {
  e.preventDefault();
  document.body.classList.remove("drag-over");
  if (e.dataTransfer?.files[0]) void load(e.dataTransfer.files[0]);
});
$("btn-play").onclick = () => {
  if (!playing && model?.round === model?.data.rounds.length) seek(0);
  setPlaying(!playing);
};
$("btn-first").onclick = () => {
  setPlaying(false);
  seek(0);
};
$("btn-last").onclick = () => {
  setPlaying(false);
  seek(model?.data.rounds.length ?? 0);
};
$("btn-prev").onclick = () => {
  setPlaying(false);
  seek((model?.round ?? 0) - 1);
};
$("btn-next").onclick = () => {
  setPlaying(false);
  seek((model?.round ?? 0) + 1);
};
function speed() {
  speedIndex = (speedIndex + 1) % speeds.length;
  $("speedbtn").textContent = `${speeds[speedIndex]}×`;
  if (playing) setPlaying(true);
}
$("speedbtn").onclick = speed;
$("seek").oninput = () => {
  setPlaying(false);
  seek(+$<HTMLInputElement>("seek").value);
};
$("jump").onchange = () => {
  setPlaying(false);
  seek(+$<HTMLInputElement>("jump").value);
};
$("chart").onclick = (e) => {
  if (!model) return;
  const b = $("chart").getBoundingClientRect();
  setPlaying(false);
  seek(((e.clientX - b.left) / b.width) * model.data.rounds.length);
};
$("views").onclick = (e) => {
  const view = (e.target as HTMLElement).dataset.view as View | undefined;
  if (model && view) {
    model.setView(view);
    render();
  }
};
$("roster").onclick = (e) => {
  const id = (e.target as HTMLElement).closest<HTMLElement>("[data-select]")
    ?.dataset.select;
  if (id != null && model) {
    model.selected = +id;
    render();
  }
};
$("dossier").onclick = (e) => {
  if ((e.target as HTMLElement).id === "focus-agent") map?.focusSelected();
};
for (const [event, active] of [
  ["pointerover", true],
  ["pointerout", false],
  ["focusin", true],
  ["focusout", false],
] as const)
  $("dossier").addEventListener(event, (e) => {
    const key = (e.target as HTMLElement).closest<HTMLElement>("[data-key]");
    if (!key || !model) return;
    model.hoverPortal = active ? Number(key.dataset.key) : null;
    highlight();
  });
$("zoom-in").onclick = () => map?.zoom(1.5);
$("zoom-out").onclick = () => map?.zoom(1 / 1.5);
$("zoom-reset").onclick = () => map?.reset();
$("zoom-box").onclick = () => {
  const enabled = $("zoom-box").getAttribute("aria-pressed") !== "true";
  $("zoom-box").setAttribute("aria-pressed", String(enabled));
  map?.setBoxMode(enabled);
};
document.addEventListener("keydown", (e) => {
  if (
    tournaments?.visible ||
    (e.target as HTMLElement).closest(
      'input,select,textarea,button,summary,[contenteditable="true"]',
    ) ||
    e.altKey ||
    e.ctrlKey ||
    e.metaKey ||
    !model
  )
    return;
  if (e.code === "Space") {
    e.preventDefault();
    $("btn-play").click();
  } else if (e.key === "ArrowLeft") {
    e.preventDefault();
    $("btn-prev").click();
  } else if (e.key === "ArrowRight") {
    e.preventDefault();
    $("btn-next").click();
  } else if (e.key === "Home") {
    e.preventDefault();
    $("btn-first").click();
  } else if (e.key === "End") {
    e.preventDefault();
    $("btn-last").click();
  } else if (e.key === "[" || e.key === "]") {
    const agents = model.agents;
    if (agents.length) {
      const index = agents.findIndex((a) => a.row[0] === model!.selected);
      model.selected =
        agents[
          (index + (e.key === "]" ? 1 : -1) + agents.length) % agents.length
        ].row[0];
      render();
    }
    e.preventDefault();
  } else if (["g", "e", "r"].includes(e.key.toLowerCase())) {
    model.setView(
      ({ g: "god", e: "enl", r: "res" } as const)[
        e.key.toLowerCase() as "g" | "e" | "r"
      ],
    );
    render();
    e.preventDefault();
  } else if (e.key.toLowerCase() === "v") {
    const views: View[] = ["god", "enl", "res", "public"];
    model.setView(views[(views.indexOf(model.view) + 1) % 4]);
    render();
  } else if (e.key === "+" || e.key === "=") map?.zoom(1.5);
  else if (e.key === "-") map?.zoom(1 / 1.5);
});

tournaments = new TournamentPanel(
  (file) => void load(file),
  () => setPlaying(false),
);

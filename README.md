# Ingress Arena

**AI 代码竞技场 v0.1**：参赛者各自编写一段程序（Client），让它在整场比赛中自主决策，控制一个阵营的 Agent 与其他程序实时对抗。比赛发生在一张 6 km × 6 km 的二维整数地图上：

- 双方阵营 **ENL** 与 **RES** 各派出 3 个 Agent；
- 占领 **Portal**（地图上 120 个固定节点）、获取 **Key**、搭建互不相交的 **Link**；
- 三个同阵营 Portal 用 Link 闭合后自动形成 **Field**，每个 Field 以面积 × 时间累积得分；
- 默认 500 轮（游戏内每分钟一轮），结束时累计分更高的一方获胜。

本仓库同时是**规则、引擎与平台**：包含规则文档、权威服务器与结算引擎、参赛方 SDK、示例参赛程序、锦标赛调度器和回放观看器。

**最直接的体验方式**：用浏览器打开 [`viewer/sample.html`](./viewer/sample.html) —— 一场真实锦标赛的完整回放，不需要任何构建步骤。

## 快速开始

前置条件：Rust 工具链 1.98.0（由 `rust-toolchain.toml` 固定）。依赖已随仓库离线固化在 [`vendor/`](./vendor/) 中，装好工具链后构建无需联网：

```sh
rustup toolchain install 1.98.0 --profile minimal --component rustfmt
cargo build --release
cargo test --workspace
```

## 先看一场比赛

[`viewer/sample.html`](./viewer/sample.html) 是自包含的回放页面（真实锦标赛 r1 中 `hybrid` 对阵 `probe` 的完整 100 轮），直接双击用浏览器打开即可：

| 操作 | 效果 |
|---|---|
| 空格 / Play 按钮 | 播放 / 暂停 |
| `←` / `→` | 逐轮步进 |
| `Home` / `End` | 跳到开头 / 最后一轮 |
| `+` / `−` | 加速 / 减速 |
| 滑块 | 快速跳转到任意轮 |
| `V` | 切换视角（ENL 视角 → RES 视角 → 公开视角 → 上帝视角） |

页面完全离线、零外部请求；在后面加 `?selftest=1` 打开可在页内运行数据自检。

把一份胜负记录（`.iarp`）导出成自己的观看页面：

```sh
cargo run --release -p replay-export -- <replay.iarp> -o out.html --faction enl
```

导出前会先完整验证回放（逐轮状态哈希、得分增量、事件序列全部重算一致）；未通过验证的输入不会写出任何 HTML。`--faction enl|res` 是生产导出，只嵌入该阵营的私有信息；调参和分析可用 `--dev 1` 嵌入双方阵营。

## 运行比赛

[`arena-runner`](./crates/arena-runner/) 是比赛调度器，几条常用入口：

```sh
cargo run --release -p arena-runner -- tournament-r1             # 冻结的 r1 锦标赛：3 个基线程序 × 14 场，沙箱内运行
cargo run --release -p arena-runner -- tournament-inprocess-r1   # 同赛程但全部在进程内运行，速度快
cargo run --release -p arena-runner -- conflict-stats            # 对归档回放做冲突结算统计
cargo run --release -p arena-runner                              # 不带参数：运行冻结的平衡实验批次（m6-r1，10000 场）
```

产物写入 `runs/...`：每场含比赛记录 `replay.iarp`、服务器日志 `result.txt` / `journal.txt`、`meta.json`；锦标赛还生成 `index.tsv`、`ratings.tsv`（Q32.32 整数 Elo）与 `standings.txt`。每场的派系分配、种子、地图哈希与参赛程序溯源信息都记录在案。`runs/` 是本地实验证据、不入版本库，正式结论见 `docs/` 下的报告。

**跑一场自己的比赛**（一条命令：一台服务器 + 两个客户端进程，可选自动导出观看页面）：

```sh
scripts/run-external-match.sh --client target/release/hybrid-client \
  --match-id hh-1 --config default --export dev
```

要点：

- 先连上的客户端视为 **ENL**，后连上的视为 **RES**；两者都必须在握手窗口内完成连接（`INITIAL_SPAWN_WINDOW + 5 s`，默认配置约 10 s）。脚本把它们在同一进程组内背靠背启动，因此在两个终端里手工分步启动很容易超时并得到 `setup_error`。
- 种子一律十进制（如 `--seed 12648430`，即 `0xC0FFEE`）。
- 产物在 `--out` 目录下按 match id 存放：`replay.iarp`、`journal.txt`、`result.txt`；`--export dev`（或 `enl` / `res`）会再把回放导出成自包含 HTML 供浏览器直接观看。
- 脚本直接运行客户端二进制（不放沙箱）；正式赛事由 `arena-runner` 经 `sandbox-runner` 执行。

等价的手工命令（与脚本内部做法相同）：

```sh
target/release/arena-server --socket /tmp/match.s --seed 12648430 \
  --map-seed 7 --config default --match-id m-0001 --out runs/external &
target/release/hybrid-client /tmp/match.s 12648430 &   # 先连者：ENL
target/release/hybrid-client /tmp/match.s 12648430 &   # 后连者：RES
```

## 游戏机制速览

完整规则见 [`规则文档`](./.scratch/ingress_arena_v0.1/rules/README.md)，这里只讲用户需要知道的部分：

```plantuml
@startuml
loop 每一轮
  :Server 发布只读快照并通知双方;
  :双方 Client 在决策窗口（默认 1 s）内提交动作流;
  :Server 按确定性顺序结算并广播事件;
end
:比较累计得分，产生胜者;
@enduml
```

- **Portal / Link / Field**：占领 Portal、持有对应 Key 即可在己方两个 Portal 之间连线（新线不许穿过其他 Portal、不许与既有线非法相交）；三线闭合自动形成 Field，无需显式声明。
- **计分**：每个存在的 Field 每轮按面积累加得分，即「面积 × 时间」积分；`score2` 用 2×面积、128 位整数表示并逐轮公开，胜负只比较它。
- **Agent 与动作**：主动作（Attack / Recharge / Capture）每轮每 Agent 一次；移动、连线、Hack、转移等辅动作按各自规则多次；每轮共 200 m 移动预算。攻防由 Supply 一补给驱动，Hack Portal 产生补给与 1–3 把 Key，每 Epoch（100 轮）重置次数与冷却并追加增援。
- **公平性与可验证**：权威结算只用整数运算且完全确定；每轮结束对权威状态做 BLAKE3 状态哈希，回放任何一步都以哈希校验；参赛程序只能通过官方 SDK 观察世界，读取不到敌方私有状态，也读不到结算阶段的共享内存。提交时机按 Priority Bucket 计入调度，响应速度快有实际收益。

## 编写参赛程序

v0.1 的唯一准入类型是 **Audited Client**：

- 安全 Rust、只使用获批的冻结依赖闭包；源码与全部依赖可由主办方离线构建并接受白盒审计（[`tools/audit`](./tools/audit/)）；
- 只能通过官方 [`arena-client-sdk`](./crates/arena-client-sdk/) 访问系统边界（只读共享内存快照、零拷贝观察视图、阶段门禁）；
- 资源与文件系统隔离由沙箱强制（rlimits + landlock，禁网络等），见 [`tools/sandbox-runner`](./tools/sandbox-runner/) 与 [`docs/sandbox.md`](./docs/sandbox.md)；
- 提交后即冻结，不允许重连或修改。

入手路径：[`bots/`](./bots/) 下三个可运行示例（`nearest-client` / `hybrid-client` / `probe-client`）是冻结基线策略通过 SDK 的完整包装，`crates/arena-bots` 是实现这 5 个基线策略的库；复制一个示例改策略即可。

## 仓库结构

| 路径 | 用途 |
|---|---|
| [`crates/arena-core`](./crates/arena-core/) | 权威游戏模型与结算（规则实现，全部整数运算） |
| [`crates/arena-server`](./crates/arena-server/) | 比赛服务器：回合驱动、共享内存发布、计时、记录 |
| [`crates/arena-client-sdk`](./crates/arena-client-sdk/) | 参赛方 API：只读观察、零拷贝视图 |
| [`crates/arena-protocol`](./crates/arena-protocol/) | 协议与共享内存布局（FlatBuffers 兼容 schema，代码已提交） |
| [`crates/arena-replay`](./crates/arena-replay/) | 回放编解码、规范编码、BLAKE3 状态哈希与验证 |
| [`crates/arena-runner`](./crates/arena-runner/) | 批量调度、锦标赛、统计、Elo 排名 |
| [`crates/arena-bots`](./crates/arena-bots/) | 5 个内置基线策略（自博弈与基准） |
| [`bots/`](./bots/) | 3 个示例参赛程序 |
| [`viewer/`](./viewer/) | 回放页面资源（含已生成示例 `sample.html`） |
| [`tools/`](./tools/) | 审计、溯源、沙箱、回放导出等 CLI 工具 |
| [`docs/`](./docs/) | 里程碑进度、验收、报告与决策记录 |
| [`scripts/`](./scripts/) | 离线构建证明、一键外部比赛等脚本 |
| [`.scratch/ingress_arena_v0.1/`](./.scratch/ingress_arena_v0.1/) | 规则与 MVP 方案文档集 |

## 文档导航

| 文档 | 内容 |
|---|---|
| [规则文档](.scratch/ingress_arena_v0.1/rules/README.md) | 游戏规则唯一事实源（按领域分章，编号 `R-*`） |
| [MVP 方案](.scratch/ingress_arena_v0.1/mvp/README.md) | 分阶段实现方案与里程碑 M1–M10 |
| [`CONTEXT.md`](./CONTEXT.md) | 统一术语表（Faction / Agent / Portal / Link / Field …） |
| [ADR 索引](docs/adr/README.md) | 每条决策的取舍记录 |
| [进度记录](docs/progress.md) | 里程碑日志与冻结的决策清单 |
| [MVP 验收](docs/acceptance-mvp.md) | 验收结论与已知残余风险 |
| [r1 锦标赛报告](docs/tournament-r1.md) | 首个排名锦标赛的结果与溯源 |
| [平衡性报告](docs/balance-report-r1.md) | 10000 场自博弈的平衡分析 |
| [观看器](docs/viewer.md)、[沙箱](docs/sandbox.md)、[构建溯源](docs/build-provenance.md) | 观看器格式、隔离机制、离线构建证明 |

## 当前状态

v0.1 MVP（M0–M10）已交付，全部测试在 debug 与 release 下通过。验收结论与**已知残余风险**（如平衡调参轮尚未执行、cgroup v2 未在本机委派等）见 [`docs/acceptance-mvp.md`](./docs/acceptance-mvp.md) 末节。

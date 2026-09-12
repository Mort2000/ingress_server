# P5—P6：Tournament、Replay 与 Viewer

Replay 的记录能力从 [`P1`](./02-core-simulator.md) 开始；本阶段补齐归档、批量比赛、公平排名和可视化分析。

## P5 Tournament Runner

Runner 至少负责 Match Scheduling、Map Management、Result Recording、Crash / Timeout Handling 和 Replay Archive。每场 Match Outcome 关联双方源码树、Lockfile、工具链与二进制 Hash，确保实际运行产物可追溯。协议或 Snapshot 版本不匹配在地图发布前记为无评分 `SETUP_ERROR`；单方 `SPAWN_FORFEIT` 或启动后的 `CLIENT_FORFEIT` 记录技术性胜负；双方均失败、双方失联或 Server / Runner 故障则记录无评分结果。所有情况都归档失败阶段与私有原因。

### P5-SCHEDULING 单场调度

每个调度项只运行一场比赛，不因同一 Map Definition 自动追加阵营交换复赛。Round Start Signal 顺序逐轮交替，Bucket 内 Tie-break 又从通知顺序相反的一方开始；MVP 认为剩余阵营先后差异不足以抵消双赛成本。Runner 仍分别报告 ENL / RES 胜率和分差，以验证这一假设；单场允许 Draw。

对同一 Bot Pair 的整批不同地图，Runner 使用已记录的 Schedule Seed 分配 Faction，并保证双方担任 ENL / RES 的次数之差不超过 1。均衡发生在批次层，不要求同一地图复赛。

### P5-RANKING 排名

初期使用 Elo 即可，Draw 按双方各 0.5 分计；后续可评估 TrueSkill。公开排行榜前先建立固定 Benchmark Seed Set，并将正式评分样本与日常调参样本分离，避免偶然地图样本主导排名。

## P5-REPLAY 文件与校验

Replay 不逐轮保存完整 State，而保存：

- Initial State、冻结的完整 Map Definition、双方接受的 Initial Spawn Set 与完整 Header；
- Submitted Actions、Resolved Order 与 Events；
- 每轮整数 `score2_delta` 和 State Hash；
- 周期性 Checkpoint，建议每 50 Round 一次。
- 若异常终止，记录最后完成的 Round、最后有效 State Hash、Match Outcome 与分类原因。

Replay 从 Header 载入冻结的 Map Definition，不调用地图生成器。Checkpoint 兼顾文件体积、快速 Seek 与确定性校验。每轮 State Hash 是 `BLAKE3-256("ingress-arena-state-v0.1" || canonical_replay_state)` 的 32-byte 结果，而不是 FlatBuffers Snapshot 的原始字节 Hash；Canonical Encoding 版本进入 Header。Public Event、诊断和输出日志仍作为 Replay 记录保存，但不进入 State Hash。重放时逐轮验证 `expected_hash == replay_hash`，尽早发现运行期 RNG 非确定性、几何语义变化、Resolver Bug、状态数据损坏或版本不兼容。State Hash 不参与胜负判断，也不替代 Replay 文件签名。

新增增援上限后，IARP 格式 3（无选手身份）／4（含选手身份）使用 32 个 `u32` 的配置编码，最后一项为 `max_reinforcement_agents_per_faction`，Canonical Encoding 版本为 2。读取旧格式 1／2 时仍采用原有 31 项配置、Canonical Encoding 版本 1 和无限累计增援语义，不把新的默认上限套用到历史比赛；旧归档可逐字节重新编码。客户端的 `MatchInfo` 同样追加此配置项，旧服务端缺失该字段时按无限上限解释。Viewer 的原生工具、WASM 与独立 host 需使用更新后的构建才能读取新格式。

## P5-GATE 完成门槛

可按 Schedule Seed 均衡阵营并批量调度单场比赛，异常有确定结果，Map Definition / 构建版本 / 配置可追溯，Replay 可归档且 Hash 校验稳定。

## P6 Replay Viewer

Viewer 首版支持：

- Play / Pause、Step Round、Jump Round；
- Portal Ownership、Link、Field；
- 指定阵营 Agent 与事件；
- Score Curve（展示层由 `score2` 换算）；
- `Faction View`、`Public View`、`God View`。

开发模式可查看双方 Agent；面向玩家时按视图模式执行 [`R-VIS`](../rules/05-visibility-resolution-rng.md) 的信息边界。Viewer 是调试与平衡分析工具，不只是观赏界面。

## P6-GATE 完成门槛

Viewer 可打开归档 Replay、按 Checkpoint 快速跳转，并在三种视图下正确呈现同一轮状态；显示结果与 Replay CLI 的 Hash / Score 一致。

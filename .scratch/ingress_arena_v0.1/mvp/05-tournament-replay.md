# P5—P6：Tournament、Replay 与 Viewer

Replay 的记录能力从 [`P1`](./02-core-simulator.md) 开始；本阶段补齐归档、批量比赛、公平排名和可视化分析。

## P5 Tournament Runner

Runner 至少负责 Match Scheduling、Seed Management、Faction Swap、Result Recording、Crash / Timeout Handling 和 Replay Archive。

### P5-FAIRNESS 配对公平

同一对 Bot、同一 Map Seed 连续进行两场：第一场 `A=ENL, B=RES`，第二场 `A=RES, B=ENL`，按两场汇总结果。这样降低阵营标识、出生提交次序和其他实现细节造成的偏差。

### P5-RANKING 排名

初期使用 Elo 即可，后续可评估 TrueSkill。公开排行榜前先建立固定 Benchmark Seed Set，并将正式评分样本与日常调参样本分离，避免偶然地图样本主导排名。

## P5-REPLAY 文件与校验

Replay 不逐轮保存完整 State，而保存：

- Initial State 与完整 Header；
- Submitted Actions、Resolved Order 与 Events；
- 每轮 Score Delta 和 State Hash；
- 周期性 Checkpoint，建议每 50 Round 一次。

Checkpoint 兼顾文件体积、快速 Seek 与确定性校验。重放时逐轮验证 `expected_hash == replay_hash`，尽早发现 RNG 非确定性、几何语义变化、Resolver Bug 或序列化版本不兼容。

## P5-GATE 完成门槛

可批量调度成对比赛，异常有确定结果，Seed / 构建版本 / 配置可追溯，Replay 可归档且 Hash 校验稳定。

## P6 Replay Viewer

Viewer 首版支持：

- Play / Pause、Step Round、Jump Round；
- Portal Ownership、Link、Field；
- 指定阵营 Agent 与事件；
- Score Curve；
- `Faction View`、`Public View`、`God View`。

开发模式可查看双方 Agent；面向玩家时按视图模式执行 [`R-VIS`](../rules/05-visibility-resolution-rng.md) 的信息边界。Viewer 是调试与平衡分析工具，不只是观赏界面。

## P6-GATE 完成门槛

Viewer 可打开归档 Replay、按 Checkpoint 快速跳转，并在三种视图下正确呈现同一轮状态；显示结果与 Replay CLI 的 Hash / Score 一致。

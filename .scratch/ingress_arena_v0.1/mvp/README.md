# MVP 分阶段实现方案 v0.1

本目录说明如何以最短反馈环验证玩法并交付竞技运行环境。规则语义以 [游戏规则规范](../rules/README.md) 为准；本方案只规定实现边界、顺序和验收门槛。

## MVP 要验证的假设

1. Field 面积 × 时间的计分具有足够策略深度。
2. Portal / Key / Link / Field 的资源与几何约束能产生多样策略。
3. Attack 与 Recharge 能形成合理攻防平衡。
4. 多 Agent 存在有价值的协作和分工。
5. 部分可观测状态与广播事件足以支持对手推理。
6. 不存在明显必胜套路、不可逆封锁或过强滚雪球。
7. 相同输入可以完全确定性重放。
8. 外部 AI Client 能在 `DECISION_WINDOW` 内稳定运行。

## 阶段目录

| 阶段 | 目标 | 详见 |
|---|---|---|
| `P0` | 固定规则模型、整数几何和 RNG 测试骨架 | [02-core-simulator.md](./02-core-simulator.md) |
| `P1` | 在单进程完整运行比赛并生成 Replay | [02-core-simulator.md](./02-core-simulator.md) |
| `P2` | 建立可解释的 Baseline Bots | [03-bots-self-play.md](./03-bots-self-play.md) |
| `P2.5` | 批量 Self-play，以数据进行第一轮调参 | [03-bots-self-play.md](./03-bots-self-play.md) |
| `P3` | 把 Bot 替换为受时限约束的外部进程 | [04-runtime-sandbox.md](./04-runtime-sandbox.md) |
| `P4` | 加入资源隔离与 Sandbox | [04-runtime-sandbox.md](./04-runtime-sandbox.md) |
| `P5` | 批量调度、排名和 Replay 归档 | [05-tournament-replay.md](./05-tournament-replay.md) |
| `P6` | 提供 Replay Viewer | [05-tournament-replay.md](./05-tournament-replay.md) |

## 专题目录

| 文件 | 审阅重点 |
|---|---|
| [01-architecture.md](./01-architecture.md) | 权威边界、模块划分、技术选择与 SDK |
| [02-core-simulator.md](./02-core-simulator.md) | `P0`—`P1` 的模型、Resolver、测试和 Replay 基础 |
| [03-bots-self-play.md](./03-bots-self-play.md) | `P2`—`P2.5` 的基线策略、实验矩阵和调参信号 |
| [04-runtime-sandbox.md](./04-runtime-sandbox.md) | `P3`—`P4` 的 SHM、RPC、超时和隔离 |
| [05-tournament-replay.md](./05-tournament-replay.md) | `P5`—`P6` 的公平赛制、归档、Hash 和 Viewer |
| [06-roadmap-acceptance.md](./06-roadmap-acceptance.md) | `M1`—`M10`、MVP Done 和后续边界 |

## 推进原则

先用纯内存 Simulator 证明 Game Core 有趣且确定，再依次引入外部 Runtime、Sandbox、Tournament 和 Viewer。正式 Server、Simulator 与 Replay 必须共享同一个 Resolver。

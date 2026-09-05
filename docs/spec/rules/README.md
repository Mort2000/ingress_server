# 游戏规则规范 v0.1

本目录是比赛行为与结算语义的唯一事实源。默认参数集中在 [`R-CONFIG`](./06-configuration.md)；工程结构、协议选型和交付顺序属于 [MVP 方案](../mvp/README.md)。统一术语见 [`CONTEXT.md`](../../../CONTEXT.md)。

## 游戏摘要

ENL 与 RES 各自用一个 AI Client 控制多个 Agent，在随机二维地图上占领 Portal、获取 Key、创建 Link 并形成 Field。每轮结束后，仍存在的 Field 按面积累积分数；比赛结束时总分更高的一方获胜。

该游戏可抽象为一个部分可观测、限时决策的多 Agent 空间图博弈，主要算法空间包括计算几何、图优化、路径规划、资源分配、对手建模与实时规划。

## 设计约束

| 编号 | 约束 |
|---|---|
| R-GOAL-01 | Client 必须自主决策，计算效率、规划和状态推理均具有竞技价值 |
| R-GOAL-02 | Key、路线、Anchor、攻防和物流应形成长期策略与多 Agent 分工 |
| R-GOAL-03 | Portal、Link、Field 全局公开；敌方 Agent 状态不直接公开，但可由事件推断 |
| R-GOAL-04 | 得分不直接增强战斗、移动或产出，避免不可逆滚雪球 |
| R-GOAL-05 | 相同 Seed、配置、动作序列、接收 Bucket 和 Server 版本必须得到完全一致的结果 |

## 专题目录

| 文件 | 内容 | 主要编号 |
|---|---|---|
| [01-match-map.md](./01-match-map.md) | Round、比赛长度、地图、出生与增援 | `R-TIME`、`R-MAP`、`R-SPAWN` |
| [02-agents-actions.md](./02-agents-actions.md) | Agent 状态、Command Stream、移动与动作分类 | `R-AGENT`、`R-ACTION`、`R-MOVE` |
| [03-portals-resources.md](./03-portals-resources.md) | Portal、Capture、Attack、Recharge、Hack、Key、Transfer | `R-PORTAL`、`R-CAPTURE`、`R-ATTACK`、`R-RESOURCE` |
| [04-links-fields-score.md](./04-links-fields-score.md) | Link、Field、Layered Field 与计分 | `R-LINK`、`R-FIELD`、`R-SCORE` |
| [05-visibility-resolution-rng.md](./05-visibility-resolution-rng.md) | 可见性、事件、提交、冲突排序与随机数 | `R-VIS`、`R-EVENT`、`R-RESOLVE`、`R-RNG` |
| [06-configuration.md](./06-configuration.md) | v0.1 默认参数单一事实源 | `R-CONFIG` |
| [07-strategy-balance-scope.md](./07-strategy-balance-scope.md) | 非规范性策略、平衡指标与明确不做项 | `R-BALANCE`、`R-SCOPE` |

## 规范性约定

- “必须 / 不得 / 最多”是强制规则；“推荐 / 可”是实现或调参建议。
- 动作是否合法，以动作实际执行时的权威状态为准。
- `Field` 是 `Link Graph` 的派生状态；缓存不能改变规则结果。
- 已确认的边界语义必须用公式或示例固化，并进入自动测试。

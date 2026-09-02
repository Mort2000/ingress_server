# Agent 与动作模型

动作的具体前置条件和效果见 [Portal 与资源](./03-portals-resources.md)及 [Link、Field 与得分](./04-links-fields-score.md)；默认数值见 [`R-CONFIG`](./06-configuration.md)。

## R-AGENT-01 Agent 状态

每个 Agent 至少包含以下权威状态：

| 字段 | 含义 |
|---|---|
| `id` | 比赛内唯一标识 |
| `faction` | `ENL` 或 `RES` |
| `position` | 整数二维坐标 |
| `supply` | `[0, MAX_SUPPLY]` 内的补给量；初始为 `INITIAL_SUPPLY` |
| `keys[portal_id]` | 对应 Portal 的 Key 数量 |

敌方能否看到这些字段由 [`R-VIS`](./05-visibility-resolution-rng.md) 定义。

## R-ACTION-01 Command Stream

每个 Agent 每 Round 提交一个有序 Command Stream。Server 按结算顺序逐条执行，并在每条命令执行前根据最新权威状态重新校验；先前合法动作不因后续动作失败而回滚。

| 分类 | 动作 | 每 Agent 每 Round 限制 |
|---|---|---|
| Major Action | `ATTACK`、`RECHARGE`、`CAPTURE` | 最多一个 |
| Auxiliary Action | `MOVE`、`HACK`、`LINK`、`TRANSFER` | 可多个，但受移动预算及协议硬限制约束 |

Major Action 执行时若因世界状态变化而非法，直接失败，且本轮 Major Action 额度仍视为已使用。不得通过提交多个候选 Major Action 让 Server 选择首个合法者。

> 待确认 `Q-R-ACTION-01`：包含多个 Major Action 的整个 Command Stream 是被拒绝，还是仅从第二个 Major Action 起丢弃。MVP 方案当前建议拒绝整个 Stream，见 [`P1-RESOLVE`](../mvp/02-core-simulator.md)。

## R-MOVE-01 移动预算

`MOVE` 是 Auxiliary Action，可在同轮多次穿插于其他动作之间。该 Agent 当轮所有 `MOVE` 路径长度之和不得超过 `MOVE_BUDGET`；每次移动后的新位置立即参与后续动作的距离校验。

例如 `MOVE 80m → HACK → MOVE 40m → CAPTURE → MOVE 50m → TRANSFER` 的累计移动为 170m，在默认预算内。

## R-ACTION-02 协议防滥用边界

Auxiliary Action 在玩法层不设数量配额，但协议层应限制 `MAX_COMMANDS_PER_AGENT_ROUND` 和 `MAX_RPC_PAYLOAD`。达到上限的精确拒绝粒度属于协议契约，需在实现时固定并纳入 Replay。

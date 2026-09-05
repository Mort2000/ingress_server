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

Agent 在空间规则中视为无碰撞体积的点。同阵营或敌对阵营的 Agent 可以占据完全相同的坐标；Agent 占位不导致出生或移动失败，也不阻挡其他 Agent。Portal 操作与 Agent 间距离仍分别按各自坐标正常判定。

敌方能否看到这些字段由 [`R-VIS`](./05-visibility-resolution-rng.md) 定义。

## R-ACTION-01 Command Stream

每个 Agent 每 Round 提交一个有序 Command Stream。Command Stream 是原子调度单元：一旦轮到该 Stream，Server 连续处理其全部命令，不穿插其他 Agent 的命令；但它不是事务，每条命令执行前仍根据最新权威状态重新校验，先前合法动作不因后续动作失败而回滚。

零命令 Stream 是结构合法的显式 `PASS`：它冻结该 Agent 的槽位并计入 Faction 输入完成条件，但不进入 Resolver 执行队列或占用 Bucket 内交替位置。Client 因此可以只让一个 Agent 主动空过，而无需用 `COMMIT_ROUND` 冻结其余 Agent。

任何结构合法且非空的 Stream 均占用一次执行排序位置；即使其中全部命令在执行期 `DROP`，Server 也不得移除该 Stream 后重新排序。

| 分类 | 动作 | 每 Agent 每 Round 限制 |
|---|---|---|
| Major Action | `ATTACK`、`RECHARGE`、`CAPTURE` | 最多一个 |
| Auxiliary Action | `MOVE`、`HACK`、`LINK`、`TRANSFER` | 可多个，但受移动预算及协议硬限制约束 |

Server 在调度前执行结构校验。若 Command Stream 包含多个 Major Action，整条 Stream 被拒绝，任何命令都不执行；Client 收到私有拒绝原因，Replay 记录该次提交。结构拒绝不允许修正重提，详见 [`R-RESOLVE-01`](./05-visibility-resolution-rng.md)。不得通过提交多个候选 Major Action 让 Server 选择首个合法者。

结构合法的 Major Action 若在实际执行时因世界状态变化而非法，则只丢弃该命令，且本轮 Major Action 额度仍视为已使用；此前成功命令不回滚。

## R-MOVE-01 移动预算

`MOVE(destination)` 是以绝对目的坐标为载荷的 Auxiliary Action，可在同轮多次穿插于其他动作之间。每条命令执行时，以 Agent 当时的最新位置到 `destination` 计算坐标差；坐标差以 `COORDINATE_UNIT` 表示，每段移动成本使用纯整数公式：

```text
move_cost = ceil_integer_sqrt(dx² + dy²)
```

该 Agent 当轮所有成功 `MOVE` 的 `move_cost` 之和不得超过 `MOVE_BUDGET`；目标坐标必须位于 [`R-MAP-01`](./01-match-map.md#r-map-01-坐标与边界) 定义的闭区间内。越界或将使累计成本超出预算的 `MOVE` 直接 `DROP`，不改变位置及已用预算，也不裁剪到边界或从另一侧环绕；Stream 中后续命令继续基于原位置执行。每次成功移动后的新位置立即参与后续动作的距离校验。向上取整使每段误差小于一个坐标单位，并防止通过拆分大量短对角线获得额外距离。例如从 `(0,0)` 移到 `(1,1)` 的成本为 2 个坐标单位。

例如三段实际计费成本分别为 80m、40m、50m 时，`MOVE → HACK → MOVE → CAPTURE → MOVE → TRANSFER` 的累计移动为 170m，在默认预算内。

## R-ACTION-02 协议防滥用边界

每条 Agent Command Stream 最多包含 `MAX_COMMANDS_PER_AGENT_ROUND` 条命令；Major 与 Auxiliary 均计数，与执行时成功或失败无关。超过上限属于提交时可判定的结构错误，依照 [`R-ACTION-01`](#r-action-01-command-stream) 整条拒绝且不允许重提。完整消息还受 `MAX_FRAME_PAYLOAD_BYTES` 硬限制。

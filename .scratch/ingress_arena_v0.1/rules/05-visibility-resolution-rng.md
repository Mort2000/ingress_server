# 可见性、结算与随机数

本页定义 Client 能看到什么，以及 Server 如何把同时提交的动作变成唯一结果。参数值见 [`R-CONFIG`](./06-configuration.md)。

## R-VIS-01 Public State

双方在每轮快照中均可读取：

- 当前 Round 和双方当前 Score；
- 所有 Portal 的位置、阵营和 HP；
- 所有 Link 与 Field；
- Public Event History。

## R-VIS-02 Private State

Client 只能读取己方 Agent 的位置、Supply 和 Key Inventory。敌方 Agent 的位置、移动、Supply、Key、Hack、Recharge 和 Transfer 不直接公开。

## R-EVENT-01 广播事件

Public Event History 至少广播以下事件：

| 类别 | 事件 |
|---|---|
| Portal | `PORTAL_CAPTURED`、`PORTAL_ATTACKED`、`PORTAL_NEUTRALIZED` |
| Link | `LINK_CREATED`、`LINK_DESTROYED` |
| Field | `FIELD_CREATED`、`FIELD_DESTROYED` |
| 比赛 | `REINFORCEMENT` |

每条事件至少包含 `round`、`event_seq`、`faction`、`agent_id` 及相关 Portal / Link / Field 标识。`MOVE`、`HACK`、`RECHARGE`、`TRANSFER` 不广播；公开事件因此可用于推断敌方位置和意图，但不直接泄露私有状态。

## R-RESOLVE-01 提交窗口

每轮从快照发布开始接受带 `round_id` 的动作，直到双方均发送 `COMMIT_ROUND` 或到达 deadline。关闭后到达且 `round_id < current_round` 的动作全部丢弃；Server 不等待慢 Client 再次就绪。

## R-RESOLVE-02 非法动作

每个动作在实际执行时根据当时状态校验。非法动作直接 `DROP`：不回滚此前成功动作，也不自动替换为候选动作。Major Action 失败时仍消耗其当轮额度，详见 [`R-ACTION-01`](./02-agents-actions.md)。

## R-RESOLVE-03 公平排序

为避免几十微秒级 OS Scheduler 抖动决定冲突，同时保留显著低延迟优势，动作先按接收时间映射到 `PRIORITY_BUCKET`，较早 Bucket 优先；同一 Bucket 使用下式生成的确定性伪随机键排序：

```text
tie_break_key = hash(game_seed, round, action_id)
```

同一 Agent 的 Command Stream 仍按命令序号保持先后。`action_id` 的构造和完整排序键必须由协议固定并写入 Replay。

若未来明确把极端低延迟也作为竞技因素，应在新规则版本中显式切换为 Pure FIFO；v0.1 使用 Priority Bucket。

> 待确认 `Q-R-RESOLVE-01`：当不同 Agent 的多条命令分布在同一 Bucket 时，排序是以整条 Command Stream 还是单条命令为单位。原稿未完全钉死，Resolver 测试需先固定该语义。

## R-RESOLVE-04 典型冲突

| 同轮提交 | 先执行结果 | 后执行结果 |
|---|---|---|
| 双方 `CAPTURE(P17)` | Portal 归先执行方 | 因 Portal 已非 Neutral 而 `DROP` |
| `LINK(A,B)` 与相交的 `LINK(C,D)` | 第一条 Link 创建成功 | 因与既有 Link 相交而 `DROP` |

## R-RNG-01 随机源隔离

比赛随机源必须按用途解耦，避免一个行为改变后续所有随机事件：

| 用途 | 输入 |
|---|---|
| Map | `hash(game_seed, "map")` |
| Hack | `hash(game_seed, round, agent_id, portal_id, hack_index)`，再映射到 Key 产量 |
| Reinforcement | `hash(game_seed, "reinforcement", epoch)` |
| 同 Bucket 排序 | `hash(game_seed, round, action_id)` |

确定性重放还要求固定哈希算法、序列化版本、地图生成算法和 Server 版本；这些元数据必须进入 Replay Header。

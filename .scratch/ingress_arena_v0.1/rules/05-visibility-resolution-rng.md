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

每轮从快照发布开始，通过 `SUBMIT_COMMAND_STREAM` 接受带 `round_id` 的 Command Stream，直到双方输入完成或到达 deadline。每次调用只携带一个 Agent 的 Stream；同一 Faction 的 Agent 可独立提交，不必等待该方所有 Agent 完成规划。

每个 Client 只使用一条持久化、全双工 Unix Domain `SOCK_STREAM` 连接，所有协议消息均为 `u32` 大端长度前缀加 Payload；SDK 必须串行写入完整帧，Server 拒绝同一 Client 的额外提交连接。Payload 长度不得超过 `MAX_FRAME_PAYLOAD_BYTES`，且不含 4 字节前缀；Server 在分配内存前检查该值，超限时不排空帧而直接关闭连接，开赛后产生 `CLIENT_FORFEIT`。上限内但 FlatBuffers Verifier 失败的完整帧仅按信封非法拒绝，不冻结 Agent 槽位。

Server 传输层在完整协议帧的最后一个字节到达时，用单调时钟记录 `received_at`，并按该连接上的帧结束顺序分配严格递增的 Faction `receive_seq`。仅 `received_at < deadline` 的完整帧有资格进入本轮；deadline 时尚未完整到达的帧直接丢弃且不冻结槽位。Client 提供的时间戳或序号不可信，后续解析耗时不改变 `received_at` 与 `receive_seq`。

Server 随后校验消息可解码、大小、协议版本、当前 Round、已认证 Faction 和 Agent 归属；只有通过这些信封校验的消息才能定位提交槽位。

每个 Agent 的首个信封合法提交一经完整接收便冻结其当轮槽位，之后才校验 Stream 内部结构。内部结构即使非法也不解冻，后续提交均不能替换；结构错误属于 Client Bug，不提供修正重提。结构非法或结构合法但为空的 Stream 均不进入 Resolver 执行队列，也不占用 Bucket 内的一次交替位置，但冻结的槽位仍计入该 Faction 的输入完成条件。空 Stream 作为合法 `PASS` 记录，结构非法 Stream 作为拒绝记录。信封非法的消息不冻结任何槽位。

Faction 在以下任一条件满足时完成本轮输入：

- Client 显式发送 `COMMIT_ROUND`；所有未提交 Agent 立即冻结为空过；
- 该 Faction 的全部 Agent 槽位都已被首个提交冻结；Server 自动视为完成。

完成状态不可撤销。双方 Faction 均完成时，Server 可立即关闭本轮；否则等到 deadline，并把剩余未提交 Agent 冻结为空过。输入关闭后到达的旧 Round 提交全部丢弃，Server 不等待慢 Client 再次就绪。

输入关闭即进入 Settlement Phase。Client 从此不得读取 Public 或己方 Private SHM，直到收到下一轮 Round Start Signal；提前读取视为作弊。MVP 不做物理访问拦截，以官方 SDK 的阶段约束和 Client 白盒代码审计执行本规则。

## R-RESOLVE-02 非法动作

每个动作在实际执行时根据当时状态校验。非法动作直接 `DROP`：不回滚此前成功动作，也不自动替换为候选动作。Major Action 失败时仍消耗其当轮额度，详见 [`R-ACTION-01`](./02-agents-actions.md)。

## R-RESOLVE-03 公平排序

Priority Bucket 是 MVP 主动验证的竞技特性，而非临时实现细节：它让规划深度与响应速度形成取舍，同时避免双方之间几十微秒级 OS Scheduler 抖动直接决定冲突。每个 Agent Command Stream 独立按权威接收时刻映射到 Bucket：

```text
receive_bucket = floor((received_at - round_start) / PRIORITY_BUCKET)
```

较早 Bucket 的 Stream 优先。只有结构合法且非空的 Stream 参与排序。每个 Bucket 内，Server 分别按完整帧到达顺序建立 ENL 与 RES 队列；同一单调时钟值再以 Server 分配的每 Faction 严格递增 `receive_seq` 排序。奇数轮先通知 RES，因此从 ENL 队列开始；偶数轮先通知 ENL，因此从 RES 队列开始。随后在两队之间逐项交替；一队耗尽后，另一队按原顺序连续排完。例如奇数轮为 `ENL-0 → RES-0 → ENL-1 → RES-1 → ...`，其中数字表示该 Faction 在该 Bucket 内的可执行 Stream 到达序号，不是 Agent ID。

Client 不提供任意 `stream_id`、排序 nonce 或其他 Tie-break 输入。选中一个 Stream 后按命令序号连续处理全部命令，不插入其他 Stream；每条命令仍按 [`R-RESOLVE-02`](#r-resolve-02-非法动作) 独立校验。即使全部命令最终 `DROP`，该 Stream 仍保留原排序位置，禁止根据执行结果重排。权威 Bucket、每 Faction 到达顺序、Round Start Signal 顺序和最终执行顺序必须写入 Replay。

若未来明确把极端低延迟也作为竞技因素，应在新规则版本中显式切换为 Pure FIFO；v0.1 使用 Priority Bucket。取舍背景见 [ADR-0002](../../../docs/adr/0002-validate-priority-bucket-in-mvp.md) 与 [ADR-0003](../../../docs/adr/0003-agent-command-stream-is-scheduling-unit.md)。

## R-RESOLVE-04 典型冲突

| 同轮提交 | 先执行结果 | 后执行结果 |
|---|---|---|
| 双方 `CAPTURE(P17)` | Portal 归先执行方 | 因 Portal 已非 Neutral 而 `DROP` |
| `LINK(A,B)` 与相交的 `LINK(C,D)` | 第一条 Link 创建成功 | 因与既有 Link 相交而 `DROP` |

## R-RNG-01 随机源隔离

地图冻结后，比赛内不维护会被调用顺序推进的全局 PRNG State。每个运行期随机结果通过带域隔离的 BLAKE3 Counter/XOF 独立派生，地址由 `(game_seed, rng_version, domain_tag, stable_indices...)` 的版本化 Canonical Encoding 构成：

| Domain | Stable Indices |
|---|---|
| `hack` | Round、Agent ID、Portal ID、成功 Hack 序号 |
| `reinforcement` | Epoch 及坐标轴 |

XOF 输出映射到有限整数范围时必须使用固定 rejection sampling，禁止直接 `% n` 引入 modulo bias。新增、删除或重排其他随机调用不得改变既有地址的输出；失败 Hack 不产生可观察样本。`rng_version`、地址编码、字节序与 Golden Vectors 属于规则版本的一部分，详见 [ADR-0011](../../../docs/adr/0011-domain-separated-blake3-random-oracle.md)。

地图生成不属于该 Random Oracle 或 Replay 状态转换；Replay Header 直接保存冻结的完整 Map Definition。确定性重放还要求记录首个提交与被忽略的重复提交，并固定运行期 RNG、序列化和 Server 版本；这些元数据必须进入 Replay Header。

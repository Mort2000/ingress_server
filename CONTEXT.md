# Ingress Arena

Ingress Arena 是一个由参赛程序控制多个单位、通过空间图结构争夺面积时间积分的代码竞技领域。本文件定义规则与实现共同使用的统一语言，不承载实现方案。

## 比赛世界

**Faction**:
比赛中的对立阵营，v0.1 固定为 ENL 与 RES。
_Avoid_: Team, side

**Agent**:
隶属于一个 Faction、在地图上占据位置并执行动作的比赛单位。Agent 视为无碰撞体积的点；同阵营或敌对阵营的多个 Agent 可以位于完全相同的坐标，且不阻挡出生或移动。
_Avoid_: Player, Client, bot

**Coordinate**:
以地图左下角为原点、使用 `COORDINATE_UNIT` 整数计量的位置。合法闭区间为 `0 <= x <= MAP_WIDTH`、`0 <= y <= MAP_HEIGHT`。
_Avoid_: GPS coordinate, wrapped coordinate

**Initial Spawn Set**:
一个 Faction 在 `INITIAL_SPAWN_WINDOW` 内一次性提交的完整初始 Agent 到 Coordinate 映射；它作为整体校验和生效，不参与 Round 的 Priority Bucket。
_Avoid_: Spawn action, per-Agent spawn stream

**Spawn Forfeit**:
一方首个信封合法的 Initial Spawn Set 内部非法，或在赛前截止时间前没有提交时产生的技术性失败；不以自动出生或修正重提兜底。双方均失败时比赛无有效结果且不计入 Elo。
_Avoid_: Default spawn, spawn retry

**Portal**:
具有固定位置、归属与生命值，可被占领并参与 Link 的地图节点。
_Avoid_: Node, point

**Supply**:
由 Agent 持有并在 Recharge 时消耗的补给资源。
_Avoid_: Energy, mana

**Key**:
由 Agent 持有、与一个 Portal 对应，创建指向该 Portal 的 Link 时消耗的资源。
_Avoid_: Portal Key, token

**Transfer**:
由 Command Stream 所属 Agent 向范围内同阵营 Agent 单向发送 Supply 或 Key 的 Auxiliary Action。发送方不能由命令载荷另行指定；一条命令中的全部资源全有或全无。
_Avoid_: Exchange, trade

**Move**:
把 Agent 移至绝对目的坐标的 Auxiliary Action；每段成本从命令执行时的最新位置计算。
_Avoid_: Relative displacement, path

**Link**:
连接同阵营两个不同 Portal、并受几何相交约束的无向简单边；方向只属于创建它的命令。
_Avoid_: Edge, connection

**Field**:
由同阵营三个 Portal 及其三条 Link 闭合后派生的三角计分区域。
_Avoid_: Area, zone

**Score**:
Faction 的累计 Field 面积时间积分；权威值 `score2` 以两倍平方坐标单位表示，展示换算不影响比较结果。
_Avoid_: Points, rounded area

**State Hash**:
每轮结束后对 Canonical Replay State 计算的 `BLAKE3-256` 指纹，用于验证 Replay 并定位首次状态分歧。只覆盖会影响后续确定性重放的权威状态，不包含 Public Event、诊断或输出日志；它不决定胜负，也不是签名或反作弊证明。
_Avoid_: Snapshot hash, result hash, signature

**Draw**:
正常完成全部 Round 后双方 `score2` 完全相等的 Game Result，不使用其他状态追加 Tie-break。
_Avoid_: Technical tie, unresolved match

**Match Outcome**:
Runner 记录的一场调度结果。它可以包含正常 Game Result，也可以是 `SPAWN_FORFEIT`、`CLIENT_FORFEIT` 或无评分 `SETUP_ERROR`；技术终止不伪装成基于 Score 的 Game Result。
_Avoid_: Final score

## 决策与结算

**Round**:
比赛的离散决策与结算周期，在游戏世界中表示一分钟。
_Avoid_: Turn, tick

**Epoch**:
连续若干 Round 组成的规则周期，用于重置 Hack 次数与冷却。
_Avoid_: Season, phase

**Observation**:
一个 Faction 在 Round 开始时可合法读取的一致状态视图，由 Public State 与该阵营的 Private State 组成。
_Avoid_: GameState, Snapshot

**Command Stream**:
一个 Agent 为一个 Round 独立提交的有序命令序列，也是 Resolver 的最小调度单元。结构合法但为空的 Stream 表示显式 Pass；它冻结槽位但不进入执行队列。结构合法且非空的 Stream 必定占用一次排序位置，即使其命令随后全部 Drop；Stream 内命令连续处理，但不构成可回滚的状态事务。
_Avoid_: Action list, batch

**Major Action**:
一个 Agent 每 Round 最多使用一次的动作类别，包括 Attack、Recharge 与 Capture。
_Avoid_: Main action

**Auxiliary Action**:
一个 Agent 可在同一 Round 多次使用、但仍受各自规则及协议上限约束的动作类别。
_Avoid_: Minor action, free action

**Priority Bucket**:
根据完整 Command Stream 帧在 Decision Window 内到达 Server 传输层的单调时钟划分的固定宽度优先级区间。较早 Bucket 先结算；同一 Bucket 内，各 Faction 按自身提交到达顺序排队，再从本轮较晚收到 Round Start Signal 的 Faction 开始交替结算。
_Avoid_: FIFO, time slot

**Faction Receive Sequence**:
Server 按一个 Client 唯一持久化有序连接上的完整帧结束顺序分配的严格递增序号，用于确定同 Faction、同 Priority Bucket 内的提交顺序。
_Avoid_: Client sequence, stream nonce

**Settlement Phase**:
本轮输入关闭后、下一轮 Round Start Signal 到达前，由 Server 结算动作并发布新状态的阶段；Client 在此期间不得读取共享内存。
_Avoid_: Decision Window, idle time

**Audited Client**:
使用安全 Rust 和获批依赖、源码与完整依赖闭包可得、通过白盒审计，并由比赛方从冻结输入构建的参赛程序。它只能通过官方 SDK 访问系统边界，是 v0.1 唯一允许进入比赛的 Client 类型。
_Avoid_: Trusted binary, closed-source Client

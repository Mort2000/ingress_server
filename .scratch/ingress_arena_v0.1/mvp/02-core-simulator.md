# P0—P1：Core 与纯内存 Simulator

这是 MVP 的最关键阶段：先固定易错数学和状态语义，再让两个本地 Bot 在同一进程完整比赛。

## P0 规则模型与测试骨架

### P0-MODEL 权威模型

Core 至少定义 `GameState`、`GameConfig`、`Portal`、`Agent`、`Link`、`Action` 与 `Event`。`GameState` 持有 Round、Portal、Agent、Link、`Score2(u128)` 和 Event Log；Field 由 Link Graph 派生，可缓存但不是独立规则事实源。`LinkId` 由排序后的两个不同 `PortalId` 构成，保证无向 Pair 唯一。`Coord` 使用 `i32`，几何中间值使用 `i64`，`Area2` 使用 `u64`；权威数学不使用浮点。

### P0-GEOMETRY 整数几何

实现并单元测试 squared distance、ceiling integer square root、move cost、orientation、point on segment、segment intersection、`area2` 和 point range check。坐标底层存储为 `i32`，但必须先提升到 `i64` 再做减法、平方、乘法或叉积；范围判断直接比较距离平方，移动成本按段使用 `ceil_integer_sqrt(dx² + dy²)`。重点覆盖绝对目的坐标、连续移动从最新位置逐段计费、`(0,0)→(1,1)`、累计取整、四条闭边界及边角、负坐标和 `MAP_WIDTH/HEIGHT + 1` 越界、奇数叉积、配置坐标上界、Checked Overflow、自环、正反向重复 Link、边穿过第三 Portal、共线、共享端点、非法交叉、极小三角形、操作范围边界与最大 Link 距离边界；计算语义必须符合 [`R-MOVE-01`](../rules/02-agents-actions.md)、[`R-MAP`](../rules/01-match-map.md) 与 [`R-LINK`](../rules/04-links-fields-score.md)。

### P0-RNG 随机源

提供无可变游标的 `RandomOracle` 深模块，按 `hack`、`reinforcement` Domain 与稳定索引返回 BLAKE3 XOF 字节或无偏有限整数；禁止暴露“取下一个随机数”的接口。地图在 Core Match 创建前生成并作为完整 Definition 输入，Stream Tie-break 也不使用 RNG。固定运行期地址编码、rejection sampling 边界和跨语言 Golden Vectors；输入定义见 [`R-RNG-01`](../rules/05-visibility-resolution-rng.md)。

### P0-GATE 完成门槛

所有关键几何、RNG 和配置边界测试通过；Initial Spawn Set 必须精确覆盖该方初始 Agent，整体接受或拒绝。测试覆盖缺失、重复及未知 Agent ID 和越界坐标；同阵营及敌对阵营 Agent 的出生、移动均允许同坐标，且不产生占位冲突。本阶段不要求真正比赛。

## P1 完整 Game Simulator

### P1-LOOP 最小循环

```text
for each round:
    snapshot := state.snapshot()
    inputs := harness.run_bots(snapshot, virtual_bucket_schedule)
    resolver.resolve(inputs)
    state.update_derived_fields_and_score()
    replay.append_round_record()
```

本地 Bot 与外部 Client 使用同一 `Bot::on_round(RoundContext<'_>)` 概念接口，但本地 Harness 不经过 RPC。Observation 可见性保持一致；Harness 为每个 Agent Stream 注入虚拟接收时间或 Bucket，不读取 Bot 的实际 wall-clock 时间。

### P1-ACTIONS 垂直实现顺序

| 批次 | 能力 | 首要验证 |
|---|---|---|
| 1 | `MOVE`、`CAPTURE`、`ATTACK` | 成功移动计费、失败无副作用、Portal 争夺与同轮冲突 |
| 2 | `HACK`、Supply、Key、`TRANSFER` | 均匀 Key 收益、失败不抽样、原子转移与 Epoch |
| 3 | `LINK`、Field、Score | 核心玩法闭环与整数几何 |
| 4 | `RECHARGE` | 完整攻防体系 |

动作语义只从 [规则专题](../rules/README.md)读取，不在 Core 中形成第二份规则。

### P1-RESOLVE Resolver

每个 `SubmittedCommandStream` 至少携带 faction、agent_id、receive_bucket、该 Faction 的权威 `receive_seq` 和有序 commands；Client 不提供可影响排序的自选 ID 或 nonce。Resolver 依次：

1. transport 已在分配前确认 Payload 不超过 65,536 bytes；FlatBuffers Verifier 通过后，再校验 Schema / 协议版本、当前 Round、Faction 认证与 Agent 归属，并转换为 Core 自有类型；
2. 通过信封校验后冻结该 Agent 的首个提交槽位，再校验 Stream 内部结构；结构非法者记为拒绝、空 Stream 记为 `PASS`，两者均不进入执行队列；
3. 仅以结构合法且非空的 Stream 按 Bucket 建立两个 Faction 到达队列，并依 [`R-RESOLVE-03`](../rules/05-visibility-resolution-rng.md) 从通知顺序相反的一方开始交替合并；
4. 固定最终 Stream 顺序后连续处理内部命令，每条命令基于最新状态校验并执行；执行结果不得反向改变排序；
5. 记录成功、失败原因、事件和 Score Delta；
6. 结算轮末 Field 与 Score；边界轮随后生成下一轮增援；最后生成 State Hash。

解析时拒绝包含多个 Major Action 或超过 10 条命令的整个 Command Stream，任何内部命令都不执行；拒绝原因进入私有响应和 Replay，且该 Agent 不得修正重提。协议兼容测试必须区分“信封拒绝且不冻结”“Stream 结构拒绝且冻结但不占交替位置”“空 Stream 合法 Pass 且不占交替位置”“非空 Stream 全部命令 Drop 但仍占原排序位置”“重复提交被忽略”与“执行期单条命令因世界状态而 `DROP`”，并覆盖奇偶轮相反首发、同 Faction 到达顺序、不同长度队列的交替合并、10 / 11 条命令边界、失败 `MOVE` 不消耗预算且后续命令使用原位置、`TRANSFER` 的发送方约束及余额不足、容量超限、距离变化时全有或全无、显式 Commit 冻结剩余 Agent、全员提交自动完成、deadline 冻结剩余 Agent、完整帧恰好跨越 deadline、Client 伪造时间戳、边界轮增援只进入下一轮、Hack 的 Round 10→13 冷却及 Round 100→101 Epoch 重置。

### P1-FIELD 增量维护

创建 Link `(A,B)` 时求 `Neighbors(A) ∩ Neighbors(B)`，为每个共同邻居生成 Field。Portal Neutralize 时删除 Incident Links，再同步删除失效 Field。增量结果必须用小规模全量重算测试作对照。

### P1-HASH Canonical State Hash

每轮完成后，以带域标签 `ingress-arena-state-v0.1` 的固定 Canonical Encoding 对 Replay State 计算 `BLAKE3-256`。编码固定字段顺序、整数宽度和字节序，所有集合按稳定 ID 排序；只包含 Round、配置身份及会影响后续确定性状态转换的权威数据，排除 Public Event、诊断、输出日志、派生缓存、内存偏移、wall-clock、连接状态和 FlatBuffers 布局。Replay 保存原始 32 bytes，CLI 可显示为十六进制。Canonical Codec 必须有 Golden Vector 和插入顺序无关测试。

### P1-REPLAY 最早建立 Replay

从 P1 起记录：

| 位置 | 必需内容 |
|---|---|
| Header | game_seed、完整 config、冻结的完整 Map Definition、双方初始出生位置、Server / 协议 / 运行期 RNG 地址编码 / Canonical State Encoding 版本 |
| Per Round | Observation 版本、Round Start Signal 顺序、首个提交及其权威接收偏移 / Bucket / Faction `receive_seq`、被忽略的重复提交、最终执行顺序、校验结果、生成事件、`score2_delta`、State Hash |

自动测试必须证明 `Replay + Same Server Version → Same Final Hash`，并验证 Final Result 只能是 `ENL_WIN`、`RES_WIN` 或 `DRAW`，其中 Draw 不读取任何次级状态。

### P1-GATE 完成门槛

两个脚本 Bot 可在单进程按全部 v0.1 规则稳定完成 `TOTAL_ROUNDS`；非法动作可定位；Replay 能无偏差重放。

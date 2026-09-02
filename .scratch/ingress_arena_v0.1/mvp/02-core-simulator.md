# P0—P1：Core 与纯内存 Simulator

这是 MVP 的最关键阶段：先固定易错数学和状态语义，再让两个本地 Bot 在同一进程完整比赛。

## P0 规则模型与测试骨架

### P0-MODEL 权威模型

Core 至少定义 `GameState`、`GameConfig`、`Portal`、`Agent`、`Link`、`Action` 与 `Event`。`GameState` 持有 Round、Portal、Agent、Link、Score 和 Event Log；Field 由 Link Graph 派生，可缓存但不是独立规则事实源。

### P0-GEOMETRY 整数几何

实现并单元测试 distance、orientation、segment intersection、triangle area 和 point range check。重点覆盖共线、共享端点、非法交叉、极小三角形、操作范围边界与最大 Link 距离边界；计算语义必须符合 [`R-MAP`](../rules/01-match-map.md) 与 [`R-LINK`](../rules/04-links-fields-score.md)。

### P0-RNG 随机源

提供独立的 `map_rng`、`hack_rng`、`reinforcement_rng`、`tie_break_rng` 接口，禁止共享一个会被调用顺序污染的全局连续 RNG。输入定义见 [`R-RNG-01`](../rules/05-visibility-resolution-rng.md)。

### P0-GATE 完成门槛

所有关键几何、RNG 和配置边界测试通过；本阶段不要求真正比赛。

## P1 完整 Game Simulator

### P1-LOOP 最小循环

```text
for each round:
    snapshot := state.snapshot()
    commands := collect(bot_enl.step(snapshot), bot_res.step(snapshot))
    resolver.resolve(commands)
    state.update_derived_fields_and_score()
    replay.append_round_record()
```

本地接口为 `Bot::step(&Observation) -> Actions`，不经过 RPC；Observation 的可见性与外部 Client 保持一致。

### P1-ACTIONS 垂直实现顺序

| 批次 | 能力 | 首要验证 |
|---|---|---|
| 1 | `MOVE`、`CAPTURE`、`ATTACK` | 移动、Portal 争夺、同轮冲突 |
| 2 | `HACK`、Supply、Key、`TRANSFER` | 资源循环与 Epoch |
| 3 | `LINK`、Field、Score | 核心玩法闭环与整数几何 |
| 4 | `RECHARGE` | 完整攻防体系 |

动作语义只从 [规则专题](../rules/README.md)读取，不在 Core 中形成第二份规则。

### P1-RESOLVE Resolver

提交动作至少携带 faction、agent_id、command_sequence、receive_bucket、action_id 和 payload。Resolver 依次：

1. 校验协议版本、Round、载荷上限与 Agent 所属；
2. 为动作建立 [`R-RESOLVE-03`](../rules/05-visibility-resolution-rng.md) 定义的确定排序键；
3. 按顺序逐条基于最新状态校验并执行；
4. 记录成功、失败原因、事件和 Score Delta；
5. 结算轮末 Field、Score 与 State Hash。

在 `Q-R-ACTION-01` 确认前，MVP 推荐解析时拒绝包含多个 Major Action 的整个 Command Stream，因为这最容易暴露 Client Bug；确定后应把行为写入协议兼容测试。

### P1-FIELD 增量维护

创建 Link `(A,B)` 时求 `Neighbors(A) ∩ Neighbors(B)`，为每个共同邻居生成 Field。Portal Neutralize 时删除 Incident Links，再同步删除失效 Field。增量结果必须用小规模全量重算测试作对照。

### P1-REPLAY 最早建立 Replay

从 P1 起记录：

| 位置 | 必需内容 |
|---|---|
| Header | game_seed、完整 config、map、Server / 协议 / RNG 版本 |
| Per Round | Observation 版本、提交动作、最终执行顺序、校验结果、生成事件、Score Delta、State Hash |

自动测试必须证明 `Replay + Same Server Version → Same Final Hash`。

### P1-GATE 完成门槛

两个脚本 Bot 可在单进程按全部 v0.1 规则稳定完成 `TOTAL_ROUNDS`；非法动作可定位；Replay 能无偏差重放。

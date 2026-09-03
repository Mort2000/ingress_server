---
status: accepted
---

# R2 策略组合与测量工具就绪门槛（B2）

B2 交付 `docs/ai-balance-plan.md` 第 6.1 节的六个 R2 策略，并冻结第 10.1 节"测量工具就绪门槛"的统计定义与阈值。本 ADR 在官方就绪批次运行之前冻结以下内容；探索性 pilot 使用独立 experiment-id，不作为门槛证据。

## 策略标识（扩展 ADR-0015）

`PolicyId` 新增七个变体，tag 与版本固定为 `r2.0`，全部走 ADR-0015 的只增不改语义：

| policy_id | family | 类别 | bot-RNG kind-tag |
|---|---|---|---|
| `greedy-coordinated-r2` | `field-builder` | 竞争 | 6 |
| `builder-defender-r2` | `defender` | 竞争 | 7 |
| `builder-disruptor-r2` | `disruptor` | 竞争 | 8 |
| `logistics-builder-r2` | `key-runner` | 竞争 | 9 |
| `blocker-builder-r2` | `blocker` | 竞争 | 10 |
| `adaptive-hybrid-r2` | `adaptive-hybrid` | 竞争 | 11 |
| `idle` | `idle` | 诊断 | 12 |

R2 策略不是 `Bot` 实现：冻结的 `Bot` trait 要求 `kind() -> BotKind`（五个变体，只读）。新最小 trait `R2Policy`（`choose_spawns` + `submit_round`，无 kind/version——身份全部由 `PolicySpec` 承载）位于 `arena_bots::r2`；`make_policy_player` 构造 `PolicyPlayer { Frozen(委托未改动的 make_bot), R2 }`，执行器只面向 `PolicyPlayer`。`PolicyId::bot_kind()` 变为 `Option<BotKind>`（在五个 m6 镜像上全域单射，由测试钉死）；`PolicyId::MIRRORED: [PolicyId; 5]` 保留冻结 m6 注册表供 `frozen_m6_specs()` 与 lockstep 测试使用。

## 结构化参数

每个策略的阈值全部进入 `PolicySpec.parameters`（第 6.3 节，无处散落常量）：

- 六个建场系策略共享 `CoreParams`（`nearequal` / `retry` / `fbpri` + 内联完整 `ValueParams`），其 overlay 各自追加类型化参数（defender 的防守加成、disruptor 的攻击份额、logistics 的运送保留、blocker 的阻断阈值、adaptive 的阶段阈值）。
- `PolicyParameters::tag()` 为**全值规范序列化**：变体前缀 + `|` + 该策略参数的 `key=value;...` 规范串（`ValueParams` 以 `value=` 内联）。`from_tag` 严格解析——未知键、缺键、乱序、非十进制值一律拒绝（身份敏感：改过参数值的记录解析不回原 spec）。
- 诊断 `idle` 无参数。

## Readiness 阶段

`Stage::Readiness`（"readiness"）加入 `ExperimentPlan`：与 `Ablation` 相同的默认配置约束——任何 variant 声明或实际改变配置字段即拒绝（belt-and-braces）。就绪批次只跑 Development 地图 + `Flat(0)`，不触碰 Holdout。

## 就绪门槛统计定义与阈值（冻结于官方运行之前）

门槛评估是**纯函数**，输入为 `BalanceMatchRecord`（加阻断分析的 replay + 归档 Map Definition），输出只含整数原始计数与 permille，不进入 Game Result、Replay Hash 或规则结算（第 9.3 节）。除注明外，分母为该集合内非 crash（含 config-failure 剔除后）的比赛数：

1. **可持续得分**：某策略"得分场次"= 其 `score2 > 0` 的非 crash 场次，"建场场次"= 其 `fields_created >= 1` 的非 crash 场次。通过条件：round-robin 中满足（得分场次占比 >= 90% 且建场场次占比 >= 80%）的竞争策略 >= 4 个，且它们覆盖 >= 3 个竞争族。
2. **Idle 对手得分**：每个 R2 竞争策略 vs `idle`：非 crash 场次 100% 满足 `score2 > 0` 且 `fields_created >= 1`。
3. **Transfer 使用**：round-robin 中存在 >= 1 个策略，其 `transfers > 0` 的非 crash 场次占比 >= 50%。
4. **主动 Blocking**（探针批次，全部腿留档 replay）：通过条件：`blocker-builder-r2` 在 >= 10% 的非 crash 探针场次中有 >= 1 次"主动阻断"，且总量 >= 5。
5. **Command Drop**：竞争性场次（两侧均为竞争策略）合并 `commands_dropped / commands_submitted < 50 permille`；超过时必须按 DropReason 分解（复用 drops.rs 机制）并修正测量偏差。
6. **Mirror 冲突**：所有 mirror 对局（同策略两侧）中，两侧 Initial Spawn Set 完全一致的单位数为 0（从 replay header 读取）；配对不对称以原始计数另行报告。
7. **记录完整性**：`matches.tsv` 每行的 policy version 与 parameters 与 `policies.tsv` 一致（档案自检）。

**主动阻断的几何代理定义**：己方 `LinkCreated` 事件，其闭线段与某"敌方候选对"的闭线段相交（共享端点不算）。敌方候选对 = 事件时刻双方阵营均敌方持有的两个 Portal，满足：距离 <= `MAX_LINK_DISTANCE`、该对上无存活 Link、闭线段上无第三个 Portal。Portal 归属时间线由 `PortalCaptured`/`PortalNeutralized` 事件 + 归档 Map Definition 重建；存活 Link 集由 `LinkCreated`/`LinkDestroyed` 事件重建。该代理是离线分析定义，只用于就绪证据。

## 记录通道的 tag 泛化

`MatchDescriptor`/`MatchRecord` 的 `bot_a`/`bot_b` 从 `BotKind` 泛化为 policy tag 字符串，使 R2 单元被如实记录。m6 批次仍写入与 `kind.tag()` 完全相同的字节——既有 `m6_matches_tsv_format_is_unchanged` 与 `runner_batch` 测试钉死其逐字节不变；`drops.rs` 在 m6 消费端经 `BotKind::from_tag` 映射回枚举。

## 不变式

- 五个冻结 `BotKind` 的行为、默认 `GameConfig`、既有 `runs/` 目录一概不动；新产物只写入新的 `runs/balance/<experiment-id>/`。
- R2 策略只读合法 Observation 与公开历史；敌方私有状态在类型上不可达。
- 六策略之外，Random/NearestPortal/DestroyAnchor 保持诊断分类，不进入竞争性汇总。

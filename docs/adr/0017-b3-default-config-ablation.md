---
status: accepted
---

# B3 默认配置消融：策略臂、实验与统计定义（ADR-0017）

B3 交付 `docs/balance/ai-balance-plan.md` 第 7.2 节（阶段 A）的默认配置消融报告：量化 m6.0 Hybrid 100% 支配（以及 B2 记录的 `builder-disruptor-r2` 强支配）的来源。本阶段不触碰任何规则参数（`Stage::Ablation` 已拒绝任何配置差异，belt-and-braces 见 `plan.rs` `validate()`），不修改默认 `GameConfig`，不改五个冻结 `BotKind` 的行为，不写任何既有 `runs/` 目录。本 ADR 在官方运行之前冻结消融策略臂、实验计划与第 9 节统计定义（第 14 节注：统计定义与策略身份/版本语义的扩展必须先有 ADR）。

## 消融策略臂（扩展 ADR-0015/0016，只增不改）

三个新 `PolicyId`，全部归入新族 `PolicyFamily::Ablation`（`is_diagnostic() == true`——测量臂永不进入竞争性汇总）：

| policy_id | family | version | 参数 | 机制 |
|---|---|---|---|---|
| `hybrid-builder-only` | `ablation` | `b3.0` | `HybridAblationParams { roles: BuilderOnly, builder_count: 3, hack_supply_floor: 3 }` | 冻结 Hybrid 的建造路径：每个 Agent（增援含）都走 builder 路径，各自带 observation-index 偏移错峰；无攻击、无防御 |
| `hybrid-builder-attack` | `ablation` | `b3.0` | `HybridAblationParams { roles: BuilderAttack, builder_count: 3, hack_supply_floor: 3 }` | 冻结 trio/rest 划分 + 供给下限 Hack 前奏 + 攻击体，**无防御分派分支**；防御永不触发时与冻结 Hybrid 逐字节相同（可执行 pin 于测试） |
| `greedy-coordinated-canonical` | `ablation` | `r2.0` | `PolicyParameters::GreedyCoordinated(CoreParams { near_equal_permille: 0, spawn_jitter_cm: 0, ..CoreParams::default() })` | 即 r2 协同核心：`near_equal_permille = 0` 只保留与最优值完全并列的候选（`sample.rs` 语义），`spawn_jitter_cm = 0` 出生点恰在三角顶点上——canonical（确定性）出生，作 plan 7.2 第 5 行 "canonical spawn vs seeded top-K spawn" 的对照 |

- 两个 Hybrid 臂只由既有公开/pub(crate) 部件组合（`FieldPlanner::agent_commands`、`attack_targets`/`attack_commands`、`HackLedger`/`CommandList`），`choose_spawns` 与冻结 Hybrid 的 seed-triangle 出生完全一致（ladder 各步只差角色）。零文件改动于八个冻结 bot 文件。
- Hybrid 臂不消耗随机流（同冻结 Hybrid 的未用 `_rng`），无 stream；`greedy-coordinated-canonical` 使用新 `R2Stream::GreedyCoordinatedCanonical`，kind-tag **13**（`rng.rs` 冻结规则 "future policies append bytes >= 13"）。它仍在 exact-tie 集合上均匀抽样（从自己的流抽取）——permille 0 下通常唯一最优，即 canonical 选择。
- `PolicyParameters::HybridAblation` 全值序列化：`hybrid-ablation|roles=<builder-only|builder-attack>;builders=3;hacksupply=3`，严格解析（身份敏感）。
- `PolicySpec::validate` 扩展：`HybridAblation` 只属版本 `b3.0` 的两个 Hybrid 臂；`GreedyCoordinated` 参数在 `greedy-coordinated-canonical` 上合法（`greedy-coordinated-r2` 带改值参数仍合法——参数即数据）。

## 冻结实验

| experiment id | 内容 | 地图 | 场次 |
|---|---|---|---|
| `b3-ablation-matrix-r1` | 8 策略 round-robin（含 mirror）：`greedy-field`(m6)、`hybrid`(m6)、`greedy-coordinated-r2`、`hybrid-builder-only`、`hybrid-builder-attack`、`greedy-coordinated-canonical`、`adaptive-hybrid-r2`、`builder-disruptor-r2`——28 互异对 + 8 mirror = 36 对阵 | Development 100 × 2 腿 | 7,200 |
| `b3-ablation-idle-r1` | 三个新臂 vs `idle`（plan 6.3 无对抗得分检查） | Development 50 × 2 腿 | 300 |
| `b3-drop-reasons-matrix-r1` | 矩阵档案的按原因 Drop 重执行统计（范围：`greedy-field` 与 `greedy-coordinated-r2` 所在的 cell） | —（读矩阵档案） | 重执行 |

阶段 `ablation`、种子集 Development、`Flat(0)`、单一 `default` variant。pilot 追加 `-pilot-r0`，地图数缩为 matrix 8 / idle 4，其余不变。replay 采样（矩阵）：每对阵 leg-0 全留档 + 8 个 mirror 对在前 25 图双腿留档；idle 不留档。config/map 生成失败记为 failure，绝不静默跳过（B1 执行器语义）。

## 第 9 节统计定义（冻结；全部为非权威离线分析）

输入为 `BalanceMatchRecord[]`（+ 留档 replay + 归档 Map Definition），输出只含整数原始计数与 permille（f64 仅在计算 Wilson 时使用，产物一律 floor/ceil 换算为保守 permille 整数），不进入 Game Result、Replay Hash 或规则结算。

1. **W-D-L**：每对阵 policy_a 胜/平/负原始计数 + crash/config-failure 计数；胜率区间为 `W/(W+L)` 的 **Wilson 95%**（z = 1.96），下界 floor、上界 ceil 到 permille（保守取整只会加宽区间）；平局单独报原始数，不入分母。
2. **配对分差分位**：对每对阵的 `pairs.tsv` 单元（base variant）整数 `margin_sum` 升序排列，按 **nearest-rank** 取 p25/p50/p75：rank = ceil(p_permille·N/1000)（1-based）。同时报 min/max/N。
3. **Score Share**：每对阵双方 score2 之和的 permille 分享（合计为 0 时分享无定义，只报计数）；**双方均为零的比赛**单独报原始计数。
4. **P(win | leading@R50/100/200/300)**：每对阵、每个 checkpoint 的列联表原始计数——policy_a 领先且最终胜 / 领先且最终负 / 领先且平局 / checkpoint 平局（tie rate 原始计数）。
5. **First Field / Field Area / Lifetime**：每策略池化原始和——`first_field_round`（只对有值场次求和，计数并列）、`field_area2_sum`、`lifetime = destroyed_lifetime_rounds_sum + surviving_lifetime_rounds_sum`、`fields_created`；派生均值打印在计数旁。
6. **Drop**：每策略池化 submitted/ok/dropped 原始计数（permille 旁边）；按 `DropReason` 分解见第 3 个实验（B0 的验证重执行模式推广到 balance cell：从行自身的 policy tag + 参数构造 `PolicyPlayer`，校验 final State Hash 与 per-faction submitted/ok/dropped，任何不匹配即中止且不写产物）。
7. **阵营不对称**：每对阵 ENL 胜 / RES 胜原始计数（faction bias 的直接证据）。
8. **Mirror 统计**：mirror 对双腿留档 replay 的 Initial Spawn Set 坐标集相同（碰撞）腿数 + mirror 单元双腿结果相同的单元数（outcome symmetry）。
9. **支配分解阶梯**（7 对照）：`greedy-field → greedy-coordinated-r2 → hybrid-builder-only → hybrid-builder-attack → hybrid` 五步 + 跨代总对照 `hybrid vs greedy-coordinated-r2`、`greedy-field vs greedy-coordinated-canonical`。

### 支配与第 11 节分类规则（机械化定义）

- **支配**：该对阵 `W/(W+L)` 的 Wilson 95% 下界（floor permille）≥ 900‰（对齐 plan 10.3 "95% 单侧置信下界仍高于 90%"）。
- 分类（对阶梯与矩阵应用 plan 第 11 节）：
  - `hybrid--greedy-field` 支配 且 `hybrid--greedy-coordinated-r2` 不支配 → **Bot 实现缺陷**：保留规则默认值，更新 Benchmark；
  - 存在策略对 ≥3 个互异对手同时支配（多个近似最佳响应仍单一支配）→ **规则风险**：继续参数实验（B4）；
  - `greedy-coordinated-canonical` mirror 出生碰撞 > 0 且 `greedy-coordinated-r2` mirror 碰撞 = 0 → **策略对称伪影**：修正 seeded tie-break（B2 的 spawn jitter 已是该行动；canonical 臂为测量而有意重建伪影）；
  - 其余（`hybrid--greedy-coordinated-r2` 仍支配但非上列）→ 残差按阶梯台阶归因于角色/出生，喂给 B4。

## 产物

每实验目录内 `analysis.txt`（上述全部统计的机器可重算文本，原始计数齐备）并折算内容 hash 入 `hashes.txt`；`docs/balance/balance-b3-ablation.md` 携带 manifest 摘要与全部产物内容 hash（plan 第 12 节，`runs/` 不跟踪）。

## 不变式

- 五个冻结 `BotKind`、默认 `GameConfig`、既有 `runs/` 目录一概不动；新产物只写新 `runs/balance/<experiment-id>/`。
- 消融臂只读合法 Observation 与公开历史。
- 冻结 Hybrid 的行为由既有 lockstep pin 继续钉死；`hybrid-builder-attack` 在防御永不触发的整场对局与之逐字节相同（新 pin）。

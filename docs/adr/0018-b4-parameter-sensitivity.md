---
status: accepted
---

# B4 参数敏感度：候选网格、地图机制变体与统计定义（ADR-0018）

B4 交付 `docs/ai-balance-plan.md` 第 7.3 节（阶段 B）的单变量参数敏感度：Pilot →（稳定者）Full →（必要者）交互实验，完成条件是"每个候选均有配对效应与副作用报告"。本 ADR 在任何官方运行之前冻结候选网格、策略与对阵名单、实验计划、地图机制变体语义与第 9 节统计定义（第 14 节注：冻结 seed、统计定义与默认 `GameConfig` 相关决定必须先有 ADR）。默认 `GameConfig` 一概不动（base variant 即未改动的 `GameConfig::default()`，config hash `0x9ead052219fa7ff3`）；五个冻结 `BotKind` 与八个冻结 bot 文件零改动；既有 `runs/` 目录只读。

## 候选网格（冻结；plan 7.3 原表）

七个参数的全部非默认档（`RECHARGE_COST` 只有一档）= 13 个候选 variant，外加 base。每个非 base variant 恰好声明一个变更字段（`Stage::Sensitivity` 由 `ExperimentPlan::validate` 强制）。候选值是 Pilot 起点，不是预设结论。

| variant_id | 字段 | 默认值 | 候选值 | 主要观察（plan 7.3） |
|---|---|---:|---:|---|
| `move-budget-160m` / `move-budget-240m` | `move_budget_cm` | 20 000 | 16 000 / 24 000 | First Field、地图覆盖、翻盘率、策略排序 |
| `max-link-1500m` / `max-link-2500m` | `max_link_distance_cm` | 200 000 | 150 000 / 250 000 | Field 面积、数量、Blocking、滚雪球 |
| `attack-damage-15` / `attack-damage-25` | `attack_damage` | 20 | 15 / 25 | 摧毁时间、攻击机会成本、Field 生命周期 |
| `recharge-amount-10` / `recharge-amount-20` | `recharge_amount` | 15 | 10 / 20 | 防守触发率、存续收益、Supply 消耗 |
| `recharge-cost-2` | `recharge_cost_supply` | 1 | 2 | 防守选择与经济约束 |
| `initial-supply-10` / `initial-supply-40` | `initial_supply` | 20 | 10 / 40 | 开局动作空间、Hack 需求、资源闲置 |
| `portal-count-80` / `portal-count-160` | `portal_count` | 120 | 80 / 160 | 几何机会、争夺密度、计算成本 |

测试逐字钉住上表（字段名 + 值 + 默认值 = 未改动的 `GameConfig::default()`）。

## 地图机制变体（对 ADR-0014 边界的修订）

ADR-0014 冻结了"跨变体共享同一 Map Definition 仅当变体不改 `generate_map` 读取的字段；`PORTAL_COUNT` 类变体（B4）无法共享，属配置失败"。该边界使 plan 7.3 的 `PORTAL_COUNT` 臂 100% 配置失败、不可测量，与 B4 完成条件冲突。修订如下（ADR-0014 的种子推导一概不动：map_seed 永不依赖 variant）：

- **六个地图定义字段**：`portal_count`、`portal_min_distance_cm`、`clustered_portal_ratio_permille`、`portal_cluster_count`、`map_width_cm`、`map_height_cm`（`generate_map` 实际读取的全部字段）。
- **地图机制变体**：声明（经 `validate_contrast` 证明的实际）变更任一地图定义字段的 variant，从**同一 map_seed**、在**自身 config** 下生成**自己的** MapDefinition（共同随机数：同一 seed 流，不同派生地图——该参数的机制就是地图机制本身）。配对键（`pair_id = <tag>#m<k:03>`）不变，按地图配对差仍然良定义；**报告必须声明此类配对差包含地图机制变化**（非纯规则对比）。
- **非地图机制变体**：保持 ADR-0014 的逐图字节共享验证；验证失败仍记为该 (variant, map) 的配置失败 cell。
- 地图机制变体的某张地图生成不可行时，该 (variant, map) 的全部 cell 记为 `config-failure`（留在结果集，绝不静默跳过，也绝不中止批次）；base variant 地图不可行仍属配置缺陷、中止批次。
- 归档：`maps/` 目录内容寻址收存地图机制变体自己的地图；`maps.tsv` 增加 `variant_id` 列（base 行 = base variant；非地图机制变体按构造与 base 同图，引用 base 行）。

## 冻结实验

所有实验 stage 如表、种子集 Development/Validation、`Flat(0)` bucket、策略参数固定为 `r2_spec()`（ADR-0016 全值参数，即 plan 7.3"固定 R2 策略参数"条款）。Pilot 追加 `-pilot-r0` 后缀并只缩减地图数（plan 8.3：绝不缩减换边与地图覆盖；若吞吐不足只裁无信息量对阵并记录）。

**对阵名单（六对阵，冻结）**：围绕 B3 标记的规则风险支配（`builder-disruptor-r2` 7×200-0-0）与跨族覆盖选取；≥3 独立对阵满足 plan 10.3。全部为竞争性策略对，无 mirror（阵营偏差由换边双臂覆盖）、无 m6 基线（跨代桥接由 B3 ladder 建立）：

| matchup tag | 构成 |
|---|---|
| `greedy-coordinated-r2--builder-disruptor-r2` | 协同建造 vs 干扰 |
| `builder-defender-r2--builder-disruptor-r2` | 防御 vs 干扰 |
| `builder-disruptor-r2--adaptive-hybrid-r2` | 干扰 vs 自适应 |
| `greedy-coordinated-r2--builder-defender-r2` | 协同建造 vs 防御 |
| `logistics-builder-r2--blocker-builder-r2` | 后勤 vs 阻断 |
| `greedy-coordinated-r2--logistics-builder-r2` | 协同建造 vs 后勤 |

| experiment id | stage | 内容 | 地图 | 场次 |
|---|---|---|---|---|
| `b4-sensitivity-pilot-r1` | sensitivity | base + 全部 13 候选 × 6 对阵 × 2 腿 | Development 100 | 16 800 |
| `b4-sensitivity-full-dev-r1` | sensitivity | base + 稳定候选（见稳定规则）× 6 对阵 × 2 腿 | Development 300 | ≤ 稳定数×3 600 |
| `b4-sensitivity-full-val-r1` | sensitivity | 同上 variant 集 | Validation 100 | ≤ 稳定数×1 200 |
| `b4-interaction-<a>-<b>-r1` | interaction | `[base, A, B, A+B]` × 6 对阵 × 2 腿 | Development 100 | 4 800 |

- replay 采样（全部 B4 实验）：每 (variant, pair) 前 25 张地图的 leg-0（B3 的 25 图先例；Pilot = 14×6×25 = 2 100 份）。
- 注册 gate（plan 10.2/10.1）：`records-integrity`（readiness 层 gate 7 复用）、`no-crashes`、`command-drop`（< 50‰）。
- Full 的 variant 集是**派生**的：由 pilot 数据经下述稳定规则机械得出（`stable_candidates` 是 pilot 数据进入 Full 名单的唯一桥梁），并在报告中冻结记录。Full 重跑全部地图而非并入 pilot 行（自包含归档；跨实验确定性已由 B3 -r3 重发证明）。
- 交互实验只对两个因子均有稳定主效应的参数运行，优先级 = plan 7.4：`move_budget_cm × max_link_distance_cm`、`attack_damage × recharge_amount`、`attack_damage × recharge_cost_supply`、`portal_count × max_link_distance_cm`。同参数两档皆稳定时，取 |合并 margin_delta 和| 较大的一档（并列取 variant_id 字典序小者）。交互实验 id 的 `<a>`/`<b>` 用短码：`mb`、`mld`、`ad`、`ra`、`rc`、`is`、`pc`。

## 第 9 节统计定义（冻结；全部为非权威离线分析）

输入为该实验自己的 `matches.tsv`/`pairs.tsv` 行（+ `maps/` 归档地图），输出只含整数原始计数与 permille（f64 仅在 Wilson 计算内部使用，一律 floor/ceil 成保守 permille 整数），不进入 Game Result、Replay Hash 或规则结算（plan 9.3）。Wilson 95% 与 nearest-rank 分位沿用 ADR-0017 的冻结定义。**逐 (variant, pair) 键控，绝不跨 variant 池化**（B3 的 `pair_summaries` 池化跨 variant，不可复用）。

1. **按地图配对效应**（plan 9.1）：对每 (variant, pair)——`margin_delta_vs_base` 的 up/down/zero 符号计数、`outcome_delta_vs_base` 符号计数、delta 和、nearest-rank 四分位（p25/p50/p75，附 min/max/N），以及方向区间 = up/(up+down) 的 Wilson 95%（整数计数旁打印，draw/zero 不入分母）。参与符号计数与分位数的单元要求该 variant 行与 base 行双腿均 played（legs=2、crashes=0、config_failures=0）；被排除单元按 crash/config-failure 类别单独报原始计数。
2. **地图聚类会计**：同图重复单元（6 对阵共享每张图）绝不计为独立地图；每 variant 报 units（=对阵×地图）与 maps 两个整数。
3. **固定 seed 按地图 bootstrap**（plan 9.3 重尾不确定性）：对 variant 的合并 `margin_delta` 分布，**按地图索引有放回重抽样**——一次抽中的地图携带其全部 pair-delta（保持图内相关）；重复次数冻结 10 000；每次重抽的统计量为重抽 delta **和**（整数）；输出 nearest-rank p025/p500/p975；种子 = `LE-u64(blake3("ingress-arena-r2-b4boot" || experiment_id || variant_id))`，域隔离、绝不从数据派生；算法版本串 `b4-map-clustered-bootstrap-v1` 随输出回显。
4. **副作用护栏**（plan 10.3，每候选 vs base、跨对阵合并）：双方零分（no-score）比赛数、draw 数、crash/config-failure 数（须为 0）、合并 drop permille（对冻结 < 50‰ 带判定，原始分子分母并列）、ENL/RES 胜场分（阵营偏差）、机制利用率 delta——transfers/recharges/attacks/attack_hits/hacks/links_created/fields_created/first-field 场数与轮次和/field lifetime（destroyed+surviving 轮次和），全部为该 variant 自身 played 记录两阵营池化的 `FactionMatchStats` 整数和，permille 只打印在计数旁。
5. **几何分层**（唯一冻结分层，喂 plan 11"地图交互"行）：特征 = 每张归档地图的"门户最近邻距离中位数"（对每 portal 取最近邻距离 cm 的 floor 整数，再 nearest-rank 中位）；切点 = base 地图特征的 nearest-rank p333/p667（三等分，越界钳入首/末层）；地图机制变体用自己的归档地图、base 切点归类。每 (variant, stratum) 报配对方向计数（up/down/zero）。
6. **稳定规则**（机械化，pilot→Full 的唯一桥梁；plan 7.1"效应方向稳定且有实际意义"）：某候选**稳定** := (a) 该 variant 零 crash 且零 config-failure；(b) 该 variant 合并 drop permille < 50；(c) ≥3 个对阵的 margin 方向 Wilson 95% 区间整体在 500‰ 一侧（lower>500 或 upper<500）且这些对阵方向一致；(d) 合并 margin_delta 的 nearest-rank 中位数 ≠ 0。任一不满足即"停止该方向并记录"（plan 7.1 else 分支），不进入 Full。
7. **交互差分中的差分**（plan 7.4 反转读出）：对每 (pair, unit)，`DiD = delta(A+B) − delta(A) − delta(B)`（margin 与 outcome 两个整数序列，均以 vs-base 配对差为输入）；每 pair 报 DiD 符号计数。**反转**判定（冻结）：条件方向 = `delta(A+B) − delta(B)`（在 B 之上加 A 的效应）的 up/(up+down) Wilson 95% 整体在一侧，且与 A 自身主效应方向（同一交互批次内 delta(A) 的同型区间方向）相反。纯加和效应 → DiD 全零、无反转。

## 产物

每实验目录内 `effects.txt`（上述全部统计的机器可重算文本 + gate 判定，原始计数齐备）并折算内容 hash 入 `hashes.txt`；`docs/balance-b4-sensitivity.md` 携带各实验 manifest 摘要与全部产物内容 hash（plan 第 12 节，`runs/` 不跟踪）。若没有任何参数稳定，诚实产物就是带逐方向停止记录的 pilot/full 报告——完成条件同样满足（每候选均有效应 + 副作用报告与停止记录）。

## 不变式

- 默认 `GameConfig`、五个冻结 `BotKind`、八个冻结 bot 文件、arena-core、既有 `runs/` 目录一概不动；新产物只写新 `runs/balance/<experiment-id>/`。
- 统计只读合法 Observation 产物（归档记录与地图），不触碰任何权威路径。
- 地图机制修订只影响 B4 起的新实验；ADR-0014 冻结的种子推导与既有实验的可复现性不受影响（B1-B3 计划不含地图定义字段，执行路径逐字节不变，由既有 pin 保证）。

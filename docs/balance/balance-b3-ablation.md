# B3 默认配置消融报告（plan §7.2 / ADR-0017）

- 状态：Accepted（7,500 场官方批次 0 crash、0 config failure；记录完整性 15,000/15,000；Drop 重执行 3,000/3,000 验证一致）
- 官方运行：`runs/balance/b3-ablation-matrix-r3/`、`runs/balance/b3-ablation-idle-r3/`、`runs/balance/b3-drop-reasons-matrix-r3/`（base commit `c7aa6777193a742c0f08421f2ae89fe5143f66b7`，12 workers，release；`-r1`/`-r2` 目录为被 `analysis.txt` 两个渲染/聚合缺陷否决的首次尝试，原样保留，见过程记录 1-2）
- 日期：2026-09-04

本报告为 `docs/balance/ai-balance-plan.md` 第 14 行 B3 里程碑（阶段 A 默认配置消融）的验收记录：量化 m6.0 Hybrid 100% 支配（以及 B2 记录的 `builder-disruptor-r2` 强支配）的来源。**未触碰任何规则参数**（`Stage::Ablation` 拒绝一切配置差异，plan §7.2；默认 `GameConfig` 全程不变，config hash `9ead052219fa7ff3`），五个冻结 `BotKind` 行为与既有 `runs/` 历史产物不动。三个测量臂（`hybrid-builder-only` / `hybrid-builder-attack`，版本 `b3.0`；`greedy-coordinated-canonical`，版本 `r2.0` + 流 kind-tag 13，族 `ablation`，诊断类）在官方运行之前由 ADR-0017 冻结；`hybrid-builder-attack` 在防御分派永不触发的整场对局与冻结 HybridBot **逐字节相同**（可执行 pin 于 `crates/arena-bots/tests/ablation_fixtures.rs`）。

## 结论（支配来源分解）

同图换边矩阵（每对阵 100 Development 地图 × 2 腿 = 200 场决定性对局，相同 map/game/bot seed，`Flat(0)`）把 m6.0 Hybrid 的支配分解为：

1. **Greedy m6.0 的自身竞争缺陷解释了它与一切建场者的差距，但不是 Hybrid 支配的来源**：协同修正后的 `greedy-coordinated-r2` 对 `greedy-field` 200-0-0（Wilson95 下界 981‰，支配）——己方错峰与任务分配修好了 Greedy；**但 Hybrid 对 `greedy-coordinated-r2` 仍是 200-0-0（支配）**。plan §7.2 的"协同修正显著消除支配 ⇒ Bot 实现缺陷"分支**不成立**。
2. **Hybrid 的支配来自攻击角色，不是建造、不是防御调度、不是出生**：
   - 阶梯第 2 步：`greedy-coordinated-r2` 对 `hybrid-builder-only` 197-3-0（956..995，支配）——Hybrid 的建造路径（seed-triangle 出生 + 冻结 FieldPlanner）单独拿出来**弱于**协同核心；
   - 第 3 步：`hybrid-builder-attack` 对 `hybrid-builder-only` 200-0-0（支配）——攻击角色的边际贡献是全部；
   - 第 4 步：冻结 Hybrid（= builder+attack+防御分派）对 `hybrid-builder-attack` 98-102-0（421..559，非支配）——**Recharge 防御分派的边际贡献 ≈ 0**（冻结 Hybrid 行为上就是 builder+attack）；
   - 第 5 步（出生对照）：`greedy-coordinated-canonical`（确定性出生）对 `greedy-coordinated-r2`（seeded top-K）112-88-0（490..628，非支配）——出生确定性对正面胜负无可检出的效应；其效应全部体现在 mirror 伪影（见下）；
   - 第 6 步（角色对照）：冻结 Hybrid 的固定角色 199-1-0 击败 `adaptive-hybrid-r2` 的动态角色切换——动态切换也不解释支配。
3. **同一攻击 overlay 加在 r2 核心上支配一切**：`builder-disruptor-r2` 对全部 7 个互异对手 7×200-0-0（每对 Wilson 下界 981‰，支配）——含 Hybrid（200-0-0，中位分差 +12.3e12）与 `hybrid-builder-attack`。攻击/破坏在默认参数下对每个建场者都是全胜策略。
4. **Plan §11 机械化分类**（domination = W/(W+L) 的 Wilson95 下界 ≥ 900‰，ADR-0017）：
   - 行 1（协同修正消除支配）：**未消除**（Hybrid 对 gc-r2 仍支配）→ 残差按阶梯归因于角色/出生，喂给 B4；
   - 行 7（多个近似最佳响应仍单一支配）：**触发（规则风险）**——Hybrid 支配 5 个互异对手、`builder-disruptor-r2` 支配 7 个；后续动作：继续参数实验（B4，优先 `ATTACK_DAMAGE`/`RECHARGE_*` 方向），参数空间无解时再讨论机制变更；
   - 行 6（mirror 偏差仅在确定性出生下出现）：**触发（策略对称伪影）**——canonical 臂 mirror 出生冲突 125/125 腿 vs gc-r2 0/125；修正动作即 B2 已落实的 seeded tie-break（本臂为测量有意重建该伪影）。

按 plan §7.1 总体流程：**支配未被协同修正消除 ⇒ 不据此修正或降级策略，进入 B4 单变量敏感度**；Hybrid 100% 支配的"主要来源"答案是：Hybrid 的攻击角色 × 默认规则参数（规则风险），而非 Greedy 的实现缺陷（它只是另一个受害者）或 Hybrid 的建造/防御/出生。

## 实验设计（冻结于官方运行之前，ADR-0017）

| 实验 | experiment id | 内容 | 场次 | 用时 |
|---|---|---|---|---|
| 消融矩阵 | `b3-ablation-matrix-r1`→`-r3` | 8 策略 round-robin（28 互异 + 8 mirror = 36 对阵）× 100 Development 图 × 2 腿 | 7,200 | 1,921.666 s |
| Idle 基准 | `b3-ablation-idle-r1`→`-r3` | 3 个新臂 vs `idle` × 50 图 × 2 腿 | 300 | 3.312 s |
| Drop 重执行 | `b3-drop-reasons-matrix-r3` | 矩阵档案中 `greedy-field` 与 `greedy-coordinated-r2` 所在的全部 15 对阵（3,000 行）逐场验证重执行 | 3,000 重执行 | 494.202 s |

三批 0 crash、0 config failure；replay 采样 = 每对阵 leg-0 全留档 + 8 个 mirror 对前 25 图双腿留档（3,800 份）；记录完整性（family/version/parameters 对 `policies.tsv` 逐行）14,400/14,400（矩阵）+ 600/600（idle）。

矩阵策略（冻结顺序）：`greedy-field`(m6.0)、`hybrid`(m6.0)、`greedy-coordinated-r2`、`hybrid-builder-only`、`hybrid-builder-attack`、`greedy-coordinated-canonical`、`adaptive-hybrid-r2`、`builder-disruptor-r2`——后两者覆盖 §7.2 角色行与 B2 标记的支配者。

## §7.2 六对照（上侧计数；n=200 决定性对局）

| 对照 | 对阵（上侧 vs 下侧） | W-L-D | Wilson95 (‰) | 支配 |
|---|---|---|---|---|
| 1 己方错峰/任务分配 | greedy-coordinated-r2 vs greedy-field | 200-0-0 | 981..1000 | 是 |
| 2 出生与 FieldPlanner | greedy-coordinated-r2 vs hybrid-builder-only | 197-3-0 | 956..995 | 是 |
| 3 攻击角色边际 | hybrid-builder-attack vs hybrid-builder-only | 200-0-0 | 981..1000 | 是 |
| 4 Recharge 调度边际 | hybrid vs hybrid-builder-attack | 98-102-0 | 421..559 | 否 |
| 5 canonical vs seeded 出生 | greedy-coordinated-canonical vs greedy-coordinated-r2 | 112-88-0 | 490..628 | 否 |
| 6 固定 vs 动态角色 | adaptive-hybrid-r2 vs hybrid | 1-199-0 | 0..28 | 否（反向支配） |

支配分解阶梯（greedy-field → gc-r2 → builder-only → builder-attack → hybrid + 跨代总对照）：

| 台阶 | W-L-D | Wilson95 (‰) | 支配 |
|---|---|---|---|
| 1 gc-r2 vs greedy-field | 200-0-0 | 981..1000 | 是 |
| 2 gc-r2 vs hybrid-builder-only | 197-3-0 | 956..995 | 是 |
| 3 hybrid-builder-attack vs hybrid-builder-only | 200-0-0 | 981..1000 | 是 |
| 4 hybrid vs hybrid-builder-attack | 98-102-0 | 421..559 | 否 |
| 跨代 hybrid vs gc-r2 | 200-0-0 | 981..1000 | 是 |
| 跨代 gc-canonical vs greedy-field | 200-0-0 | 981..1000 | 是 |
| 跨代 hybrid vs greedy-field（m6 结论复现） | 200-0-0 | 981..1000 | 是 |

`builder-disruptor-r2` 行（规则风险的直接证据）：对 greedy-field / hybrid / gc-r2 / hybrid-builder-only / hybrid-builder-attack / gc-canonical / adaptive-hybrid-r2 全部 200-0-0。

## Mirror 与阵营不对称（原始计数）

| mirror 对 | 留档腿数 | 出生冲突腿 | 单元数 | 双腿同结果单元 | ENL/RES 胜 |
|---|---|---|---|---|---|
| greedy-coordinated-canonical | 125 | **125** | 100 | 100 | 152/48 |
| greedy-field（m6） | 125 | 125 | 100 | 100 | 142/58 |
| hybrid（m6） | 125 | 125 | 100 | 100 | 140/60 |
| hybrid-builder-only | 125 | 125 | 100 | 100 | 144/56 |
| hybrid-builder-attack | 125 | 125 | 100 | 100 | 150/50 |
| greedy-coordinated-r2（seeded） | 125 | **0** | 100 | 100 | 108/92 |
| adaptive-hybrid-r2（seeded） | 125 | 0 | 100 | 100 | 124/76 |
| builder-disruptor-r2（seeded） | 125 | 0 | 100 | 100 | 82/112（6 平） |

确定性出生（m6 系 + canonical 臂）的 mirror 100% 首轮出生冲突并带强 ENL 偏置（`Flat(0)` 先手，+40..+52/200）；seeded top-K + 出生抖动（B2 修正）把冲突降为 0、ENL 偏置收敛（三个 seeded mirror：gc-r2 108/92、adaptive 124/76、disruptor 反向 82/112，|偏置| ≤30/200）。mirror 单元双腿结果 100% 同阵营胜（对称性本身符合预期，按原始计数如实记录）。

## 滚雪球（P(win | leading@R100)，原始计数 won/lost/drew，tie 另计）

| 对阵 | R100 列联 | P(win\|led) |
|---|---|---|
| hybrid vs greedy-coordinated-r2 | 3/0/0（hybrid 在 R50/R100 仅 3 场领先、leader 零平局——其余 197 场的领先者是 gc-r2 且全部落败；R200 132/0/0，R300 199/0/0） | 1000‰ |
| greedy-coordinated-r2 vs hybrid-builder-only | 192/2/0 | 989‰ |
| hybrid-builder-only vs hybrid-builder-attack | 0/100/0（反向 100/0/0） | 0‰ / 1000‰ |
| builder-disruptor-r2（对全部对手） | 领先即全胜 | 1000‰ |

R100 领先的预测力按对局类型分裂（上表均为上侧计数，原始证据在 `analysis.txt` checkpoint 表）：建场者互角与 `builder-disruptor-r2` 参与的对局触发 plan §10.3 的 >90% 硬告警带（gc-r2 对 hybrid-builder-only 989‰；disruptor 领先即全胜）；而含攻击 overlay 的对局恰好相反——R100 领先者（建场方）几乎必败（hybrid 对 gc-r2 时 gc-r2 在 R100 领先 197/200 却 0 胜；对 gc-canonical 0/189、对 adaptive-hybrid-r2 1/188 同型）。全矩阵非 mirror 合并 P(win|led@R100) = 4,003/5,600（715‰）——默认参数下没有普适的 R100 锁定，但被支配方同样没有翻盘路径，与"规则风险"分类一致，喂给 B4。

## 机制与 Drop（整数原始计数）

池化机制（每策略 1,800 个 faction slot = 7 对手 × 200 + mirror 双侧 400；矩阵全量）：`greedy-coordinated-r2` first-field 均值 35 轮、29,067 场次建场、Drop 0‰；`greedy-field`(m6) 均值 93 轮（n=1,696）、11,260 场次、52‰；`hybrid` 与两臂 71 轮 / 11,939-14,845 场 / 5-8‰——首场轮次与 Drop 与冻结 Hybrid 同级（`hybrid-builder-only` 无攻击分派，建场数 14,845 反而高于 hybrid 的 12,061），建场量与 gc 核心差约 2 倍，差距来自建场协调而非出生；`greedy-coordinated-canonical` 38 轮 / 28,648 场 / 1‰（与 gc-r2 一致，进一步佐证出生对照无正面效应）。

Drop 按原因分解（`b3-drop-reasons-matrix-r3`：3,000 行验证重执行——每行从自身 policy tag + 全值参数重构 `PolicyPlayer`，final State Hash 与 per-faction submitted/ok/dropped 对档核验，任何不匹配即中止；per-reason 合计与档内 `commands_dropped` 逐策略对账 OK）：

| 策略 | 场次 | submitted | dropped (‰) | 主导原因（占其 dropped 的 ‰） |
|---|---|---|---|---|
| greedy-field | 1,600 | 4,500,000 | 236,552 (52) | CaptureTargetNotNeutral 144,179 (610)；LinkAlreadyExists 85,188 (360) |
| hybrid | 400 | 940,997 | 5,034 (5) | LinkAlreadyExists 3,145 (625)；CaptureTargetNotNeutral 1,642 (326) |
| hybrid-builder-only | 400 | 737,508 | 5,635 (7) | LinkAlreadyExists 3,945 (700) |
| hybrid-builder-attack | 400 | 941,895 | 4,948 (5) | LinkAlreadyExists 3,118 (630) |
| greedy-coordinated-r2 | 1,600 | 1,518,366 | 933 (0) | CaptureTargetNotNeutral 669 (717) |
| greedy-coordinated-canonical | 400 | 335,249 | 225 (0) | CaptureTargetNotNeutral 198 (880) |
| adaptive-hybrid-r2 / builder-disruptor-r2 | 各 400 | 565,640 / 863,536 | 153 / 205 (均 0) | CaptureTargetNotNeutral 为主 |

（Drop 范围 = 15 个含 greedy-field 或 gc-r2 的对阵；greedy-field/gc-r2 各 1,600 场，其余策略为其交手对。）池化 `analysis.txt` 机制表（矩阵全量 1,800 slot/策略）与该表独立一致：greedy-field dropped 236,552、gc-r2 933，逐数相同。B0 的 GD 病理（CaptureTargetNotNeutral 主导）复现：greedy-field 61% 的 Drop 仍是抢已被占领的 Portal；协同核心把 Drop 压到 ≤1‰。

## Idle 基准（plan §6.3 无对抗得分）

| 臂 | scored AND fielded | 场次 |
|---|---|---|
| hybrid-builder-only | 100 | 100 |
| hybrid-builder-attack | 100 | 100 |
| greedy-coordinated-canonical | 100 | 100 |

## 过程记录（诚实性）

1. **首次官方运行（`-r1`）被一个渲染缺陷否决**：`analysis.txt` 的池化机制表把 ok 与 dropped 两列连排成一列歧义文本（格式串漏分隔符）。原始逐场记录完全正确、7,200 场 0 crash——但 `analysis.txt` 是本里程碑的中心产物。修复渲染并以同一冻结计划重发 `-r2`。
2. **`-r1` 之上的 Drop 重执行在自身对账关口中止（未写任何产物）**：聚合把 leg-1 行的重执行 tally 配到了对手侧的档内计数。逐场验证（faction 对 faction）全程正确，对账交叉检查按设计拒绝不一致。修复按行自身 `policy_a_is_enl` 同时配对 tally 与档内计数，并新增 leg-1 行（两侧计数互异）测试。
3. **`-r2` 的机制表被原始数据复核否决**：mirror 记录只计入了 policy_a 一侧的 faction slot（gc-r2 submitted 1,350,223 vs 真值 1,518,366——后者由验证重执行独立确认）。修复为按行自身阵营配对汇入双侧 slot、列名改 `slots`，新增 mirror 行测试，重发 `-r3`。W-D-L、分位、Wilson、checkpoint、mirror、阶梯与分类不受影响（不经过该聚合）。
4. **三次重发的轨迹逐字段一致**：map/game/bot seed 不依赖 experiment id——`-r1`/`-r2`/`-r3` 的 7,200+300 行逐场结果、比分、final_state_hash 与全部统计列完全一致（仅 experiment_id / 由其派生的 schedule_seed / elapsed_ms 不同；`pairs.tsv`、`replays.tsv`、`maps/`、`configs.tsv`、`policies.tsv` 逐字节相同；两次 Drop 重执行的 `drop-reasons.tsv`/`summary.txt` 哈希相同）。`-r1`/`-r2` 目录按不覆盖历史原则保留。
5. Pilot 先行（`--pilot-r0`，8/4 图，scratch 目录，运行后删除）：576+24 场 0 crash、analysis 与 mirror 探针工作正常后才发起官方批次。
6. 全部统计为非权威离线分析（plan §9.3）：整数原始计数是记录，permille 与 Wilson 界限只是派生展示；`analysis.txt` 由 `matches.tsv`+`pairs.tsv`+replay 头机器可重算；无任何派生值进入 Game Result、Replay Hash 或规则结算。
7. **报告文本层的四处叙述错误在提交后的独立复核中被发现并更正（原始数据胜出，本条即更正记录；运行产物与 `analysis.txt` 从未有过这些错误）**：(a) mirror 表 `hybrid-builder-only` 的 ENL/RES 胜误写 104/96，档内真值 144/56（`analysis.txt` per-pair 表与原始 `matches.tsv` 一致）；(b) 机制段落把两臂建场数并入 "11,939-12,061 场" 区间——`hybrid-builder-only` 实际 14,845 场（区间只覆盖 hybrid 与 builder-attack）；(c) 滚雪球表 hybrid 行的括注 "R50 前几乎无人领先" 与档内事实相反——R50/R100 每场都有领先者（leader 零平局），197 场的领先者是 gc-r2（全部落败），hybrid 自己仅 3 场领先；(d) 结语句 "领先者在 R100 后几乎必胜、>90% 硬告警带全面触发" 过度概括——全矩阵非 mirror 合并 P(win|led@R100)=4,003/5,600（715‰），含攻击 overlay 的对局中 R100 领先者（建场方）几乎必败。所有表格中的 W-D-L、Wilson、分位、mirror 结构计数与 Drop 数字经逐项独立重算无误。
8. **归档 `report.md` / `manifest.txt` note 的 B1 模板残文（评审复核发现，本条即更正记录；逐场数据、`analysis.txt` 与各 manifest 的 `reproduce=` 行无恙）**：共享归档模板 `report_text` 把 B1 冒烟批的文案硬编码——matrix 与 idle 两实验的三轮归档（`-r1`/`-r2`/`-r3`，共六个目录；`-r1`/`-r2` 为被否决的历史尝试，见第 1-3 条）每个目录的 `report.md` 里：(a) Reproduce 行写作 `balance-b1`，照它会重跑 B1 冒烟实验而非本批（正确命令是 `balance-b3`，本文件"复现"节）；(b) Gates 节写作 "None registered — B1 …"，与同目录 `manifest.txt` 矛盾（`gates=2 registered`（matrix：records-integrity、no-crashes）/`gates=1 registered`（idle：idle-scoring）；`idle-scoring` 门实际 100/100 通过，matrix 0 crash / 0 config failure / 记录完整性 15,000/15,000，见 `analysis.txt`）；(c) 各 manifest 的 `note=` 指向 `docs/balance/balance-b1-smoke.md`——B3 报告与全部内容哈希的实际承载文件是本文件。历史 `runs/` 永不覆盖，这些误导性指针按缺陷原样冻结在档。**权威指针**：`manifest.txt` 的 `reproduce=` 行（三批均正确写 `balance-b3`）与本文件"复现"节；本文件"产物与内容哈希"节（3,910 matrix / 60 idle / 3 drop-reasons 条）与档内 `hashes.txt` 逐项核对一致。模板已改为按 stage 渲染（reproduce 子命令、Gates 节逐条列出计划注册的门并注明评估落点、note 指向各阶段承载文档），并由 `archive.rs::manifest_and_report_are_stage_aware`（smoke/readiness/ablation×2 四例逐项钉住子命令、门数与文档名）与 `runner_ablation.rs::b3_scratch_run_archives_analysis_and_refuses_existing_dirs`（端到端断言归档 report.md）钉住。B2 归档的 `report.md`/note 残留同源（其 manifest reproduce 行已先行修复），已在 `docs/balance/balance-b2-readiness.md` 过程记录第 6 条补记。

## 产物与内容哈希（plan §12；`runs/` 不跟踪）

**b3-ablation-matrix-r3**（replays 3,800 份 + maps 100 份；hashes.txt 3,910 条，自身 sha256 `3249349fc547b412806240ec21b8f9510e3462d309d77446e35a687cfba23e3f`）

```text
blake3:a17fff94742917a904f009261746351efb5d89577406325cbc08db5ea6564e54  configs.tsv
blake3:ba5fe310d5280e956ebddd41fb8045054a44430609e12c5d07b5251606566508  manifest.txt
blake3:8d7711605cbfd72e674dc9386eacf9f7d4c4e815ff4e21a778305e8dbe2bb57f  maps.tsv
blake3:c6ddabce61484bce7e464cf752e516f328d3aa8ae4a94e4f3e7330035c1591be  matches.tsv
blake3:d118f38b93695f06c3cc33a8368f277e858f0b5139dce8e7b4ad10eddec76a4d  pairs.tsv
blake3:841bf0f340b632e61a03f641c6e78bbec1dd2a85308230475f72b8ff8521141e  policies.tsv
blake3:df4eda6ac922ad9cc8e0168688a56e69985c68a312daba58ac7c2dc06a4594b2  replays.tsv
blake3:54f703b2a194b6fe1cb68c4cac3aa397298ad0a21f3b1dda8dd3a055921ef5f7  report.md
blake3:7ca4a7049c813c7c7f21b3abd808f27603210be993867c2a767814a94501b4d9  summary.txt
blake3:940836c34cbf65fa0fb25892396cc8764ef5970f6811ae897d3a84e43dbce5f2  analysis.txt
```

**b3-ablation-idle-r3**（无 replay；maps 50 份；hashes.txt 60 条，自身 sha256 `1a17009bf7bcb35775cd9c0e7cccb8138fb72c87fca34c298da64fbcd9e7ec24`）

```text
blake3:a17fff94742917a904f009261746351efb5d89577406325cbc08db5ea6564e54  configs.tsv
blake3:62a7427959e108ccf4f74fb3ced98f30a0e73b2df9da6ee0b0ce3466aa3512b2  manifest.txt
blake3:abe6090d8edc719f501a01c6ec958dd21a18e8dc30e69b7d46a93a9ed82fdaa5  maps.tsv
blake3:99f6aa640d7101e8d8508fb9d45ff746db2d1e89275a8441fb1f1250855079b1  matches.tsv
blake3:d47827ee5a7cd0a9af6272f739ee40bdefa9b34f159a396f73f6966230699d2b  pairs.tsv
blake3:5393fe438c46da409502b4b5b7f3702dc770b406a3fd5b4253ceb4b2e1c8d871  policies.tsv
blake3:e68e22719a38b3851662d443b1a9835db1937bd2a151084d15f2f39b6d121011  replays.tsv
blake3:34ab22e2618bd0383a36be3ee4fc865468810a38a59b2ebabeb8a1a81697655a  report.md
blake3:0e4cea32ff664f1e0383b7e699c6f860897e2d6c69c2d5e4ef2bcc09e2bef543  summary.txt
blake3:5f0787a5a624f7785e6c8a21e434c8bcb72e6c7edea9c4178faff201e74df3b0  analysis.txt
```

**b3-drop-reasons-matrix-r3**（hashes.txt 3 条，自身 sha256 `060682f5e3d864233212302f56c5b8927c23bfb22f12fd9574f61a8c420bb259`）

```text
blake3:be69a36d7fe305ac7cbca54373475194275943653171b46faf0b89657beb53d5  manifest.txt
blake3:26d02d8e2d7c5c68ada0e01c44d639da83a5cacc2940c5498d3232872907f8b2  drop-reasons.tsv
blake3:e6fdba8432b64ec37e0d7b21ee8706734504bf2af83870fb905999c0cc55ba8a  summary.txt
```

`configs.tsv` 两批相同（默认配置逐字）；每份 replay 与 map 的 blake3 见各目录 `hashes.txt`（`maps/` 内容寻址，`replays.tsv` 逐份带 hash）。

## 复现

```sh
cargo run --release -p arena-runner -- balance-b3 runs <base_commit>            # 官方两批（新目录；已存在则拒绝）
cargo run --release -p arena-runner -- balance-b3 runs <base_commit> --pilot    # 探索性小批（-pilot-r0）
cargo run --release -p arena-runner -- balance-drop-reasons runs/balance/b3-ablation-matrix-r3 runs <base_commit> greedy-field greedy-coordinated-r2
```

## 冻结不变式（核验）

- `git diff 8d506b5..HEAD -- crates/arena-bots/src/{bot,hybrid,greedy_field,destroy_anchor,nearest,random,common,rng}.rs crates/arena-core` 为空；默认 `GameConfig` 零改动（config hash `9ead052219fa7ff3` 与 m6 相同）。
- 既有 `runs/` 目录（含 `-r1`/`-r2`）零写入；新产物只在新的 `runs/balance/<experiment-id>/`。
- 消融臂只读合法 Observation 与公开历史；五冻结 BotKind 的 lockstep pin 全绿。

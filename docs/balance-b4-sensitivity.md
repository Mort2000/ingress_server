# B4 参数敏感度：单变量 Pilot 报告（阶段 B）

本文件承载 B4 单变量参数敏感度 Pilot 记录（`docs/ai-balance-plan.md` 7.3/8.3/9/10.3 节，ADR-0018）。`runs/` 不入库；按第 12 节，manifest 摘要与全部产物内容 hash 在此提交。全部统计为非权威离线分析：整数原始计数齐备，permille 仅为展示，不进入 Game Result、Replay Hash 或规则结算。

## 1. 设计（运行前冻结于 ADR-0018）

- **网格**：plan 7.3 的七个参数、十三个单字段候选（全部非默认档；`RECHARGE_COST` 只有一档），每个候选相对未改动的 `GameConfig::default()`（config hash `0x9ead052219fa7ff3`，测试钉住）恰声明一个变更字段。候选值是 Pilot 起点，不是预设结论。
- **名单**：六个冻结对阵 × `PolicyId::R2_COMPETITIVE`，固定 `r2_spec()` 参数（版本 r2.0、逐行全值记录；plan 7.3"固定 R2 策略参数"条款）；无 mirror（阵营偏差由换边双臂覆盖）、无 m6 基线（跨代桥接是 B3 的 ladder）。
- **配对**：每可比较单元（map, game_seed, bot_seed, pair, bucket policy）在全部 14 个 variant 下以相同单元种子打双腿换边（共同随机数，ADR-0014）。
- **地图机制臂**（`portal-count-80/160`，ADR-0018）：从同一 map_seed、在自身 config 下生成自己的地图；其配对差**包含地图机制变化**（该参数的机制即地图）。规则型臂逐图字节共享 base 地图（执行器验证）。
- **规模**：base + 13 候选 × 6 对阵 × 100 张 Development 地图 × 2 腿 = 16,800 场；replay 采样 = 每 (variant, pair) 前 25 图的 leg-0 = 2,100 份 IARP。
- **稳定规则**（pilot→Full 的唯一机械桥梁，ADR-0018）：零 crash/config-failure 且合并 drop < 50‰ 且（c）Wilson 95% 整体单侧的对阵中**同方向 ≥3 个且严格多于反方向**（3+3 平局无方向；反方向单侧对阵逐个记录为异议，plan 10.3 门槛非全体一致）且合并 margin_delta 中位数 ≠ 0。不满足即"停止该方向并记录"（plan 7.1 else 分支）。

## 2. 溯源（manifest 摘要）

| 项 | 值 |
|---|---|
| experiment id | `b4-sensitivity-pilot-r2`（stage `sensitivity`） |
| 种子集 | Development（master `0x0072326400000300`），100/300 图 |
| base variant | `default` = 未改动 `GameConfig::default()`，hash `0x9ead052219fa7ff3` |
| 变体 config hash | `move-budget-160m=0x725c766d85812289`、`move-budget-240m=0x3306905b65b4ce76`、`max-link-1500m=0xa2f58b577f71a639`、`max-link-2500m=0x5fdd0378cec52015`、`attack-damage-15=0x5fbe29ecdfb3d703`、`attack-damage-25=0x905b9a1a3de08b2b`、`recharge-amount-10=0xb2dc86ebdae39d15`、`recharge-amount-20=0x1eef6dfdbb37da65`、`recharge-cost-2=0x71261b591af2aa17`、`initial-supply-10=0x0af6d1e499b2d4fe`、`initial-supply-40=0x3e9cce3655b1929e`、`portal-count-80=0x6f04f035845cd012`、`portal-count-160=0x0e6d1ec71e52827e` |
| 策略 | 六个 R2 竞争策略 @ `r2_spec()`（版本 r2.0），逐行 family/version/parameters 记录 |
| 地图 | `maps.tsv` 301 行 = base 100 + portal-count-80 自有 100 + portal-count-160 自有 100（内容寻址去重） |
| 场次 | 16,800（12 workers，release，12,105.9 s）；replay 2,100 份 |
| base_commit | `411ad1a`（reproduce：`cargo run --release -p arena-runner -- balance-b4 runs 411ad1a`） |
| 种子推导 | map/game/bot/schedule 见 manifest（ADR-0014 公式；schedule_seed 含 experiment id，其余不含） |

## 3. 实验完整性门槛（`effects.txt` gates，原始分子/分母）

| gate | 计数 | permille | 判定 |
|---|---|---:|---|
| records-integrity | 33,600 / 33,600（14 variant × 16,800/1200 行 × 双方 family/version/parameters 对 `policies.tsv`） | 1000 | PASS |
| no-crashes | 0 / 16,800 | 0 | PASS（crash 0、config-failure 0） |
| command-drop | 31,552 / 40,296,424 | 0（<1‰，带内 <50‰） | PASS |

两个地图机制臂的各 100 张自有地图**全部生成**（0 config-failure；规则型臂逐图字节共享验证通过）。逐场数据可重算本报告全部汇总（关键数字已用独立脚本对 `pairs.tsv` 复核：8,400 行配对差、per-pair 符号计数与 Wilson 与 `effects.txt` 完全一致）。

## 4. 每候选配对效应与副作用（完成条件）

Pooled = 六对阵合并的 `margin_delta_vs_base` 符号计数（每候选 600 单元 = 6 对阵 × 100 图；每单元为同图换边配对差）。单侧(+/-) = Wilson 95% 整体在 500‰ 一侧的对阵数。全部候选：settled 1,200/1,200、both-zero 0/0、draw 0/0、crash 0、config-failure 0、drop < 1‰（原始分子/分母见 `effects.txt`），ENL/RES 分裂 619–646/554–581（base 自身 631/569）——无任何副作用护栏触发。

| 候选 | pooled up/down/zero | 单侧(+/-) | 合并中位 delta | 判定（停止原因逐字见 effects.txt） |
|---|---|---|---:|---|
| move-budget-160m | 335/265/0 | 2/1 | +2,579,715,331,759 | 停止（<3 同向单侧对阵） |
| move-budget-240m | 285/315/0 | 1/2 | −2,351,830,049,517 | 停止（同上） |
| **max-link-1500m** | 345/255/0 | **3**/2 | **+9,339,480,376,150** | **稳定（方向 +，2 个异议对阵已记录）→ 入 Full 名单** |
| **max-link-2500m** | 254/346/0 | 2/**3** | **−13,265,588,768,688** | **稳定（方向 −，2 个异议对阵已记录）→ 入 Full 名单** |
| attack-damage-15 | 182/118/300 | 2/1 | 0 | 停止（<3 同向 + 中位数=0；600 单元中 300 个 delta 恰为 0） |
| attack-damage-25 | 140/160/300 | 1/2 | 0 | 停止（同上） |
| recharge-amount-10 | 92/96/412 | 0/0 | 0 | 停止（同上；412/600 单元 delta=0） |
| recharge-amount-20 | 84/92/424 | 1/1 | 0 | 停止（同上） |
| recharge-cost-2 | 25/26/549 | 0/0 | 0 | 停止（同上；549/600 单元 delta=0） |
| initial-supply-10 | 7/17/576 | 0/1 | 0 | 停止（同上） |
| initial-supply-40 | 0/2/598 | 0/0 | 0 | 停止（同上；598/600 单元 delta=0——该参数在这些对阵中几乎从不改变轨迹） |
| portal-count-80 | 319/281/0 | 2/2 | +1,983,254,145,256 | 停止（<3 同向；含地图机制变化，ADR-0018 注记） |
| portal-count-160 | 290/310/0 | 1/2 | −1,347,871,954,043 | 停止（同上） |

**稳定臂细节**（全部原始计数在 `effects.txt`）：

- `max-link-1500m`：单侧 + 的对阵 `greedy-coordinated-r2--builder-disruptor-r2`（100 图 up/down = 98/2，Wilson [963,1000]）、`builder-defender-r2--builder-disruptor-r2`（98/2，[929,995]）、`logistics-builder-r2--blocker-builder-r2`（69/31，[593,773]）；异议单侧 −：`builder-disruptor-r2--adaptive-hybrid-r2`（0/100，[0,37]）、`greedy-coordinated-r2--logistics-builder-r2`（32/68，[236,417]）。按地图聚类 bootstrap（10,000 次重抽，种子只由 experiment/variant 标签派生）的合并 delta 和 95% 带 [+5.31e15, +7.26e15]，中位 +6.28e15——**整体为正**。机制：links_created 128,809 vs base 112,673（+143‰）、fields_created 45,025 vs 37,521（+199‰）——缩短链距反而多建链/场（小链更易不交叉），attack_hits +5‰。
- `max-link-2500m`：镜像方向——单侧 − 的同三个对阵（gc--bdis 25/75、bd--bdis 29/71、lb--bb 39/61），异议单侧 +：bdis--ah 73/27、gc--lb 67/33；bootstrap 95% 带 [−8.33e15, −5.23e15]，中位 −6.77e15；links_created −105‰、fields_created −135‰（长链更受几何交叉约束）。

**几何分层**（唯一冻结分层：门户最近邻距离中位数，base 切点 12,200/12,649 cm 三等分）已逐候选入档 `effects.txt`——max-link-1500m 的正方向在三层上一致（up/down = 122/82、110/88、113/85，无单层反转），plan 11"地图交互"行暂无触发；全部 13 候选的分层数据供 Full 阶段使用。

## 5. 稳定判定与 Full 名单

`full_roster: max-link-1500m,max-link-2500m`（派生、可审计，`stable_candidates` 是 pilot 数据进入 Full 的唯一桥梁）。**两侧同参数（缩短/加长链距）都以稳定方向进入 Full**——注意两者方向相反（+ 与 −），即默认 2,000 m 之下"更短更好"（在 3 个对阵上）、之上"更长更差"的一致读法尚需 Full（300 dev + 100 val 图）与后续 Holdout 确认；其余 11 个候选方向全部停止并记录（经济类参数的效应在这些对阵里大量恰好为零——attack/recharge/initial-supply 臂的机制利用率见 `effects.txt` mechanics 表，recharge 系全部接近 base，与 B3"防御分派几乎不触发"一致）。

## 6. 产物清单与内容 hash（plan 第 12 节）

`runs/balance/b4-sensitivity-pilot-r2/`：`manifest.txt`、`configs.tsv`（14 行 × 31 字全配置）、`policies.tsv`（6 行全值参数）、`maps.tsv`（301 行）+ `maps/`（300 个内容寻址 .bin）、`matches.tsv`（16,800 行）、`pairs.tsv`（8,400 行）、`summary.txt`、`report.md`、`replays.tsv` + `replays/`（2,100 份 IARP）、`effects.txt`、`hashes.txt`（**2,410 条**，逐条 blake3 已用独立临时测试重哈希核验——全部一致；scratch pilot `…-pilot-r0` 的 706 条亦同）。

关键产物 blake3：

| 产物 | blake3 |
|---|---|
| matches.tsv | `aa9f2bb5c03396166ceed7673ede8b3b3a024da7f2c23a4cd855a00be4a65150` |
| pairs.tsv | `a96ca751b976bf0b095bfe864b78bbbbaa5038bc07ff286816f97796c1590297` |
| effects.txt | `cd0a6811fd9789e1b36552de87320a41b2881ce9283f53327b3c1f515c7be4d0` |
| configs.tsv | `3da78cf97f22f7e5038ee9d3c1623616b25b06b15378bf9bdb26be73bd2257b4` |
| policies.tsv | `3fc22559b0abac9f54b81f03e1b225b0fa444d7c20510f1f8645eb31f9446cbb` |
| manifest.txt | `554330e578b0aa7bf3faad93677655e1e022f6d8c0a1f36b67d8bc40da251cb7` |
| summary.txt | `24c1a40ce1e0619f89d6a7241d986ac287f5ca5fd182346092228683fcd1d081` |
| report.md | `ab91407b3b298da938d9b9a3d4f577ce448695f877725436018c7e82cf3a06c2` |
| maps.tsv | `cc8d9a391d146e96b899f4f3eb8946b1a6dd07de2c16b85afffbd7b75b6001fb` |
| replays.tsv | `8dbaf3d1c3d34931d606f807c013196f57bb24e0f357e15f524033ac74bf5aaf` |
| hashes.txt（自 身 sha256） | `3383dd05944e5ac28ccd9e967aef53131388b5664b377c5fb11c4abf8d2284c6` |

## Honesty log

1. ADR-0018 初稿把网格写作"14 个候选"；冻结实现清点为 13（`RECHARGE_COST` 只有一个非默认档）。已在同一提交内修正；候选值本身未变。
2. **稳定规则 (c) 的首次实现错误地要求"全部单侧对阵一致"**，比 ADR 句意的自然读法与 plan 10.3 原门槛（"至少三个独立策略对同向"）更严，且在 builder-disruptor 对阵系统性反向的结构下不可通过。首发官方批 `b4-sensitivity-pilot-r1`（0 crash/0 config-failure，原始数据正确）以严格版规则归档、判定全停；随后澄清 ADR-0018（任何 Full 运行之前）、修正实现（同向 ≥3 且严格多于反向；异议逐对阵记录）并以 `-r2` 重发。**逐场轨迹等价已验证**：16,800 行 matches.tsv 除 experiment_id/schedule_seed/elapsed_ms 外全同，8,400 行 pairs.tsv 逐字节相同；`-r1` 目录保留为历史。
3. -r2 渲染层遗留一个展示小缺陷：无决定性单元的对阵其 Wilson 列印作 `0 0`（规则本身读 `Option`，不受影响；独立复核脚本曾被该显示误导后以原始 up/down 计数纠正）。渲染已改为 `-`；-r2 归档按"不覆写历史"保持原样。
4. 后台任务包装器在 ~60 分钟处杀掉了第一次 -r1 尝试（未写任何产物）；改用 setsid 分离重跑完成。scratch pilot `b4-sensitivity-pilot-r1-pilot-r0`（8 图、1,344 场、706 产物）为探索性运行，不作为任何判定证据（ADR-0018 惯例）。

## 后续（Full / 交互，非本 chunk 范围）

- `balance-b4 runs <commit> --full runs/balance/b4-sensitivity-pilot-r2` 将以派生名单（base + max-link-1500m + max-link-2500m）跑 Development 300（3 变体 × 6 对阵 × 300 图 × 2 腿 = 10,800 场）与 Validation 100（3,600 场）。
- 交互实验：按 ADR-0018 优先级，两因子均须稳定——当前仅 `max_link_distance_cm` 稳定，故 plan 7.4 四个因子对暂无可运行者（`pc×mld` 缺 pc、`mb×mld` 缺 mb）；待 Full 数据复核名单后定。

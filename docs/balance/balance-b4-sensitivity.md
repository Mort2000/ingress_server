# B4 参数敏感度报告（阶段 B：Pilot → Full → 交互判定）

本文件承载 B4 单变量参数敏感度全记录（`docs/balance/ai-balance-plan.md` 7.3/7.4/8.3/9/10.3 节，ADR-0018）：Pilot（§1–6，`b4-sensitivity-pilot-r2`）、稳定臂的 Full 扩展（§7，`b4-sensitivity-full-dev-r1`/`-val-r1`）、交互实验判定（§8）与里程碑结论（§10）。`runs/` 不入库；按第 12 节，manifest 摘要与全部产物内容 hash 在此提交。全部统计为非权威离线分析：整数原始计数齐备，permille 仅为展示，不进入 Game Result、Replay Hash 或规则结算。

## 1. 设计（运行前冻结于 ADR-0018）

- **网格**：plan 7.3 的七个参数、十三个单字段候选（全部非默认档；`RECHARGE_COST` 只有一档），每个候选相对未改动的 `GameConfig::default()`（config hash `0x9ead052219fa7ff3`，测试钉住）恰声明一个变更字段。候选值是 Pilot 起点，不是预设结论。
- **名单**：六个冻结对阵 × `PolicyId::R2_COMPETITIVE`，固定 `r2_spec()` 参数（版本 r2.0、逐行全值记录；plan 7.3"固定 R2 策略参数"条款）；无 mirror（阵营偏差由换边双臂覆盖）、无 m6 基线（跨代桥接是 B3 的 ladder）。
- **配对**：每可比较单元（map, game_seed, bot_seed, pair, bucket policy）在全部 14 个 variant 下以相同单元种子打双腿换边（共同随机数，ADR-0014）。
- **地图机制臂**（`portal-count-80/160`，ADR-0018）：从同一 map_seed、在自身 config 下生成自己的地图；其配对差**包含地图机制变化**（该参数的机制即地图）。规则型臂逐图字节共享 base 地图（执行器验证）。
- **规模**：base + 13 候选 × 6 对阵 × 100 张 Development 地图 × 2 腿 = 16,800 场；replay 采样 = 每 (variant, pair) 前 25 图的 leg-0 = 2,100 份 IARP。
- **稳定规则**（pilot→Full 的唯一机械桥梁，ADR-0018）：零 crash/config-failure 且合并 drop < 50‰ 且（c）Wilson 95% 整体单侧的对阵中**同方向 ≥3 个且严格多于反方向**（3+3 平局无方向；反方向单侧对阵逐个记录为异议，plan 10.3 门槛非全体一致）且合并 margin_delta 中位数 ≠ 0。不满足即"停止该方向并记录"（plan 7.1 else 分支）。

## 2. Pilot 溯源（manifest 摘要）

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

## 3. Pilot 实验完整性门槛（`effects.txt` gates，原始分子/分母）

| gate | 计数 | permille | 判定 |
|---|---|---:|---|
| records-integrity | 33,600 / 33,600（14 variant × 16,800/1200 行 × 双方 family/version/parameters 对 `policies.tsv`） | 1000 | PASS |
| no-crashes | 0 / 16,800 | 0 | PASS（crash 0、config-failure 0） |
| command-drop | 31,552 / 40,296,424 | 0（<1‰，带内 <50‰） | PASS |

两个地图机制臂的各 100 张自有地图**全部生成**（0 config-failure；规则型臂逐图字节共享验证通过）。逐场数据可重算本报告全部汇总（关键数字已用独立脚本对 `pairs.tsv` 复核：8,400 行配对差、per-pair 符号计数与 Wilson 与 `effects.txt` 完全一致）。

## 4. Pilot 每候选配对效应与副作用（完成条件：13/13 候选）

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

## 5. Pilot 稳定判定与 Full 名单

`full_roster: max-link-1500m,max-link-2500m`（派生、可审计，`stable_candidates` 是 pilot 数据进入 Full 的唯一桥梁）。**两侧同参数（缩短/加长链距）都以稳定方向进入 Full**——注意两者方向相反（+ 与 −），即默认 2,000 m 之下"更短更好"（在 3 个对阵上）、之上"更长更差"的一致读法尚需 Full（300 dev + 100 val 图）与后续 Holdout 确认；其余 11 个候选方向全部停止并记录（经济类参数的效应在这些对阵里大量恰好为零——attack/recharge/initial-supply 臂的机制利用率见 `effects.txt` mechanics 表，recharge 系全部接近 base，与 B3"防御分派几乎不触发"一致）。

## 6. Pilot 产物清单与内容 hash（plan 第 12 节）

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

## 7. Full 运行（`b4-sensitivity-full-dev-r1` / `b4-sensitivity-full-val-r1`，稳定臂扩展）

名单由 pilot 数据经冻结稳定规则机械派生（`--full` 在运行前独立重放 pilot 归档并复现 `full_roster: max-link-1500m,max-link-2500m`），Full 重跑全部地图而非并入 pilot 行（自包含归档，ADR-0018）。两 run 同一 variant 集（base + 两 mld 臂，`configs.tsv` 逐字节相同）、同一六对阵、固定 `r2_spec()` 参数。

### 7.1 溯源（manifest 摘要）

| 项 | dev | val |
|---|---|---|
| experiment id / stage | `b4-sensitivity-full-dev-r1` / sensitivity | `b4-sensitivity-full-val-r1` / sensitivity |
| 种子集 | Development（master `0x0072326400000300`），300/300 图 | Validation（master `0x0072327600000100`），100/100 图 |
| variants | `default:-`、`max-link-1500m:max_link_distance_cm`、`max-link-2500m:max_link_distance_cm` | 同左 |
| 场次 | 3×6×300×2 = 10,800（12 workers，release，6,449.33 s） | 3×6×100×2 = 3,600（2,136.08 s） |
| schedule_seed | `0x962a3657bd1ce108` | `0xe6c5dec8b1023acf` |
| replay 采样 | 450 份（3 variant × 6 对阵 × 前 25 图 leg-0） | 450 份（同式） |
| base_commit / reproduce | `14e9861`（`cargo run --release -p arena-runner -- balance-b4 runs 14e9861 --full runs/balance/b4-sensitivity-pilot-r2`，一次进程顺序产出两 run） | 同左 |
| 地图 | `maps.tsv` 300 行全为 base 行（两 mld 臂非地图机制，逐图字节共享 base 地图，执行器验证）；`maps/` 300 个内容寻址 .bin | 100 行 / 100 .bin |

`policies.tsv` 与 pilot 逐字节相同（六策略全值参数，blake3 `3fc22559…`）。两 run 的 `pairs.tsv` 分别 5,400 / 1,800 行。

### 7.2 实验完整性门槛（`effects.txt` gates，原始分子/分母）

| gate | dev | val |
|---|---|---|
| records-integrity | 21,600/21,600 PASS | 7,200/7,200 PASS |
| no-crashes | 0/10,800 PASS（config-failure 0） | 0/3,600 PASS（config-failure 0） |
| command-drop | 20,325/25,949,448 = 0‰ PASS（带内 <50‰） | 6,602/8,651,595 = 0‰ PASS |

**跨实验轨迹连续性**（独立脚本核验）：full-dev 前 100 图与 pilot 的全部共享单元——两臂 1,200 个 `margin_delta_vs_base` 加 base 600 个 `margin_sum`——逐单元**完全相同**（map/game/bot seed 不含 experiment id；plan 10.2"不同批次可复现"再次实证）。pooled up/down/zero、合并中位与 delta 和亦经独立脚本对 `pairs.tsv` 重算，与 `effects.txt` 完全一致。

### 7.3 每臂配对效应（per-pair up/down 与 Wilson 95%，原图全量）

**max-link-1500m**（dev：pooled up/down/zero = 1051/749/0，合并中位 delta +10,311,633,440,606；val：359/241/0，+9,131,979,958,202）：

| 对阵 | dev up/down（Wilson95） | val up/down（Wilson95） |
|---|---|---|
| greedy-coordinated-r2--builder-disruptor-r2 | 300/0 [987,1000] | 99/1 [945,999] |
| builder-defender-r2--builder-disruptor-r2 | 298/2 [976,999] | 100/0 [963,1000] |
| builder-disruptor-r2--adaptive-hybrid-r2 | 1/299 [0,19] | 1/99 [1,55] |
| greedy-coordinated-r2--builder-defender-r2 | 144/156 [424,537] | 51/49 [413,606] |
| logistics-builder-r2--blocker-builder-r2 | 218/82 [673,774] | 69/31 [593,773] |
| greedy-coordinated-r2--logistics-builder-r2 | 90/210 [250,355] | 39/61 [300,488] |

**max-link-2500m**（镜像方向；dev pooled 749/1051/0，中位 −11,284,786,340,180；val 257/343/0，−12,920,800,945,885）：

| 对阵 | dev up/down（Wilson95） | val up/down（Wilson95） |
|---|---|---|
| greedy-coordinated-r2--builder-disruptor-r2 | 3/297 [3,29] | 2/98 [5,71] |
| builder-defender-r2--builder-disruptor-r2 | 5/295 [7,39] | 2/98 [5,71] |
| builder-disruptor-r2--adaptive-hybrid-r2 | 293/7 [952,989] | 98/2 [929,995] |
| greedy-coordinated-r2--builder-defender-r2 | 146/154 [430,544] | 53/47 [432,625] |
| logistics-builder-r2--blocker-builder-r2 | 101/199 [285,392] | 31/69 [227,407] |
| greedy-coordinated-r2--logistics-builder-r2 | 201/99 [614,721] | 71/29 [614,790] |

按地图聚类 bootstrap（10,000 次重抽，种子只由 experiment/variant 标签派生）的合并 delta 和 95% 带全部整体单侧：dev 1500m [+16.51e15, +19.99e15]（p500 +18.25e15）、2500m [−20.71e15, −15.85e15]（−18.27e15）；val 1500m [+5.10e15, +7.08e15]、2500m [−6.86e15, −4.08e15]。

**稳定复核**：两臂在 dev 300 图与 val 100 图上**均保持稳定**，且单侧对阵结构（3 同向 + 2 异议，同一批对阵）与 pilot 完全一致——方向在未参与名单派生的 200 张新 Development 图与全部 100 张 Validation 图上复现（plan 10.3"至少三个独立策略对同向"持续满足；异议对阵保持记录）。胜负翻转极少：`outcome_delta` 六对阵求和 dev 两臂各为 −14（1,800 单元）、val 为 +1 与 +8——效应主体是**比分边际**而非胜负改写（plan 9.1 W-D-L 在这些对阵上基本不动）。

对阵结构读法（数据第一）：+方向 = 该对阵 policy-A 的同图换边边际增大。三处单侧一致指向**同一机制叙事**——缩短链距时 `builder-disruptor-r2` 的边际在三个对阵上全部收窄（gc--bdis、bd--bdis 中作为 B 受损；bdis--ah 中作为 A 受损），`logistics-builder-r2` 在两个对阵上获益（lb--bb 作为 A、gc--lb 作为 B）；加长链距全表镜像。

### 7.4 机制利用率与副作用护栏（variant/base 整数和，permille 并列）

| 字段 | dev 1500m | val 1500m | dev 2500m | val 2500m |
|---|---:|---:|---:|---:|
| links_created | 384,526/335,676（+145‰） | 127,635/111,888（+140‰） | 301,304/335,676（−102‰） | 100,948/111,888（−97‰） |
| fields_created | 134,028/111,216（+205‰） | 44,657/36,951（+208‰） | 96,665/111,216（−130‰） | 32,472/36,951（−121‰） |
| hacks | 364,925/326,006（+119‰） | 121,122/108,297（+118‰） | 299,426/326,006（−81‰） | 100,210/108,297（−74‰） |
| recharges | 18,952/14,187（+335‰） | 6,161/4,709（+308‰） | 11,019/14,187（−223‰） | 3,811/4,709（−190‰） |
| transfers | 12,023/11,623（+34‰） | 4,083/3,950（+33‰） | 10,883/11,623（−63‰） | 3,651/3,950（−75‰） |
| attacks / attack_hits | 395,940/394,275 与 395,967/394,294（+4‰/+4‰） | 131,131/129,011 与 131,137/129,011（+16‰/+16‰） | 397,743/394,275 与 397,778/394,294（+8‰/+8‰） | 132,125/129,011 与 132,129/129,011（+24‰/+24‰） |
| first_field_round_sum | 231,126/315,311（−266‰） | 76,018/103,916（−268‰） | 407,959/315,311（+293‰） | 134,192/103,916（+291‰） |
| field_lifetime_rounds | 21,758,342/18,672,660（+165‰） | 7,265,942/6,228,338（+166‰） | 16,455,231/18,672,660（−118‰） | 5,498,055/6,228,338（−117‰） |

first_field_count 几乎不动（dev 1500m 7,200/7,199、2500m 7,188/7,199；val 2,400/2,400 与 2,396/2,400——首场迟早都建得出，变的是建场**轮次**与场数/存续）。

护栏（两 run、两臂）：both-zero 0/0、draw 0/0、crash 0、config-failure 0、合并 drop 0‰（原始分子/分母见 `effects.txt`；远低于 plan 10.1 的 5% 按因解释线，故未触发 drop 原因分解）——**无任何副作用护栏触发**。阵营分裂（ENL/RES 胜场）：dev base 1889/1711、1500m 1926/1674、2500m 1887/1713；val base 631/569、1500m 642/558、2500m 623/577（与 base 同量级，无未解释恶化）。

几何分层（唯一冻结分层：门户最近邻距离中位数三分位；dev 切点 12,106/12,676 cm、val 12,085/12,643 cm）：两臂方向在**全部三层上一致**，无单层反转（1500m dev up/down = 362/244、341/265、348/240；val 116/88、124/74、119/79；2500m 镜像）——plan 11"地图交互"行不触发。

滚雪球（plan 9.1/10.3 P(win|leading@R)，全部为竞争性策略对；独立脚本自 `matches.tsv` checkpoint 列重算，整数分子/分母）：

| 变体 | run | R50 | R100 | R200 | R300 |
|---|---|---:|---:|---:|---:|
| default | dev | 794‰（2783/3503，tie 97） | 893‰（3217/3600） | 935‰（3369/3600） | 961‰（3462/3600） |
| max-link-1500m | dev | 786‰（2823/3588，tie 12） | 873‰（3145/3600） | 928‰（3342/3600） | 960‰（3457/3600） |
| max-link-2500m | dev | 820‰（2686/3274，tie 326） | 902‰（3247/3599） | 946‰（3406/3600） | 966‰（3480/3600） |
| default | val | 790‰（923/1168，tie 32） | 885‰（1062/1200） | 925‰（1111/1200） | 950‰（1141/1200） |
| max-link-1500m | val | 782‰（938/1198，tie 2） | 880‰（1056/1200） | 923‰（1108/1200） | 953‰（1144/1200） |
| max-link-2500m | val | 806‰（854/1059，tie 141） | 898‰（1078/1200） | 944‰（1133/1200） | 961‰（1154/1200） |

plan 10.3 硬告警带（P(win|led@R100) > 90%，竞争对适用）：dev 上 max-link-2500m 为 902‰（3,247/3,599），**边际越线**；val 上 898‰ 未越线；base 自身 885–893‰ 紧贴线下——该告警主要继承 B3 已记录的规则风险背景（builder-disruptor 支配），非 mld 臂新造：缩短链距方向（1500m）反而**降低**滚雪球（873/880‰ vs base 893/885‰）。R50 平局数在 2500m 显著增多（dev 326 vs base 97；val 141 vs 32）——长链延缓早期分离，与其 links/fields 下降一致。

## 8. 交互实验判定（plan 7.4）

Full 归档复核后由 CLI 机械判定（`--interaction runs/balance/b4-sensitivity-full-dev-r1,runs/balance/b4-sensitivity-full-val-r1`）：两个 Full run 均复现两 mld 臂 STABLE（dev 中位 ±10.3e12/−11.3e12、val +9.1e12/−12.9e12），其余因子无任何稳定臂。plan 7.4 的四个优先因子对（`mb×mld`、`ad×ra`、`ad×rc`、`pc×mld`）**均不满足"两因子均有稳定主效应"的前置**（ADR-0018：只对两因子均稳定者运行）——判定输出 `no plan-7.4 factor pair has two stable arms — nothing to run`，退出码 0。**B4 不运行任何交互实验**；这是记录在案的诚实停止，不是遗漏。交互层（DiD + 反转判定，ADR-0018 §7）已实现并留有 pin 测试，供 B5 或后续阶段在出现双稳定因子时使用。

## 9. Full 产物清单与内容 hash（plan 第 12 节）

`runs/balance/b4-sensitivity-full-dev-r1/`：`manifest.txt`、`configs.tsv`（3 行 × 31 字）、`policies.tsv`、`maps.tsv`（300 行）+ `maps/`（300 .bin）、`matches.tsv`（10,800 行）、`pairs.tsv`（5,400 行）、`summary.txt`、`report.md`、`replays.tsv` + `replays/`（450 份 IARP）、`effects.txt`、`hashes.txt`（**760 条**）。`runs/balance/b4-sensitivity-full-val-r1/` 同构（100 行 maps / 3,600 / 1,800 / 450 / **560 条**）。两 run 全部 1,320 条 hash 已用独立临时测试逐条重哈希核验——全部一致（临时 harness 按惯例于报告提交前删除）。

关键产物 blake3：

| 产物 | dev | val |
|---|---|---|
| matches.tsv | `b0ec0f51125a9a7d727563b3218e854fdfa05f17f34f30691d85c3931ed21bf4` | `9edd0e776ffaeb7cb85fea5e35b1cf5cbffe849335c6116148d493dc63560733` |
| pairs.tsv | `eaf07ef8432ac90177fd671be99c74a2223b9f43d0781964ddbbbc1a5ad64b8b` | `d264f87f9122a45ab5c1cfd52486a0cd514cba4e4d714f2f1ec7de4fcec90b31` |
| effects.txt | `d6971d71aad39f476dc5171b997d1dc4447edbda15a652e2330f38d501912d9c` | `2630dad9fccf63cdf7bd30d736b5a1a45cb5a4ba21824eb2bea64569fac24775` |
| configs.tsv | `f4d29e258894d235b3a883de54b4da304e000531f92a57a4605f180922a2a513` | 同左（同 variant 集，逐字节相同） |
| policies.tsv | `3fc22559b0abac9f54b81f03e1b225b0fa444d7c20510f1f8645eb31f9446cbb` | 同左 |
| manifest.txt | `c0e013b909bd7a69872db093afd4b691fb0ccf4e0dd3b94617686c41bbfdd6bc` | `a5c3ca3e61c66090344af6d15ae2c71071193db325990fbf6afdd24670c93ae5` |
| summary.txt | `b44f827a124ae109c630109081abb0d543d3bcf75372bc607dcbe2815ddfdfb0` | `05246ae9dfe0888bd931f8cf5ce33ca67a6bb4ad90f013b3834fef6c4178883a` |
| report.md | `8c02a0ef0fe62913374b7d3309bf1b24d7827f1d204d5367c4e1814a758214d3` | `3bbf6c28d82893ea36eef0083677f9e4379e1295c0d0b27e47bbf3db76900ca5` |
| maps.tsv | `264f9623cc3245ad83f69c44bdbea76622a4afecafce65e49e08bb10b960289e` | `e3507ed71d6a3aa81eefdafabb3e3d90250607a7a2f6df57e36009d328cf756f` |
| replays.tsv | `fb46dd52b64ce9dbc263634b2c327869d03ecd91c2105a4f2b88b45e176f2a46` | `1a1250f055f9aa7b66018260a851e1ccb943dbe56d7a8fdadcdabb533504b546` |
| hashes.txt（自身 sha256） | `47b5b76cb9aa842f7994bf494b8dbceb011803d74c33277cb63d68d4af68a436` | `d472488918374fa52518a4e688c7580478ac6b1d9fae25e6c50f1c187c905c2f` |

## 10. B4 里程碑结论（完成条件核验）与 B5 交接

- **完成条件（plan 14 行 B4"每个候选均有配对效应与副作用报告"）**：13/13 候选在 pilot §4 各有配对效应表 + 副作用护栏 + 停止/入单判定；两个稳定臂另在 Full（400 张图，两 seed 集）复验（§7）。11 个不稳定/零效应方向全部显式停止并记录（pilot §4 逐字停止原因在 `effects.txt`）。
- **唯一的参数级发现**：`max_link_distance_cm` 双向稳定——缩短到 1,500 m 使比分边际向建造/后勤侧移动（+方向，3 对阵单侧、bootstrap 全正带、links/fields +145/+205‰、滚雪球下降），加长到 2,500 m 镜像（−方向）。胜负（W-D-L）几乎不动，无副作用护栏触发，几何三层一致。**B4 数据本身不授权修改默认值**：plan 10.3 要求 Holdout 复现 + Retuned-policy 检查，这是 B5 的范围。
- **经济类参数（attack_damage / recharge_* / initial_supply）零效应**与其机制利用率不动一致（B3：防御分派几乎不触发；本批 base 的 recharges 亦仅 14,187 次 / dev 3,600 场两阵营合并），支持 plan 11"当前 Bot 不会使用该机制 ≠ 机制没有价值"的区分——对这些参数的调优结论保持停止，直到出现使用这些机制的策略。
- **B5 交接**：候选 = `max-link-1500m` 与 `max-link-2500m`（同参数两档、方向相反）；按 plan 7.5/8.3，B5 须做 Retuned-policy 评估与 Holdout（200 图，master `0x0072326800000200`）确认后方可提出默认值变更 ADR。plan 10.3 的 R100 滚雪球告警带记录如 §7（dev 2500m 902‰ 边际越线、val 不复现、base 自身贴线）——B5 决策时应一并复核。

## Honesty log

1. ADR-0018 初稿把网格写作"14 个候选"；冻结实现清点为 13（`RECHARGE_COST` 只有一个非默认档）。已在同一提交内修正；候选值本身未变。
2. **稳定规则 (c) 的首次实现错误地要求"全部单侧对阵一致"**，比 ADR 句意的自然读法与 plan 10.3 原门槛（"至少三个独立策略对同向"）更严，且在 builder-disruptor 对阵系统性反向的结构下不可通过。首发官方批 `b4-sensitivity-pilot-r1`（0 crash/0 config-failure，原始数据正确）以严格版规则归档、判定全停；随后澄清 ADR-0018（任何 Full 运行之前）、修正实现（同向 ≥3 且严格多于反向；异议逐对阵记录）并以 `-r2` 重发。**逐场轨迹等价已验证**：16,800 行 matches.tsv 除 experiment_id/schedule_seed/elapsed_ms 外全同，8,400 行 pairs.tsv 逐字节相同；`-r1` 目录保留为历史。
3. -r2 渲染层遗留一个展示小缺陷：无决定性单元的对阵其 Wilson 列印作 `0 0`（规则本身读 `Option`，不受影响；独立复核脚本曾被该显示误导后以原始 up/down 计数纠正）。渲染已改为 `-`；-r2 归档按"不覆写历史"保持原样。
4. 后台任务包装器在 ~60 分钟处杀掉了第一次 -r1 尝试（未写任何产物）；改用 setsid 分离重跑完成。scratch pilot `b4-sensitivity-pilot-r1-pilot-r0`（8 图、1,344 场、706 产物）为探索性运行，不作为任何判定证据（ADR-0018 惯例）。
5. Full chunk 过程记录（2026-09-05）：(a) 批次本身 setsid 分离运行（dev 6,449 s + val 2,136 s，一次进程顺序产出，退出码 0），但**监控用**的后台 watcher 任务再次在 ~60 分钟处被工具包装器杀掉——批次不受影响，后续改为前台轮询；(b) §7 的滚雪球表**不在** ADR-0018 冻结的 `effects.txt` 统计集内（那七项不含 checkpoint 列联），由独立 scratch 脚本自 `matches.tsv` 的 `leader_r*`/`result` 列重算（plan 原则 8：逐场数据可重算一切汇总），整数分子/分母随表并列；(c) drop 原因未分解：两 run 合并 drop = 0‰（20,325/25,949,448 与 6,602/8,651,595），远低于 plan 10.1 的 5% 按因解释线，未触发该义务（需要时 `balance-drop-reasons` 可对归档重执行）。
6. **两个 Full 归档的 `manifest.txt`/`report.md` reproduce 行是泛化形式**（`balance-b4 runs <base_commit>`，不带模式旗标）——照抄会重跑 Pilot 而非本实验。权威命令是本报告 §7.1 的 `--full runs/balance/b4-sensitivity-pilot-r2`（与 `-val` 同进程产出）。渲染已按 e8b553d 先例修向前：`b4_reproduce_mode` 由计划形状机械推导（pilot 形状=裸命令；base+网格真子集=`--full <pilot 归档>`；Interaction=`--interaction <两个 Full 归档>`；非任何冻结形状=可见 `MODE-NOT-DERIVABLE` 标记），manifest 与 report 两处接线并有 pin 测试。冻结归档不覆写，本条即为指针更正记录。

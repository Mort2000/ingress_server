# B2 测量工具就绪报告（plan §10.1 / ADR-0016）

- 状态：Accepted（8/8 就绪门槛通过）
- 官方运行：`runs/balance/b2-readiness-rr-r3/`、`runs/balance/b2-readiness-idle-r3/`、`runs/balance/b2-readiness-block-r3/`（base commit `9af267a610cbfe52c6bd842720b39ce7eac92955`，12 workers，release）
- 日期：2026-09-04

本报告为 `docs/balance/ai-balance-plan.md` 第 14 行 B2 里程碑的验收记录。六个 R2 策略（`greedy-coordinated-r2`、`builder-defender-r2`、`builder-disruptor-r2`、`logistics-builder-r2`、`blocker-builder-r2`、`adaptive-hybrid-r2`，版本 `r2.0`，标识/参数/随机流命名空间见 ADR-0015/0016）在默认 `GameConfig`、Development 地图集、`Flat(0)` bucket 下验证测量工具就绪性。逐场记录按 plan §10.1 第 6 条完整携带每个策略的独立版本与全值参数序列化（`policy_a/b_parameters` 列，与 `policies.tsv` 逐行一致）。默认配置未改动；冻结的 m6.0 行为与 `runs/` 历史产物未触碰（`-r1`/`-r2` 目录原样保留）。

## 实验设计（冻结于运行之前）

| 实验 | experiment id | 对阵 | 地图 × 换边 | 场次 | 用时 |
|---|---|---|---|---|---|
| Round-robin | `b2-readiness-rr-r3` | 21 对（15 互异 + 6 mirror）× 6 策略 | 100 × 2 | 4,200 | 2,195.966 s |
| Idle 基准 | `b2-readiness-idle-r3` | 6 竞争策略 vs `idle` | 50 × 2 | 600 | 41.791 s |
| Blocking 探针 | `b2-readiness-block-r3` | `blocker-builder-r2` vs `adaptive-hybrid-r2`，全腿留档 replay | 50 × 2 | 100 | 76.284 s |

三批均 0 crash、0 config failure。mirror 单元（6 对 × 100 图）双腿 replay 全留档（RR replay 共 2,172 份：每对 leg-0 + mirror 双腿前 12 图）。

## 门槛结果（整数原始计数；permille 仅为展示换算）

| 门槛 | 原始计数 | permille | 阈值 | 结果 |
|---|---|---|---|---|
| 可持续得分 | 6/4 个策略达标（6 族） | — | ≥4 策略、≥3 族 | 通过 |
| Idle 得分 | 600/600 | 1000 | 100% | 通过 |
| Transfer 使用 | 1200/1200（logistics） | 1000 | ≥1 策略 ≥50% | 通过 |
| 主动 Blocking | 93/100 场 ≥1 次；合计 1396 次 | 930 | ≥10% 且 ≥5 次 | 通过 |
| Command Drop（RR 竞争池） | 6,169 / 9,487,035 | 0（0.65） | < 50‰ | 通过 |
| Command Drop（探针池） | 106 / 196,597 | 0（0.54） | < 50‰ | 通过 |
| Mirror 出生冲突 | 0/672 腿相同出生点集 | 0 | = 0 | 通过 |
| 记录完整性 | 8,400/8,400 family/version/parameters 一致 | 1000 | 100% | 通过 |

### 可持续得分明细（每策略非 crash 场次的得分/建场占比）

| 策略 | score2>0 | fields≥1 |
|---|---|---|
| greedy-coordinated-r2 | 1200/1200 (1000‰) | 1200/1200 |
| builder-defender-r2 | 1200/1200 (1000‰) | 1200/1200 |
| builder-disruptor-r2 | 1176/1200 (980‰) | 1178/1200 (981‰) |
| logistics-builder-r2 | 1198/1200 (998‰) | 1199/1200 (999‰) |
| blocker-builder-r2 | 1200/1200 (1000‰) | 1200/1200 |
| adaptive-hybrid-r2 | 1200/1200 (1000‰) | 1200/1200 |

Transfer 使用明细：仅 `logistics-builder-r2` 使用 Transfer（1200/1200；其余策略按设计为 0/1200 —— Key Runner 是显式运输策略，其余策略不内建该 overlay，符合消融分离原则）。Mirror 附带观察：600 个 mirror 单元 100% 双腿同阵营胜（`Flat(0)` 下先手阵营次序主导 mirror 对局）——按原始计数如实报告，不构成门槛。

### Round-robin 基线（B3 消融的参照点；非门槛）

| 策略 | W-D-L（1,400 场） | mean score2 | mean first-field round |
|---|---|---|---|
| builder-disruptor-r2 | 1194-12-194 | 50.45e12 | 47 (n=1356) |
| blocker-builder-r2 | 691-0-709 | 56.11e12 | 38 |
| adaptive-hybrid-r2 | 664-0-736 | 51.21e12 | 38 |
| greedy-coordinated-r2 | 651-0-749 | 55.86e12 | 39 |
| builder-defender-r2 | 645-0-755 | 55.37e12 | 39 |
| logistics-builder-r2 | 349-0-1051 | 46.93e12 | 52 (n=1399) |

`builder-disruptor-r2` 目前强支配（85.3% 胜率）——这是 B3 阶段 A 消融的输入（策略实现优势 vs 规则效应），不作为参数结论。上表由 `-r3` 的 `matches.tsv` 重算，与 `-r2` 数值逐项相同（见过程记录 5 的轨迹一致性）。

## 过程记录（诚实性）

1. **首次官方运行（`-r1` 目录）被一次核心死锁推翻**：核心级 `Standby`（驻守链源）任务的优先级高于一切步行 `Link`——当每条边的钥匙都握在错误源头的 Agent 手里时双方永久对峙。竞争对局会被对手扰动打破（RR 侧 996–1000‰ 得分），idle 对手则永不扰动：600 场 idle 中 269 场 0-0、策略近乎休眠（500 轮 16 条命令），在 development-002 上定位复现。修复：`Standby` 请求移入 logistics overlay（唯一保证送达的策略），核心五策略恢复"持钥者步行至源"。`-r1` 目录按不覆盖历史原则保留为被否决的首次尝试。
2. **idle 门槛参与判定缺陷**：原以“score2=0 且 fields=0 则跳过”判定参与，静默漏掉了失败场次（报 331/331 而实为 600 场）。已改为按记录两侧 policy tag 判定参与；修复后 600/600。
3. **Mirror 单例带碰撞**：当地图最大三角严格超出 near-equal 带时，双方都会规范化选它（种子 tie-break 无带可破）。修复：`CoreParams.spawn_jitter_cm`（默认 1000）——驻点分配不变，出生坐标加入阵营键控的 recorded 抖动（运营半径内）。`-r1` RR 中 8/96 腿碰撞 → `r2` 0/672 → `r3` 0/672。
4. **已知外观缺陷（已随 `-r3` 消除）**：`-r2` 三个目录 manifest 的 `reproduce` 行写作 `balance-b1`（B1 归档代码的硬编码，生成于修复前）；归档不可改写，当时在此记录。`-r3` 的 manifest 已写正确的 `balance-b2` 命令。
5. **第二轮评审修正（`-r2` → `-r3`）：逐场记录缺少参数列**。plan §10.1 第 6 条要求"每个策略的独立版本和参数完整进入逐场记录"，ADR-0016 门槛 7 冻结为"`matches.tsv` 每行的 policy version 与 parameters 与 `policies.tsv` 一致"；`-r2` 的 balance `matches.tsv` 只携带 id/family/version（参数仅存在于 `policies.tsv`），门槛 7 的 parameters 半边因此空转，且 B4/B5 Retuned-policy 模式下同 id+version 行将无法逐行区分参数集。修复：`BalanceMatchRecord` 增加 `policy_a/b_parameters`（类型化 `PolicyParameters`，全值规范序列化写入逐场行、严格解析回读——篡改的参数串解析不回任何参数集），`records_gate` 补齐对计划注册表（`policies.tsv` 来源）的逐行 parameters 比对；冻结的 m6 `matches.tsv` 字节格式不动（balance 为独立格式）。官方三批以新 experiment id `-r3` 重跑：map/game/bot seed 均不依赖 experiment id，`-r3` 与 `-r2` 的 4,200 行逐场结果、比分、final_state_hash 与全部统计列完全一致（仅 experiment_id/schedule_seed 回显、elapsed 与新增参数列不同；pilot 336/336 行同样一致；`pairs.tsv`、`replays.tsv` 与 `-r2` 逐字节相同）。`-r2` 目录按不覆盖历史原则保留。
6. **第 4 条的补充更正（B3 评审发现）：`-r1`..`-r3` 的 `report.md` 与 manifest `note=` 仍带 B1 模板残文**。第 4 条只修了 manifest 的 `reproduce=` 行；共享模板 `report_text`/`manifest_text` 的其余文案仍硬编码 B1——全部九个 B2 目录（rr/idle/block × `-r1`/`-r2`/`-r3`）的 `report.md` Reproduce 行写作 `balance-b1`（照它会重跑 B1 冒烟实验；正确命令 `balance-b2`，本文件"复现"节）、Gates 节写作 "None registered — B1 …"（与同目录 `manifest.txt` 的 gate 计数及 `gates.txt` 的实测结论矛盾——各目录的门均已评估且全部通过），`manifest.txt` 的 `note=` 指向 `docs/balance/balance-b1-smoke.md`（B2 报告与内容哈希的承载文件是本文件）。历史 `runs/` 永不覆盖，残文按缺陷原样冻结；权威指针为各 `manifest.txt` 的 `reproduce=` 行（`-r1`/`-r2` 的同样错误并已由第 4 条记录，`-r3` 正确写 `balance-b2`）、`gates.txt`（门评估）与本文件。模板已随 B3 修复改为按 stage 渲染并以回归测试钉住（详见 `docs/balance/balance-b3-ablation.md` 过程记录第 8 条）。

## 归档与内容哈希（plan §12；`runs/` 未跟踪）

每个实验目录含 `manifest.txt`、`configs.tsv`、`policies.tsv`（全策略参数全值序列化）、`maps/` + `maps.tsv`、`matches.tsv`（含逐行 `policy_a/b_parameters` 全值参数列）、`pairs.tsv`、`summary.txt`、`report.md`、`gates.txt`、`replays/` + `replays.tsv`、`hashes.txt`（每件产物一份 blake3）。顶层产物哈希：

**b2-readiness-rr-r3**（replays 2,172 份；hashes.txt 2,282 条，自身 sha256 `dff3b67598c98d0a7572e73c4eb81e65a159059b2275f5ef317f37208d0cef55`）
```text
a17fff94742917a904f009261746351efb5d89577406325cbc08db5ea6564e54  configs.tsv
f1d7a169980d9092222af18e733e86926c002e201c5da1f8a4d716287fe95d40  manifest.txt
8d7711605cbfd72e674dc9386eacf9f7d4c4e815ff4e21a778305e8dbe2bb57f  maps.tsv
a61703fbafd1d5b1b58159101bb6af00ca990f00604ea4bd0b6f7e54969f1dcb  matches.tsv
01e23873cac432254ee636bbfb9b5fd2362665feb0315a277ae71789d1128e95  pairs.tsv
f47bea46589a3f70831695669722ec413d9f19d21cbded5d2f8440f95aaece27  policies.tsv
d9c572d35c8b296c958c6f258dfa2fe0c1d4f5c7cc7738cb9626dac12c491e58  replays.tsv
73406915ca9255a61008f1f31352221fb362ab324988201eb7a6a634c91d2047  report.md
076499aa46ddf9e0152871762df913a83dbeded7c1f027e70360cd5d182347c8  summary.txt
a4ab46f54f7ee26e3970e824aaea95cac88a0b7f2bf22a88c5918b2e30773c5a  gates.txt
```

**b2-readiness-idle-r3**（无 replay；hashes.txt 60 条，sha256 `6b2192b311e61c3ce5a0af774881ccca63cdc8026fcec04ecb4294530f85745f`）
```text
a17fff94742917a904f009261746351efb5d89577406325cbc08db5ea6564e54  configs.tsv
a717b05cc2ffc0692e33ad3ae40c4ff6342959a74862e02d2fa482cac99ba3ec  manifest.txt
abe6090d8edc719f501a01c6ec958dd21a18e8dc30e69b7d46a93a9ed82fdaa5  maps.tsv
b8a53b9d4a9769c4af449a47237b181453a84cccd192f1a840050fada1f63f68  matches.tsv
8b20d35f7fc0ed37fe782ac55433fcdb6cf3a460fb8493364b80df4960d4a1f7  pairs.tsv
f47bea46589a3f70831695669722ec413d9f19d21cbded5d2f8440f95aaece27  policies.tsv
e68e22719a38b3851662d443b1a9835db1937bd2a151084d15f2f39b6d121011  replays.tsv
c0b2afc9112c62863ffbedabff4b5a47f6994d7346877abdd31368ba435bb550  report.md
342b00282be24fec6783300c4ca4077bcea6c1323dc480cf2be978ed30079563  summary.txt
ab725037611458446858ec56111a62cd2a6320bcf06c341fcd4eadcb77a8b6c4  gates.txt
```

**b2-readiness-block-r3**（replays 100 份；hashes.txt 160 条，sha256 `0ba301c91dc5c8aee69d4dc521170cacea07e95e027c4d536fc742186a2237ce`）
```text
a17fff94742917a904f009261746351efb5d89577406325cbc08db5ea6564e54  configs.tsv
34ae9bfc820e316a46f8a1fd1eb1962ed99616d0a668a8022305e8c1e13e9214  manifest.txt
abe6090d8edc719f501a01c6ec958dd21a18e8dc30e69b7d46a93a9ed82fdaa5  maps.tsv
6833c4d313f7b700cb76e010ac0c1e71fc73715ad2fc095d942a3f6e0d17e3f2  matches.tsv
6469a86f3180fd842cd18e54763f0bc0bec1881af36c2ff65c0d13ce6365e032  pairs.tsv
f47bea46589a3f70831695669722ec413d9f19d21cbded5d2f8440f95aaece27  policies.tsv
d03c276385de03bedcfece3326e168cdf2889fdf56749cf7dd39db19b03d9652  replays.tsv
7cc60881767cb473f12e0f26c89404d11a5d51afb98a0c51e10c417a0cf5a461  report.md
ddf6bf2b7bf6b556e922dc72c66567b637dbf8d4862e0ddb718fdae709bf7c58  summary.txt
1accda5ae7f702e581581471a7e12414b6ac26b69d7b1276be22af72f2b9b430  gates.txt
```

`configs.tsv` 三批相同（默认配置逐字）；`policies.tsv` 同为一份：7 行、全参数记录。每份 replay 的 blake3 见各目录 `hashes.txt`/`replays.tsv`。

## 复现

```sh
cargo run --release -p arena-runner -- balance-b2 runs <base_commit>          # 官方三批（新目录；已存在则拒绝）
cargo run --release -p arena-runner -- balance-b2 runs <base_commit> --pilot  # 探索性小批（独立 -pilot-r0 id）
```

Worker 数不影响非时间结果（`runner_readiness::tiny_r2_batch_is_identical_under_one_and_many_workers` 钉死：记录逐字段一致——仅 elapsed_ms——且 replay 字节一致）。

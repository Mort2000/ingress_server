# B2 测量工具就绪报告（plan §10.1 / ADR-0016）

- 状态：Accepted（8/8 就绪门槛通过）
- 官方运行：`runs/balance/b2-readiness-rr-r2/`、`runs/balance/b2-readiness-idle-r2/`、`runs/balance/b2-readiness-block-r2/`（base commit `f6502f74ca98c94ac79ae0e0c24c5817f56c4da2`，12 workers，release）
- 日期：2026-09-04

本报告为 `docs/ai-balance-plan.md` 第 14 行 B2 里程碑的验收记录。六个 R2 策略（`greedy-coordinated-r2`、`builder-defender-r2`、`builder-disruptor-r2`、`logistics-builder-r2`、`blocker-builder-r2`、`adaptive-hybrid-r2`，版本 `r2.0`，标识/参数/随机流命名空间见 ADR-0015/0016）在默认 `GameConfig`、Development 地图集、`Flat(0)` bucket 下验证测量工具就绪性。默认配置未改动；冻结的 m6.0 行为与 `runs/` 历史产物未触碰。

## 实验设计（冻结于运行之前）

| 实验 | experiment id | 对阵 | 地图 × 换边 | 场次 | 用时 |
|---|---|---|---|---|---|
| Round-robin | `b2-readiness-rr-r2` | 21 对（15 互异 + 6 mirror）× 6 策略 | 100 × 2 | 4,200 | 2,182.270 s |
| Idle 基准 | `b2-readiness-idle-r2` | 6 竞争策略 vs `idle` | 50 × 2 | 600 | 41.438 s |
| Blocking 探针 | `b2-readiness-block-r2` | `blocker-builder-r2` vs `adaptive-hybrid-r2`，全腿留档 replay | 50 × 2 | 100 | 75.417 s |

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
| 记录完整性 | 8,400/8,400 family/version 一致 | 1000 | 100% | 通过 |

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

`builder-disruptor-r2` 目前强支配（85.3% 胜率）——这是 B3 阶段 A 消融的输入（策略实现优势 vs 规则效应），不作为参数结论。

## 过程记录（诚实性）

1. **首次官方运行（`-r1` 目录）被一次核心死锁推翻**：核心级 `Standby`（驻守链源）任务的优先级高于一切步行 `Link`——当每条边的钥匙都握在错误源头的 Agent 手里时双方永久对峙。竞争对局会被对手扰动打破（RR 侧 996–1000‰ 得分），idle 对手则永不扰动：600 场 idle 中 269 场 0-0、策略近乎休眠（500 轮 16 条命令），在 development-002 上定位复现。修复：`Standby` 请求移入 logistics overlay（唯一保证送达的策略），核心五策略恢复"持钥者步行至源"。`-r1` 目录按不覆盖历史原则保留为被否决的首次尝试。
2. **idle 门槛参与判定缺陷**：原以“score2=0 且 fields=0 则跳过”判定参与，静默漏掉了失败场次（报 331/331 而实为 600 场）。已改为按记录两侧 policy tag 判定参与；修复后 600/600。
3. **Mirror 单例带碰撞**：当地图最大三角严格超出 near-equal 带时，双方都会规范化选它（种子 tie-break 无带可破）。修复：`CoreParams.spawn_jitter_cm`（默认 1000）——驻点分配不变，出生坐标加入阵营键控的 recorded 抖动（运营半径内）。`-r1` RR 中 8/96 腿碰撞 → `r2` 0/672。
4. **已知外观缺陷**：三个 `-r2` 目录 manifest 的 `reproduce` 行写作 `balance-b1`（B1 归档代码的硬编码，生成于修复前）；正确命令为 `cargo run --release -p arena-runner -- balance-b2 runs <base_commit>`。归档不可改写，此处记录。

## 归档与内容哈希（plan §12；`runs/` 未跟踪）

每个实验目录含 `manifest.txt`、`configs.tsv`、`policies.tsv`（全策略参数全值序列化）、`maps/` + `maps.tsv`、`matches.tsv`、`pairs.tsv`、`summary.txt`、`report.md`、`gates.txt`、`replays/` + `replays.tsv`、`hashes.txt`（每件产物一份 blake3）。顶层产物哈希：

**b2-readiness-rr-r2**（replays 2,172 份；hashes.txt 2,282 条，自身 sha256 `217a90c5a294a314445bf66fb36d0c57c3dc66664ea07297df51febeb2858c40`）
```text
a17fff94742917a904f009261746351efb5d89577406325cbc08db5ea6564e54  configs.tsv
6af58072c242d9d506857181cc42e93763b808cd1801257739f395e0d016f22b  manifest.txt
960da459a251dccdc7e39bc4d4fcd87cff18974bae9fdffb12784fcb588138f3  matches.tsv
01e23873cac432254ee636bbfb9b5fd2362665feb0315a277ae71789d1128e95  pairs.tsv
f47bea46589a3f70831695669722ec413d9f19d21cbded5d2f8440f95aaece27  policies.tsv
d9c572d35c8b296c958c6f258dfa2fe0c1d4f5c7cc7738cb9626dac12c491e58  replays.tsv
7a34dd6cb0c5bae186985d550cd23d93372fb42c499a73896bf43f0dd3efbe68  report.md
30b96d4c9773f356dd6e793716aa9b4a90c61b9bac525fdb0f34333559a6cff5  summary.txt
511ebe83b9e1ff86070e2ef2d5751de3e3e51d4b9c985781517981b7807c942c  gates.txt
```

**b2-readiness-idle-r2**（无 replay；hashes.txt 60 条，sha256 `8401085b411ac5c74dc705d8571287a42636c335fa3109de212ad743ae65b3d9`）
```text
a17fff94742917a904f009261746351efb5d89577406325cbc08db5ea6564e54  configs.tsv
6831adb91dd3cae29ff39830cb111f6e196343ab4eebc6375a55a9f6b915a618  manifest.txt
89a888e951d3db07e70ae90bec7189a4726a7dd4b11c873b1e2aeee06f496d38  matches.tsv
8b20d35f7fc0ed37fe782ac55433fcdb6cf3a460fb8493364b80df4960d4a1f7  pairs.tsv
f47bea46589a3f70831695669722ec413d9f19d21cbded5d2f8440f95aaece27  policies.tsv
e68e22719a38b3851662d443b1a9835db1937bd2a151084d15f2f39b6d121011  replays.tsv
3ab2ce4c9002785b500651d426a17158e47d0eb33a5ed83aa35ddfeb1730494c  report.md
55bca5489f991cda3d8305cb1c973a8d76bfc22ffd7dff821b00b1b938c6a5b1  summary.txt
ab725037611458446858ec56111a62cd2a6320bcf06c341fcd4eadcb77a8b6c4  gates.txt
```

**b2-readiness-block-r2**（replays 100 份；hashes.txt 160 条，sha256 `21a814ab5cde2e83bf13df8cf213f069c6c5f92937ea4c26c99be90247761b8d`）
```text
a17fff94742917a904f009261746351efb5d89577406325cbc08db5ea6564e54  configs.tsv
61383552a7e8476e9bdf273ce57c1e718a2c69ad9c462a4016cb334be54a716f  manifest.txt
58117299f607086c9e5beb70ff8cdf407350392efcc88622548fa1a2677aba0d  matches.tsv
6469a86f3180fd842cd18e54763f0bc0bec1881af36c2ff65c0d13ce6365e032  pairs.tsv
f47bea46589a3f70831695669722ec413d9f19d21cbded5d2f8440f95aaece27  policies.tsv
d03c276385de03bedcfece3326e168cdf2889fdf56749cf7dd39db19b03d9652  replays.tsv
f8533492437c7b3040209e5ae949cfae067b4ab5d6cb0b0c6a29312218700caa  report.md
b70d85cb66c9e345b140b1453ec77d5e29766277da5d1fede1c377919dbc1e3b  summary.txt
1accda5ae7f702e581581471a7e12414b6ac26b69d7b1276be22af72f2b9b430  gates.txt
```

`configs.tsv` 三批相同（默认配置逐字；`policies.tsv` 同为一份：7 行、全参数记录）。每份 replay 的 blake3 见各目录 `hashes.txt`/`replays.tsv`。

## 复现

```sh
cargo run --release -p arena-runner -- balance-b2 runs <base_commit>          # 官方三批（新目录；已存在则拒绝）
cargo run --release -p arena-runner -- balance-b2 runs <base_commit> --pilot  # 探索性小批（独立 -pilot-r0 id）
```

Worker 数不影响非时间结果（`runner_readiness::tiny_r2_batch_is_identical_under_one_and_many_workers` 钉死：记录逐字段一致——仅 elapsed_ms——且 replay 字节一致）。

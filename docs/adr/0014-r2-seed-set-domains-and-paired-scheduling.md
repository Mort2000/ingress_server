---
status: accepted
---

# R2 Seed Set 主种子、推导域与同图换边配对（B1）

依据 `docs/ai-balance-plan.md` 8.1/8.2，自本决策起冻结 R2 平衡实验的种子集合与推导（`arena-runner::balance::plan`）：

**主种子与集合大小（立即冻结，永不改值）**：Development 300 张地图，master seed `0x0072326400000300`；Validation 100 张，`0x0072327600000100`；Holdout 200 张，`0x0072326800000200`（低 32 位承载集合大小，便于肉眼核对）。三者的推导 payload 均含集合 tag（`development`/`validation`/`holdout`）与 master seed，故即使某个 label 被误用，不同集合的种子流也不相交（测试在 0..300 上逐 k 比较种子**值**）。

**推导域（新 label，与既有域全部分离）**：`ingress-arena-r2-map`、`ingress-arena-r2-game`、`ingress-arena-r2-bot`、`ingress-arena-r2-sched`，全部为 `LE-u64(blake3(label || payload))`（D58 同构）。map：`seed_set_tag || master_le64 || k_le32`；game/bot：`seed_set_tag || master_le64 || pair_tag || k_le32`；schedule：`experiment_id || master_le64`。测试证明相同 payload 在 m6 label 与 r2 label 下取值不同，且四个 r2 域互不相同。

**可比较实验单位（8.2）**：`(seed-set 地图, game_seed, bot_seed, policy 对, bucket policy)`。game_seed 与 bot_seed 只由 `(集合, pair_tag, k)` 推导——**绝不含 variant 或 leg**，因此同一单位的换边两局与全部配置变体严格共用同一地图、Game Seed 与 Bot Seed（共同随机数）。换边是显式的确定性 faction swap：leg 0 = policy_a 任 ENL，leg 1 = policy_a 任 RES；两个分配按构造同时出现（D55 want-bit 调度不动，继续服务 m6 与 Tournament 的既有语义）。

**Holdout 纪律**：Holdout 地图在 B5 Confirm 阶段之前**绝不生成、绝不运行**；种子值可在测试中比较（仅值，不生成地图）。`ExperimentPlan::validate` 拒绝 Holdout + 非 Confirm 的组合。查看 Holdout 结果后若需继续调策略或参数，当前 Holdout 降级为 Validation，并须另建新的域分离 Holdout。

**配对的地图共享边界（已知限制；地图定义字段部分被 [ADR-0018](./0018-b4-parameter-sensitivity.md) 修订）**：跨变体共享同一 Map Definition 仅当变体不改 `generate_map` 读取的字段（维度与 portal 族字段）。`move_budget_cm` 类变体可共享（执行器逐一断言两变体从同一 map_seed 生成字节相同的地图）；`PORTAL_COUNT` 类变体（B4）无法共享——ADR-0014 原文将其归为配置失败；自 ADR-0018 起此类**地图机制变体**改为从同一 map_seed 在自身 config 下生成自己的地图（配对键不变，配对差含地图机制变化），不可行生成仍记 `config-failure`。本 ADR 的种子推导不受影响：map_seed 永不依赖 variant。

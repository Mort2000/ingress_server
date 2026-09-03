---
status: accepted
---

# 策略独立标识与版本语义（B1）

平衡实验（`docs/ai-balance-plan.md` 第 5.1 节）中，策略以独立的 `PolicySpec { policy_id, family, version, parameters }` 标识，不再由单一全局 `BOT_VERSION` 代表所有策略：`BOT_VERSION`（`"m6.0"`）今后仅表示五个冻结 M6 基线本身。B1 的 `PolicyId` 与五个冻结 `BotKind` 一一对应（映射全域且单射，由测试钉死），tag 复用 `BotKind::tag` 字符串以保持与 `runs/m6-r1/` 产物词汇的连续性；`make_policy_bot` 按 spec 逐一委托给未改动的 `make_bot`，且锁定测试证明：相同 `(bot_seed, faction, config, map)` 与观察序列下，策略路径与 `make_bot` 路径逐轮提交完全一致。版本语义是**只增不改**：策略行为的任何变化必须以新 `version`（B2 起为新 `policy_id`）进入，冻结的 m6.0 镜像永不修改；结构化参数（第 6.3 节）同样以新 `PolicyParameters` 变体进入。bot RNG 的 kind-tag 命名空间随本决策冻结：字节 1–5 永久属于五个 m6 BotKind，B2 新策略必须取 >= 6 的新字节，保证新策略的随机流与所有历史流（D53）不相交。诊断族（Random、Nearest、AnchorDestroyer）在类型上与竞争族（FieldBuilder、AdaptiveHybrid，B2 增 Defender、Disruptor、KeyRunner、Blocker）区分，诊断策略不进入竞争性平衡汇总。

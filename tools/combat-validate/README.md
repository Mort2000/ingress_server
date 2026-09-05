# 攻防收益对照工具

固定 Development 地图、规则和 game/bot seed，在同一策略身份内部切换参数。
每场使用独立且持续存活的影子策略核对全部有序提交，再完整验证 IARP。
对手在每个实验臂中独立决策，不沿用另一个实验臂的后续动作。

```sh
cargo build --release -p combat-validate --offline
target/release/combat-validate runs/balance/b2-readiness-rr-r3 runs/balance/NEW-factorial factorial 8 8
target/release/combat-validate runs/balance/b2-readiness-rr-r3 runs/balance/NEW-adaptive adaptive-defense 8 4
python3 tools/combat-validate/analyze.py runs/balance/NEW-factorial runs/balance/NEW-adaptive
```

输出目录必须不存在；地图数量限定为 1–20，线程数量为 1–32。只接受
`development-*`、默认 GameConfig、Flat(0) 提交桶，以及完整的地图/换边配对。
读取来源归档中 Defender–Disruptor 的输入，不以旧策略参数代替新实验臂参数。
来源 IARP 的 digest 与逐场输入会核对；新策略自行选择出生点并写入新 IARP。

| 组 | 实验臂 | 用途 |
|---|---|---|
| `factorial` | Defender cap 0/1/2 × Disruptor share 0/1000 | 在固定另一方参数时，估计攻击/防守开关的分差效果 |
| `adaptive-defense` | Adaptive cap 0/2，对默认 Disruptor | 检查反攻策略加入防守的增量价值 |
| `adaptive` | Adaptive opening/lead 为 300/550、300/0、0/0 | 隔离阶段门槛；`legacy` 只表示门槛值，不代表运行了旧版代码 |

除明确列出的参数外，使用构建版本的完整默认参数；完整展开值写入 `result.json`。
复现历史版本须使用相应源码或冻结二进制，不能用当前默认参数冒充历史输入。

输出包括 `manifest.txt`、`matches.jsonl`，以及每场的 `result.json`、`replay.iarp`
和 `trace.jsonl`。同名场不能跨批次混用。分析器核对实验臂覆盖、初始化 hash、
seed、规则/决策验证标记、分数与逐轮收入、场生命周期收入、实际伤害和恢复量。
早期 r2.3 trace 无 `area2`，仅核对逐轮收入总和，不冒称完成生命周期面积核对。

`decision.commands` 和 `drop.reason` 是供人工阅读的诊断字符串，不是稳定的解析协议；
分析指标只读取结构化任务字段及实际结算效果。`field-destroyed-by.side` 是破坏者，
不是被破坏场的所有者。场归属须从本次生命周期的创建记录读取。

攻击收益以攻击方视角的分差变化表示；防守收益以防守方视角表示，均拆分己方及
对手得分变化。换边腿按地图汇总，报告描述性分布，不把相关腿当成独立样本作显著性检验。
攻击次数、恢复 HP、花费 Supply 是投入/作用量，不等同于净分数收益。
不分析 Transfer，不修改默认规则，不使用 Holdout，也不替代正式 B2/B3/B4 验收。

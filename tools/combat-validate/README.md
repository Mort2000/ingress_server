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
| `compound` | Defender / Disruptor 各自 `core.combat_auxiliaries` 开/关 | 固定防守 cap=2、攻击开启，隔离双方组合动作的交互效应 |
| `supply-defense` / `supply-adaptive` | Defender / Adaptive 的 `supplyreserve` 为 0/5 | 固定 Disruptor 补给关闭，比较独立补采和远程防守补给路线 |

除明确列出的参数外，使用构建版本的完整默认参数；完整展开值写入 `result.json`。
复现历史版本须使用相应源码或冻结二进制，不能用当前默认参数冒充历史输入。
从 r2.5 起，`combataux=1` 为默认；旧参数串缺失该字段时解析为 0，不能替旧档案开启新行为。
从 r2.6 起，Defender/Adaptive 默认 `supplyreserve=5`；其他策略默认 0，旧参数缺省
为 0。它按 Recharge 成本计库存预警，不是 Supply 规则变化。无主任务但有补给提交
不能计为空闲；应同时读取 `decision.submitted` 与实际动作。
主动补给移动只选择一轮内可进入操作范围并完成 Hack 的目标，不发起多轮补给绕路；
已有建设路线上的本地补采仍保留。设计与固定对照结果见
[r2.6 补给验证](../../docs/balance/r26-supply-validation.md)。

输出包括 `manifest.txt`、`matches.jsonl`，以及每场的 `result.json`、`replay.iarp`
和 `trace.jsonl`。同名场不能跨批次混用。分析器核对实验臂覆盖、初始化 hash、
seed、规则/决策验证标记、分数与逐轮收入、场生命周期收入、实际伤害和恢复量。
早期 r2.3 trace 无 `area2`，仅核对逐轮收入总和，不冒称完成生命周期面积核对。

`decision.commands` 和 `drop.reason` 是供人工阅读的诊断字符串，不是稳定的解析协议；
分析指标只读取结构化任务字段及实际结算效果。`field-destroyed-by.side` 是破坏者，
不是被破坏场的所有者。场归属须从本次生命周期的创建记录读取。

r2.5 增加 `companion`（同一 Agent 的兼容任务）、`hack`、`link`、`actions` 记录。
组合指标只计算**同一执行流内双方动作均成功**的 Recharge+Hack/Link/Move、
Attack+Hack/Link，不把候选共存当作执行成功。`link.source/target` 表示无向边的规范端点，
不表示实际消费 Key 的命令方向。`grants` 仍计主任务 Agent-round；任务分类包含 companion，
因此分类之和可以大于 grants。不得用该和当作人数或投入轮数。

攻击收益以攻击方视角的分差变化表示；防守收益以防守方视角表示，均拆分己方及
对手得分变化。换边腿按地图汇总，报告描述性分布，不把相关腿当成独立样本作显著性检验。
攻击次数、恢复 HP、花费 Supply 是投入/作用量，不等同于净分数收益。
不分析 Transfer，不修改默认规则，不使用 Holdout，也不替代正式 B2/B3/B4 验收。

## Supply 与少量回放审查

`review_supply.py` 读取已有 r2.5 的两个 cap=2 组（32 场），核对逐 Agent Supply
账本，并检查已声明的两场压力样本。不生成新比赛，不改变生产策略或历史归档。

```sh
python3 tools/combat-validate/review_supply.py runs/balance/NEW-supply-review --inspector /path/to/r25-replay-inspect
python3 -m unittest discover -s tools/combat-validate -p 'test_*.py'
```

输出目录必须不存在。检查器恢复归档参数和持续记忆，分别核对双方生产策略和
请求级影子的全部有序提交，再输出结构化快照。`supply_probe` 只改变一个 Agent
观察中的库存，`supply_team_probe` 同时恢复零库存者；二者均不是合法资源来源
或整局反事实。`new_recharge_grants` 只计真正新增的充电分配，候选数变化不算。
`legal_alternative` 则从原始状态实际结算声明的单轮附加 Hack 或 Move+Hack，
核对原有效果与本轮收入不变；不沿用原后续轨迹推算新胜负。

完整结论及局限见 [Supply 与防守抽查](../../docs/balance/r25-supply-defense-review.md)。
旧审查必须使用 r2.5 源码构建的检查器（本地冻结于
`.scratch/r26-supply-20260905/replay-inspect-r25`），不能传当前版本冒充旧实现。

### r2.6 修复验证

```sh
cargo build --release --offline -p combat-validate --bins --examples
target/release/combat-validate runs/balance/b2-readiness-rr-r3 runs/balance/NEW-supply-defense supply-defense 8 6
target/release/combat-validate runs/balance/b2-readiness-rr-r3 runs/balance/NEW-supply-adaptive supply-adaptive 8 6
python3 tools/combat-validate/evaluate_supply.py runs/balance/NEW-supply-defense runs/balance/NEW-supply-adaptive runs/balance/NEW-supply-evaluation
```

评估脚本审计 64 场账本/收益，要求 32 场关闭组与 r2.5 对应完整 IARP 字节一致，
并对预先固定的 Defender m3/leg0、Adaptive m6/leg0 修复组作规则/生产策略/请求影子
三重检查。输出配对汇总、逐 Agent 账本、身份、完整样本 trace 与 SHA-256。

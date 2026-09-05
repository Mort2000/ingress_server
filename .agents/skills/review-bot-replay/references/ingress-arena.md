# Ingress Arena 回放取证入口

仅在具有 `arena-core`、`arena-bots`、`arena-replay`、`arena-runner` 的 Ingress Arena 仓库使用。本文件中的路径均相对仓库根；先通过 `rg` 确认当前 API，归档版本与当前实现不一致时，以实际生产该批次的实现为准。这里记录方法和格式，不携带某次比赛的预设诊断。

## 定位单场与设计预期

| 来源 | 用途 |
|---|---|
| `docs/balance/ai-balance-plan.md` §6、§9、§10 | 策略应具备的行为、质量约束、指标解释边界 |
| `docs/adr/0015-independent-policy-identity.md`、`0016-r2-policy-portfolio-and-readiness-gates.md` | 独立策略身份、参数和门槛含义 |
| `runs/balance/<experiment-id>/matches.tsv` | 逐场输入、版本／参数、结果、机制计数、状态 hash |
| 同目录 `replays.tsv` | `cell_index` 对应逐场 `index`；从 `file` 列定位 `.iarp`，核对 captured 状态和 hash |
| 同目录 `configs.tsv`、`policies.tsv`、`manifest.txt`、`maps.tsv`、`maps/` | 完整输入与构建来源；B4 地图机制变体可能使用自身地图 |

按列名解析 TSV，不使用固定列号。核对 `variant_id`、`pair_id`、`leg`、`map_id` 以防取错场次。**policy A 不一定是 ENL**，从 `policy_a_is_enl` 映射；换边腿和 mirror 尤其容易记反。非 balance 历史批次格式不同，先查看其 schema。

不要通过文件名猜 index 或修改旧归档。不带模式的 `balance-b*` 复现命令可能启动整批甚至错误阶段；单场检查优先读现成 replay。没有 replay 时，寻找当前执行器的单场复现入口，按归档完整输入复现该 cell。

## 导出可查看的已验证回放

已有工具 `tools/replay-export`：

```sh
cargo run --release -p replay-export -- <replay.iarp> -o <新的输出目录>/match.html --dev 1
```

先创建输出目录；不要将生成物写回历史运行目录。该命令先进行完整 IARP 验证，再生成包含双方开发视角的独立 HTML。需要只包含某方私有信息时，改用 `--dev 0 --faction enl` 或 `res`。开发导出只服务于分析，Bot 的决策依据仍必须来自合法 Observation。

API 入口：

- `arena_replay::replay::{decode_log, replay}`：解码和完整验证；`ReplayFile::round_records()` 提供原始逐轮记录。
- `replay_export::data::{build_payload, export, extract_embedded_json}`：验证后构造页面数据，或从导出页面读回数据。
- `tools/replay-export/src/data.rs::materialize`：已有的按档案提交重执行、构造观察并逐轮核对 hash 的参考实现。

不要只看 HTML 公共事件列表：**Hack、Transfer、Recharge 不广播公共事件**；仅从事件数推断这些动作不存在会漏报。页面 Agent 快照也不包含完整 Key 库存，无法替代原始状态／观察。

## 还原命令、效果和策略意图

需要公开事件之外的证据时，用小型临时 Rust 检查器复用项目模块，无须重写 IARP 解析器或规则。优先建立独立临时 crate 并使用明确的 path dependencies；避免随意选取残留 rlib 或混用不同构建产物。

本目录提供两个可直接复制的模板：

- `inspect-template.rs`：单场只读检查器。规则逐轮重执行并校验 state hash；可选 BuilderCore 系策略影子还原（`enl=BuilderDefender,res=BuilderDisruptor` 形式的 spec），逐轮 assert 生成的命令与档案提交一致；证据输出 JSONL。仓库 vendor 离线且没有 serde_json，模板手写 JSON，只依赖三个 path 依赖。Logistics/Adaptive 有私有状态机，不在模板支持范围。
- `analyze.py`：消费该 JSONL 的标准库分析器（无任务率、任务分布、补血窗口的到达时间 vs 倒塌时间、攻击命中 vs 受害方存活场、Drop 原因），可 `from analyze import Trace` 作库扩展。

检查器输出统一走 JSONL：一行一个带 `kind` 的对象，Python `json.loads` 后按字段名过滤。不要用 Debug 字符串 TSV 加 awk 数列号消费输出——Debug 表示和列序都不是稳定接口，字段名才是。Rust 只负责解码、重执行、校验和导出；窗口、join、几何等分析放在 Python 侧。

**规则重执行**可参考 `materialize`：从回放 header 的完整 map/config/spawns 建立 `GameState`，使用 `Resolver::with_seed(header.game_seed)`。每轮先取得双方 `Observation::for_faction`，再向 `run_round` 传入该轮原始 `submissions`，读取 `RoundReport`，最后 `advance_round` 并与记录的 state hash 对齐。

| 数据 | 取证内容 |
|---|---|
| `RoundRecord.submissions` | 原始有序命令、阵营、Agent、接收时间和顺序 |
| `RoundReport.execution_order` | 实际执行顺序、每命令 `Ok(CommandEffect)`／`Drop(DropReason)` |
| `CommandEffect::Recharged` | `hp_before`、`hp_after`，识别零恢复 |
| `CommandEffect::Transferred` | 接收者、实际 Supply 和 Keys |
| `CommandEffect::Attacked`、`Linked` | 实际攻击命中、连线和新闭合的场 |
| 轮前 `Observation.own_agents` | 位置、Supply、Keys，核对局部替代行动 |
| 候选请求与 `TaskBoard` grants | 任务评分、容量、预留、无任务原因；不在 replay 中，需单独还原 |

**策略重执行**另需 `matches.tsv` 和 `policies.tsv` 中的 bot seed、身份和全部参数，IARP 的 game seed 不能代替 bot seed。可复用 `balance::record::parse_balance_tsv`、`PolicyParameters::from_tag`、`PolicySpec::validate`、`make_policy_player`，并检查真实版本；旧格式缺参数时从该批次其他完整记录恢复，不能静默填当前默认值。

调用原有 `choose_spawns` 并核对出生集合，因为它可能消耗策略 RNG 和初始化计划。之后保持同一 player 实例，从 R1 连续输入合法观察，比较生成的有序提交与档案。仅从 checkpoint 重建 GameState 不包含策略 RNG、HackLedger 或运输状态，不能用一个新 player 冒充原策略。

需要解释候选排序时，优先给匹配版本的原策略增加只读 trace。已有公开模块支持部分策略的影子还原，但不要假设所有策略只有一个 BuilderCore：Logistics 还有私有运输状态，Adaptive 可能维护额外记忆。任何还原方式都应先对齐该策略的全部原始提交。

## 代码定位索引

| 路径 | 关键入口／用途 |
|---|---|
| `crates/arena-bots/src/r2/build.rs` | `BuilderCore::refresh/build_requests/fallback_requests/execute`；活动计划、任务供给与命令生成 |
| `crates/arena-bots/src/r2/tasks.rs` | `TaskRequest`、`TaskBoard`、`allocate`；优先级、容量、每 Agent 分配 |
| `crates/arena-bots/src/r2/value.rs` | `field_value`、`attack_value`、`defense_value`、`transfer_value`、`block_values`；函数存在后还须搜索真实调用 |
| `crates/arena-bots/src/r2/builder_defender.rs` | `defense_overlay`、目标上限与建造任务合并 |
| `crates/arena-bots/src/r2/builder_disruptor.rs` | `attack_overlay`、攻击容量、路程代价 |
| `crates/arena-bots/src/r2/logistics_builder.rs` | `apply_runner_scheme`、`pay_deliveries`；运输记忆、驻站、支付和命令替换 |
| `crates/arena-bots/src/r2/blocker_builder.rs` | `blocking_requests`；阻断候选是否改变建造行为 |
| `crates/arena-bots/src/r2/adaptive_hybrid_r2.rs` | 阶段、角色选择及其状态 |
| `crates/arena-bots/src/common.rs` | `link_is_legal`、`in_range`、`CommandList`；合法性和命令排序 |
| `crates/arena-bots/src/policy.rs`、`r2/rng.rs` | 身份／参数构造、独立 RNG namespace |
| `crates/arena-core/src/resolve.rs`、`types.rs` | 真实命令语义、成功效果和 Drop 原因 |

## 容易误读的细节

- 导出 `rounds[r-1]` 和 `agents[faction][r-1]` 是 **R 轮结算后**快照，不是该轮 Bot 决策输入。决定 R 轮行动时要看轮前 Observation；第一轮则来自初始状态。
- Viewer 的 Portal owner 编码为 `0=Neutral, 1=ENL, 2=RES`，Link／Field faction 编码为 `0=ENL, 1=RES`，不能混用。FieldDestroyed 事件的 faction 可能是执行破坏的一方；归属应回看被破坏的 Field。
- `Attack` 对操作范围内所有敌方 Portal 生效，没有显式目标参数。规划目标需从任务 trace 读取，实际命中从效果读取；顺带命中不一定是规划器有意选择。
- R2 通常按轮前观察选动作，命令会按真实顺序结算。转交后接收者能否同轮使用资源，要查提交和执行顺序；不要凭“Transfer 是辅助动作”推定可以。
- `Move` 本身不是闲置，距离不变的 Move 也不等于有效任务进展。报告人员利用率时保留无任务、等待、移动和有效动作各自的口径。
- `r2_spec()` 等当前默认构造不能代替归档的参数集；换成另一个策略时 RNG stream 也可能改变，不能据此声称只消融了一个角色。
- 主动 Blocking 的 readiness 计数是几何代理。普通建场也可能满足代理；应检查 overlay 是否改变任务或命令，及被阻止的候选，而非把全部交叉都算作策略收益。

## 输出建议

在新的分析目录保存单场身份摘要、验证结果、关键窗口表和可查看回放；完整 trace 按需保存。不要依赖既有 `.scratch/` 私人工具或旧报告才可复核。若使用脚本产生辅助指标，保存脚本或明确可重复的调用方式。

针对发现选择真正相关的收益：实际恢复 HP／延长得分轮次、交接相对直接连线的时间差、任务无人执行的连续区间、增援实际贡献、阻断导致的决策差异。固定数值门槛应来自该批次设计；本技能不另造一个“动作次数足够即通过”的验收门槛。

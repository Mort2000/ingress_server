# Alpha 规则与参考选手拆仓

2026-09-06。按用户决定，Alpha 默认充电恢复量从 15 HP 改为 **20 HP**，最大链距从 2000 m 改为 **1500 m**。Attack 仍为 20 HP，Recharge 仍消耗 1 Supply。权威值在 `arena-core::GameConfig::default()`，配置表同步更新。

## 决策依据与适用范围

[r2.6 充电敏感度](../balance/r26-recharge-sensitivity.md)显示 15→20 HP 在固定 Development 样本中改善防守收益，20→25 的增量不稳定。[B4 链距敏感度](../balance/balance-b4-sensitivity.md)显示缩短链距改善建设空间并收窄 Disruptor 的比分优势。这是 Alpha 的规则采用决定，不代表两个参数的组合已完成 Holdout／重新寻优确认，也不宣称已找到最优数值。

现阶段结束围绕现有启发式 Bot 的细粒度平衡扫描。已有数据支持主动进攻重要、混合策略中的按需防守有收益；在已覆盖测试内未发现重大规则漏洞。Transfer 已有局部正向证据，稳定整局收益继续暂缓评估。历史的阵营顺序、滚雪球与地图依赖继续保留为观察项。

停止细调的依据是现有策略响应不足以可靠区分更小的规则差异，而非要求 Bot 先达到局部最优。r2.6 已记录补给接替、连续维护和任务规划缺口，继续围绕这些行为调整规则容易把策略限制固化进规则。后续由 Alpha 选手提供更丰富的策略与反例，再决定是否重开针对性的规则实验。

Transfer 的保留依据包括 [r2.3 的实际交接对照](../balance/r23-behavior-validation.md)：交付 Key 后提前重建场，并改善该场比赛的最终分差。它说明机制有可兑现的价值，但不足以证明跨地图的稳定优势；复杂后勤规划是否值得投入，留待持续补给、资源需求和接替策略更成熟后再评估。

## 仓库职责

| 仓库 | 当前职责 |
|---|---|
| `ingress/server` | 规则、结算、协议、SDK、外部比赛、审核、沙箱、溯源与回放 |
| `ingress/bot` | 独立 Git／Cargo 仓，只有 Adaptive Alpha 参考选手和其测试、说明 |

Bot 通过同级 server 的官方 SDK／core 路径依赖构建，有自己的 Cargo.lock。server workspace 不引用 bot；平台测试使用脚本化动作与协议夹具，不依赖选手策略。`tools/probe-client` 是原协议故障夹具的新位置，仅供握手、断连、超时与沙箱测试，不是参赛策略。

```plantuml
@startuml
component "独立 bot 仓
Adaptive / ClientBot" as Bot
component "server 仓
官方 SDK / core 类型" as SDK
component "server 仓
外部比赛调度器" as Runner
component "server 仓
权威服务器" as Server
Bot --> SDK : 编译依赖
Runner --> Bot : 启动选手二进制
Runner --> Server : 创建比赛
Bot <--> Server : 官方协议与合法观察
@enduml
```

Adaptive 来源是迁移前 server 提交 `65a5855a27b5da037bf1705ee43f0810113926c9` 的 `crates/arena-bots/src/r2/`，行为为 r2.6 默认混合策略：从开局允许进攻、按需最多两人防守、辅助动作组合、五次充电成本的预警补采。保留其随机流身份与命令顺序；其它策略、注册表、历史消融参数序列化和诊断记录接口不进入新仓。攻击／防守模块只是同一 Adaptive 的任务候选生成器。

旧 `crates/arena-bots`、`bots/hybrid-client`、`bots/nearest-client`、专用 `combat-validate`／`r22-micro` 和 runner 中的历史策略实验模块从当前 server 源码删除。通用外部锦标赛保留，当前命令与选手接入方式以两个仓库的 README 为准。

## 历史兼容

历史报告、IARP 与运行参数不重写。旧报告中的“默认值”指其归档的 GameConfig；回放从记录头读取完整配置，因此仍可验证 15 HP／2000 m 的比赛。

旧报告中的源码链接和冻结实验命令需要在其对应历史提交中使用。迁移前完整源码可从 `65a5855a27b5da037bf1705ee43f0810113926c9` 检出；更早行为必须使用报告指定的提交或冻结源码，不能拿 Alpha 默认值替代归档规则。当前版本不提供 `balance-*`、内置策略排名或冻结的 `tournament-*-r1` 入口。

## 验证范围

迁移验收包含：默认参数与实际结算、server 独立构建和平台测试、Bot 独立构建与策略测试、旧／新 Adaptive 同输入逐轮命令比较、SDK 实际比赛、审核独立参与者以及回放导出。行为对照用于证明提取正确，不用于重新评估 Alpha 平衡性。

2026-09-06 最终结果如下。两仓均使用固定 Rust 1.98.0，以 `--release --locked --offline` 构建、测试；server 先构建 workspace 二进制，再运行 workspace 测试。

| 验收项 | 结果 |
|---|---|
| server 全套测试 | 471 项通过、0 失败；1 项既有 SDK doctest 忽略；包括协议故障、外部锦标赛、审核、沙箱与回放导出 |
| 独立 bot | 10 项测试通过；文档构建、格式检查通过；官方 `audit --root ../server --participant .` 为 PASS |
| 提取行为对照 | 8 张地图 × 2 阵营 × 2 对手（原 Adaptive／Disruptor）× 2 规则配置，共 64 场、32,000 轮；出生选择和每轮有序提交与原 Adaptive 全部一致 |
| 对照配置 | 旧规则 15 HP／2000 m 与 Alpha 20 HP／1500 m；每场 500 轮，双方始终重新决策 |
| 最终 SDK 对局 | `runs/external/alpha-reference-final-20260906`，500 轮正常结束，服务器及两个客户端退出码均为 0；完整回放验证与 HTML 导出通过 |
| 沙箱接入 | `runs/alpha-reference-sandbox`，两场各 100 轮均正常结束，阵营连接归属验证通过、无决策超时；两份回放均完整验证 |
| 仓库边界 | server 的 12 个 workspace 成员不依赖 bot；bot 只有一个 workspace 成员，依赖官方 SDK／core 与 blake3 |

沙箱接入记录中的 Landlock 为 ABI 7；本机没有 cgroup 委派权限，资源控制的这一层按平台既有规则记录为降级，不计为已验证的 cgroup 强制隔离。未安装 Clippy，未将其列为已完成检查。

替换测试选手时发现并修复了快速客户端的握手竞态：服务器须先登记连接再启动读取线程；调度器须及时保留进程身份。健康协议夹具的锦标赛测试现要求所有比赛正常完成。平台测试按当前构建 profile 查找二进制，避免误用旧 debug 产物。规则回放的冻结字节测试显式使用旧配置，未改写历史编码或哈希。

本地验证日志和提取对照程序在 `ingress/.scratch/alpha-extraction/`：全套平台结果为 `server-tests-r6.log`，Bot 为 `bot-tests-final.log`，行为对照为 `parity-final.log` 及八份 `parity-final-map*.log`，实际比赛为 `live-match-final.log`。最终 SDK 回放状态 hash 为 `8105dec56cf15850529906a5dd077241dd1caa32951ba5c292368b0e403a0d15`。源码与证据 SHA-256 清单见同目录 `verification.json`；运行产物不入 Git。

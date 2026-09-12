# 开发进度

本文记录当前阶段、关键里程碑和未完成事项。阶段结论与验证证据通过链接指向专题文档。

## 当前状态

- **已进入 Alpha**：2026-09-06 采用 20 HP／1 Supply 充电、1500 m 最大链距；规则与配置以 [规则文档](spec/rules/README.md) 为准。
- **平台与选手已拆仓**：server 维护规则、结算、协议、SDK、比赛调度、审核、沙箱与回放；Adaptive 参考选手位于同级独立 `adaptive_bot/`，server 可独立构建和测试。
- **独立选手验证延续 Alpha 决策**：Alpha 决策时尚未开展工作区 `reports/` 下的后续分析；之后引入 Astra 等多个独立 Bot，再开展策略、回放与参数复核。这是在落实“由独立选手提供新策略和反例”，并非恢复围绕旧参考 Bot 的细粒度扫描；见 [Alpha 决策](design/alpha-reference.md#决策依据与适用范围)与[独立选手分析](../../reports/README.md)。
- **正式回放工具已就绪**：[Tactical Replay](../viewer/app/README.md) 在浏览器本地打开 `.iarp`，完整验证后展示逐轮状态、库存、轨迹和命令结果。
- **四参数 Development 复核已完成**：综合 Astra 策略自博弈、抽样回放与外部对照，建议暂保留 20 HP 充电／20 HP 攻击／1500 m 链距／200 m 移动。15 HP 维修保留为候选方向，其结论受连续维修与协同策略能力限制；见 [实验报告](../../reports/astra-selfplay-balance-20260911/README.md)。
- **基础工程三批已交付**：第一批提供可信执行、恢复补赛、日志与回放身份；第二批补齐地图集、声明式赛事、冻结构建与版本对照；第三批提供 SDK 回执／计时、共享策略适配、从开局恢复的离线场景与 trace、动态领取及合作运行器本机资源池。见 [第一批说明](spec/iteration-batch-1.md)、[第二批说明](spec/iteration-batch-2.md)、[第三批说明](spec/iteration-batch-3.md)、[第三批验收](reports/iteration-batch-3.md)。离线复现及资源准入的适用边界已明确，cgroup 限制未消除；继续调参不作为本阶段主线。

## 关键里程碑

| 阶段 | 状态 | 关键成果与证据 |
|---|---|---|
| M0–M4：基础规则与结算 | 已完成 | 几何、随机数、战斗、经济、连线与场计分落地，支持完整 500 轮比赛。 |
| M5：确定性回放 | 已完成 | IARP 录制、校验与重执行完成；通过 1,000 场逐轮一致性验收。 |
| M6：基线策略与自对战 | 历史阶段已完成 | 完成 10,000 场基线实验，识别攻击支配、建造缺陷与阵营偏差；见 [R1 报告](balance/balance-report-r1.md)。内置策略现已移出 server。 |
| M7–M9：外部运行与赛事平台 | 已完成 | 协议与 SDK、独立客户端、审核与沙箱、构建溯源、锦标赛调度及归档排名完成；见 [MVP 验收](reports/acceptance-mvp.md) 和 [锦标赛记录](reports/tournament-r1.md)。 |
| M10：回放查看 | 已完成 | 当前由 Tactical Replay 承接，数据处理与浏览器验证见 [回放设计](design/viewer.md)。 |
| M-final：MVP 收尾 | 已完成 | 确认的评审问题已修复，补齐冲突分析和 300 Portal 验收；当轮 debug／release 测试及格式检查通过。验收结论与剩余风险见 [MVP 验收](reports/acceptance-mvp.md)。 |
| B0–B1：平衡实验基础 | 历史阶段已完成 | 修正基线证据，建立配对实验、种子隔离、可复现归档与统计基础；见 [实验方案](balance/ai-balance-plan.md) 和 [B1 验证](balance/balance-b1-smoke.md)。 |
| B2–B3：策略就绪与默认配置消融 | 历史阶段已完成 | 当时版本通过就绪门槛并完成消融；结果受策略实现影响，后续复核已修正收益归因。见 [B2](balance/balance-b2-readiness.md)、[B3](balance/balance-b3-ablation.md) 及 [后续报告索引](balance/README.md)。 |
| B4：参数敏感度 | 历史阶段已完成 | 完成 Pilot、Development／Validation Full 与交互筛选；1500 m 链距方向稳定，B5 确认未完成。见 [B4 报告](balance/balance-b4-sensitivity.md)。 |
| r2.2–r2.6：策略行为复核 | 阶段验证已完成 | 修复建造、攻防、组合动作与补给行为；取得 Transfer 局部正例及 15→20 HP 充电的方向性证据。见 [平衡档案](balance/README.md)。 |
| Alpha：规则采用与拆仓 | 已完成 | server／Bot 独立构建测试、64 场逐轮命令对照、SDK 实际比赛、沙箱接入和回放验证通过；见 [迁移验收](design/alpha-reference.md#验证范围)。 |
| Alpha 后续：独立选手与新反例 | 已开展并形成分析 | 引入 Astra 等多个独立 Bot 后，开展工作区 `reports/` 下的策略比较、回放审查和攻防实验，承接 Alpha 的独立选手验证路线；见 [分析索引](../../reports/README.md)。 |
| 独立选手四参数复核 | Development 评估已完成 | 984 场固定桶自博弈、192 场有效外部对照／时序复核及 14 场预选回放审查；结果与限制见 [实验报告](../../reports/astra-selfplay-balance-20260911/README.md)，不替代 B5。 |
| Alpha 基础设施改进 | 三批已完成 | 第一批 INFRA-01／02／07：可信执行、恢复与回放身份；第二批 INFRA-03／04／05：地图集、声明式排程、冻结版本对照；第三批 INFRA-06／08：SDK 反馈、离线场景与 trace、动态领取及本机资源准入。第三批 Rust 全量 491 项通过，双轴审查发现已关闭。见 [第一批验收](reports/iteration-batch-1.md)、[第二批验收](reports/iteration-batch-2.md)、[第三批说明](spec/iteration-batch-3.md)、[第三批验收](reports/iteration-batch-3.md)。 |

历史 B2–B4 的完成状态仅对应报告中的策略版本和归档配置。后续策略修复未替代正式重验，Alpha 参数采用也不代表完成了 B5 或找到了最优参数。

## 未完成与观察项

- **B5 保留历史未完成状态**：Holdout 未使用，现有 Development 复核未完成策略重新寻优；B5 不作为基础工程改进或新一轮选手迭代的前置条件。后续若独立选手暴露具体规则风险或形成明确的参数采用问题，再启动针对性验证。
- **Transfer 稳定整局收益评估暂缓**：已有局部收益证据，跨地图稳定优势仍待更成熟的持续补给与接替策略验证。
- **持续观察策略与规则边界**：补给接替、连续维护、任务规划，以及阵营顺序、滚雪球和地图依赖；见 [Alpha 决策依据](design/alpha-reference.md#决策依据与适用范围)。
- **隔离验证仍有环境限制**：既有验收环境未委派 cgroup，相关控制按降级记录；隔离边界见 [沙箱设计](design/sandbox.md)。
- **历史工程欠项待随模块复核**：公共 API 前置条件、错误类型、边界测试及高负载计时稳定性等，见 [MVP 剩余风险](reports/acceptance-mvp.md#residual-risk-known-debt-that-remains-at-mvp-handoff)。

## 更新与追溯

后续更新只记录阶段状态、关键结果、阻塞或待办，并链接验证证据；实现决策、排查过程和逐项测试明细分别放入 ADR、设计文档或专题报告。构建与运行入口见 [仓库 README](../README.md)。

历史报告中的策略版本、默认值、源码路径和实验命令按其归档理解。旧 `D-*` 决策及中间记录可通过 Git 查询整理前版本：`git show 8f6d4e051851:docs/progress.md`。

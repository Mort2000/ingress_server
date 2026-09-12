# AI 代码竞技场 v0.1 文档集

本目录维护规范性规则、历史 MVP 实施方案，以及后续阶段的工程需求与交付计划。

## 文档入口

| 文档集 | 作用 | 入口 |
|---|---|---|
| 游戏规则 | 定义比赛中什么行为合法、如何结算；属于规则事实源 | [rules/README.md](./rules/README.md) |
| MVP 方案 | 定义以什么顺序实现、验证和交付；不得覆盖规则 | [mvp/README.md](./mvp/README.md) |
| Alpha 基础设施改进 | 选手自主迭代、地图资产、赛事调度、回放身份及三批交付计划 | [iteration-infrastructure.md](./iteration-infrastructure.md) |
| 第一批使用说明 | 可信执行、恢复补赛、日志、选手身份及 IARP v2 兼容 | [iteration-batch-1.md](./iteration-batch-1.md) |
| 领域语言 | 统一规则与实现使用的核心名词 | [CONTEXT.md](../../CONTEXT.md) |
| 决策记录 | 记录难以反转且存在真实取舍的决定 | [ADR 索引](../adr/README.md) |

## 建议审阅顺序

1. 玩法与规则审阅：规则总览 → 比赛/地图 → Agent/动作 → Portal/资源 → Link/Field/得分 → 可见性/结算/RNG → 参数 → 平衡性与范围。
2. 工程方案审阅：MVP 总览 → 架构 → Core Simulator → Bot/Self-play → Runtime/Sandbox → Tournament/Replay → 里程碑/验收。
3. 只审数值时直接查看 [`R-CONFIG`](./rules/06-configuration.md)；只审交付边界时直接查看 [`M1`—`M10`](./mvp/06-roadmap-acceptance.md)。
4. 新一轮基础工程按 [Alpha 基础设施改进与交付计划](./iteration-infrastructure.md)审阅需求、缺口和验收条件。

## 引用约定

- 规则条款使用稳定编号 `R-<DOMAIN>-<NN>`，例如 `R-SCORE-01`。章节移动时编号不变，删除后不复用。
- 研发阶段使用 `P0`—`P6`，里程碑使用 `M1`—`M10`，验收项使用 `A-<DOMAIN>`。
- Alpha 基础设施需求使用 `INFRA-01`—`INFRA-08`，验收项使用 `A-INFRA-*`；它们不覆盖历史 MVP 的阶段与完成状态。
- 参数名使用大写常量名；所有默认值以 [`06-configuration.md`](./rules/06-configuration.md) 为准，其他文档只引用参数名。
- 相对链接指向具体专题文件；讨论和 Issue 中同时写“文件 + 编号”，避免依赖中文标题生成的锚点。

## 变更边界

本次重编压缩重复说明、集中参数并显式标出原稿中的待确认项，不改变 v0.1 的玩法目标。后续若修改规则，应先改规则文档，再同步实现方案与测试。

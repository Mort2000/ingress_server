# 文档索引

本目录按用途分组:`spec/rules/` 是当前规则事实源,`spec/mvp/` 保留 MVP 实施方案,`adr/` 记录已冻结的架构取舍,`design/` 说明子系统机制,`balance/` 收录平衡实验计划与阶段报告,`reports/` 保存正式结论,`progress.md` 汇总当前阶段、关键里程碑与未完成事项。

| 分组 | 内容 |
|---|---|
| [spec/](./spec/) | v0.1 规则文档集（唯一事实源，条款编号 `R-*`）、历史 MVP 实施方案与 [Alpha 基础设施改进计划](./spec/iteration-infrastructure.md) |
| [adr/](./adr/) | 架构决策记录:难以反转且存在真实取舍的决定 |
| [design/](./design/) | 子系统设计:沙箱隔离、回放观看器、构建溯源 |
| [balance/](./balance/) | 历史平衡实验与 r2.2–r2.6 报告；当前状态见 [平衡档案索引](./balance/README.md) |
| [reports/](./reports/) | 正式结论:MVP 验收、r1 锦标赛 |
| [progress.md](./progress.md) | 当前进度、关键里程碑、待办与证据入口 |

临时分析、验证脚本与中间产物放在仓库根的 `.scratch/`,批次运行原始数据放在 `runs/`,两者均不入版本库;正式结论只从本目录引用。术语统一见根目录 [`CONTEXT.md`](../CONTEXT.md)。

Alpha 当前规则与仓库职责见 [规则与参考选手拆仓](design/alpha-reference.md)。

# 文档索引

本目录按用途分组:`spec/` 是规则与实施方案的规范性事实源,`adr/` 记录已冻结的架构取舍,`design/` 说明子系统机制,`balance/` 收录平衡实验计划与阶段报告,`reports/` 保存正式结论,`progress.md` 是按时间推进的里程碑日志。

| 分组 | 内容 |
|---|---|
| [spec/](./spec/) | v0.1 规则文档集(唯一事实源,条款编号 `R-*`)与 MVP 实施方案(里程碑 `M1`–`M10`) |
| [adr/](./adr/) | 架构决策记录:难以反转且存在真实取舍的决定 |
| [design/](./design/) | 子系统设计:沙箱隔离、回放观看器、构建溯源 |
| [balance/](./balance/) | 平衡实验计划与 B1–B4 阶段报告；当前状态见 [r2.3 Bot 行为优化与验收](./balance/r23-behavior-validation.md) |
| [reports/](./reports/) | 正式结论:MVP 验收、r1 锦标赛 |
| [progress.md](./progress.md) | 里程碑日志与冻结决策清单(`D-*` 编号) |

临时分析、验证脚本与中间产物放在仓库根的 `.scratch/`,批次运行原始数据放在 `runs/`,两者均不入版本库;正式结论只从本目录引用。术语统一见根目录 [`CONTEXT.md`](../CONTEXT.md)。

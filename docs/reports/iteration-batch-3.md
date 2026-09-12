# 第三批验收记录

范围：INFRA-06／08。审查基点：`c230e8b`。使用入口与明确限制见 [交付说明](../spec/iteration-batch-3.md)。

当前状态：已交付并验收。实现提交 `4754bed`，审查修复提交 `c596a79`；验收只覆盖下述公开接口及说明中声明的复现／资源边界，不代表任意策略或主机的强隔离保证。

已运行的定向验证：

- SDK 真实对局中，回执保留 round／Agent、接受与重复状态，最后一轮回执也送达；逐次提交时间与回调墙钟时间分开记录。
- 显式预算控制的状态型策略保持提前提交，预算缺失报错；离线历史从开局恢复并定位首次命令差异。
- `scenario` 复制后不依赖原回放；独立模板使用同一策略在 SDK 实际比赛与离线观察历史产生相同有序命令，trace 保留第二轮策略状态。
- 真实短长比赛组合中，空闲 worker 继续领取第三项，早于长任务结束；结果仍归属原赛程项。
- 两个独立 runner 共享单槽池，执行区间无重叠；CPU／批次内存硬限制缺失明确标记降级。
- `timing-report` 同时保留客户端声明耗时和服务端实际接收信息，不混为 CPU 计算时间。
- 真实 CLI 测试覆盖无 IARP 的赛前失败及正常回放两种场次；非 UTF-8 日志不阻止报告，保留原始路径及损坏标记；逐场开始／结束记录带时间戳的主机快照。

## 验证证据

在 server 独立仓库执行，未修改其他 Bot 仓库：

| 检查 | 结果 |
|---|---|
| `cargo check --workspace` | 通过 |
| `cargo build --workspace` | 通过，包含独立策略模板产物 |
| `cargo test --workspace` 最终全量 | 491 项通过、56 个结果组；保留 1 项既有 ignored doctest |
| `cargo fmt --all -- --check`、`git diff --check` | 通过 |
| SDK runtime／strategy／history | 真实反馈、提前提交、显式预算、有状态历史恢复通过 |
| runner `diagnostic_cli` 3 项 | 场景自包含、同策略在线／离线输出、异常日志与时序证据通过 |
| runner `iteration_cli` 9 项 | 包含第二批回归及新增预检、动态领取、跨进程资源协调，通过 |

本地完整日志：`/tmp/ingress-batch3-tests-final.log`。没有设置跳过实际外部测试的开关。Viewer／WASM 接口未改；原生 Viewer 回归随 Rust 全量执行，本批没有另做浏览器验收。

初次全量 `/tmp/ingress-batch3-tests.log` 在既有 `split_and_pipelined_writes_reassemble_in_order` 测试失败：实际回执 Agent 0，而握手 roster 首项是 Agent 3。独立循环可复现；测试把名为 enl 的并发线程误当作必然先连接，提交中硬编码 0／1。改用实际握手 roster 后原断言不变，连续 50 次独立通过（`/tmp/ingress-batch3-transport-fixed.log` 保存最后一次结果），随后全量通过。仅修正测试输入，不修改服务端接收顺序或放宽断言。

## Standards

独立审查发现 0 项硬违反、1 项判断性重复代码：场景入口与 SDK reconstruct 重复完整重放校验。已统一到 reconstruct，在创建输出目录前完成校验；复核关闭，无未关闭发现。

## Spec

独立审查发现 3 项运行测量缺口：逐场克隆陈旧预检负载、无回放场次遗漏资源／日志、非 UTF-8 输出阻止整份报告。新增公开 CLI 回归先红后绿，分别补充逐场采样、按 attempt 获取证据及日志容错；复核全部关闭，无未关闭发现。

环境与限制：本机资源池只约束合作运行器；cgroup 环境限制未消除。离线精确比较限于显式初始化、合法观察和注入预算决定输出的策略；完整回执驱动状态、真实时钟／线程竞争不在自动复现保证内。无中途 Bot 状态快照恢复。

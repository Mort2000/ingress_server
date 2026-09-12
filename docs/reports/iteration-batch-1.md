# Alpha 基础设施第一批验收

日期：2026-09-12。范围：[第一批需求](../spec/iteration-infrastructure.md)的 A-INFRA-01／02／07。用法与编码约定见 [交付说明](../spec/iteration-batch-1.md)。默认规则和计分未改动，不开展新的平衡实验。

## 交付与验证

实现提交：`f17a995`；审查修复：`4c7e221`、`2754105`。审查基线为用户确认的 `5df7c8f`。以真实 CLI／进程、IARP 编解码、Viewer 浏览器为测试边界。

| 验收项 | 检查与结果 | 可重复证据 |
|---|---|---|
| A-INFRA-01 | 同名批次不同目录并发；客户端实际读取的实例、阵营与 SHM 名称一致；socket／SHM 冲突不删除现有资源；阵营证明失败拒绝计分 | [CLI 集成测试](../../crates/arena-runner/tests/tournament_cli.rs)、[服务端隔离测试](../../crates/arena-server/tests/isolation.rs)、[外部比赛测试](../../crates/arena-runner/tests/tournament_external.rs) |
| A-INFRA-02 | 计划先落盘；中断后复用已完成项；部分初始化可恢复；清单提交后丢失状态或写诊断失败不重赛；故障重试保留证据和原因；同批次锁冲突拒绝；损坏回放拒绝复用；直接／沙箱高输出持续排空并显示截断 | [CLI 集成测试](../../crates/arena-runner/tests/tournament_cli.rs)共 10 项通过；[持久化实现](../../crates/arena-runner/src/tournament/durable.rs) |
| A-INFRA-07 | 注册身份与独立服务端自报信息进入单文件回放；IARP v1 仍可读；v2 身份损坏拒绝、状态哈希不变；同名不同版本、未知版本、Unicode／特殊字符、异常回放、跨轮／视角／文件切换；身份 payload 原生与 WASM 一致 | [编解码测试](../../crates/arena-replay/tests/replay_flow.rs)、[独立服务端测试](../../crates/arena-server/tests/live_match.rs)、[Viewer 夹具](../../crates/arena-viewer/tests/payload.rs)、[浏览器测试](../../viewer/app/tests/replay.spec.ts) |

验证命令与结果：

- `cargo build --workspace`、`cargo check --workspace`、`cargo fmt --all -- --check`：通过。
- `cargo test --workspace`：通过，包含真实服务端／客户端、沙箱和 1,000 场确定性回放 gate，没有设置跳过真实运行的环境变量。
- 最后的提交点诊断修复后，`cargo test -p arena-runner --test tournament_cli`：10 项通过。
- Viewer `npm run build`（包括 WASM 构建与 TypeScript 类型检查）、`npm test`：通过，5 项浏览器测试。新增身份回放的完整 WASM payload 与原生输出一致。
- `git diff --check`：通过。

测试曾揭示并修复：沙箱等待退出后才排空日志、服务端确认先于进程树快照刷新造成误拒绝、旧格式负例仍将 v2 当作未知格式、旧 SHM 测试要求覆盖残留，以及 Viewer 装饰性 Key 圆环拦截 Portal 鼠标事件。相关失败未被跳过或改为放宽成功断言。

## Standards

独立规范审查：0 项明确规范违反，2 项实现／设计问题，均已关闭。子进程立即进入资源守卫；日志线程创建失败以错误返回，先终止进程再等待输出。尝试状态使用统一类型和恢复规则，不将未知值默认当作中断。

## Spec

独立需求审查：4 项原始缺口及 1 项后续存储失败变体，均已关闭。具体为无状态目录恢复、已提交结果的统一采用、无沙箱工作目录隔离、异常退出诊断收尾，以及提交点之后的监督错误记录不得破坏证据清单。

审查复核结论：Standards 0 项未关闭；Spec 0 项未关闭。

## 保留边界

- 冻结地图内容已用于恢复执行，但多布局生成器、地图集工具、声明式赛事、新旧版本比较仍属第二批。
- 第一批核对登记路径的二进制哈希，不是不可变构建仓库；执行期间不得替换该文件。完整构建登记在第二批完成。
- 不自动升级旧赛事归档，不自动删除归属不明的崩溃残留；不会重新使用旧实例的资源名。
- 单机操作系统文件锁负责同一赛事排他运行；多批次全局资源预算和动态调度仍属第三批。
- 无沙箱模式不提供对恶意派生／逃逸进程的完整隔离。现有 cgroup 未委派环境限制不变，继续记录降级。
- 回放校验与阵营绑定记录都不是构建身份的密码学认证。

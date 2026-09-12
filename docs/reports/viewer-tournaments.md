# Viewer 独立赛事监控验收

日期：2026-09-12。范围与使用说明见 [规范](../spec/viewer-tournaments.md)。审查基点 `ff56010`。

## 已交付

- 官方只读归档查询，复用计划／单场验证与 Elo，不启动进程、获取执行锁或改写汇总。
- `viewer/host` 独立 Cargo workspace 的回环 HTTP 后台，令牌授权，同源静态资源与只读查询，多赛事发现、增量缓存、分页及筛选。
- Viewer 赛事页：完成进度、明确未确认的运行状态、最新结果、暂定排名、尝试历史、按需载入正式 replay、返回保留筛选及断线提示。
- 保留纯静态手动文件模式，不修改规则、协议、IARP、runner CLI 或审核白名单。

## 验证记录

按已确认的三个公共边界实施 TDD，先观察缺失行为的失败，再实现并复测：

| 边界 | 覆盖 |
|---|---|
| 官方只读查询（9 项） | 真实计划／完成结果、整批结束前可见的单场提交、未提交不开放、恢复尝试不重复计分、缓存失效、计划重校验、外部符号链接拒绝、无 IARP 的技术终止、非 ASCII 哈希隔离、临时目录检查、有界文本读取 |
| 后台真 HTTP／CLI（3 项） | 同源页面、启动后发现新计划、多赛事与不完整计划共存、分页／版本／地图／结果筛选、原始 replay 字节一致、目录失联保留旧快照、令牌／Host／Origin／写方法／路径拒绝、慢请求与并发、非法启动参数 |
| 浏览器新增流程（1 项） | 真实后台和 runner、自动更新、Unicode 名称与版本、筛选、排名、真实 WASM 回放、返回保留筛选、390px 布局、断线保留旧列表、同端口后台重启后重新授权和原有文件模式 |

最终全量验证：

| 命令（server 根目录） | 结果 |
|---|---|
| `cargo test --workspace -- --test-threads=1` | 500 项通过、0 失败；原有 recorder 文档示例 1 项 ignored，无新增忽略 |
| `cargo test --manifest-path viewer/host/Cargo.toml` | 3 项 HTTP／CLI 测试通过 |
| `npm --prefix viewer/app test` | 6 项通过（原有 5 项 + 新增 1 项） |
| `npm --prefix viewer/app run typecheck`、`npm --prefix viewer/app run build` | 类型检查、WASM／生产构建通过 |
| `cargo fmt --all --check`、`cargo fmt --manifest-path viewer/host/Cargo.toml --check`、`git diff --check` | 全部通过 |

本批文档 54 个相对文件链接检查通过；1440px 和 390px 赛事页面截图人工检查通过。

## 双轴审查

固定基点 `ff56010`，主实现 `2d3fe2e`，修复 `3981bb2`。按 implement／code-review 流程由两个独立审查者分别核查，复核后无未关闭发现。

### Standards

没有硬性规范违反。两项 P3 可维护性建议已关闭：后台缓存类型化 Snapshot，在 HTTP 边界序列化分页行；attempt 状态采用枚举及统一标签，计数来自实际状态，移除重复列表。

### Spec

三项 P2 和一项 P3 已关闭：损坏 UTF-8 哈希不再使扫描线程 panic；临时目录接受类型／深度／数量检查；未提交状态与原因文本限制为 64 KiB；未知比赛／attempt 返回 404，已知但不可播放的 attempt 返回 409。新增回归先复现失败，再验证修复。未发现新增范围扩张。

审查汇总：Standards 2 项、Spec 4 项，全部关闭；两轴均无剩余最高级别问题。

## 集成修正与边界

初次 Rust 全量测试发现审计工具对根 Cargo.lock 的外部依赖集合做严格检查，HTTP 解析器即使不在 SDK 依赖树中，也不能加入该锁文件。独立命令稳定复现后，将后台移入与 viewer/wasm 相同的独立 workspace 结构；根锁文件恢复不变，26 项审计回归通过，没有扩大选手白名单或修改审计逻辑。

并行 Rust 全量复测曾分别遇到既有 `address_space_limit_aborts_the_malloc_bomb` 的限制归类失败，以及 `structured_arguments_and_environment_reach_sandboxed_registered_builds` 的首连接确认超时。前者单项和整组 enforcement 11 项复测通过，最终串行全量中两项均通过。没有修改沙箱、执行逻辑、测试断言或超时阈值；最终全量以 `--test-threads=1` 执行，减少同一测试程序内的并行竞争。现象与负载／时序敏感性一致，但本批未完成其根因定位，不能据此宣称默认并行测试已稳定。

文件时间信息仅用于缓存失效，不是签名。下载复核实际返回字节的哈希。操作目录须由操作者控制，不宣称能抵御恶意同用户进程并发替换路径；正式比赛不得向选手提供监控资料。只读状态不是进程存活证明，也没有逐轮遥测。后台与比赛共享 CPU／磁盘，首次验证可能较慢，没有零开销或固定延迟承诺。

纯静态模式不需要赛事后台，但重新加载 Worker／WASM 仍需静态资源可访问。没有新增离线应用缓存。远程暴露、AF_UNIX、网络挂载以及运行控制不在本批范围。

# Tactical Replay 前端验收

日期：2026-09-06。界面依据本地 `replay-hud-design.html` 设计，正式源码位于 `viewer/app/`，独立 WASM 桥接位于 `viewer/wasm/`。操作与构建步骤见 [前端 README](../../viewer/app/README.md)。

## 需求与证据

| 需求 | 实现与验证 |
|---|---|
| 正式 Vite HUD | TypeScript + SVG；保留顶部比分、阵营名册、地图、Agent 档案、底部曲线 / 时间轴布局；生产构建成功，1440×900 与 390 px 布局截图检查通过 |
| 浏览器直接读取 IARP | 文件选择和拖放传入 Worker，Rust WASM 执行完整验证 / 物化；浏览器产出的完整数据与同一归档的原生 JSON 深度相等，无外部资源请求 |
| 局部放大 | 滚轮锚点缩放、按钮、框选、平移、全图及 Agent 定位；自动测试检查 viewBox 变化和复位、缩放时标记屏幕尺寸稳定 |
| 选中 Agent 轨迹及当轮高亮 | 使用成功 CommandOutcome 的每段 from / to，历史淡色、当前轮亮白；两段成功移动夹一次失败 Move 的真实 IARP 验证折点、排除失败意图 |
| Supply 与 Key | 逐轮 Observation 对照库存，UI Supply 数值与原生状态一致；初始状态 / 增援 / 消耗后的库存按当前轮显示 |
| Key → Portal、Portal → 持有人 | 指针及焦点事件测试；地图 Portal、地图 Agent 和名册联动；按当前视角过滤持有人 |
| 设计细节 | 配置驱动的操作范围圈、HP 外弧、轨迹轮次、全部成功移动预算、命令结果、公共事件、得分曲线及快捷键 |
| 文件与回合边界 | 损坏输入报错并保留原回放；文件替换、异常结束、零轮、增援前后和四种视角均有浏览器覆盖 |

设计稿 MOCK 的移动标注使用 SU，正式实现依据实际规则改为距离预算（米），不会把移动描述为 Supply 消耗。

另外在开发页面读取现有 500 轮 / 120 Portal 对局，约 0.4 秒完成读取与校验；跳至 R350 后可见 12 个 Agent，未产生浏览器运行错误。该耗时为本机单次观察，不是性能保证。

## 首次前端实现的检查结果

- `npm run build`：WASM、TypeScript 与 Vite 生产构建通过。
- `npm test`：4 项 Chromium 端到端测试通过。
- 数据处理现由 `arena-viewer` 提供；当前验证命令见前端 README。
- 根工作区回归按包完成验证：首次 `cargo test --workspace` 中包括耗时较长的 `arena-runner` 全部通过；修复下述依赖集成问题后运行 `cargo test --workspace --exclude arena-runner` 验证其余包，并对沙箱用例单独复核。
- `cargo test -p audit --test audit`：最终目录布局下 24 项通过。
- `cargo test -p sb-fixtures --test enforcement -- --test-threads=1`：释放本次 Vite 预览进程后 11 项全部通过。
- `cargo test --workspace --doc`：通过。
- 独立 workspace 的 `cargo test --locked --manifest-path viewer/wasm/Cargo.toml --target-dir target/replay-web`：编译及文档测试通过；WASM 运行行为由上述浏览器测试覆盖。
- Standards / Spec 双轴审查完成，发现的重复高亮调度与设计缺漏已修正。

首次全量运行不是单次全绿：WASM 依赖加入根 lockfile 曾触发冻结白名单检查；现将桥接独立到 `viewer/wasm`，根 `Cargo.toml` 和 `Cargo.lock` 与开发前一致，保留原审计闭包。并行测试时沙箱出现过 `Cannot fork` 和内存测试超时；串行复核、释放预览进程后全部通过，沙箱实现和限制未修改。

## 工作区清理后的验证

正式前端的数据处理现位于 `crates/arena-viewer/`，WASM 直接调用该库。旧页面资源和 HTML 导出入口已移除。

- `cargo test -p arena-viewer --locked`：10 项单元测试、7 项集成测试通过。
- `npm run build`：WASM、TypeScript 与 Vite 生产构建通过。
- Rust 全工作区经构建、串行测试与定向补跑，462 项不同测试均有通过记录，1 项文档示例忽略。首次实时测试使用了过期客户端二进制；重建后通过。沙箱测试缓存还保留工作区迁移前的绝对路径；清理并重新构建 `sandbox-runner`、`sb-fixtures` 后，两组测试共 21 项通过。这不是单次全仓测试全绿。
- `npm test`：3 项通过、1 项失败。失败位于 `tests/replay.spec.ts:270`，Agent 的 `key-ring` 圆环遮挡 Portal 文本，导致悬停超时；WASM 与原生数据一致性检查通过。此次清理未修改前端交互代码。
- Rust 格式、脚本语法和 `git diff --check` 通过。

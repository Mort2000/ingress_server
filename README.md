# Ingress Arena Server

AI 代码竞技场的官方规则与运行平台。参赛程序控制 ENL 或 RES 阵营，在 6 km × 6 km 的整数地图上占领 Portal、获取 Key、连接 Link，并以 Field 的面积和存续时间累计得分。默认地图 120 个 Portal，每方初始 3 个 Agent，比赛 500 轮。

Alpha 默认规则采用 **20 HP／1 Supply 充电、1500 m 最大链距**。本仓包含权威结算、服务器、参赛 SDK、外部比赛调度、审核、沙箱和回放工具。参赛策略位于同级独立 [bot 仓](../adaptive_bot/README.md)，仅保留 Adaptive 参考选手。

## 构建

需要 Linux 和 `rust-toolchain.toml` 固定的 Rust 1.98.0：

```sh
rustup toolchain install 1.98.0 --profile minimal --component rustfmt
cargo build --release --workspace --locked
cargo test --release --workspace --locked
```

server 可独立构建和测试，不依赖 bot 仓。参考选手单独构建：

```sh
cargo build --release --locked --manifest-path ../adaptive_bot/Cargo.toml
```

## 运行比赛

一场 Adaptive 自对战，自动启动服务器和两个客户端并记录 IARP 回放：

```sh
scripts/run-external-match.sh --match-id adaptive-demo
```

默认客户端为 `../adaptive_bot/target/release/adaptive-bot`，也可通过 `--client <binary>` 替换。产物在 `runs/external/adaptive-demo/`，每次使用新的 match id。脚本直接运行客户端；首个连接分配为 ENL，进程启动先后不保证阵营。两个客户端必须都在握手窗口内连接。

用调度器运行不同选手，按赛程分配阵营并使用沙箱：

```sh
target/release/arena-runner tournament --out runs/alpha-duel \
  --client ../adaptive_bot/target/release/adaptive-bot --client /absolute/path/to/your-bot \
  --matches 2 --maps 1 --rounds 500 --seed 12648430 \
  --sandbox target/release/sandbox-runner
```

`--client` 可重复，任意两名选手之间进行 `--matches` 指定的偶数场比赛；同一路径可登记两次自对战。`--workers` 默认 1；输出目录必须是新的。省略 `--sandbox` 可直接运行开发客户端。种子使用十进制。

锦标赛保存每场 IARP、日志、完整规则、地图、阵营映射、种子与二进制哈希，并输出赛程、排名和整数 Elo。正式锦标赛中的每一场单独计分；这里的偶数赛程用于两边覆盖。

`--fast`（单场脚本中为 `--config fast`）使用 40 ms 决策窗口，只适用于协议冒烟。测试 Adaptive 时使用默认 1 s 窗口；缩短比赛可用 `--rounds 50`。

## 编写与审核参赛程序

从 [Adaptive 仓 README](../adaptive_bot/README.md) 的 `main.rs → client.rs → adaptive.rs` 路径入手。客户端启动接口为 `<server-socket> <decimal-seed>`，由官方 SDK 接收阵营、地图与配置，读取本阵营观察并提交动作流。

审核独立仓库：

```sh
target/release/audit --root . --participant ../adaptive_bot
```

`--participant` 可重复。审核检查选手源码、依赖声明与各仓 Cargo.lock，合法的官方路径依赖指向本仓 `crates/*`／`tools/*` 成员。准入仍要求安全 Rust、获批依赖和官方 SDK 系统接口；沙箱说明见 [sandbox.md](docs/design/sandbox.md)。

## 规则与回放

- [规则文档](docs/spec/rules/README.md) 是规则事实源；[配置表](docs/spec/rules/06-configuration.md) 列出默认值。
- Attack／Recharge／Capture 共用每轮一个主动作槽；Move／Hack／Link／Transfer 按各自限制组合，移动预算每轮 200 m。Attack 不消耗 Supply，Recharge 消耗 1 Supply。
- Field 每轮按面积计分，`score2` 为两倍面积的整数累积值；中和锚点会删除相关 Link 与 Field。
- 权威结算、状态哈希和回放验证均使用确定性整数规则。客户端实际提交时间影响 Priority Bucket。

[Tactical Replay](viewer/app/README.md) 是正式回放查看工具。浏览器本地打开 `.iarp`，完成完整验证后查看逐轮状态、轨迹、库存和命令结果，无需上传文件。

## 仓库导航

| 路径 | 职责 |
|---|---|
| [crates/arena-core](crates/arena-core/) | 权威游戏模型、规则结算与通用测试 Harness |
| [crates/arena-server](crates/arena-server/) | 实时服务器、共享内存、计时和比赛记录 |
| [crates/arena-client-sdk](crates/arena-client-sdk/) | 参赛接口与合法观察 |
| [crates/arena-protocol](crates/arena-protocol/) | 协议及共享内存布局 |
| [crates/arena-replay](crates/arena-replay/) | 回放编解码、哈希与验证 |
| [crates/arena-runner](crates/arena-runner/) | 外部选手比赛调度、归档与排名 |
| [crates/arena-viewer](crates/arena-viewer/) | Tactical Replay 的已验证逐轮快照与遥测数据 |
| [tools](tools/) | 审核、沙箱、溯源；probe-client 仅是协议故障测试夹具 |
| [viewer](viewer/) | Tactical Replay 前端与 WASM 桥接 |
| [docs](docs/README.md) | 规则、设计、进度和历史报告 |

旧内置策略、策略消融驱动及 `balance-*`／`tournament-*-r1` 冻结命令已退出当前源码。历史报告和配置保留原样，复现应检出对应历史提交；当前布局及规则采用依据见 [Alpha 迁移记录](docs/design/alpha-reference.md)。

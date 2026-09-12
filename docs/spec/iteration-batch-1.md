# 第一批：可信且可恢复的执行

对应 [交付计划](iteration-infrastructure.md) 的 INFRA-01、INFRA-02、INFRA-07。实现日期：2026-09-12。规则、默认参数和计分方式不变。

## 官方入口

`arena-runner plan` 与 `tournament` 接收同一组选项；前者只冻结计划，后者冻结后立即执行。

```sh
arena-runner plan --out runs/demo --client /path/bot-old --client /path/bot-new --name old --name new --bot-version v1 --bot-version v2 --matches 2 --maps 1 --attempt-limit 3
arena-runner status --out runs/demo
arena-runner resume --out runs/demo
arena-runner retry --out runs/demo --item 0
arena-runner rebuild --out runs/demo
```

- `--name`、`--bot-version` 可以分别省略；提供时各按 `--client` 顺序提供一次。登记名在赛事内唯一，自报名称可以相同；展示名允许 Unicode，但不能含 TSV 分隔符。缺少 Bot 版本不会从文件名或 SDK 版本推断。
- `--attempt-limit` 在执行前冻结，默认 3，范围 1–100；初次运行和中断都消耗一次尝试。恢复不会自动重跑已完成结果或基础设施故障；后者需显式指定 `retry --item`。Bot 技术判负和双方技术失败都是已完成比赛，不具备故障补赛资格。
- `status` 不启动选手，`rebuild` 不需要选手二进制仍然存在。`resume`、`retry` 和 `rebuild` 使用操作系统文件锁，拒绝同一赛事的并发修改；进程死亡后锁自动释放，不删除锁文件。
- `--fast` 只用于协议功能检查。正式运行仍使用真实到达时间桶；并发和负载变化可能改变比赛结果。
- 地图池仍按既有赛程循环使用；`--maps 10 --matches 2` 不等于逐图换边。多布局地图集、声明式赛事与稳定版本登记仍属第二批。

## 持久化与采用规则

| 路径 | 语义 |
|---|---|
| `plan/tournament.json` | 解析后的选手引用、对阵、种子、并发、沙箱及预算 |
| `plan/config.bin` | 官方 codec 编码的完整规则配置 |
| `plan/maps/<index>.bin` | 完整 Portal 编码；执行使用冻结内容，不重新生成地图 |
| `plan/schedule.tsv` | 展开的逻辑赛程与阵营映射，保留既有随机输入 |
| `plan/attempt-limit`、`plan/manifest` | 重试上限与输入完整性清单；清单最后发布 |
| `matches/m<index>/attempts/<number>/` | 每次尝试的独立归档，补赛不覆盖旧目录 |
| `state`、`reason.txt` | 尝试状态及初次运行／中断恢复／故障重试原因 |
| `execution-id` | 独立于赛事名称与逻辑项的 128 位执行实例 ID |
| `result.tsv`、`meta.json`、`artifacts` | 单场结果、核验信息与产物哈希清单 |
| `matches/m<index>/adopted`、`adoptions.tsv` | 已完成且验证通过的采用结果及理由；可从尝试记录重建 |
| `index.tsv`、`ratings.tsv`、`standings.txt` | 可丢弃并重建的兼容汇总 |

元数据先写临时文件、同步、原子重命名并同步父目录。启动前先保存完整计划；单场完成立即保存，不等待 worker 或整批结束。恢复复核产物哈希及完整回放，检查地图、规则、种子、结局与阵营证明。计分按冻结赛程顺序重算，而不是完成顺序，不重复计分。

状态区分 pending、running、completed、infrastructure_fault 和 interrupted。运行中进程死亡后，状态查询将遗留 running 解释为 interrupted；恢复另建尝试。已验证完成结果不可按胜负重跑。故障证据留在原目录，补赛原因写入新尝试及采用记录。存储写入失败使命令显式失败；若连故障元数据都无法保存，保留原状态并在恢复时视作中断，不能宣称成功。

第一批冻结二进制路径及内容哈希，运行前和每场启动前核对，发现开发目录重建即拒绝。它不是第二批的不可变构建仓库；执行期间不得替换路径指向的文件。旧批次没有 `plan/manifest`，不猜测其输入，也不自动升级或覆盖。

## 进程、连接与日志

每次尝试从内核随机源生成实例 ID。SHM 和 `/tmp/ia-<id>/arena.sock` 使用实例身份；短运行路径不受归档目录深度和 Linux UDS 路径上限影响。已有 socket 或 SHM 名称一律拒绝覆盖；本批不自动清理无法证明归属的崩溃残留。

运行器先启动计划 ENL 选手，等待服务端驱动确认接受的连接及 `SO_PEERCRED` 属于该选手进程树，再启动 RES。失败即停止；不再依赖“进程持有任意已连接 socket”的猜测。赛后继续核验两个实际 peer PID。注册信息只在阵营映射通过核验后加入回放，客户端自报名称不用于认证。

无沙箱时双方 stdout／stderr 分开持续读取，每路保留前 1 MiB，另存总字节数、保留字节数和截断标志。沙箱内部同时排空双方输出，每路保留前 64 KiB，超出后继续消费并附截断说明；运行器还保存沙箱 JSON、沙箱自身输出与退出状态。日志写入失败归为运行器故障，不参与计分。无沙箱执行仍不承诺对任意派生／逃逸进程的完整安全隔离。

## 握手与 IARP 兼容

`ClientHello` 末尾添加可选 `bot_version`、`sdk_version` 字符串。FlatBuffers 旧客户端省略时解码为空，旧服务端忽略新信息；原有版本三元组和规则语义不变。保留 `client_name` 与 `client_version` 原值。

SDK 用 `ConnectOptions::new(socket, name).with_bot_version(version)` 声明策略版本，自动上报 SDK 版本。手动设置 `client_version` 仍只表示客户端版本，不等价于显式 Bot 版本。

IARP v2 在原头部双方出生列表后增加：`u32` 元数据长度、元数据字节、32 字节 BLAKE3 校验值。数字沿用小端编码。元数据为长度前缀 UTF-8 字符串：执行 ID，然后按实际 ENL／RES 顺序，每方包含注册 ID、登记名称、登记 Bot 版本、构建哈希、自报名称、原始客户端版本、自报 Bot 版本、SDK 版本、阵营核验状态（`verified` 或 `unverified`）。长度上限 256 KiB，未知状态、坏 UTF-8、校验错误和尾随字节均拒绝。

身份不参与游戏状态哈希。无身份的旧回放及离线 Recorder 仍可编码为 v1；读取器支持 v1、v2，但不放宽规则、RNG 或状态编码版本校验。旧解码器不能读取 v2，需升级；旧归档不迁移。

Tactical Replay 从单个 `.iarp` 经原生／WASM 同一数据处理读取身份。比分区显示阵营、选手名和 Bot 版本，详情分别列出登记信息、自报信息、构建哈希和核验状态。优先登记字段，其次显式自报，最后“选手未知／版本未知”。长字符串以纯文本显示，可通过标题和可展开详情读取全文；切换视角不改变身份，切换文件完整替换。回放通过规则校验不等于身份签名或构建认证。

## 验证记录

测试边界经确认：运行器 CLI／真实进程、IARP 编解码、Viewer 浏览器行为。测试包含同名批次并发的客户端 SHM 归属、中断恢复、已完成项复用、故障补赛留痕、拒绝结果重跑、直接／沙箱高输出、socket 冲突、旧格式兼容、身份损坏拒绝、单文件展示、同名不同版本、Unicode、跨文件替换及原生／WASM 一致性。最终测试与审查结果见 [进度](../progress.md)。

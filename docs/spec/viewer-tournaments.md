# Viewer 独立赛事监控

实现日期：2026-09-12。审查基点 `ff56010`，测试边界经用户确认：官方只读归档查询、真实后台 HTTP／CLI、浏览器赛事浏览与回放。验收记录见 [专题报告](../reports/viewer-tournaments.md)。

## 范围与职责

独立 Rust 后台同源提供 Tactical Replay 静态资源和只读赛事查询；直接读取显式允许的赛事目录，不依赖 runner 进程、网络接口或前端托管。支持赛事运行中接入、退出后浏览、多赛事、计划尚未完整发布、单场增量提交、结果筛选、暂定排名、attempt 历史及已提交 replay。现有手动打开 IARP 保留。

不修改规则、协议、IARP 或 runner CLI；不启动比赛、不自动恢复／补赛／重建，不添加遥测、心跳、逐轮实时观战或 AF_UNIX。没有稳定存活证据时，只显示磁盘记录的运行状态，不能把静默推断为死亡。

```plantuml
@startuml
participant "arena-runner" as Runner
database "赛事归档目录" as Archive
participant "Viewer 独立后台" as Backend
participant "浏览器 Viewer" as UI
Runner -> Archive : 发布冻结计划及单场结果
Backend -> Archive : 只读增量扫描、校验
UI -> Backend : 同源查询赛事／比赛／排名
Backend --> UI : 带扫描时间的快照
UI -> Backend : 读取指定已提交 replay
Backend --> UI : IARP
UI -> UI : 原有 Worker／WASM 验证与展示
@enduml
```

## 正确性要求

- 计划 manifest 和单场 artifacts 是发布契约；临时文件与未提交回放不作为结果。runner 收尾的根目录汇总不是实时事实源。
- 复用官方计划、产物、回放验证、结果采用及 Elo 语义，不另建计分规则。按冻结赛程顺序聚合当前有效结果，排名明确标记暂定；技术结果不混入正常 W/D/L。
- 每个逻辑比赛只计一次，历史 attempt 不重复计分。基础设施故障、技术结果、无回放、损坏分别展示。
- 原归档只读，监控不获取执行锁、不调用 rebuild。已验证内容按文件元信息缓存，变化后失效；下载前再核验所读字节。缓存不是防恶意篡改的签名机制。
- 扫描采用串行增量轮询；前端断线保留上次显示并标记过期，恢复后刷新。列表分页，回放按需加载，不预先物化全赛事。

## 安全与资源

默认仅回环地址；显式配置归档和静态资源根目录。严格验证 Host／Origin，不开放 CORS 通配。不接受任意文件路径，不暴露二进制、环境配置或原始目录；拒绝路径穿越、符号链接和非普通文件。访问只支持读取，限制请求头、连接处理数、超时和文件规模。

操作者必须控制归档目录，不能在读取过程中恶意替换目录／文件；本机服务不是任意恶意同用户程序的隔离边界，监控内容包含双方赛后私有信息，不授予参赛 Bot 访问权。CPU／I/O 与比赛仍共享，限制并发和重复验证，不宣称零影响。

## 使用

从 server 根目录构建（首次安装和 WASM 工具链见 [Viewer README](../../viewer/app/README.md)）：

```sh
cargo build --workspace
cargo build --manifest-path viewer/host/Cargo.toml
npm --prefix viewer/app run build
viewer/host/target/debug/arena-viewer serve --runs /path/to/tournaments --assets viewer/app/dist
```

打开终端打印的完整链接。默认绑定 `127.0.0.1:8080`，`--listen 127.0.0.1:0` 可分配空闲端口。每次启动生成随机访问令牌，通过 URL fragment 交给页面；页面移除 fragment，仅在当前标签页 sessionStorage 保存，查询和下载使用 Authorization 请求头。后台重启后重新打开新链接。不要公开包含令牌的终端输出。

`--runs DIR` 可以重复提供。每个 DIR 可以是一个赛事目录，或包含多个赛事的集合目录；只发现直接子目录，不递归扫描工作区。根目录需要预先存在，随后新创建的赛事会自动发现。赛事按路径生成定位 ID，复制到另一目录视为另一个入口；计划内容身份另行保存，不把名称当唯一身份。未完整发布或不兼容的计划显示错误／未就绪，不自动迁移历史目录。

`--poll-ms` 默认 2000，范围 100–60000。后台每轮扫描结束后等待该间隔，浏览器赛事页每 2 秒查询；“立即刷新”只读取最新快照，不强制重放整个赛事。首次验证随档案规模增长，不承诺固定延迟。相同计划被重新写入时复核内容；同一路径的计划身份改变则拒绝继续采用，需要操作者重启监控确认。单个事件损坏不阻止其他事件显示；集合目录暂时不可读时保留旧快照并禁用下载。

通过“赛事”按钮打开监控，“返回回放”退出。支持名称／注册 ID／版本、地图编号、状态和结果筛选，每页 50 场。最新完成排序使用第三批 resources 中的完成时间，旧 attempt 缺少时间时用赛程编号作为稳定后备，不伪造完成时间。排名总是整个赛事的已验证结果，不随筛选变化；分数按原始 `score2` 十进制整数展示。尝试记录可展开，历史结果不重复计分；无 IARP 的赛前失败显示无可用回放。

纯静态部署仍可手动打开 IARP，不必启动赛事后台。后台断开时已加载回放仍可查看、旧赛事列表保留；重新载入页面／Worker／WASM 仍需静态资源可访问，不承诺离线缓存整个应用。

远程使用建议后台与归档位于同一主机，通过 SSH 转发同一端口访问；Host 校验要求浏览器使用打印地址的 authority（例如远端监听与本地转发均为 `127.0.0.1:8080`）。不支持非回环地址或跨源模式。AF_UNIX、网络共享目录和远程多执行器汇聚不属于本批验收范围。

## 只读接口

所有数据接口要求 `Authorization: Bearer <token>`，页面与接口同源。仅 GET，无请求体，无 CORS 通配。未知资源返回 404；非法查询返回 400；未授权返回 401；Host／Origin 不符返回 403；写方法返回 405；回放未就绪／验证失效返回 409。

| 路径 | 返回 |
|---|---|
| `/api/events` | `arena-catalog-v1`：赛事摘要、扫描时间及目录级错误 |
| `/api/events/<id>` | `arena-monitor-v1`：选手、计数、排名、分页比赛及 attempts；未就绪时只有定位与错误字段 |
| `/api/events/<id>/matches/<index>/attempts/<number>/replay` | 已提交且验证通过的原始 IARP 字节 |

详情查询参数：`offset`（默认 0）、`limit`（默认 50，1–200）、`q`（名称／ID／版本子串）、`map`（地图编号）、`state`、`outcome`、`order=latest|schedule`。空筛选代表全部，重复和未知参数拒绝。状态为 pending、recorded_running、publishing、completed、infrastructure_fault、interrupted、corrupt；liveness 始终为 unknown，不获取执行锁来推断存活。

后台使用一个串行扫描线程和 8 个固定 HTTP worker；请求头至多 16 KiB／64 个字段，读取请求头总期限 5 秒，写响应总期限 15 秒，一次连接只处理一个请求。单文件上限 512 MiB，其中 attempt 的 state／reason.txt 文本进一步限制为 64 KiB；归档树限制深度与条目数，临时目录也接受检查。这是局部防御上限，不是进程 RSS／CPU 配额。首次归档验证和磁盘读取不受网络写超时强制中断，后台不提供实时性能 SLA。

HTTP 请求解析使用独立工具依赖 [httparse 1.10.1](https://docs.rs/httparse/1.10.1/httparse/struct.Request.html)，不提供通用 Web 服务器功能。后台采用 [viewer/host](../../viewer/host/src/main.rs) 独立 Cargo workspace／锁文件，沿用 viewer/wasm 的隔离方式；HTTP 依赖不进入根 Cargo.lock、SDK 或 WASM 依赖闭包，不扩大选手审核白名单。官方只读查询位于 [monitor 模块](../../crates/arena-runner/src/tournament/monitor.rs)，复用既有结果验证和计分逻辑；后台只依赖库，不需要启动 arena-runner。

## 验收

1. 真实 runner 计划／比赛可被只读查询；进行中单场已提交结果在整批结束前可见，崩溃／退出后仍可浏览。
2. 原始目录不被监控改写；未提交文件不发布；损坏不伪装完成；重试不重复计分；分数以十进制字符串跨 JSON 传递。
3. HTTP 真进程覆盖静态文件、查询、原始 replay、路径／Host／Origin／写方法拒绝、并发和启动参数错误。
4. 浏览器真后台覆盖刷新、筛选、选手版本、排名、打开 replay／返回保留筛选、断线和手动文件模式；保留现有回放回归测试。
5. 单项测试和类型检查随迭代执行；结束执行全量 Rust／浏览器测试、格式检查、双轴审查并提交。

---
status: accepted
---

# 协议 Payload 与 SHM Snapshot 统一使用 FlatBuffers

MVP 以 FlatBuffers Schema 定义有序连接的 `ProtocolEnvelope`、Public SHM 的 `PublicSnapshot` 和各阵营 Private SHM 的 `PrivateSnapshot`。Schema 与生成绑定显式版本化且不暴露 Rust ABI；Client 与 Server 在地图发布前握手，三个版本必须精确匹配，不做兼容协商，否则以无评分基础设施错误中止。Rust SDK 在验证 Buffer 后直接借出绑定到 `RoundContext` 的只读 View，所有 Client 输入也必须先通过 Verifier 再转换为 Core 自有权威类型。统一格式减少消息与零拷贝 SHM 两套边界模型的漂移，并保留未来生成其他语言 SDK 的路径；代价是同一场赛事必须锁定完整 Schema 版本，引入生成代码、演进纪律及需要审计的 FlatBuffers Runtime。

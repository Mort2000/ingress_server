---
status: accepted
---

# MVP 在 Client 侧使用零拷贝 Observation

官方 SDK 验证 FlatBuffers SHM Snapshot 后，直接向参赛逻辑暴露当前已发布 Buffer 的只读 `ObservationView<'round>`，不先复制为 Client 私有对象，以验证零拷贝状态读取路径。View 只能从非 `static` 的 `RoundContext<'round>` 借用，不得保存到 Bot 长期状态或交给脱离作用域的线程；Scoped Worker 必须在作用域结束前 Join，提交最后一个 Agent Stream 或 Commit 后 Context 关闭。回调越过 deadline 属于 Client Bug，Settlement Phase 合规仍依赖白盒审计；共享内存布局由独立版本化 Schema 定义且不得使用 Rust ABI，避免该选择破坏未来多语言 Client 的可能性。

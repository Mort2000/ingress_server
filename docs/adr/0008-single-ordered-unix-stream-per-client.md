---
status: accepted
---

# 每个 Client 使用一条有序 Unix Stream

MVP 为每个 Client 只建立一条持久化、全双工 Unix Domain `SOCK_STREAM`，并用不含自身的 `u32` 大端 Payload 长度前缀划分消息；Payload 硬限制为 65,536 bytes，Server 必须在分配前检查，超限则不排空帧而关闭连接。SDK 串行写入完整帧，Server 拒绝额外提交连接。Server 在完整帧结束时记录 `received_at`，并以连接帧序分配权威 Faction `receive_seq`。`INITIAL_SPAWN_START` 后连接关闭或 Client 退出即产生 `CLIENT_FORFEIT`，不支持重连；双方或 Server 侧故障为无评分基础设施错误。这为同阵营提交先后、跨 deadline 半帧与断线结果提供单一语义，也简化本机 Container 与 SHM 架构；代价是接受连接级 Head-of-line Blocking、瞬时连接错误即判负和 Linux 本机部署约束。Payload 保持语言无关，未来远程传输需要新的协议版本。

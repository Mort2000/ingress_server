---
status: accepted
---

# 用 Rust 实现 MVP，但保持竞技协议语言无关

MVP 的权威 Game Core、Server、Runner、Replay、官方 SDK 和 Baseline Bots 使用 Rust，以统一确定性语义、降低系统边界实现成本，并支持后续资源隔离。对外协议和 Replay 格式不得依赖 Rust ABI；协议 Payload 与 SHM Snapshot 由 FlatBuffers Schema 定义。v0.1 只正式提供和验收 Rust SDK，未来仍可增加 Python、C++ 等语言的 Client，而无需改写比赛协议。

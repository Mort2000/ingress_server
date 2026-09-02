---
status: accepted
---

# 使用 Canonical Encoding 与 BLAKE3-256 校验逐轮状态

每轮结束后，Core 对带域标签 `ingress-arena-state-v0.1` 的 Canonical Replay State 计算 `BLAKE3-256`。Canonical Encoding 固定字段顺序、整数宽度和字节序，集合按稳定 ID 排序，只包含会影响后续确定性状态转换的权威数据；Public Event、诊断、输出日志、派生缓存、内存地址、wall-clock、连接状态及 FlatBuffers 布局均排除。Replay 保存 32-byte Hash 与编码版本。该指纹用于验证确定性重放并定位首次状态分歧，不决定胜负，也不提供签名或反作弊保证。相比直接哈希 FlatBuffers，它需要维护一个小型 Canonical Codec，但不会因 Builder、默认字段或生成器变化而误报状态分歧。

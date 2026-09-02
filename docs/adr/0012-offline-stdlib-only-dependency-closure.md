---
status: accepted
---

# 离线环境下的零第三方依赖闭包

MVP 的构建环境无网络访问：crates.io 不可达，本地 registry 缓存不完整（缺少 `blake3` 本体），也没有 FlatBuffers 编译器 `flatc`。因此整个 Workspace 采用零第三方依赖的纯 `std` Rust 闭包：BLAKE3（hash / keyed / derive_key / XOF）在 `crates/arena-blake3` 内按公开规范以可移植实现并提供 Golden Vector 测试；ADR-0009 所需的 FlatBuffers 线格式由 `crates/arena-protocol` 内实现经审计的 Schema 子集（Builder / Verifier / 定型访问器），Schema 文件保留为语言中立的契约文档。该决定与 ADR-0005 的受审计闭包方向一致，使 `cargo build --release --locked --offline` 在无网络容器中天然可复现，代价是需要自行维护这些算法实现并固化其测试向量；未来环境恢复网络后可在补充跨实现向量验证的前提下评估是否切回上游 crate。

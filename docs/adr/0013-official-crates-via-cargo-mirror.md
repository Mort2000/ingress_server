---
status: superseded
---

# 经 cargo 镜像恢复使用官方 crate

MVP 构建环境实际可经配置的 cargo 镜像访问 crates.io：此前对默认下载 CDN 的直连探测超时，导致误判为完全离线并作出 ADR-0012 的零依赖决定；实测 `cargo add` / `cargo build` 均可正常解析并完成下载。因此撤销零第三方依赖约束：规则关键的 BLAKE3 直接使用官方 `blake3` crate，以提交的 `Cargo.lock` 固定版本与传递闭包；ADR-0009 的 FlatBuffers 线格式在 M7 经 `planus`（纯 Rust 的 FlatBuffers 编译器与配套运行时，线格式与 FlatBuffers 规范兼容，无需 `flatc` 二进制）实现——`planus` 生成的是其自身运行时的代码，不与官方 `flatbuffers` crate 混用；生成的代码提交入库，使常规构建不依赖 planus 工具链。依赖纪律保持最小闭包：除 `blake3` 与 FlatBuffers 家族外仍以 std 实现，Replay 与 Canonical State Encoding 维持自定义的确定性编码；M8 的离线可复现构建改以 `cargo vendor` 冻结完整依赖闭包并审计实现。探测教训：判断网络可用性应使用 cargo 本身（镜像在 cargo 配置层生效），而不是对默认 CDN 做 curl 直连。

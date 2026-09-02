---
status: accepted
---

# MVP 只运行由比赛方构建的受审计源码

v0.1 只接受源码可得的 Rust Client，并由比赛方使用固定工具链构建后运行；提交必须包含 `rust-toolchain.toml` 与 `Cargo.lock`，比赛方在无网络的一次性容器中执行 `cargo build --release --locked --offline`。参赛者 crate 必须使用安全 Rust 和获批依赖，只有官方 SDK 可以包含经审计的 `unsafe` / FFI 并访问 SHM、RPC 与时钟；含 Build Script、Proc Macro、Native Library 或直接系统调用能力的新依赖需单独批准。审计覆盖完整依赖图，并记录源码树、Lockfile、工具链和最终二进制 Hash；不接受参赛者直接提供的闭源或任意预编译二进制。MVP 的 Settlement Phase 读限制依赖官方 SDK 与白盒代码审计，Container 和 cgroup 只提供资源及部分信息隔离，不能被描述为足以安全执行不可信程序；支持任意不可信 Client 属于后续阶段。

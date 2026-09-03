# 架构决策记录

| ADR | 状态 | 决定 |
|---|---|---|
| [0001](./0001-rust-mvp-language-neutral-protocol.md) | accepted | 用 Rust 实现 MVP，但保持竞技协议语言无关 |
| [0002](./0002-validate-priority-bucket-in-mvp.md) | accepted | 在 MVP 中验证 Priority Bucket |
| [0003](./0003-agent-command-stream-is-scheduling-unit.md) | accepted | 以 Agent Command Stream 为原子调度单元 |
| [0004](./0004-first-command-stream-submission-is-final.md) | accepted | 首个可定位的 Round 或出生提交即冻结，不提供修正重提 |
| [0005](./0005-mvp-runs-audited-source-built-by-operator.md) | accepted | MVP 只运行由比赛方构建的受审计源码 |
| [0006](./0006-zero-copy-client-observation.md) | accepted | MVP 在 Client 侧使用零拷贝 Observation |
| [0007](./0007-fixed-width-integer-game-math.md) | accepted | 使用定宽整数表达游戏数学 |
| [0008](./0008-single-ordered-unix-stream-per-client.md) | accepted | 每个 Client 使用一条有序 Unix Stream |
| [0009](./0009-flatbuffers-for-protocol-and-shm.md) | accepted | 协议 Payload 与 SHM Snapshot 统一使用 FlatBuffers |
| [0010](./0010-canonical-blake3-state-hash.md) | accepted | 使用 Canonical Encoding 与 BLAKE3-256 校验逐轮状态 |
| [0011](./0011-domain-separated-blake3-random-oracle.md) | accepted | 用带域隔离的 BLAKE3 Random Oracle 派生比赛内随机结果 |
| [0012](./0012-offline-stdlib-only-dependency-closure.md) | superseded | 离线环境使用零第三方依赖的 std-only 闭包（被 0013 撤销） |
| [0013](./0013-official-crates-via-cargo-mirror.md) | accepted | 经 cargo 镜像使用官方 blake3 与 flatbuffers/planus，其余保持 std-only |
| [0015](./0015-independent-policy-identity.md) | accepted | 策略以独立 policy_id/family/version/parameters 标识；版本只增不改；bot RNG kind-tag 1-5 永属 m6 |

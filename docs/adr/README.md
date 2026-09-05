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
| [0013](./0013-official-crates-via-cargo-mirror.md) | superseded | 经 cargo 镜像使用官方 blake3 与 flatbuffers/planus，其余保持 std-only（镜像采用仍有效；vendor 冻结条款被 progress.md D94 撤销） |
| [0014](./0014-r2-seed-set-domains-and-paired-scheduling.md) | accepted | 冻结 R2 Seed Set 主种子、推导域与同图换边配对单位 |
| [0015](./0015-independent-policy-identity.md) | accepted | 策略以独立 policy_id/family/version/parameters 标识；版本只增不改；bot RNG kind-tag 1-5 永属 m6 |
| [0016](./0016-r2-policy-portfolio-and-readiness-gates.md) | accepted | 冻结 B2 六策略组合（kind-tag 6-12、结构化参数全值序列化、Readiness 阶段）与第 10.1 节就绪门槛的统计定义、阈值与主动阻断几何代理 |
| [0017](./0017-b3-default-config-ablation.md) | accepted | 冻结 B3 默认配置消融的三个测量臂（hybrid-builder-only/hybrid-builder-attack b3.0、greedy-coordinated-canonical r2.0 + kind-tag 13）、两个矩阵/Idle 实验与第 9 节统计定义（Wilson、nearest-rank 分位、支配分类） |
| [0018](./0018-b4-parameter-sensitivity.md) | accepted | 冻结 B4 单变量敏感度的 13 候选网格、六对阵 R2 名单、Pilot/Full/交互实验、地图机制变体（对 0014 地图共享边界的修订）与第 9 节统计定义（按地图配对效应、按地图聚类 bootstrap、副作用护栏、稳定规则、交互 DiD） |

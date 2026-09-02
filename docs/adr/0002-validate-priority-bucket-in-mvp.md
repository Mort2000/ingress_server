---
status: accepted
---

# 在 MVP 中验证 Priority Bucket

MVP 有意让截止时间内的计算速度影响冲突先手：Command Stream 按完整协议帧最后一字节到达 Server 传输层时的单调时钟进入固定宽度的 Priority Bucket，较早 Bucket 先结算；Client 时间戳和后续解析耗时均不参与。同一 Bucket 内先按各 Faction 自身的权威到达顺序排队，再从本轮较晚收到 Round Start Signal 的 Faction 开始交替结算；奇数轮通知 RES 在先而结算 ENL 在先，偶数轮相反。Client 不提供 `stream_id` 或排序 nonce。这既补偿通知先后，又允许单个 Client 明确安排己方 Agent 顺序。Round Start Signal 同时是 Client 恢复读取共享内存的许可边界。MVP 不增加 READY、赛前公平性校准或结算期读隔离，而以官方 SDK 和白盒代码审计约束 Client。这个选择用于验证“规划深度与响应速度的取舍”是否能形成有价值的竞技维度；代价是确定性输入必须包含 Bucket、每 Faction 到达次序和通知次序，而不能把语义相同的动作序列视为完整重放输入。

纯内存 Simulator 与 `P2.5` 平衡实验不测量 Bot 的实际 wall-clock 时间，而由 Harness 注入确定性 Bucket；默认双方所有 Stream 均为 Bucket 0，另用显式 Bucket 场景测试排序。真实提交延迟只在 `P3` 外部 Runtime 验证，以免机器负载污染大批量平衡数据。

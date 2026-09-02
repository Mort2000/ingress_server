---
status: accepted
---

# 首个可定位的 Client 提交即冻结

每个 Agent 在一个 Round 中只有一次 Command Stream 提交机会：消息通过可解码、大小、协议版本、当前 Round、已认证 Faction 和 Agent 归属等信封校验后，首个提交立即冻结该 Agent 的槽位；后续的 Stream 结构校验即使失败也不解冻，重复提交均不替换它。结构非法的 Stream 不进入执行队列或占用 Bucket 内交替位置。结构错误属于 Client Bug，应由 Client 在提交前避免；Server 不提供修正重提，以保持协议简单，并防止 Client 先占据早期 Priority Bucket 再补全计划。无法合法定位当前 Agent 槽位的消息不触发冻结。

同一原则用于赛前 Initial Spawn Set：首个通过 framing、大小、协议版本与 Faction 认证的集合冻结该方赛前槽位。集合内部非法或截止前缺失直接产生 `SPAWN_FORFEIT`，不允许修正重提，也不生成自动出生位置；双方均失败则比赛无有效结果并排除出 Elo。

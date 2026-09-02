---
status: accepted
---

# 以 Agent Command Stream 为原子调度单元

每个 Agent 的 Command Stream 可独立提交并在完整到达时取得自己的 Priority Bucket；结构非法的 Stream 冻结 Agent 槽位但不进入执行队列或占用交替位置，结构合法的空 Stream 同样不参与排序并表示显式 `PASS`。Resolver 对其余结构合法且非空的 Stream 先按 Bucket，再按每 Faction 的到达顺序建队，并从本轮 Round Start Signal 顺序相反的一方开始交替合并。Client 不能自选排序 ID。选中 Stream 后连续处理其全部命令，不插入其他 Agent 的命令；即使所有命令执行期均失败，该 Stream 也保留排序位置且不触发重排。Stream 不是状态事务：每条命令仍基于最新权威状态独立校验，失败不回滚此前成功命令；该选择让每个 Agent 独立承担规划延迟、允许 Client 控制己方 Agent 顺序，并避免把整个 Faction 的提交变成单一原子操作。

---
status: accepted
---

# 用带域隔离的 BLAKE3 Random Oracle 派生随机结果

地图在比赛事件前一次性生成，Replay 直接保存并载入完整 Map Definition，因此地图生成不属于本决策。地图冻结后的 Hack 与增援随机结果由 `(game_seed, rng_version, domain_tag, stable_indices...)` 的版本化 Canonical Encoding 经 BLAKE3 Counter/XOF 独立派生，不维护或暴露可变的全局 PRNG 游标。有限范围整数使用固定 rejection sampling，禁止直接取模；地址编码、字节序与跨语言 Golden Vectors 随 `rng_version` 固定。该模型确保一个系统增删随机调用不会污染其他运行期结果，并复用 State Hash 已需的 BLAKE3；代价是每类随机事件必须设计稳定且永不含糊的索引。

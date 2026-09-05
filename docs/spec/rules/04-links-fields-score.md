# Link、Field 与得分

默认距离与比赛长度见 [`R-CONFIG`](./06-configuration.md)。

## R-LINK-01 创建 Link

`LINK(source_portal, target_portal)` 是 Auxiliary Action。执行时必须同时满足：

1. Source 与 Target 均属于 Agent 阵营；
2. Agent 到 Source 的距离不超过 `PORTAL_OPERATION_RANGE`；
3. Agent 至少持有一枚 Target Key；
4. Source 与 Target 的距离不超过 `MAX_LINK_DISTANCE`；
5. Source 与 Target 不是同一 Portal；
6. 该无向 Portal Pair 尚不存在 Link；
7. 除 Source 与 Target 外，没有其他 Portal 位于新 Link 的闭线段上；
8. 新 Link 不与任何既有 Link 非法相交。

成功后消耗一枚 Target Key。`source` / `target` 方向只影响 Agent 站位与 Key 消耗；生成的 Link 是无向简单边，以排序后的 `(min_portal_id, max_portal_id)` 作为唯一身份。已有 `A—B` 时，`A→B` 与 `B→A` 都非法。

## R-LINK-02 几何相交

线段相交及点在线段上应使用整数叉积判断。新 Link 不得穿过任何非端点 Portal；例如 A、C、B 共线且 C 位于 A—B 内部时，`LINK(A,B)` 非法。与现有 Link 共享 Portal 端点合法；除此之外的几何交叉非法。Field 区域本身不阻止在其内部创建 Link，只检查 Portal 穿越及新 Link 与既有 Link 的交叉，从而降低大型 Field 对后续局势的封锁程度。

## R-FIELD-01 Field 派生

Field 无需显式创建。若同阵营三个 Portal `A`、`B`、`C` 之间同时存在 `A-B`、`B-C`、`C-A` 三条 Link，则自动形成三角 Field `(A,B,C)`。

Field 是 Link Graph 的派生状态。实现可以缓存 Field 以提高查询和事件生成效率，但 Link 的创建或删除必须同步更新缓存。

## R-FIELD-02 面积与 Layering

三角形的权威面积表示为：

```text
area2(A, B, C) = abs(cross(B - A, C - A))
Area(A, B, C) = area2 / 2
```

`area2` 以“两倍平方坐标单位”保存，允许精确表示半平方厘米。Core、Replay、Hash 和胜负比较不得将其转换为浮点；仅展示层换算为 m² 或 km²。

Field 可以重叠，每个独立三角形单独计分。创建 Link `(A,B)` 后，`Neighbors(A) ∩ Neighbors(B)` 中每个共同邻居 `C` 都生成一个 Field，因此一条 Link 可同时完成多个 Layered Field。

## R-SCORE-01 面积时间积分

每轮所有动作结算完毕后，以最终仍存在的 Field 计算一次 `score2` 增量：

```text
score2_delta(faction, round) = sum(area2(field) for active faction fields)
score2(faction) = sum(score2_delta(faction, round) for all rounds)
```

由此可得：

- 本轮创建后又在本轮被摧毁的 Field 得 0 分；
- 本轮最终状态仍存在的 Field 获得一次面积积分；
- 最后一轮新建且存续至轮末的 Field 仍计一次；
- Field 重叠时分别计分；
- wall-clock 时间不进入得分。

比赛结束时直接比较双方 `score2`；数值更高的一方获胜，相等则为 Draw。Portal 数、剩余 Supply、最后得分时间等状态不作为次级 Tie-break。

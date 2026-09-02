# Link、Field 与得分

默认距离与比赛长度见 [`R-CONFIG`](./06-configuration.md)。

## R-LINK-01 创建 Link

`LINK(source_portal, target_portal)` 是 Auxiliary Action。执行时必须同时满足：

1. Source 与 Target 均属于 Agent 阵营；
2. Agent 到 Source 的距离不超过 `PORTAL_OPERATION_RANGE`；
3. Agent 至少持有一枚 Target Key；
4. Source 与 Target 的距离不超过 `MAX_LINK_DISTANCE`；
5. 新 Link 不与任何既有 Link 非法相交。

成功后消耗一枚 Target Key。Link 有方向用于表达“Source 发起、Target 消耗 Key”，但 Field 判定按两 Portal 间存在连接处理。

## R-LINK-02 几何相交

线段相交应使用整数叉积判断。与现有 Link 共享 Portal 端点合法；除此之外的几何交叉非法。Field 区域本身不阻止在其内部创建 Link，只检查新 Link 与既有 Link 的交叉，从而降低大型 Field 对后续局势的封锁程度。

## R-FIELD-01 Field 派生

Field 无需显式创建。若同阵营三个 Portal `A`、`B`、`C` 之间同时存在 `A-B`、`B-C`、`C-A` 三条 Link，则自动形成三角 Field `(A,B,C)`。

Field 是 Link Graph 的派生状态。实现可以缓存 Field 以提高查询和事件生成效率，但 Link 的创建或删除必须同步更新缓存。

## R-FIELD-02 面积与 Layering

三角形面积为：

```text
Area(A, B, C) = abs(cross(B - A, C - A)) / 2
```

为保持确定性，可在内部保存两倍面积 `area2 = abs(cross(...))`，仅在显示或最终计分表示时换算为 m² 或 km²。

Field 可以重叠，每个独立三角形单独计分。创建 Link `(A,B)` 后，`Neighbors(A) ∩ Neighbors(B)` 中每个共同邻居 `C` 都生成一个 Field，因此一条 Link 可同时完成多个 Layered Field。

## R-SCORE-01 面积时间积分

每轮所有动作结算完毕后，以最终仍存在的 Field 计算一次增量：

```text
score_delta(faction, round) = sum(Area(field) for active faction fields)
Score(faction) = sum(score_delta(faction, round) for all rounds)
```

由此可得：

- 本轮创建后又在本轮被摧毁的 Field 得 0 分；
- 本轮最终状态仍存在的 Field 获得一次面积积分；
- 最后一轮新建且存续至轮末的 Field 仍计一次；
- Field 重叠时分别计分；
- wall-clock 时间不进入得分。

比赛结束时总分更高的一方获胜。

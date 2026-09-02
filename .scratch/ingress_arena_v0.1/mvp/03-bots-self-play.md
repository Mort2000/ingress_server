# P2—P2.5：Baseline Bots 与 Self-play

在接入外部 Runtime 前，先证明游戏规则值得继续开发，并用可复现实验完成第一轮调参。

## P2 Baseline Bots

| Bot | 最小行为 | 验证目的 |
|---|---|---|
| `RandomBot` | 随机移动、Hack、Capture、Link、Attack、Recharge | Smoke Test；发现崩溃与状态机漏洞 |
| `NearestPortalBot` | 寻找最近未占 Portal，移动、Hack、Capture | 基本空间扩张是否成立 |
| `GreedyFieldBot` | 搜索当前可获得的最大面积 Field，并规划 Key 与路线 | 单纯“大三角”策略是否过强 |
| `DestroyAnchorBot` | 计算敌方 Portal 所承载 Field 面积总和，优先攻击最高者 | 防守是否容易被单点瓦解 |
| `HybridBot` | 组合 Field Construction、Critical Portal Attack 与 Recharge | 作为后续默认 Benchmark |

Bot 应保持简单、可解释和固定版本；它们是测量工具，不追求成为最终强 AI。

## P2.5 批量比赛

首轮运行 1,000—10,000 场不同地图 Seed 的比赛，至少覆盖以下配对，并对每个 Seed 交换阵营：

- Random vs Random；
- Nearest vs Nearest、Nearest vs Greedy；
- Greedy vs DestroyAnchor、Greedy vs Hybrid；
- Hybrid vs Hybrid。

实验记录必须绑定 Game Config、Seed、双方 Bot 构建版本和阵营，失败或超时也计入结果集。

## P2.5-METRICS 指标

除 [`R-BALANCE-02`](../rules/07-strategy-balance-scope.md) 的玩法指标外，至少汇总 Win Rate、Score Ratio、Attack / Recharge / Hack Count、Supply Remaining，以及崩溃、非法动作和超时率。

滚雪球重点计算：

```text
P(win | leading at Round 50/100/200/300)
```

若 `P(win | leading at Round 100) > 90%`，优先检查 `MAX_LINK_DISTANCE`、Portal 密度、Attack / Recharge 强度、Blocking 强度和 Field 面积权重。

## P2.5-TUNING 调参顺序

依次优先调整：

1. `MOVE_BUDGET`；
2. `MAX_LINK_DISTANCE`；
3. `ATTACK_DAMAGE`；
4. `RECHARGE_AMOUNT`、`RECHARGE_COST`；
5. `INITIAL_SUPPLY`；
6. Portal 密度。

每轮实验只改少量变量，不在此阶段引入新机制。默认值的唯一事实源是 [`R-CONFIG`](../rules/06-configuration.md)。

## P2.5-GATE 完成门槛

10,000 场可复现 Self-play 完成并产出第一份平衡分析；没有已知简单策略在绝大多数随机地图上以超过 90% 胜率碾压其他合理 Baseline，或相关问题已被明确记录和排期。

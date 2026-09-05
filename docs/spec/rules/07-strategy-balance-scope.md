# 策略、平衡与范围

本页是非规范性设计附录，用于验证规则是否产生预期策略空间；它不要求参赛 Client 采用特定算法。

## R-BALANCE-01 预期策略角色

| 角色 | 主要决策 |
|---|---|
| Field Builder | Portal 几何、Key 获取、Link 顺序与 Layered Field，最大化面积时间积分 |
| Anchor Defender | 用 `sum(Area(field containing p))` 估计 Anchor 价值并保护高价值 Portal |
| Disruptor | 优先攻击 Field 边际价值最大的敌方 Portal |
| Key Runner | Hack、运输 Key、Transfer，并为远距 Link 预置资源 |
| Blocker | 用 Link 阻挡对手连线、切割几何空间、降低大型 Field 可行性 |
| Opponent Modeler | 从公开事件估计敌方位置、目标、路线和可能的 Key Inventory |

可探索的算法包括计算几何、图优化、车辆路径、资源分配、搜索树、Bayesian Estimation、Particle Filter 与多 Agent Planning。

## R-BALANCE-02 重点指标

批量 Self-play 至少观察：

| 类别 | 指标 |
|---|---|
| 得分与节奏 | First Field Round、Final Score Ratio、Average Field Area、Field Average Lifetime |
| 地图控制 | Portal Turnover Rate、Layering Depth、Anchor Attack Frequency |
| 资源 | Average Supply、Hack Count、Key Utilization、Attack / Recharge Count |
| 事件影响 | Reinforcement Impact |
| 滚雪球 | `P(win ∣ leading at Round 50/100/200/300)` |
| 阵营偏差 | ENL / RES Win Rate 与 Score Delta 分布 |

若 `P(win | leading at Round 100)` 接近或超过 90%—95%，应重点检查 Link 距离、Portal 密度、攻防强度、Blocking 及大 Field 权重；约 60%—75% 通常表示仍有明显翻盘空间。具体实验流程见 [`P2.5`](../mvp/03-bots-self-play.md)。

## R-SCOPE-01 v0.1 明确不包含

- Resonator、Mod、Portal Level、Portal 防御模组；
- Agent Combat / PvP、装备等级、阵营技能、Field Buff；
- Terrain、Portal Fog of War、复杂经济系统；
- 玩家永久成长和 Client 人工操作。

这些机制仅在核心博弈通过验证后重新评估。

## 规则摘要

玩家移动 Agent、控制 Portal、获取 Key 并构建不可交叉的 Link Graph；图中的三角闭环按面积和存续轮数持续得分，攻击、远程 Recharge、物流与空间阻断则用于破坏或保护高价值结构。

# v0.1 默认参数

## R-CONFIG-01 参数表

本表是规则默认值的唯一事实源。这些值是首轮实验起点，不是最终平衡承诺；修改时应同步 Replay 配置快照与相关边界测试。

| 分组 | 参数 | 默认值 |
|---|---|---:|
| 时间 | `TOTAL_ROUNDS` | 800 Round |
| 时间 | `INITIAL_SPAWN_WINDOW` | 5000 ms |
| 时间 | `DECISION_WINDOW` | 1000 ms |
| 时间 | `PRIORITY_BUCKET` | 5 ms |
| 时间 | `EPOCH_LENGTH` | 100 Round |
| 地图 | `COORDINATE_UNIT` | 1 cm |
| 地图 | `MAP_WIDTH` | 6000 m |
| 地图 | `MAP_HEIGHT` | 6000 m |
| 地图 | `PORTAL_COUNT` | 120 |
| 地图 | `PORTAL_MIN_DISTANCE` | 80 m |
| 地图 | `CLUSTERED_PORTAL_RATIO` | 70% |
| 地图 | `PORTAL_CLUSTER_COUNT` | 5—8 |
| Agent | `INITIAL_AGENTS_PER_FACTION` | 3 |
| Agent | `REINFORCEMENT_INTERVAL` | 100 Round |
| Agent | `REINFORCEMENT_AGENTS_PER_FACTION` | 1 |
| Agent | `MAX_REINFORCEMENT_AGENTS_PER_FACTION` | 4（每阵营每场累计增援上限，默认总人数最多 7） |
| Agent | `MOVE_BUDGET` | 200 m / Round |
| Agent | `PORTAL_OPERATION_RANGE` | 50 m |
| Agent | `TRANSFER_RANGE` | 50 m |
| Agent | `INITIAL_SUPPLY` | 20 |
| Agent | `MAX_SUPPLY` | 100 |
| Portal | `PORTAL_MAX_HP` | 100 HP |
| Portal | `ATTACK_DAMAGE` | 20 HP |
| Portal | `RECHARGE_AMOUNT` | 20 HP |
| Portal | `RECHARGE_COST` | 1 Supply |
| Hack | `HACK_SUPPLY_YIELD` | 1 Supply |
| Hack | `HACK_KEY_YIELD` | `DiscreteUniform{1,2,3}` Keys |
| Hack | `HACK_COOLDOWN` | 3 Round |
| Hack | `HACK_LIMIT_PER_EPOCH` | 5 / `(agent, portal)` |
| Link | `MAX_LINK_DISTANCE` | 8000 m |
| 动作 | `MAX_COMMANDS_PER_AGENT_ROUND` | 10 |
| 协议防护 | `MAX_FRAME_PAYLOAD_BYTES` | 65,536 bytes（64 KiB，硬上限） |

## R-CONFIG-02 调参纪律

- 一次实验只改变少量参数，并记录配置、Seed Set、Bot 版本和结果。
- 优先调整 `MOVE_BUDGET`、`MAX_LINK_DISTANCE`、`ATTACK_DAMAGE`、`RECHARGE_AMOUNT`、`RECHARGE_COST`、`INITIAL_SUPPLY`、Portal 密度。
- 在现有参数空间未验证前，不用新增机制掩盖平衡问题。

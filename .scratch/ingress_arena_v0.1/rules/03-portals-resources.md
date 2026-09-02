# Portal、战斗与资源

本页定义 Portal 相关动作；动作通用语义见 [`R-ACTION`](./02-agents-actions.md)，参数值见 [`R-CONFIG`](./06-configuration.md)。

## R-PORTAL-01 Portal 状态与操作范围

Portal 的权威字段为 `id`、`position`、`owner` 和 `hp`。`owner` 取 `Neutral`、`ENL` 或 `RES`；被占领时 HP 设为 `PORTAL_MAX_HP`，被摧毁而中立化时 HP 设为 0。

除远程 Recharge 外，现场 Portal 操作要求 `distance(agent, portal) <= PORTAL_OPERATION_RANGE`，边界值合法。

## R-CAPTURE-01 占领

`CAPTURE(portal)` 是 Major Action。

| 前置条件 | 成功效果 |
|---|---|
| Agent 在操作范围内；`portal.owner == Neutral` | `owner = agent.faction`；`hp = PORTAL_MAX_HP` |

## R-ATTACK-01 攻击

`ATTACK()` 是 Major Action，对 Agent 操作范围内的所有敌方 Portal 各造成固定 `ATTACK_DAMAGE` 点伤害。伤害是固定 HP 点数，不是当前 HP 的百分比。

## R-ATTACK-02 Portal 摧毁

当 Portal HP 降至 0 或以下时，立即按以下顺序更新：

1. `owner = Neutral`，`hp = 0`。
2. 删除与该 Portal 相连的所有 Link。
3. 删除因这些 Link 消失而不再成立的 Field。

更新立即影响本轮后续动作的合法性。攻击不伤害 Agent；v0.1 不含 Agent Combat / PvP。

## R-RECHARGE-01 充能

`RECHARGE(portal)` 是 Major Action。Portal 必须属于己方，Agent 至少持有 `RECHARGE_COST` Supply，并满足下列位置条件之一：

- Agent 在 Portal 操作范围内；或
- Agent 持有至少一枚该 Portal 的 Key。

成功后 `portal.hp = min(PORTAL_MAX_HP, portal.hp + RECHARGE_AMOUNT)`，并扣除 `RECHARGE_COST` Supply。远程 Recharge 不消耗 Key。

## R-HACK-01 Hack 收益

`HACK(portal)` 是 Auxiliary Action。Agent 必须在操作范围内；Portal 可为中立、己方或敌方。成功后：

- 获得该 Portal 的 `HACK_KEY_YIELD` 枚 Key，数量由专用 Hack RNG 决定；
- 获得 `HACK_SUPPLY_YIELD` Supply，但不超过 `MAX_SUPPLY`。

## R-HACK-02 冷却与 Epoch 上限

每个 `(agent, portal)` 组合独立记录冷却和本 Epoch 成功次数：

- 两次合法 Hack 受 `HACK_COOLDOWN` 约束；
- 每个 Epoch 最多执行 `HACK_LIMIT_PER_EPOCH` 次合法 Hack；
- Epoch 长度为 `EPOCH_LENGTH`，依次为 Round 1—100、101—200，以此类推；
- 新 Epoch 开始时，该组合的 `hack_count` 清零，冷却重置。

> 待确认 `Q-R-HACK-01`：原稿未定义“冷却 3 Round”的端点。例如 Round 10 Hack 后，下一次合法 Round 是 13 还是 14；冻结协议前需用示例固定。

## R-KEY-01 Key

Key 与 Portal 一一对应，以 `keys[portal_id] = count` 记录。创建指向该 Portal 的 Link 时消耗一枚；对该 Portal 远程 Recharge 只要求持有、不消耗。

## R-TRANSFER-01 转移

两个同阵营 Agent 的距离不超过 `TRANSFER_RANGE` 时，可用 Auxiliary Action `TRANSFER` 交换任意数量的 Key 和 Supply。转移后接收方 Supply 仍不得超过 `MAX_SUPPLY`；具体命令载荷必须明确发送方、接收方及各资源数量。

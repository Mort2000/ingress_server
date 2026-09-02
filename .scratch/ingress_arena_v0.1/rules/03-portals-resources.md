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

- 获得该 Portal 的 `HACK_KEY_YIELD` 枚 Key；每次成功 Hack 独立地在 `{1, 2, 3}` 中离散均匀取值；
- 获得 `HACK_SUPPLY_YIELD` Supply，但不超过 `MAX_SUPPLY`。

只有通过全部执行期校验的 Hack 才取得一个随机样本；因范围、冷却或 Epoch 上限而 `DROP` 的命令不抽样。Portal 阵营、Agent Supply 和此前样本值不改变分布。

## R-HACK-02 冷却与 Epoch 上限

每个 `(agent, portal)` 组合独立记录冷却和本 Epoch 成功次数：

- 仅当 `current_round - last_hack_round >= HACK_COOLDOWN` 时才能再次 Hack；默认值下 Round 10 Hack 后，Round 13 首次重新合法；
- 每个 Epoch 最多执行 `HACK_LIMIT_PER_EPOCH` 次合法 Hack；
- Epoch 长度为 `EPOCH_LENGTH`，依次为 Round 1—100、101—200，以此类推；
- 新 Epoch 开始时，该组合的 `hack_count` 与冷却立即重置；例如 Round 100 Hack 不阻止 Round 101 再次 Hack。

## R-KEY-01 Key

Key 与 Portal 一一对应，以 `keys[portal_id] = count` 记录。创建指向该 Portal 的 Link 时消耗一枚；对该 Portal 远程 Recharge 只要求持有、不消耗。

## R-TRANSFER-01 转移

`TRANSFER(target_agent, supply, keys)` 是从 Command Stream 所属 Agent 到同阵营 Target Agent 的单向 Auxiliary Action；命令载荷不能指定其他 Agent 作为发送方。执行时双方距离不得超过 `TRANSFER_RANGE`，转移后接收方 Supply 仍不得超过 `MAX_SUPPLY`。

一条命令中的 Supply 和所有 Key 作为整体转移。执行时若目标、距离、发送方任一资源余额或接收方 Supply 容量不合法，整条命令 `DROP`，不扣除也不增加任何资源；Server 不截断数量，也不执行部分转移。

若策略需要双向交换，双方 Agent 必须分别在自己的 Command Stream 中提交一条 `TRANSFER`，并按两个 Stream 各自的结算顺序生效。

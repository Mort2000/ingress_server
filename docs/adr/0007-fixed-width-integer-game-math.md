---
status: accepted
---

# 使用定宽整数表达游戏数学

Rust Core 使用 `Coord(i32)`，以地图左下角为 `(0, 0)`，并限制在 `0..=MAP_WIDTH`、`0..=MAP_HEIGHT`；默认最大坐标为 `(600000, 600000)` cm。在减法前提升到 `i64` 完成距离、平方和叉积；单个 Field 使用 `Area2(u64)`，累计得分使用 `Score2(u128)`，其中后两者均以两倍平方坐标单位计量。所有运算采用 Checked Arithmetic，`GameConfig` 预先验证上界，运行期溢出视为使比赛无效的 Server Bug；权威状态、Replay 和胜负比较不使用浮点、Wrapping 或 Saturating Arithmetic。越界移动无效，不裁剪也不环绕。

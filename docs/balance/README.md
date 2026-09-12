# 平衡评估档案

当前阶段已转入 Alpha：默认采用 **20 HP／1 Supply 充电、8000 m 最大链距、800 轮比赛、每方累计最多 4 个增援 Agent**，以[规则配置](../spec/rules/06-configuration.md)为准。围绕现有 Bot 的细粒度规则扫描已停止，Transfer 稳定收益专项暂缓。进入 Alpha 时的决策依据、能力边界和独立参考选手迁移见 [Alpha 记录](../design/alpha-reference.md)。

以下报告均为历史实验记录。文中的“当前／默认规则”、策略版本、源码路径和命令按报告当时的归档理解；不会随 Alpha 更新改写历史数据。旧实验源码与入口从当前 server 移出，复现需检出报告指定的提交或冻结源码。

| 报告 | 主要证据 |
|---|---|
| [评估方案](ai-balance-plan.md) | 历史 B0–B5 设计，区分 Bot 缺陷、规则效果与统计边界 |
| [R1 基线](balance-report-r1.md) | 简单策略下的攻击支配、建造缺陷和 mirror 风险 |
| [B1](balance-b1-smoke.md)、[B2](balance-b2-readiness.md) | 实验可复现与测量工具就绪；机制使用次数不等于收益 |
| [B3 消融](balance-b3-ablation.md) | 旧策略攻击角色的支配来源 |
| [B4 敏感度](balance-b4-sensitivity.md) | 1500 m 链距在 Development／Validation 上的方向；未完成 B5 |
| [r2.2](r22-micro-validation.md)、[r2.3](r23-behavior-validation.md) | 基本执行修复、默认规则下的 Transfer 正例 |
| [r2.4](r24-combat-validation.md) | 开放及时反攻后，Disruptor 的压倒性优势消失 |
| [r2.5 攻防](r25-combat-validation.md) | 组合动作兑现，Adaptive 按需防守有实际分差收益 |
| [r2.5 补给抽查](r25-supply-defense-review.md)、[r2.6 补给](r26-supply-validation.md) | 局部资源约束与规划缺口；修复不等于整局收益单调提高 |
| [r2.6 充电效率](r26-recharge-sensitivity.md) | 15→20 HP 的方向性改善，20→25 增益不稳定 |

# 术语表

| 术语 | 含义 |
|---|---|
| Server | 唯一权威状态源，发布观察、接收命令并结算比赛 |
| Client / Bot | 玩家编写的自主决策程序；只能读取合法观察并提交命令 |
| Faction | 阵营：启蒙军（ENL）或抵抗军（RES） |
| Agent | 由 Client 控制、在地图中移动和执行动作的单位 |
| Portal | 可被占领、攻击、充能、Hack 和连接的固定地图节点 |
| Supply | Agent 的补给资源，用于 Recharge，范围由配置定义 |
| Key | 与某一 Portal 一一对应的物品；创建指向该 Portal 的 Link 时消耗 |
| Link | 同阵营两个 Portal 之间不可与既有 Link 非法相交的边 |
| Field | 同阵营三个 Portal 由三条 Link 闭合后派生出的三角区域 |
| Round | 离散结算单位；一个 Round 表示游戏世界中的一分钟 |
| Epoch | 用于重置 Hack 次数和冷却的 Round 区间 |
| Observation | Server 向某阵营提供的 Public State、己方 Private State 与事件 |
| Command Stream | 某个 Agent 在一个 Round 内按顺序提交的命令序列 |
| Major Action | 每个 Agent 每 Round 最多执行一次的主动作 |
| Auxiliary Action | 受各自条件及协议硬限制约束、可在同轮多次执行的辅助动作 |
| Snapshot | 某轮开始时发布给 Client 的一致只读状态视图 |
| Replay | 初始状态、已提交动作、结算事件与检查点组成的可重放记录 |

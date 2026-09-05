#!/usr/bin/env python3
"""分析 inspect-template.rs 产出的 JSONL trace（review-bot-replay 技能）。

CLI 全场摘要：
    python3 analyze.py trace.jsonl

作为库使用：
    from analyze import Trace
    t = Trace("trace.jsonl")
    t.no_task_rates()          # {'enl': (1631, 2500, 65.24), ...}
    t.defense_windows("enl")   # 补血任务窗口表（含到达时间 vs 倒塌时间）
    t.attack_hits_vs_live_fields("enl")  # 攻击命中落在有无存活场的分布

行类型见 inspect-template.rs 头注释；本文件不做任何规则判断，只做过滤、
join 和几何计算，结论仍以 Rust 侧逐轮 hash 校验为准。仅用标准库。
"""

import json
import math
import sys
from collections import Counter, defaultdict


def rows(path):
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                yield json.loads(line)


class Trace:
    def __init__(self, path):
        self.path = path
        self.config = {}
        self.summary = None
        self.portal_pos = {}                    # pid -> (x, y) cm
        self.agents = {}                        # (round, faction, agent) -> row
        self.portals = {}                       # (round, pid) -> row（仅被占领的）
        self.fields = []                        # field 行
        self.grants = []                        # grant 行
        self.overlay = []                       # overlay 行
        self.overlay_changes = []
        self.effects = []                       # effect 行
        self.commands = []
        self.transfers_direct = []
        for r in rows(path):
            k = r["kind"]
            if k == "config":
                self.config = r
            elif k == "summary":
                self.summary = r
            elif k == "portal_pos":
                self.portal_pos[r["portal"]] = (r["x"], r["y"])
            elif k == "agent":
                self.agents[(r["round"], r["faction"], r["agent"])] = r
            elif k == "portal":
                self.portals[(r["round"], r["portal"])] = r
            elif k == "field":
                self.fields.append(r)
            elif k == "grant":
                self.grants.append(r)
            elif k == "overlay":
                self.overlay.append(r)
            elif k == "overlay_change":
                self.overlay_changes.append(r)
            elif k == "effect":
                self.effects.append(r)
            elif k == "command":
                self.commands.append(r)
            elif k == "transfer_direct":
                self.transfers_direct.append(r)
        self.field_rounds = {f: {r["round"] for r in self.fields if r["faction"] == f}
                             for f in ("enl", "res")}

    # ---- 通用 ----

    def distance_cm(self, portal, agent_row):
        px, py = self.portal_pos[portal]
        return math.dist((px, py), (agent_row["x"], agent_row["y"]))

    def moves_needed(self, distance):
        """进入操作范围所需的整轮 Move 数（每轮 move_budget_cm，操作范围来自 config）。"""
        budget = self.config["move_budget_cm"]
        rng = self.config["portal_operation_range_cm"]
        return max(0, math.ceil((distance - rng) / budget))

    def portal_hp(self, round_, portal, default=None):
        row = self.portals.get((round_, portal))
        return default if row is None else row["hp"]

    # ---- 利用率 ----

    def no_task_rates(self):
        """每阵营 agent-round 无任务率（task 为 null 的 grant 行 / 全部 grant 行）。"""
        total = Counter(g["faction"] for g in self.grants)
        idle = Counter(g["faction"] for g in self.grants if g["task"] is None)
        return {f: (idle[f], total[f], 100.0 * idle[f] / total[f]) for f in total}

    def grant_breakdown(self, faction):
        """按任务种类统计获分的 agent-round；None 键 = 无任务。"""
        c = Counter(g["task"]["kind"] if g["task"] else None for g in self.grants
                    if g["faction"] == faction)
        return c

    def idle_by_agent(self, faction):
        total = Counter(g["agent"] for g in self.grants if g["faction"] == faction)
        idle = Counter(g["agent"] for g in self.grants
                       if g["faction"] == faction and g["task"] is None)
        return {a: (idle[a], total[a]) for a in sorted(total)}

    # ---- 补血 / 防御 ----

    def recharges(self, faction=None):
        """已执行的 Recharged 效果；zero_heal 标记满血浪费（hp_before == hp_after）。"""
        out = []
        for e in self.effects:
            res = e["result"]
            if res.get("effect") == "recharged" and (faction is None or e["faction"] == faction):
                res = dict(res)
                res["zero_heal"] = res["hp_before"] == res["hp_after"]
                res.update(round=e["round"], faction=e["faction"], agent=e["agent"])
                out.append(res)
        return out

    def _portal_fell_round(self, faction, portal, after_round, horizon=30):
        """portal 失去 faction 控制的结算轮 r（轮前 r 仍属 faction、轮前 r+1 已不是）。"""
        def owned(r):
            row = self.portals.get((r, portal))
            return row is not None and row["owner"] == faction
        for r in range(after_round, after_round + horizon):
            if owned(r) and not owned(r + 1):
                return r
        return None

    def defense_windows(self, faction, max_gap=3):
        """补血任务窗口：同一 portal 的 recharge grant 连续段。

        每个窗口给出：首次 grant 轮、受派 Agent、当轮 portal HP、距离、需要的
        Move 轮数、最早可执行补血轮、portal 实际脱离控制的轮、窗口内实际执行的
        补血次数。用于对照「到达时间 vs 倒塌时间」。
        """
        grants = sorted((g for g in self.grants
                         if g["faction"] == faction
                         and g["task"] and g["task"]["kind"] == "recharge"),
                        key=lambda g: g["round"])
        by_portal = defaultdict(list)
        for g in grants:
            by_portal[g["task"]["portal"]].append(g)
        windows = []
        for portal, gs in sorted(by_portal.items()):
            start_i = 0
            for i in range(1, len(gs) + 1):
                if i == len(gs) or gs[i]["round"] - gs[i - 1]["round"] > max_gap:
                    win = gs[start_i:i]
                    g0 = win[0]
                    agent = self.agents.get((g0["round"], faction, g0["agent"]))
                    dist = self.distance_cm(portal, agent) if agent else None
                    moves = self.moves_needed(dist) if dist is not None else None
                    executed = [r for r in self.recharges(faction)
                                if r["portal"] == portal
                                and g0["round"] <= r["round"] <= win[-1]["round"]]
                    windows.append({
                        "portal": portal,
                        "first_grant": g0["round"],
                        "last_grant": win[-1]["round"],
                        "agents": sorted({g["agent"] for g in win}),
                        "pre_hp": self.portal_hp(g0["round"], portal),
                        "distance_m": round(dist / 100) if dist is not None else None,
                        "moves": moves,
                        "earliest_recharge": g0["round"] + moves if moves is not None else None,
                        "fell_round": self._portal_fell_round(faction, portal, g0["round"]),
                        "recharges_executed": len(executed),
                    })
                    start_i = i
        return windows

    # ---- 攻击 ----

    def attack_hits(self, faction=None):
        """展平的攻击命中（每个 hit 一条：round/faction/agent/portal/hp_after/neutralized）。"""
        out = []
        for e in self.effects:
            res = e["result"]
            if res.get("effect") != "attacked":
                continue
            if faction is not None and e["faction"] != faction:
                continue
            for h in res["hits"]:
                out.append(dict(round=e["round"], faction=e["faction"], agent=e["agent"], **h))
        return out

    def attack_hits_vs_live_fields(self, victim_faction):
        """攻击命中按受害方当轮有无存活场分桶（攻击者=另一方）。"""
        live = self.field_rounds[victim_faction]
        with_field = without = 0
        for h in self.attack_hits():
            other = "res" if victim_faction == "enl" else "enl"
            if h["faction"] == other and h["round"] in live:
                with_field += 1
            elif h["faction"] == other:
                without += 1
        return with_field, without

    def drops(self, faction=None):
        c = Counter()
        for e in self.effects:
            if e["result"]["status"] == "drop" and (faction is None or e["faction"] == faction):
                c[e["result"]["reason_kind"]] += 1
        return c


def _fmt_window(w):
    return (f"portal {w['portal']}: grant R{w['first_grant']}-R{w['last_grant']} "
            f"agents={w['agents']} pre_hp={w['pre_hp']} "
            f"dist={w['distance_m']}m moves={w['moves']} "
            f"earliest_recharge=R{w['earliest_recharge']} fell={w['fell_round']} "
            f"executed={w['recharges_executed']}")


def main(path):
    t = Trace(path)
    print(f"== {path} ==")
    print(f"summary: {t.summary}")
    print(f"config: move_budget={t.config['move_budget_cm']}cm "
          f"op_range={t.config['portal_operation_range_cm']}cm "
          f"attack={t.config['attack_damage']} recharge=+{t.config['recharge_amount']}")
    print("\n== 无任务率 ==")
    for f, (idle, total, pct) in sorted(t.no_task_rates().items()):
        print(f"  {f}: {idle}/{total} = {pct:.2f}%")
    print("\n== 任务分布 ==")
    for f in ("enl", "res"):
        print(f"  {f}: {dict(t.grant_breakdown(f))}")
    print("\n== 补血效果 ==")
    for r in t.recharges():
        print(f"  R{r['round']} {r['faction']} agent {r['agent']} -> portal {r['portal']}: "
              f"{r['hp_before']}->{r['hp_after']}{'' if not r['zero_heal'] else '  (零恢复!)'}")
    print("\n== 防御窗口（补血任务） ==")
    for f in ("enl", "res"):
        wins = t.defense_windows(f)
        if wins:
            print(f"  [{f}]")
            for w in wins:
                print(f"    {_fmt_window(w)}")
    print("\n== 攻击命中 vs 受害方存活场 ==")
    for victim in ("enl", "res"):
        with_f, without = t.attack_hits_vs_live_fields(victim)
        print(f"  victim {victim}: 有场 {with_f} / 无场 {without}")
    print("\n== Drop 原因 ==")
    for f in ("enl", "res"):
        print(f"  {f}: {dict(t.drops(f))}")
    if t.transfers_direct:
        print("\n== 交接时发送方可直连的边（transfer_direct） ==")
        for r in t.transfers_direct:
            print(f"  R{r['round']} {r['faction']} agent {r['agent']} key={r['key']} "
                  f"sources={r['sources']}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "trace.jsonl")

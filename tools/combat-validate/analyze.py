#!/usr/bin/env python3
"""Audit typed combat traces and report paired, descriptive score contrasts.

Usage: python3 tools/combat-validate/analyze.py RUN_DIR [RUN_DIR ...]
JSON goes to stdout; diagnostics use logging. No third-party dependencies.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
from dataclasses import asdict, dataclass
import json
import hashlib
import logging
from pathlib import Path
from statistics import mean, median


@dataclass(frozen=True)
class FieldId:
    a: int
    b: int
    c: int


@dataclass
class FieldLife:
    side: int
    area: int
    created: int
    income: int = 0


@dataclass
class AuxiliaryAudit:
    companions: int = 0
    hacks: int = 0
    supply_gained: int = 0
    links: int = 0
    recharge_hack: int = 0
    recharge_link: int = 0
    recharge_move: int = 0
    attack_hack: int = 0
    attack_link: int = 0

    def read_event(self, event: dict) -> None:
        """JSON input boundary; metrics never parse the diagnostic command string."""
        if event['kind'] == 'companion':
            self.companions += 1
        elif event['kind'] == 'hack':
            self.hacks += 1
            self.supply_gained += event['supply']
        elif event['kind'] == 'link':
            self.links += 1
        elif event['kind'] == 'actions':
            self.recharge_hack += event['recharge'] and event['hack']
            self.recharge_link += event['recharge'] and event['link']
            self.recharge_move += event['recharge'] and event['move']
            self.attack_hack += event['attack'] and event['hack']
            self.attack_link += event['attack'] and event['link']


@dataclass
class Match:
    name: str
    arm: str
    map_index: int
    leg: int
    side: int
    own: int
    enemy: int
    initial_hash: str
    game_seed: int
    bot_seed: int
    parameters: str
    opponent_parameters: str
    version: str
    stats: dict[str, int]  # Serialized metrics boundary; never inferred from Debug.
    opponent_stats: dict[str, int]

    @classmethod
    def read(cls, path: Path) -> Match:
        raw = json.loads(path.read_text())
        assert raw['rules_verified'] and raw['decisions_verified'], path
        return cls(raw['match'], raw['arm'], raw['map_index'], raw['leg'],
                   raw['subject_side'], raw['subject_score'], raw['opponent_score'],
                   raw['initial_hash'], raw['game_seed'], raw['bot_seed'],
                   raw['subject_parameters'], raw['opponent_parameters'], raw['version'],
                   raw['subject_stats'], raw['opponent_stats'])

    def audit(self, path: Path) -> None:
        live: dict[FieldId, FieldLife] = {}
        scores = [0, 0]
        healed = [0, 0]
        heal_count = [0, 0]
        damage = [0, 0]
        dropped = [0, 0]
        auxiliary = [AuxiliaryAudit(), AuxiliaryAudit()]
        income_rounds: list[int] = []
        lifecycle_area_available = True
        for line in path.read_text().splitlines():
            event = json.loads(line)
            kind = event['kind']
            if 'side' in event:
                auxiliary[event['side']].read_event(event)
            if kind == 'field-created':
                field = FieldId(*event['field'])
                assert field not in live, (path, event)
                lifecycle_area_available &= 'area2' in event
                live[field] = FieldLife(event['side'], event.get('area2', 0), event['round'])
            elif kind == 'field-destroyed-by':
                field = FieldId(*event['field'])
                life = live.pop(field)
                assert life.side != event['side'], (path, event)
            elif kind == 'income':
                income_rounds.append(event['round'])
                actual = [event['enl'], event['res']]
                expected = [0, 0]
                for life in live.values():
                    expected[life.side] += life.area
                    life.income += life.area
                if lifecycle_area_available:
                    assert expected == actual, (path, event, expected)
                for side in range(2):
                    scores[side] += actual[side]
            elif kind == 'decision':
                assert event['task'] == 'none' or event['submitted'], (path, event)
            elif kind == 'recharge':
                amount = event['after'] - event['before']
                assert amount > 0, (path, event)
                healed[event['side']] += amount
                heal_count[event['side']] += 1
            elif kind == 'attack':
                amount = event['before'] - event['after']
                assert amount > 0, (path, event)
                damage[event['side']] += amount
            elif kind == 'drop':
                dropped[event['side']] += 1
        assert income_rounds == list(range(1, 501)), path
        assert scores[self.side] == self.own and scores[1 - self.side] == self.enemy, path
        for side, stats in [(self.side, self.stats), (1 - self.side, self.opponent_stats)]:
            assert stats['empty_grants'] == stats['stationary'] == 0, (path, stats)
            assert healed[side] == stats['hp'], path
            assert heal_count[side] == stats['recharges'], path
            assert damage[side] == stats['damage'], path
            assert dropped[side] == stats['drops'], path
            if 'companions' in stats:
                for name, value in asdict(auxiliary[side]).items():
                    assert value == stats[name], (path, name, value, stats[name])


@dataclass
class PairedEffect:
    map_index: int
    leg: int
    own_change: int
    enemy_change: int
    margin_change: int


def contrast(rows: list[Match], control: str, treatment: str, actor: str) -> dict:
    before = {(r.map_index, r.leg): r for r in rows if r.arm == control}
    after = {(r.map_index, r.leg): r for r in rows if r.arm == treatment}
    assert before.keys() == after.keys()
    effects: list[PairedEffect] = []
    maps: dict[int, list[int]] = defaultdict(list)
    for key in sorted(before):
        a, b = before[key], after[key]
        own = b.own - a.own
        enemy = b.enemy - a.enemy
        if actor == 'opponent':
            own, enemy = enemy, own
        effect = PairedEffect(a.map_index, a.leg, own, enemy, own - enemy)
        effects.append(effect)
        maps[a.map_index].append(effect.margin_change)
    margins = [e.margin_change for e in effects]
    return {
        'control': control, 'treatment': treatment, 'actor': actor,
        'mean_own_change': mean(e.own_change for e in effects),
        'mean_enemy_change': mean(e.enemy_change for e in effects),
        'mean_margin_change': mean(margins), 'median_margin_change': median(margins),
        'min_margin_change': min(margins), 'max_margin_change': max(margins),
        'positive_legs': sum(m > 0 for m in margins),
        'positive_maps': sum(mean(values) > 0 for values in maps.values()),
        'map_mean_margin_changes': {m: mean(v) for m, v in maps.items()},
        'pairs': [asdict(e) for e in effects],
    }


def analyze(directory: Path) -> dict:
    manifest = dict(line.split('=', 1) for line in
                    (directory / 'manifest.txt').read_text().splitlines() if '=' in line)
    assert manifest['status'] == 'complete'
    rows = [Match.read(path) for path in sorted(directory.glob('*/result.json'))]
    assert len(rows) == int(manifest['matches'])
    group = manifest['group']
    expected_arms = {
        'factorial': {f'def{d}-atk{a}' for d in range(3) for a in range(2)},
        'adaptive-defense': {'adaptive-def0', 'adaptive-def2'},
        'adaptive': {'legacy', 'mid-open', 'all-open'},
        'compound': {f'daux{d}-aaux{a}' for d in range(2) for a in range(2)},
    }[group]
    assert {r.arm for r in rows} == expected_arms
    expected_cases = {(m, leg) for m in range(int(manifest['maps'])) for leg in range(2)}
    baseline: dict[tuple[int, int], Match] = {}
    summaries = {}
    for arm in sorted(expected_arms):
        selected = [r for r in rows if r.arm == arm]
        assert len(selected) == len(expected_cases)
        assert {(r.map_index, r.leg) for r in selected} == expected_cases
        assert len({(r.parameters, r.opponent_parameters, r.version) for r in selected}) == 1
        for row in selected:
            assert row.version == manifest['version'] and row.side == row.leg
            previous = baseline.setdefault((row.map_index, row.leg), row)
            assert row.initial_hash == previous.initial_hash
            assert (row.game_seed, row.bot_seed) == (previous.game_seed, previous.bot_seed)
            row.audit(directory / row.name / 'trace.jsonl')
        summaries[arm] = {
            'games': len(selected), 'wins': sum(r.own > r.enemy for r in selected),
            'ties': sum(r.own == r.enemy for r in selected),
            'mean_own': mean(r.own for r in selected),
            'mean_enemy': mean(r.enemy for r in selected),
            'pooled_score_share': sum(r.own for r in selected) / sum(r.own + r.enemy for r in selected),
            'subject_stats': {k: sum(r.stats[k] for r in selected) for k in selected[0].stats},
            'opponent_stats': {k: sum(r.opponent_stats[k] for r in selected) for k in selected[0].opponent_stats},
        }
    if group == 'compound':
        contrasts = [contrast(rows, f'daux0-aaux{a}', f'daux1-aaux{a}', 'subject') for a in range(2)]
        contrasts += [contrast(rows, f'daux{d}-aaux0', f'daux{d}-aaux1', 'opponent') for d in range(2)]
    elif group == 'factorial':
        contrasts = [contrast(rows, f'def{d}-atk0', f'def{d}-atk1', 'opponent') for d in range(3)]
        contrasts += [contrast(rows, f'def0-atk{a}', f'def{d}-atk{a}', 'subject')
                      for a in range(2) for d in [1, 2]]
        contrasts += [contrast(rows, 'def1-atk1', 'def2-atk1', 'subject')]
    elif group == 'adaptive-defense':
        contrasts = [contrast(rows, 'adaptive-def0', 'adaptive-def2', 'subject')]
    else:
        contrasts = [contrast(rows, 'legacy', arm, 'subject') for arm in ['mid-open', 'all-open']]
    logging.info('Audited %s: %d matches, all traces and pairings checked', directory, len(rows))
    artifacts = {}
    for path in sorted(directory.rglob('*')):
        if path.is_file():
            with path.open('rb') as source:
                artifacts[str(path.relative_to(directory))] = hashlib.file_digest(source, 'sha256').hexdigest()
    return {'directory': str(directory), 'manifest': manifest, 'arms': summaries,
            'contrasts': contrasts, 'artifacts_sha256': artifacts}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('runs', nargs='+', type=Path)
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO)
    print(json.dumps([analyze(path) for path in args.runs], ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()

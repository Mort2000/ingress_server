#!/usr/bin/env python3
"""Audit the frozen r2.6 recharge-only experiment and its two declared replays."""
from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import json
import logging
from pathlib import Path
from statistics import mean
import subprocess

from analyze import Match, contrast
from evaluate_supply import InventorySummary
from review_supply import SupplyAudit, digest


MAPS = [0, 1, 3, 6]
LEVELS = [15, 20, 25]


@dataclass(frozen=True)
class Experiment:
    group: str
    modes: list[str]
    previous: str
    active_mode: str
    sample_map: int


@dataclass(frozen=True)
class InitialCase:
    map_index: int
    leg: int
    recharge_amount: int


EXPERIMENTS = [
    Experiment('recharge-factorial', ['def0-atk0', 'def0-atk1', 'def2-atk0', 'def2-atk1'],
               'r26-supply-defense-r2', 'def2-atk1', 3),
    Experiment('recharge-adaptive', ['adaptive-def0', 'adaptive-def2'],
               'r26-supply-adaptive-r2', 'adaptive-def2', 6),
]


@dataclass
class RuleRecord:
    match: Match
    directory: Path
    config: dict[str, int]  # Complete GameConfig at the JSON boundary.
    world_hash: str

    @classmethod
    def read(cls, path: Path) -> RuleRecord:
        raw = json.loads(path.read_text())
        return cls(Match.read(path), path.parent, raw['config'], raw['initial_world_hash'])

    @property
    def amount(self) -> int:
        return self.config['recharge_amount']

    def audit_config(self, base: dict[str, int]) -> None:
        assert len(self.config) == 31
        assert self.amount in LEVELS
        assert {**self.config, 'recharge_amount': 15} == base, self.directory
        assert self.match.arm.startswith(f'h{self.amount}-'), self.directory

    def audit_supply(self) -> SupplyAudit:
        audit = SupplyAudit(self.match.name, str(self.directory), self.match.side)
        for line in (self.directory / 'trace.jsonl').open():
            event = json.loads(line)
            if event['kind'] == 'recharge':
                assert 0 < event['after'] - event['before'] <= self.amount
                assert event['after'] <= self.config['portal_max_hp']
            audit.read(event)
        audit.finish()
        assert audit.agent_rounds == self.match.stats['agent_rounds']
        assert sum(a.spent for a in audit.agents.values()) == self.match.stats['supply']
        assert sum(a.gained for a in audit.agents.values()) == self.match.stats['supply_gained']
        return audit


def mechanism_effects(rows: list[Match], experiment: Experiment, amount: int) -> dict:
    def effect(before: str, after: str, actor: str) -> dict:
        return contrast(rows, f'h{amount}-{before}', f'h{amount}-{after}', actor)
    if experiment.group == 'recharge-factorial':
        return {
            'attack_without_defense': effect('def0-atk0', 'def0-atk1', 'opponent'),
            'attack_with_defense': effect('def2-atk0', 'def2-atk1', 'opponent'),
            'defense_without_attack': effect('def0-atk0', 'def2-atk0', 'subject'),
            'defense_under_attack': effect('def0-atk1', 'def2-atk1', 'subject'),
        }
    return {'defense_increment': effect('adaptive-def0', 'adaptive-def2', 'subject')}


def inspect_sample(experiment: Experiment, directory: Path, output: Path, inspector: Path) -> None:
    sample = directory / f'h25-{experiment.active_mode}-m{experiment.sample_map}-leg0'
    raw = json.loads((sample / 'result.json').read_text())
    target = output / sample.name
    target.mkdir()
    args = [str(inspector.resolve()), str(sample / 'replay.iarp'), str(raw['bot_seed']),
            str(raw['subject_side']), raw['subject_parameters'], raw['opponent_parameters'], raw['version']]
    trace = target / 'inspection.jsonl'
    with trace.open('w') as destination, (target / 'inspection.log').open('w') as log:
        subprocess.run(args, stdout=destination, stderr=log, check=True)
    with trace.open() as source:
        first = last = json.loads(next(source))
        for line in source:
            last = json.loads(line)
    assert first['config'] == raw['config']
    assert first['initial_hash'] == raw['initial_hash']
    assert last['kind'] == 'verified' and last['rounds'] == 500
    assert last['rules'] and last['production_policies'] and last['request_shadow']
    assert last['final_hash'] == raw['final_hash']
    identity = {'selection': asdict(experiment), 'archive': str(sample), 'result': raw,
                'verification': last, 'inspector_sha256': digest(inspector),
                'source_sha256': {name: digest(sample / name)
                                  for name in ['result.json', 'replay.iarp', 'trace.jsonl']}}
    (target / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')
    logging.info('Verified all 500 rounds, production policies and request shadow: %s', sample.name)


def analyze(experiment: Experiment, directory: Path, output: Path, inspector: Path) -> None:
    manifest = dict(line.split('=', 1) for line in (directory / 'manifest.txt').read_text().splitlines())
    assert manifest['status'] == 'complete' and manifest['group'] == experiment.group
    assert manifest['version'] == 'r2.6'
    assert manifest['map_indices'] == ','.join(map(str, MAPS)) and manifest['maps'] == '4'
    assert manifest['recharge_amounts'] == '15,20,25'
    records = [RuleRecord.read(p) for p in sorted(directory.glob('*/result.json'))]
    expected = {f'h{amount}-{mode}' for amount in LEVELS for mode in experiment.modes}
    assert {r.match.arm for r in records} == expected
    count = len(expected) * len(MAPS) * 2
    assert len(records) == int(manifest['matches']) == count
    assert int(manifest['rules_verified']) == int(manifest['decisions_verified']) == count
    base_config = next(r.config for r in records if r.amount == 15)
    assert base_config['recharge_amount'] == 15 and base_config['attack_damage'] == 20
    assert base_config['recharge_cost_supply'] == base_config['hack_supply_yield'] == 1
    assert base_config['initial_supply'] == 20 and base_config['total_rounds'] == 500
    worlds: dict[tuple[int, int], RuleRecord] = {}
    initial: dict[InitialCase, str] = {}
    summaries = {}
    ledgers = []
    controls = []
    rows = [r.match for r in records]
    for arm in sorted(expected):
        selected = [r for r in records if r.match.arm == arm]
        assert {(r.match.map_index, r.match.leg) for r in selected} == {(m, leg) for m in MAPS for leg in range(2)}
        assert len(selected) == len(MAPS) * 2
        assert len({(r.match.parameters, r.match.opponent_parameters, r.match.version) for r in selected}) == 1
        inventory = InventorySummary()
        for record in selected:
            row = record.match
            record.audit_config(base_config)
            assert row.version == manifest['version'] and row.side == row.leg
            key = (row.map_index, row.leg)
            baseline = worlds.setdefault(key, record)
            assert record.world_hash == baseline.world_hash
            assert (row.bot_seed, row.game_seed) == (baseline.match.bot_seed, baseline.match.game_seed)
            case = InitialCase(row.map_index, row.leg, record.amount)
            assert row.initial_hash == initial.setdefault(case, row.initial_hash)
            row.audit(record.directory / 'trace.jsonl')
            audit = record.audit_supply()
            inventory.include(audit)
            ledgers.append(asdict(audit))
            mode = arm.split('-', 1)[1]
            same_policy = next(r for r in records if r.match.arm == f'h15-{mode}' and
                               (r.match.map_index, r.match.leg) == key)
            assert (row.parameters, row.opponent_parameters) == (same_policy.match.parameters, same_policy.match.opponent_parameters)
            if record.amount == 15 and mode == experiment.active_mode:
                previous = Path('runs/balance') / experiment.previous / f'reserve5-m{row.map_index}-leg{row.leg}'
                old = Match.read(previous / 'result.json')
                assert (row.parameters, row.opponent_parameters, row.version) == (old.parameters, old.opponent_parameters, old.version)
                assert (row.bot_seed, row.game_seed, row.initial_hash) == (old.bot_seed, old.game_seed, old.initial_hash)
                sha = digest(record.directory / 'replay.iarp')
                assert sha == digest(previous / 'replay.iarp'), record.directory
                controls.append({'match': row.name, 'previous': str(previous), 'sha256': sha})
        subjects = [r.match for r in selected]
        stats = {k: sum(r.stats[k] for r in subjects) for k in subjects[0].stats}
        summaries[arm] = {
            'config': selected[0].config, 'games': len(selected),
            'wins': sum(r.own > r.enemy for r in subjects),
            'ties': sum(r.own == r.enemy for r in subjects),
            'mean_own': mean(r.own for r in subjects), 'mean_enemy': mean(r.enemy for r in subjects),
            'pooled_score_share': sum(r.own for r in subjects) / sum(r.own + r.enemy for r in subjects),
            'subject_stats': stats, 'opponent_stats': {k: sum(r.opponent_stats[k] for r in subjects) for k in subjects[0].opponent_stats},
            'inventory': asdict(inventory), 'actual_hp_per_supply': stats['hp'] / stats['supply'] if stats['supply'] else None,
        }
    result = {
        'directory': str(directory), 'manifest': manifest, 'arms': summaries,
        'mechanism_effects': {amount: mechanism_effects(rows, experiment, amount) for amount in LEVELS},
        'rule_sensitivity': [contrast(rows, f'h15-{mode}', f'h{amount}-{mode}', 'subject')
                             for mode in experiment.modes for amount in [20, 25]],
        'baseline_replays_identical': controls,
        'artifacts_sha256': {str(p.relative_to(directory)): digest(p)
                            for p in sorted(directory.rglob('*')) if p.is_file()},
    }
    target = output / experiment.group
    target.mkdir()
    (target / 'analysis.json').write_text(json.dumps(result, indent=2) + '\n')
    (target / 'ledgers.json').write_text(json.dumps(ledgers, indent=2) + '\n')
    logging.info('Audited %s: %d matches, all config/policy pairings and Supply ledgers', experiment.group, count)
    inspect_sample(experiment, directory, target, inspector)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('factorial', type=Path)
    parser.add_argument('adaptive', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--inspector', type=Path, default=Path('target/release/examples/replay_inspect'))
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO)
    args.output.mkdir(parents=True, exist_ok=False)
    for experiment, directory in zip(EXPERIMENTS, [args.factorial, args.adaptive], strict=True):
        analyze(experiment, directory, args.output, args.inspector)
    hashes = {str(p.relative_to(args.output)): digest(p) for p in sorted(args.output.rglob('*')) if p.is_file()}
    (args.output / 'hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Audit existing r2.5 Supply ledgers and inspect two declared stress samples.

Requires an inspector built from the frozen r2.5 implementation, not current defaults.
Run: python3 tools/combat-validate/review_supply.py NEW_OUTPUT_DIR --inspector R25_BINARY
No games are generated and no strategy or historical artifact is changed.
"""
from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass, field
import hashlib
import json
import logging
from pathlib import Path
import subprocess

from analyze import Match


@dataclass(frozen=True)
class Cohort:
    directory: str
    arm: str
    sample: str
    probe_round: int
    probe_agent: int
    probe_portal: int
    probe_move: bool


COHORTS = (
    Cohort('r25-combat-factorial-r2', 'def2-atk1', 'def2-atk1-m3-leg0', 412, 6, 90, False),
    Cohort('r25-combat-adaptive-r2', 'adaptive-def2', 'adaptive-def2-m6-leg0', 430, 6, 99, True),
)


@dataclass
class SupplyLedger:
    initial: int
    current: int
    minimum: int
    gained: int = 0
    spent: int = 0
    zero_rounds: list[int] = field(default_factory=list)
    idle_zero_rounds: list[int] = field(default_factory=list)


@dataclass
class SupplyAudit:
    match: str
    directory: str
    subject_side: int
    agent_rounds: int = 0
    agents: dict[int, SupplyLedger] = field(default_factory=dict)
    inventory_histogram: dict[int, int] = field(default_factory=dict)

    def read(self, event: dict) -> None:
        """Typed archive JSON boundary; no interpretation of Debug commands."""
        if event.get('side') != self.subject_side:
            return
        kind = event['kind']
        if kind == 'decision':
            amount = event['supply']
            ledger = self.agents.setdefault(event['agent'], SupplyLedger(amount, amount, amount))
            assert ledger.current == amount, (self.match, event, ledger)
            self.agent_rounds += 1
            self.inventory_histogram[amount] = self.inventory_histogram.get(amount, 0) + 1
            ledger.minimum = min(ledger.minimum, amount)
            if amount == 0:
                ledger.zero_rounds.append(event['round'])
                if not event['submitted']:
                    ledger.idle_zero_rounds.append(event['round'])
        elif kind == 'recharge':
            ledger = self.agents[event['agent']]
            # These archived cohorts are default GameConfig: cost=1.
            assert ledger.current >= 1
            ledger.current -= 1
            ledger.spent += 1
            ledger.minimum = min(ledger.minimum, ledger.current)
        elif kind == 'hack':
            ledger = self.agents[event['agent']]
            assert ledger.current + event['supply'] == event['supply_after']
            ledger.current = event['supply_after']
            ledger.gained += event['supply']

    def finish(self) -> None:
        for ledger in self.agents.values():
            assert ledger.initial == 20
            assert ledger.initial + ledger.gained - ledger.spent == ledger.current


def digest(path: Path) -> str:
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def scan(cohort: Cohort) -> list[SupplyAudit]:
    directory = Path('runs/balance') / cohort.directory
    paths = sorted(directory.glob(f'{cohort.arm}-*/result.json'))
    assert len(paths) == 16
    audits = []
    for path in paths:
        row = Match.read(path)
        assert row.version == 'r2.5' and row.arm == cohort.arm
        trace = path.parent / 'trace.jsonl'
        row.audit(trace)
        audit = SupplyAudit(row.name, str(path.parent), row.side)
        with trace.open() as source:
            for line in source:
                audit.read(json.loads(line))
        audit.finish()
        assert audit.agent_rounds == row.stats['agent_rounds']
        assert sum(a.spent for a in audit.agents.values()) == row.stats['supply']
        assert sum(a.gained for a in audit.agents.values()) == row.stats['supply_gained']
        audits.append(audit)
    return audits


def inspect(cohort: Cohort, output: Path, executable: Path) -> None:
    directory = Path('runs/balance') / cohort.directory / cohort.sample
    raw = json.loads((directory / 'result.json').read_text())
    target = output / cohort.sample
    target.mkdir()
    args = [str(executable.resolve()), str(directory / 'replay.iarp'), str(raw['bot_seed']),
            str(raw['subject_side']), raw['subject_parameters'], raw['opponent_parameters'], raw['version'],
            str(cohort.probe_round), str(cohort.probe_agent), str(cohort.probe_portal), str(cohort.probe_move).lower()]
    with (target / 'inspection.jsonl').open('w') as trace, (target / 'inspection.log').open('w') as log:
        subprocess.run(args, check=True, stdout=trace, stderr=log)
    with (target / 'inspection.jsonl').open() as source:
        first = json.loads(next(source))
        last = first
        for line in source:
            last = json.loads(line)
    assert first['initial_hash'] == raw['initial_hash']
    assert last['kind'] == 'verified' and last['rounds'] == 500
    assert last['rules'] and last['production_policies'] and last['request_shadow']
    assert last['final_hash'] == raw['final_hash']
    identity = {'selection': asdict(cohort), 'result': raw,
                'source_sha256': {name: digest(directory / name)
                                  for name in ['result.json', 'replay.iarp', 'trace.jsonl']},
                'inspector_sha256': digest(executable), 'verification': last}
    (target / 'identity.json').write_text(json.dumps(identity, ensure_ascii=False, indent=2) + '\n')
    logging.info('Verified full replay, policies and request shadow: %s', cohort.sample)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--inspector', type=Path, required=True,
                        help='Inspector built from the archived r2.5 source')
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO)
    args.output.mkdir(parents=True, exist_ok=False)
    audits = [audit for cohort in COHORTS for audit in scan(cohort)]
    (args.output / 'supply.json').write_text(
        json.dumps([asdict(a) for a in audits], ensure_ascii=False, indent=2) + '\n')
    for cohort in COHORTS:
        inspect(cohort, args.output, args.inspector)
    hashes = {str(p.relative_to(args.output)): digest(p)
              for p in sorted(args.output.rglob('*')) if p.is_file()}
    (args.output / 'hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')


if __name__ == '__main__':
    main()

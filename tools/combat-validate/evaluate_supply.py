#!/usr/bin/env python3
"""Evaluate the frozen r2.6 Supply contrasts and replay the two fixed samples.

Consumes completed runs; creates a new evidence directory, never changes archives.
"""
from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import json
import logging
from pathlib import Path
import subprocess

from analyze import Match, analyze
from review_supply import SupplyAudit, digest


@dataclass(frozen=True)
class Comparison:
    group: str
    previous: str
    previous_arm: str
    sample_map: int


COMPARISONS = (
    Comparison('supply-defense', 'r25-combat-factorial-r2', 'def2-atk1', 3),
    Comparison('supply-adaptive', 'r25-combat-adaptive-r2', 'adaptive-def2', 6),
)


@dataclass
class InventorySummary:
    games: int = 0
    agent_rounds: int = 0
    zero_games: int = 0
    zero_agents: int = 0
    zero_rounds: int = 0
    idle_zero_rounds: int = 0
    initial: int = 0
    gained: int = 0
    spent: int = 0
    final: int = 0

    def include(self, audit: SupplyAudit) -> None:
        self.games += 1
        self.agent_rounds += audit.agent_rounds
        self.zero_games += any(a.zero_rounds for a in audit.agents.values())
        for agent in audit.agents.values():
            self.zero_agents += bool(agent.zero_rounds)
            self.zero_rounds += len(agent.zero_rounds)
            self.idle_zero_rounds += len(agent.idle_zero_rounds)
            self.initial += agent.initial
            self.gained += agent.gained
            self.spent += agent.spent
            self.final += agent.current


def evaluate(comparison: Comparison, directory: Path, output: Path, inspector: Path) -> None:
    result = analyze(directory)
    assert result['manifest']['group'] == comparison.group
    assert result['manifest']['version'] == 'r2.6'
    assert result['manifest']['maps'] == '8'
    destination = output / comparison.group
    destination.mkdir()
    summaries = {arm: InventorySummary() for arm in ('reserve0', 'reserve5')}
    audits = []
    controls = []
    for path in sorted(directory.glob('*/result.json')):
        row = Match.read(path)
        audit = SupplyAudit(row.name, str(path.parent), row.side)
        with (path.parent / 'trace.jsonl').open() as source:
            for line in source:
                audit.read(json.loads(line))
        audit.finish()
        assert audit.agent_rounds == row.stats['agent_rounds']
        assert sum(a.spent for a in audit.agents.values()) == row.stats['supply']
        assert sum(a.gained for a in audit.agents.values()) == row.stats['supply_gained']
        summaries[row.arm].include(audit)
        audits.append(asdict(audit))
        if row.arm == 'reserve0':
            previous = (Path('runs/balance') / comparison.previous /
                        f'{comparison.previous_arm}-m{row.map_index}-leg{row.leg}')
            # IARP includes every original stream and every state hash, not
            # just final score. Version/parameter metadata is verified separately.
            before = Match.read(previous / 'result.json')
            assert before.version == 'r2.5'
            assert row.parameters.replace(';supplyreserve=0', '') == before.parameters
            assert row.opponent_parameters.replace(';supplyreserve=0', '') == before.opponent_parameters
            assert (row.bot_seed, row.game_seed, row.side) == (before.bot_seed, before.game_seed, before.side)
            current_digest = digest(path.parent / 'replay.iarp')
            assert current_digest == digest(previous / 'replay.iarp'), path
            controls.append({'match': row.name, 'previous': str(previous), 'sha256': current_digest})
    result['supply'] = {arm: asdict(summary) for arm, summary in summaries.items()}
    result['controls_identical_to_r25'] = controls
    (destination / 'analysis.json').write_text(json.dumps(result, indent=2) + '\n')
    (destination / 'ledgers.json').write_text(json.dumps(audits, indent=2) + '\n')

    sample = directory / f'reserve5-m{comparison.sample_map}-leg0'
    raw = json.loads((sample / 'result.json').read_text())
    sample_output = destination / sample.name
    sample_output.mkdir()
    arguments = [str(inspector.resolve()), str(sample / 'replay.iarp'), str(raw['bot_seed']),
                 str(raw['subject_side']), raw['subject_parameters'], raw['opponent_parameters'], raw['version']]
    trace = sample_output / 'inspection.jsonl'
    with trace.open('w') as target, (sample_output / 'inspection.log').open('w') as log:
        subprocess.run(arguments, stdout=target, stderr=log, check=True)
    with trace.open() as source:
        first = json.loads(next(source))
        last = first
        for line in source:
            last = json.loads(line)
    assert first['initial_hash'] == raw['initial_hash']
    assert last['kind'] == 'verified' and last['rounds'] == 500
    assert last['rules'] and last['production_policies'] and last['request_shadow']
    assert last['final_hash'] == raw['final_hash']
    identity = {'archive': str(sample), 'result': raw, 'verification': last,
                'inspector_sha256': digest(inspector), 'replay_sha256': digest(sample / 'replay.iarp')}
    (sample_output / 'identity.json').write_text(json.dumps(identity, indent=2) + '\n')
    logging.info('%s: 32 ledgers, 16 byte-identical controls; sample verified: %s', comparison.group, sample.name)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('defense', type=Path)
    parser.add_argument('adaptive', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--inspector', type=Path, default=Path('target/release/examples/replay_inspect'))
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO)
    args.output.mkdir(parents=True, exist_ok=False)
    for comparison, directory in zip(COMPARISONS, (args.defense, args.adaptive), strict=True):
        evaluate(comparison, directory, args.output, args.inspector)
    hashes = {str(path.relative_to(args.output)): digest(path)
              for path in sorted(args.output.rglob('*')) if path.is_file()}
    (args.output / 'hashes.json').write_text(json.dumps(hashes, indent=2) + '\n')


if __name__ == '__main__':
    main()

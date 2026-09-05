#!/usr/bin/env python3
"""Recover an interrupted frozen recharge batch into NEW directories.

Never overwrite source partial traces. Completed matches are audited/copied;
the driver runs only missing names. Replays retain their producing identity.
"""
from __future__ import annotations

import argparse
import json
import logging
from pathlib import Path
import shutil
import subprocess

from analyze import Match
from analyze_recharge import EXPERIMENTS, LEVELS, MAPS, RuleRecord
from review_supply import digest


def resume(source: Path, retry: Path, output: Path, binary: Path, workers: int) -> None:
    manifest = dict(line.split('=', 1) for line in (source / 'manifest.txt').read_text().splitlines())
    experiment = next(e for e in EXPERIMENTS if e.group == manifest['group'])
    assert manifest['version'] == 'r2.6' and manifest['recharge_amounts'] == '15,20,25'
    assert manifest['map_indices'] == '0,1,3,6'
    assert not retry.exists() and not output.exists()
    names = [f'h{amount}-{mode}-m{m}-leg{leg}' for amount in LEVELS
             for mode in experiment.modes for m in MAPS for leg in range(2)]
    completed = [name for name in names if (source / name / 'result.json').exists()]
    missing = [name for name in names if name not in completed]
    source_hashes = {}
    for name in completed:
        match = Match.read(source / name / 'result.json')
        assert match.name == name and match.version == manifest['version']
        match.audit(source / name / 'trace.jsonl')
        rule = RuleRecord.read(source / name / 'result.json')
        assert rule.amount in LEVELS
        rule.audit_supply()
        source_hashes[name] = {file: digest(source / name / file)
                               for file in ['result.json', 'replay.iarp', 'trace.jsonl']}
    logging.info('%s: %d complete audited records; rerun only %d missing matches', source, len(completed), len(missing))
    if missing:
        subprocess.run([str(binary.resolve()), manifest['source_archive'], str(retry), manifest['group'],
                        'ids:0,1,3,6', str(workers), 'only:' + ','.join(missing)], check=True)
    output.mkdir(parents=True, exist_ok=False)
    results = []
    for name in names:
        origin = source if name in completed else retry
        path = origin / name
        match = Match.read(path / 'result.json')
        match.audit(path / 'trace.jsonl')
        if name in completed:
            assert all(digest(path / file) == checksum for file, checksum in source_hashes[name].items())
        shutil.copytree(path, output / name)
        results.append((path / 'result.json').read_text().strip())
    merged = {k: v for k, v in manifest.items() if k not in ['selected_matches', 'recovery_source', 'recovery_retry']}
    merged.update(status='complete', matches=str(len(names)), rules_verified=str(len(names)), decisions_verified=str(len(names)),
                  recovery_source=str(source), recovery_retry=str(retry), recovery_workers=str(workers))
    (output / 'manifest.txt').write_text(''.join(f'{key}={value}\n' for key, value in merged.items()))
    (output / 'matches.jsonl').write_text('\n'.join(sorted(results)) + '\n')
    recovery = {'source': str(source), 'retry': str(retry), 'reused': completed, 'rerun': missing,
                'binary_sha256': digest(binary), 'reused_sha256': source_hashes}
    (output / 'recovery.json').write_text(json.dumps(recovery, indent=2) + '\n')
    logging.info('Recovered complete batch into %s; original archives unchanged', output)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('retry', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--workers', type=int, default=6)
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO)
    resume(args.source, args.retry, args.output, args.binary, args.workers)


if __name__ == '__main__':
    main()

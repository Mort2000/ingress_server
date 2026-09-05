"""Regression checks for the evidence consumer, independent of Rust Debug text."""
import json
from pathlib import Path
import tempfile
import unittest

from analyze import Match, contrast


def match(arm: str, own: int, enemy: int) -> Match:
    stats = {'empty_grants': 0, 'stationary': 0, 'hp': 0,
             'recharges': 0, 'damage': 0, 'drops': 0}
    return Match(arm, arm, 0, 0, 0, own, enemy, 'initial', 1, 2, 'params',
                 'opponent', 'r2.4', stats, stats.copy())


class AnalysisTests(unittest.TestCase):
    def test_attack_effect_is_oriented_to_the_attacking_opponent(self) -> None:
        result = contrast([match('off', 100, 200), match('on', 20, 190)],
                          'off', 'on', 'opponent')
        self.assertEqual(result['mean_own_change'], -10)
        self.assertEqual(result['mean_enemy_change'], -80)
        self.assertEqual(result['mean_margin_change'], 70)

    def test_missing_paired_leg_is_rejected(self) -> None:
        with self.assertRaises(AssertionError):
            contrast([match('off', 100, 200)], 'off', 'on', 'subject')

    def test_lifecycle_income_and_destroyer_mapping(self) -> None:
        events = [{'kind': 'field-created', 'round': 1, 'side': 0,
                   'field': [0, 1, 2], 'area2': 100},
                  {'kind': 'income', 'round': 1, 'enl': 100, 'res': 0},
                  {'kind': 'field-destroyed-by', 'round': 2, 'side': 1,
                   'field': [0, 1, 2], 'area2': 100}]
        events += [{'kind': 'income', 'round': r, 'enl': 0, 'res': 0} for r in range(2, 501)]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'trace.jsonl'
            path.write_text('\n'.join(json.dumps(e) for e in events))
            match('ok', 100, 0).audit(path)
            events[2]['side'] = 0  # The event faction is the actor, not the owner.
            path.write_text('\n'.join(json.dumps(e) for e in events))
            with self.assertRaises(AssertionError):
                match('wrong-owner', 100, 0).audit(path)

    def test_zero_effect_recharge_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'trace.jsonl'
            path.write_text(json.dumps({'kind': 'recharge', 'side': 0, 'before': 100, 'after': 100}))
            with self.assertRaises(AssertionError):
                match('zero-heal', 0, 0).audit(path)


if __name__ == '__main__':
    unittest.main()

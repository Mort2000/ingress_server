import unittest
from pathlib import Path

from analyze_recharge import EXPERIMENTS, RuleRecord, mechanism_effects
from test_analyze import match


class RechargeTests(unittest.TestCase):
    def test_attack_and_defense_effects_use_their_own_faction_orientation(self) -> None:
        rows = [match('h20-def0-atk0', 100, 100), match('h20-def0-atk1', 20, 110),
                match('h20-def2-atk0', 90, 100), match('h20-def2-atk1', 60, 100)]
        effects = mechanism_effects(rows, EXPERIMENTS[0], 20)
        self.assertEqual(effects['attack_with_defense']['mean_margin_change'], 30)
        self.assertEqual(effects['defense_under_attack']['mean_margin_change'], 50)
        self.assertEqual(effects['attack_without_defense']['mean_margin_change'], 90)

    def test_a_second_rule_change_is_rejected(self) -> None:
        base = {f'field{n}': n for n in range(29)}
        base.update(recharge_amount=15, attack_damage=20)
        record = RuleRecord(match('h25-def2-atk1', 0, 0), Path('sample'),
                            {**base, 'recharge_amount': 25}, 'world')
        record.audit_config(base)
        record.config['attack_damage'] = 25
        with self.assertRaises(AssertionError):
            record.audit_config(base)

    def test_a_level_label_must_match_the_actual_configuration(self) -> None:
        base = {f'field{n}': n for n in range(30)}
        base['recharge_amount'] = 15
        record = RuleRecord(match('h25-def2-atk1', 0, 0), Path('sample'), base, 'world')
        with self.assertRaises(AssertionError):
            record.audit_config(base)


if __name__ == '__main__':
    unittest.main()

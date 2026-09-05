"""Supply accounting is separate from decisions and structured JSON serialization."""
from dataclasses import asdict
import json
import unittest

from review_supply import SupplyAudit
from evaluate_supply import InventorySummary


class SupplyTests(unittest.TestCase):
    def test_hack_after_recharge_reconciles_next_observation(self) -> None:
        audit = SupplyAudit('sample', 'sample', 0)
        audit.read({'kind': 'decision', 'side': 0, 'agent': 1, 'supply': 20,
                    'round': 1, 'submitted': True})
        audit.read({'kind': 'recharge', 'side': 0, 'agent': 1})
        audit.read({'kind': 'hack', 'side': 0, 'agent': 1, 'supply': 1, 'supply_after': 20})
        audit.read({'kind': 'decision', 'side': 0, 'agent': 1, 'supply': 20,
                    'round': 2, 'submitted': False})
        audit.finish()
        self.assertEqual(audit.agents[1].spent, 1)
        self.assertEqual(audit.agents[1].gained, 1)
        self.assertEqual(json.loads(json.dumps(asdict(audit)))['inventory_histogram'], {'20': 2})

    def test_unrecorded_supply_change_is_rejected(self) -> None:
        audit = SupplyAudit('sample', 'sample', 0)
        event = {'kind': 'decision', 'side': 0, 'agent': 1, 'supply': 20,
                 'round': 1, 'submitted': True}
        audit.read(event)
        with self.assertRaises(AssertionError):
            audit.read({**event, 'round': 2, 'supply': 19})

    def test_zero_inventory_does_not_mean_no_submission(self) -> None:
        audit = SupplyAudit('sample', 'sample', 0)
        audit.read({'kind': 'decision', 'side': 0, 'agent': 1, 'supply': 0,
                    'round': 1, 'submitted': True})
        self.assertEqual(audit.agents[1].zero_rounds, [1])
        self.assertEqual(audit.agents[1].idle_zero_rounds, [])

    def test_summary_counts_stockouts_separately_from_idle(self) -> None:
        audit = SupplyAudit('sample', 'sample', 0)
        for turn, submitted in [(1, True), (2, False)]:
            audit.read({'kind': 'decision', 'side': 0, 'agent': 1, 'supply': 0,
                        'round': turn, 'submitted': submitted})
        summary = InventorySummary()
        summary.include(audit)
        self.assertEqual(summary.agent_rounds, 2)
        self.assertEqual(summary.zero_games, 1)
        self.assertEqual(summary.zero_agents, 1)
        self.assertEqual(summary.zero_rounds, 2)
        self.assertEqual(summary.idle_zero_rounds, 1)


if __name__ == '__main__':
    unittest.main()

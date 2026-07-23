#!/usr/bin/env python3
"""Focused source-level regression checks for product-gap closure records.

These checks preserve the distinction between generic source completion and
selected-host, runtime, release, publication, and field evidence.
"""

from __future__ import annotations

from pathlib import Path
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = ROOT / "contracts/plan/product-gap-closure-story-inventory-v1.yaml"
CAMPAIGN_PATH = ROOT / "contracts/plan/product-gap-closure-campaign-v1.yaml"
PLAN_PATH = ROOT / "contracts/plan/product-gap-closure-plan.yaml"

EXPECTED_SOURCE_COMPLETE = {
    "C1.2.work.1",
    "C1.2.work.2",
    "C1.2.work.3",
    "C1.2.work.4",
    "C1.3.work.3",
    "C1.3.work.4",
    "C3.1.work.3",
    "C3.1.work.5",
    "FRUST-002",
    "FRUST-010",
    "FRUST-011",
    "FRUST-060",
}


class ProductGapRecordTests(unittest.TestCase):
    def test_generic_source_closures_retain_evidence_boundary(self) -> None:
        inventory = yaml.safe_load(INVENTORY_PATH.read_text(encoding="utf-8"))
        records = {record["id"]: record for record in inventory["current_records"]}
        for record_id in EXPECTED_SOURCE_COMPLETE:
            with self.subTest(record_id=record_id):
                record = records[record_id]
                self.assertEqual(record["status"], "source_complete")
                self.assertTrue(record["source_complete"])
                self.assertEqual(record["remaining_source_work"], [])
                self.assertIsNone(record["owner"])
                self.assertIsNotNone(record["checkpoint"])

    def test_campaign_source_closures_remain_pending_evidence(self) -> None:
        campaign = yaml.safe_load(CAMPAIGN_PATH.read_text(encoding="utf-8"))
        items = {item["id"]: item for item in campaign["items"]}
        for item_id in ("C1.2", "C3.1"):
            with self.subTest(item_id=item_id):
                item = items[item_id]
                self.assertEqual(item["status"], "implemented_pending_evidence")
                self.assertIsNotNone(item["checkpoint"])
                self.assertTrue(item["checkpoint"]["remaining_work"])

    def test_reference_host_selection_remains_none(self) -> None:
        plan = yaml.safe_load(PLAN_PATH.read_text(encoding="utf-8"))
        first_phase = next(
            phase
            for phase in plan["phases"]
            if phase["id"] == "C1-first-use-authority-vertical-slice"
        )
        c1_1 = next(item for item in first_phase["sequence"] if item["id"] == "C1.1")
        self.assertEqual(
            c1_1["screening_checkpoint"]["selected_reference_host"]["kind"],
            "none",
        )


if __name__ == "__main__":
    unittest.main()

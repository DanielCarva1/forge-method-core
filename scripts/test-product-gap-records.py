#!/usr/bin/env python3
"""Focused source-level regression checks for product-gap closure records.

These checks preserve the distinction between generic source completion and
selected-host, runtime, release, publication, and field evidence.
"""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import yaml


ROOT = Path(__file__).resolve().parents[1]
SPEC_PATH = ROOT / "contracts/spec/solo-dogfood-readiness-v0.yaml"
INVENTORY_PATH = ROOT / "contracts/plan/product-gap-closure-story-inventory-v1.yaml"
CAMPAIGN_PATH = ROOT / "contracts/plan/product-gap-closure-campaign-v1.yaml"
PLAN_PATH = ROOT / "contracts/plan/product-gap-closure-plan.yaml"
COMMAND_GATE_PATH = ROOT / "scripts/block-deferred-build-command.py"

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


def load_command_gate():
    spec = importlib.util.spec_from_file_location(
        "forge_product_gap_command_gate", COMMAND_GATE_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {COMMAND_GATE_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ProductGapRecordTests(unittest.TestCase):
    def setUp(self) -> None:
        self.authorities = {
            "spec": yaml.safe_load(SPEC_PATH.read_text(encoding="utf-8")),
            "campaign": yaml.safe_load(CAMPAIGN_PATH.read_text(encoding="utf-8")),
            "inventory": yaml.safe_load(INVENTORY_PATH.read_text(encoding="utf-8")),
            "plan": yaml.safe_load(PLAN_PATH.read_text(encoding="utf-8")),
        }

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

    def test_strict_reference_host_selection_remains_none(self) -> None:
        plan = self.authorities["plan"]
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

    def test_solo_authority_opens_implementation_not_publication(self) -> None:
        gate = load_command_gate()
        self.assertEqual(
            gate.stage_permissions(self.authorities),
            {
                "solo_development": True,
                "stabilization": False,
                "publication": False,
                "field": False,
            },
        )
        self.assertIsNone(
            gate.blocked_reason(
                "/home/user/.cargo/bin/cargo test --workspace",
                self.authorities,
            )
        )
        self.assertIsNotNone(
            gate.blocked_reason("/usr/bin/git push origin HEAD", self.authorities)
        )
        self.assertIsNotNone(
            gate.blocked_reason("/usr/bin/forge field apply", self.authorities)
        )

    def test_solo_authority_projection_drift_fails_closed(self) -> None:
        gate = load_command_gate()
        divergent = copy.deepcopy(self.authorities)
        divergent["inventory"]["current_product_authority"]["milestone_qualified"] = True
        self.assertEqual(
            gate.stage_permissions(divergent),
            {
                "solo_development": False,
                "stabilization": False,
                "publication": False,
                "field": False,
            },
        )
        self.assertIsNotNone(
            gate.blocked_reason(
                "/home/user/.cargo/bin/cargo test --workspace",
                divergent,
            )
        )

    def test_preserved_strict_profile_retains_external_gates(self) -> None:
        gate = load_command_gate()
        strict = copy.deepcopy(self.authorities)
        for document in strict.values():
            document["current_product_authority"]["readiness_profile"] = (
                "strict_external"
            )
        self.assertEqual(
            gate.stage_permissions(strict),
            {
                "solo_development": False,
                "stabilization": False,
                "publication": False,
                "field": False,
            },
        )
        self.assertIsNotNone(
            gate.blocked_reason(
                "/home/user/.cargo/bin/cargo test --workspace",
                strict,
            )
        )

    def test_rank_one_spec_drift_absence_and_top_level_corruption_fail_closed(self) -> None:
        gate = load_command_gate()
        closed = {
            "solo_development": False,
            "stabilization": False,
            "publication": False,
            "field": False,
        }
        for mutate in (
            lambda value: value["spec"]["current_product_authority"].__setitem__(
                "milestone_qualified", True
            ),
            lambda value: value["spec"]["current_product_authority"][
                "authority_chain"
            ][0].__setitem__("rank", 2),
            lambda value: value["spec"].__setitem__("unexpected_authority", True),
        ):
            divergent = copy.deepcopy(self.authorities)
            mutate(divergent)
            self.assertEqual(gate.stage_permissions(divergent), closed)
        missing = copy.deepcopy(self.authorities)
        del missing["spec"]
        self.assertEqual(gate.stage_permissions(missing), closed)

    def test_rank_one_spec_symlink_is_not_loaded(self) -> None:
        gate = load_command_gate()
        with tempfile.TemporaryDirectory() as raw:
            symlink = Path(raw) / "solo-spec.yaml"
            symlink.symlink_to(SPEC_PATH)
            with mock.patch.object(gate, "SPEC", symlink):
                self.assertIsNone(gate.load_authorities())

    def test_top_level_authority_identity_drift_fails_closed(self) -> None:
        gate = load_command_gate()
        closed = {
            "solo_development": False,
            "stabilization": False,
            "publication": False,
            "field": False,
        }
        probes = (
            ("spec", "created_at", "2026-07-28"),
            ("campaign", "base_commit", "drift"),
            ("campaign", "status", "completed"),
            ("plan", "source_checkpoint", "drift"),
            ("inventory", "status", "completed"),
        )
        for authority, field, value in probes:
            with self.subTest(authority=authority, field=field):
                divergent = copy.deepcopy(self.authorities)
                divergent[authority][field] = value
                self.assertEqual(gate.stage_permissions(divergent), closed)
                self.assertIsNotNone(
                    gate.blocked_reason(
                        "/home/user/.cargo/bin/cargo test --workspace",
                        divergent,
                    )
                )

    def test_contradictory_projection_boundaries_fail_runtime_gate(self) -> None:
        gate = load_command_gate()
        closed = {
            "solo_development": False,
            "stabilization": False,
            "publication": False,
            "field": False,
        }
        divergent_plan = copy.deepcopy(self.authorities)
        divergent_plan["plan"]["sequencing_policy"][
            "active_item_evidence_boundary"
        ] += "; publication also closes"
        self.assertEqual(gate.stage_permissions(divergent_plan), closed)

        divergent_campaign = copy.deepcopy(self.authorities)
        divergent_campaign["campaign"]["authority"]["precedence"][0][
            "owns"
        ] += "; field qualification"
        self.assertEqual(gate.stage_permissions(divergent_campaign), closed)

        divergent_inventory = copy.deepcopy(self.authorities)
        divergent_inventory["inventory"][
            "preserved_strict_external_record_projection"
        ]["evidence_boundary"] += "; records also qualify Solo"
        self.assertEqual(gate.stage_permissions(divergent_inventory), closed)

    def test_solo_development_allowlist_blocks_arbitrary_execution(self) -> None:
        gate = load_command_gate()
        for command in (
            "/home/user/.cargo/bin/cargo test --workspace",
            "/home/user/.cargo/bin/cargo build --release",
            "/home/user/.cargo/bin/cargo clippy --workspace",
            "/usr/bin/gh workflow run ci.yml",
        ):
            self.assertIsNone(
                gate.blocked_reason(command, self.authorities),
                command,
            )
        for command in (
            "/home/user/.cargo/bin/cargo test --workspace --manifest-path /tmp/evil/Cargo.toml",
            "/home/user/.cargo/bin/cargo install --git https://example.invalid/evil.git",
            "/home/user/.cargo/bin/cargo install --path /tmp/evil",
            "/home/user/.cargo/bin/rustup toolchain uninstall 1.85.1",
            "/home/user/.cargo/bin/rustup target remove x86_64-unknown-linux-gnu",
            "/usr/bin/act -W /tmp/untrusted.yml",
            "/usr/bin/gh run rerun 1234",
            "/usr/bin/gh release create v1.0.0",
            "/usr/bin/gh workflow run release.yml",
            "/tmp/arbitrary-runner cargo test --workspace",
        ):
            self.assertIsNotNone(
                gate.blocked_reason(command, self.authorities),
                command,
            )


if __name__ == "__main__":
    unittest.main()

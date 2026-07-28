#!/usr/bin/env python3
"""Focused regressions for the authoritative static-check inventory."""

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest import mock

import yaml


CHECKER_PATH = Path(__file__).resolve().with_name("check-static-structured-text.py")


def load_checker():
    spec = importlib.util.spec_from_file_location(
        "forge_static_structured_text_checker", CHECKER_PATH
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {CHECKER_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load_checker()


def reference(name: str, command_index: int) -> dict[str, object]:
    return {"name": name, "kind": "lint", "command_index": command_index}


class StaticCheckInventoryTests(unittest.TestCase):
    def test_repository_inventory_is_ordered_complete_and_duplicate_free(self) -> None:
        static_checks = checker.pi_check_inventory()
        self.assertEqual(
            [check["name"] for check in static_checks],
            [
                "structured-text",
                "doc-links",
                "msrv-policy",
                "release-locking",
                "diff-check",
                "workspace-compile-feedback",
            ],
        )
        self.assertEqual(
            [check["command"] for check in static_checks],
            checker.PI_COMMANDS,
        )
        self.assertEqual(
            len(static_checks), len(checker.REQUIRED_PI_CHECK_DESCRIPTORS)
        )
        self.assertEqual(
            len({check["name"] for check in static_checks}), len(static_checks)
        )
        self.assertEqual(
            len({check["command"] for check in static_checks}), len(static_checks)
        )
        self.assertNotIn("public-promises", {check["name"] for check in static_checks})

    def test_missing_command_reference_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            checker.StaticCheckInventoryError,
            r"missing command references: indices \[1\]",
        ):
            checker.resolve_check_inventory(
                ["first-command", "second-command"],
                [reference("first", 0)],
            )

    def test_duplicate_command_reference_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            checker.StaticCheckInventoryError,
            "duplicate command reference: index 0",
        ):
            checker.resolve_check_inventory(
                ["first-command", "second-command"],
                [
                    reference("first", 0),
                    reference("duplicate-first", 0),
                    reference("second", 1),
                ],
            )

    def test_out_of_bounds_command_reference_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            checker.StaticCheckInventoryError,
            "points out of bounds to command index 2; command_count=2",
        ):
            checker.resolve_check_inventory(
                ["first-command", "second-command"],
                [reference("first", 0), reference("outside", 2)],
            )

    def test_reference_reordering_is_rejected(self) -> None:
        with self.assertRaisesRegex(
            checker.StaticCheckInventoryError,
            r"deterministic command order; expected=\[0, 1\], actual=\[1, 0\]",
        ):
            checker.resolve_check_inventory(
                ["first-command", "second-command"],
                [reference("second", 1), reference("first", 0)],
            )

    def test_entrypoint_converts_inventory_errors_to_validator_failures(self) -> None:
        with mock.patch.object(
            checker,
            "PI_CHECK_REFERENCES",
            [reference("first", 0)],
        ), mock.patch.object(
            checker,
            "PI_COMMANDS",
            ["first-command", "second-command"],
        ), self.assertRaisesRegex(
            SystemExit,
            "static structured-text check failed: authoritative static check inventory "
            r"is invalid: missing command references: indices \[1\]",
        ):
            checker.authoritative_pi_checks()

    def test_bilateral_command_and_reference_removal_is_rejected(self) -> None:
        commands = list(checker.PI_COMMANDS)
        references = [dict(value) for value in checker.PI_CHECK_REFERENCES]
        del commands[1]
        del references[1]
        for value in references[1:]:
            value["command_index"] -= 1
        with self.assertRaisesRegex(
            checker.StaticCheckInventoryError,
            "differs from independent required descriptor authority",
        ):
            checker.resolve_check_inventory(
                commands,
                references,
                checker.REQUIRED_PI_CHECK_DESCRIPTORS,
            )

    def test_malformed_compile_descriptor_fails_controlled(self) -> None:
        malformed = [dict(value) for value in checker.PI_CHECK_REFERENCES]
        malformed[-1] = {
            "name": "workspace-compile-feedback",
            "command_index": len(checker.PI_COMMANDS) - 1,
        }
        with self.assertRaisesRegex(
            checker.StaticCheckInventoryError,
            "must contain exactly",
        ):
            checker.resolve_check_inventory(
                list(checker.PI_COMMANDS),
                malformed,
                checker.REQUIRED_PI_CHECK_DESCRIPTORS,
            )

    def test_real_pi_loop_loads_and_matches_authority(self) -> None:
        document = json.loads(checker.PI_LOOP.read_text(encoding="utf-8"))
        checker.validate_pi_loop(document)
        self.assertEqual(document["checks"], checker.pi_check_inventory())

    def test_real_solo_dogfood_spec_and_ticket_dag_match(self) -> None:
        document = yaml.safe_load(
            checker.SOLO_DOGFOOD_SPEC.read_text(encoding="utf-8")
        )
        checker.validate_solo_dogfood_spec(document)

    def test_solo_dogfood_global_root_claim_and_ticket_dag_fail_closed(self) -> None:
        document = yaml.safe_load(
            checker.SOLO_DOGFOOD_SPEC.read_text(encoding="utf-8")
        )
        wrong_root = copy.deepcopy(document)
        wrong_root["implementation_decisions"]["host_support"]["required_claims"][1] = (
            "canonical WSL root resolution"
        )
        with self.assertRaisesRegex(
            SystemExit, "host required claims drifted"
        ):
            checker.validate_solo_dogfood_spec(wrong_root)

        cyclic = copy.deepcopy(document)
        cyclic["delivery_backlog"]["tickets"][0]["blocked_by"] = ["21"]
        with self.assertRaisesRegex(SystemExit, "dependency cycle"):
            checker.validate_solo_dogfood_spec(cyclic)

    def test_solo_dogfood_ticket_blocker_parity_is_checked(self) -> None:
        document = yaml.safe_load(
            checker.SOLO_DOGFOOD_SPEC.read_text(encoding="utf-8")
        )
        with tempfile.TemporaryDirectory() as directory:
            ticket_directory = Path(directory) / "issues"
            shutil.copytree(checker.SOLO_TICKET_DIRECTORY, ticket_directory)
            ticket = ticket_directory / "13-ship-host-neutral-conformance-kit.md"
            source = ticket.read_text(encoding="utf-8")
            ticket.write_text(
                source.replace(
                    "**Blocked by:** 12 —",
                    "**Blocked by:** 11 —",
                    1,
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(SystemExit, "blocker parity drifted"):
                checker.validate_solo_dogfood_spec(
                    document, ticket_directory=ticket_directory
                )

    def test_allowlisted_markdown_rejects_form_feed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ticket.md"
            path.write_bytes(b"# Ticket\ncorrupt\x0ccontrol\n")
            with self.assertRaisesRegex(SystemExit, r"forbidden control U\+000C"):
                checker.read_strict_utf8_text(
                    path, "test Markdown ticket", reject_controls=True
                )

    def test_gap_010_notes_are_an_authoritative_string_list(self) -> None:
        document = yaml.safe_load(checker.INVENTORY.read_text(encoding="utf-8"))
        _, records = checker.validate_inventory(document)
        notes = records["GAP-010.codex-conformance"]["notes"]
        self.assertIsInstance(notes, list)
        self.assertTrue(notes)
        self.assertTrue(all(isinstance(note, str) for note in notes))


if __name__ == "__main__":
    unittest.main()

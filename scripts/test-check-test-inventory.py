#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("check-test-inventory.py")


class InventoryTests(unittest.TestCase):
    def run_tool(self, root: Path, update: bool, output: str) -> subprocess.CompletedProcess[str]:
        baseline = root / "baseline.json"
        report = root / "report.json"
        args = [
            sys.executable,
            str(SCRIPT),
            "--baseline",
            str(baseline),
            "--label",
            "fixture",
            "--report",
            str(report),
        ]
        if update:
            args.append("--update")
        args.extend(["--", sys.executable, "-c", f"print({output!r})"])
        return subprocess.run(args, text=True, capture_output=True)

    def test_update_check_and_drift(self) -> None:
        first = "     Running tests/a.rs (target/a)\nalpha: test\nbeta: test\n"
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            self.assertEqual(self.run_tool(root, True, first).returncode, 0)
            baseline = json.loads((root / "baseline.json").read_text())
            self.assertEqual(
                baseline["tests"],
                ["tests/a.rs::alpha", "tests/a.rs::beta"],
            )
            self.assertEqual(self.run_tool(root, False, first).returncode, 0)
            drift = "     Running tests/a.rs (target/a)\nalpha: test\n"
            result = self.run_tool(root, False, drift)
            self.assertEqual(result.returncode, 3)
            self.assertIn("removed", result.stderr)

    def test_ansi_colored_cargo_harnesses_are_normalized(self) -> None:
        colored = (
            "\x1b[1m\x1b[92m     Running\x1b[0m tests/a.rs (target/a)\n"
            "alpha: test\n"
            "\x1b[1m\x1b[92m   Doc-tests\x1b[0m fixture_crate\n"
            "src/lib.rs - example (line 1): test\n"
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            result = self.run_tool(root, True, colored)
            self.assertEqual(result.returncode, 0, result.stderr)
            baseline = json.loads((root / "baseline.json").read_text())
            self.assertEqual(
                baseline["tests"],
                [
                    "doc:fixture_crate::src/lib.rs - example (line 1)",
                    "tests/a.rs::alpha",
                ],
            )
            self.assertEqual(self.run_tool(root, False, colored).returncode, 0)

    def test_command_failure_is_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--baseline",
                    str(Path(temp) / "baseline.json"),
                    "--label",
                    "fixture",
                    "--",
                    sys.executable,
                    "-c",
                    "raise SystemExit(7)",
                ],
                text=True,
                capture_output=True,
            )
            self.assertEqual(result.returncode, 7)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Focused tests for the cross-platform CI tier timing wrapper."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest


WRAPPER = Path(__file__).resolve().with_name("run-ci-tier.py")


class RunCiTierTests(unittest.TestCase):
    def run_case(
        self, *, command_exit: int, budget_seconds: float, sleep_seconds: float = 0
    ) -> tuple[subprocess.CompletedProcess[str], dict[str, object], str]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "nested" / "timing.json"
            summary = root / "summary.md"
            child = (
                "import time; "
                f"time.sleep({sleep_seconds!r}); "
                f"raise SystemExit({command_exit})"
            )
            env = os.environ.copy()
            env.update(
                {
                    "GITHUB_STEP_SUMMARY": str(summary),
                    "RUNNER_OS": "test-os",
                    "RUNNER_ARCH": "test-arch",
                    "RUNNER_NAME": "test-runner",
                }
            )
            completed = subprocess.run(
                [
                    sys.executable,
                    str(WRAPPER),
                    "--tier",
                    "test-tier",
                    "--budget-seconds",
                    str(budget_seconds),
                    "--report",
                    str(report),
                    "--cache-context",
                    "rust-cache-hit=true",
                    "--",
                    sys.executable,
                    "-c",
                    child,
                ],
                check=False,
                capture_output=True,
                text=True,
                env=env,
            )
            return completed, json.loads(report.read_text()), summary.read_text()

    def assert_common_evidence(self, report: dict[str, object], summary: str) -> None:
        self.assertEqual(report["tier"], "test-tier")
        self.assertEqual(report["cache_context"], "rust-cache-hit=true")
        self.assertEqual(report["runner_context"]["os"], "test-os")
        self.assertEqual(report["runner_context"]["arch"], "test-arch")
        self.assertIn("elapsed_seconds", report)
        self.assertIn("CI tier: `test-tier`", summary)
        self.assertIn("rust-cache-hit=true", summary)

    def test_pass(self) -> None:
        completed, report, summary = self.run_case(command_exit=0, budget_seconds=60)
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(report["outcome"], "passed")
        self.assertEqual(report["command_status"], "passed")
        self.assertEqual(report["budget_status"], "within_budget")
        self.assert_common_evidence(report, summary)

    def test_command_failure_preserves_exit_code(self) -> None:
        completed, report, summary = self.run_case(command_exit=7, budget_seconds=60)
        self.assertEqual(completed.returncode, 7)
        self.assertEqual(report["outcome"], "command_failed")
        self.assertEqual(report["command_exit_code"], 7)
        self.assertEqual(report["budget_status"], "within_budget")
        self.assert_common_evidence(report, summary)

    def test_timeout_kills_child_and_has_distinct_exit_code(self) -> None:
        completed, report, summary = self.run_case(
            command_exit=0, budget_seconds=0.01, sleep_seconds=10
        )
        self.assertEqual(completed.returncode, 124)
        self.assertEqual(report["outcome"], "timed_out")
        self.assertEqual(report["command_exit_code"], 124)
        self.assertEqual(report["command_status"], "timed_out")
        self.assertEqual(report["budget_status"], "exceeded")
        self.assertTrue(report["timed_out"])
        self.assertIn(report["termination"], {"terminated", "killed"})
        self.assert_common_evidence(report, summary)

    def test_timeout_kills_descendant_process_tree(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "orphan-wrote.txt"
            report = root / "timing.json"
            grandchild = (
                "import pathlib,time; time.sleep(1); "
                f"pathlib.Path({str(marker)!r}).write_text('orphan')"
            )
            parent = (
                "import subprocess,sys,time; "
                f"subprocess.Popen([sys.executable, '-c', {grandchild!r}]); "
                "time.sleep(10)"
            )
            completed = subprocess.run(
                [
                    sys.executable,
                    str(WRAPPER),
                    "--tier",
                    "tree-timeout",
                    "--budget-seconds",
                    "0.1",
                    "--report",
                    str(report),
                    "--",
                    sys.executable,
                    "-c",
                    parent,
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 124)
            self.assertEqual(json.loads(report.read_text())["outcome"], "timed_out")

            time.sleep(1.2)
            self.assertFalse(marker.exists(), "timed-out descendant survived its process group")

    @unittest.skipIf(os.name == "nt", "POSIX TERM-to-KILL escalation")
    def test_timeout_escalates_when_child_ignores_term(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "timing.json"
            child = (
                "import signal,time; "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                "time.sleep(10)"
            )
            completed = subprocess.run(
                [
                    sys.executable,
                    str(WRAPPER),
                    "--tier",
                    "forced-timeout",
                    "--budget-seconds",
                    "0.1",
                    "--report",
                    str(report),
                    "--",
                    sys.executable,
                    "-c",
                    child,
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            evidence = json.loads(report.read_text())
            self.assertEqual(completed.returncode, 124)
            self.assertEqual(evidence["termination"], "killed")


    def test_launch_failure_still_writes_report(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "timing.json"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(WRAPPER),
                    "--tier",
                    "missing-command",
                    "--budget-seconds",
                    "1",
                    "--report",
                    str(report),
                    "--",
                    str(Path(directory) / "does-not-exist"),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            evidence = json.loads(report.read_text())
            self.assertEqual(completed.returncode, 127)
            self.assertEqual(evidence["outcome"], "command_failed")
            self.assertIsNotNone(evidence["launch_error"])


if __name__ == "__main__":
    unittest.main()

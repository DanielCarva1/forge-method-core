#!/usr/bin/env python3
"""Regression tests for mandatory CI verdict and summary semantics."""

from __future__ import annotations

import importlib.util
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
CI_WORKFLOW = ROOT / ".github/workflows/ci.yml"


def load_checker():
    path = ROOT / "scripts/check-ci-verdict.py"
    spec = importlib.util.spec_from_file_location("forge_ci_verdict", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


checker = load_checker()


def load_topology_checker():
    path = ROOT / "scripts/check-msrv.py"
    spec = importlib.util.spec_from_file_location("forge_ci_topology", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


topology = load_topology_checker()


class CiVerdictTests(unittest.TestCase):
    def row(self, job_id: str, state: str):
        return checker.JobResult(job_id, f"label for {job_id}", state)

    def test_lists_every_mandatory_terminal_state_and_passes_only_success(self) -> None:
        mandatory = [
            self.row("static_docs", "success"),
            self.row("msrv", "success"),
            self.row("focused", "success"),
        ]
        summary, passed = checker.render_summary(mandatory, [])
        self.assertTrue(passed)
        for result in mandatory:
            self.assertIn(f"`{result.job_id}`", summary)
            self.assertIn("`success`", summary)
        self.assertIn("Required source-only verdict: PASS", summary)

    def test_failure_cancel_skip_missing_and_unknown_fail_closed(self) -> None:
        for state in ("failure", "cancelled", "skipped", "", "timed_out"):
            with self.subTest(state=state):
                summary, passed = checker.render_summary(
                    [self.row("static_docs", "success"), self.row("focused", state)],
                    [],
                )
                self.assertFalse(passed)
                self.assertIn("Required source-only verdict: FAIL", summary)
                self.assertIn("**FAIL**", summary)

    def test_real_yaml_topology_makes_mandatory_failure_red(self) -> None:
        source = CI_WORKFLOW.read_text(encoding="utf-8")
        topology.check_workflow_source(source)
        jobs = topology.parse_workflow(source)["jobs"]

        self.assertEqual(jobs["msrv"]["needs"], "static_docs")
        self.assertEqual(jobs["msrv"]["if"], "always()")
        self.assertEqual(jobs["focused"]["needs"], "static_docs")
        self.assertNotIn("continue-on-error", jobs["focused"])
        self.assertEqual(jobs["ci-verdict"]["if"], "always()")
        self.assertEqual(
            jobs["ci-verdict"]["needs"],
            [
                "static_docs",
                "msrv",
                "focused",
                "platform",
                "expensive-journey",
            ],
        )
        self.assertEqual(jobs["platform"]["continue-on-error"], "true")
        self.assertEqual(jobs["expensive-journey"]["continue-on-error"], "true")

        mandatory = [
            self.row(job_id, "failure" if job_id == "focused" else "success")
            for job_id in topology.REQUIRED_CI_RESULT_JOBS
        ]
        informational = [
            self.row(job_id, "failure")
            for job_id in topology.INFORMATIONAL_CHANNEL_JOBS
        ]
        summary, passed = checker.render_summary(mandatory, informational)
        self.assertFalse(passed)
        self.assertEqual(summary.count("| **FAIL** |"), 1)
        self.assertEqual(summary.count("**none (excluded)**"), 2)

    def test_prerelease_observations_cannot_satisfy_or_fail_readiness(self) -> None:
        mandatory = [self.row("focused", "success")]
        informational = [
            self.row("platform", "failure"),
            self.row("expensive-journey", "success"),
        ]
        summary, passed = checker.render_summary(mandatory, informational)
        self.assertTrue(passed)
        self.assertIn("Prerelease-channel informational observations (excluded)", summary)
        self.assertEqual(summary.count("**none (excluded)**"), 2)
        self.assertIn("cannot satisfy this verdict", summary)
        self.assertIn("continue-on-error", summary)
        self.assertNotIn("0.12.0-alpha.1", summary)

    def test_empty_mandatory_set_and_duplicate_classification_are_rejected(self) -> None:
        with self.assertRaisesRegex(
            checker.VerdictConfigurationError, "empty readiness set"
        ):
            checker.render_summary([], [self.row("platform", "success")])
        with self.assertRaisesRegex(
            checker.VerdictConfigurationError, "both mandatory and informational"
        ):
            checker.render_summary(
                [self.row("focused", "success")], [self.row("focused", "failure")]
            )

    def test_cli_writes_summary_and_returns_nonzero_for_mandatory_failure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            summary_path = Path(directory) / "summary.md"
            argv = [
                "--mandatory",
                "static_docs",
                "Tier 0 static and docs",
                "success",
                "--mandatory",
                "focused",
                "Focused package evidence",
                "failure",
                "--informational",
                "platform",
                "Informational prerelease platform matrix",
                "success",
            ]
            stdout = io.StringIO()
            stderr = io.StringIO()
            with (
                mock.patch.dict(
                    os.environ, {"GITHUB_STEP_SUMMARY": str(summary_path)}, clear=False
                ),
                redirect_stdout(stdout),
                redirect_stderr(stderr),
            ):
                result = checker.main(argv)
            self.assertEqual(result, 1)
            summary = summary_path.read_text(encoding="utf-8")
            self.assertIn("`static_docs`", summary)
            self.assertIn("`focused`", summary)
            self.assertIn("Required source-only verdict: FAIL", summary)
            self.assertIn("**none (excluded)**", summary)


if __name__ == "__main__":
    unittest.main()

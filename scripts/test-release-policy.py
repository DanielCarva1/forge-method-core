#!/usr/bin/env python3
"""Adversarial tests for protected-base release policy admission."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/release.yml"
POLICY_WORKFLOW = ROOT / ".github/workflows/release-policy.yml"


def load_checker():
    path = ROOT / "scripts/check-release-policy.py"
    spec = importlib.util.spec_from_file_location("forge_release_policy_checker", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load_checker()
lock_checker = checker._load_release_lock_checker()


class ReleasePolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.policy_source = POLICY_WORKFLOW.read_text(encoding="utf-8")

    def replace_policy_once(self, old: str, new: str) -> str:
        self.assertEqual(self.policy_source.count(old), 1, old)
        return self.policy_source.replace(old, new, 1)

    def assert_policy_rejected(self, old: str, new: str, reason: str) -> None:
        with self.assertRaisesRegex(checker.ReleasePolicyError, reason):
            checker.check_policy_workflow_source(self.replace_policy_once(old, new))

    def copied_candidate(self, destination: Path) -> None:
        paths = set(lock_checker.GOVERNED_FILE_SHA256)
        paths.update(
            {
                ".github/workflows/release.yml",
                ".github/workflows/release-policy.yml",
                "scripts/check-release-locking.py",
                "scripts/check-release-policy.py",
            }
        )
        for relative in sorted(paths):
            source = ROOT / relative
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)

    def test_repository_contract_is_complete(self) -> None:
        invocations = checker.check()
        self.assertEqual(len(invocations), 5)
        self.assertEqual(
            [(item.tool, item.subcommand) for item in invocations],
            [
                ("cargo", "install"),
                ("cross", "build"),
                ("cargo", "build"),
                ("cargo", "install"),
                ("cargo", "metadata"),
            ],
        )

    def test_policy_rejects_trigger_permission_and_job_bypasses(self) -> None:
        mutations = [
            ("  pull_request_target:\n", "  pull_request:\n", "workflow triggers"),
            ("    branches: [main]\n", "    branches: [develop]\n", "workflow triggers"),
            ("  contents: read\n", "  contents: write\n", "workflow permissions"),
            (
                "    timeout-minutes: 10\n",
                "    timeout-minutes: 10\n    if: false\n",
                "enforce job keys",
            ),
            (
                "    timeout-minutes: 10\n",
                "    timeout-minutes: 10\n    continue-on-error: true\n",
                "enforce job keys",
            ),
            (
                "jobs:\n  enforce:\n",
                "jobs:\n  bypass:\n    runs-on: ubuntu-latest\n"
                "    steps: []\n  enforce:\n",
                "policy jobs keys",
            ),
        ]
        for old, new, reason in mutations:
            with self.subTest(mutation=new.strip().splitlines()[-1]):
                self.assert_policy_rejected(old, new, reason)

    def test_policy_rejects_mutable_checkout_and_candidate_execution(self) -> None:
        mutations = [
            (
                "      - name: Checkout trusted base policy\n"
                f"        uses: {checker.CHECKOUT_ACTION}",
                "      - name: Checkout trusted base policy\n"
                "        uses: actions/checkout@v4",
            ),
            (
                "          ref: ${{ github.event.pull_request.base.sha }}\n",
                "          ref: main\n",
            ),
            (
                "          ref: ${{ github.event.pull_request.head.sha }}\n",
                "          ref: ${{ github.event.pull_request.head.ref }}\n",
            ),
            (
                "          path: candidate\n          persist-credentials: false\n",
                "          path: candidate\n          persist-credentials: true\n",
            ),
            (
                checker.POLICY_COMMAND,
                checker.POLICY_COMMAND.replace("trusted/scripts", "candidate/scripts"),
            ),
            (
                "      - name: Provision exact YAML parser\n",
                "      - name: Run candidate checker\n"
                "        run: python candidate/scripts/check-release-policy.py\n\n"
                "      - name: Provision exact YAML parser\n",
            ),
        ]
        for old, new in mutations:
            with self.subTest(mutation=new.splitlines()[0]):
                self.assert_policy_rejected(old, new, "release policy steps")

    def test_policy_rejects_yaml_ambiguity(self) -> None:
        duplicate = self.replace_policy_once(
            "    runs-on: ubuntu-latest\n",
            "    runs-on: ubuntu-latest\n    runs-on: windows-latest\n",
        )
        with self.assertRaisesRegex(checker.ReleasePolicyError, "duplicate YAML key"):
            checker.check_policy_workflow_source(duplicate)

        anchor = self.replace_policy_once("  enforce:\n", "  enforce: &template\n")
        with self.assertRaisesRegex(checker.ReleasePolicyError, "anchors and aliases"):
            checker.check_policy_workflow_source(anchor)

    def test_release_workflow_tamper_is_rejected_by_trusted_checker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory)
            self.copied_candidate(candidate)
            workflow = candidate / ".github/workflows/release.yml"
            source = workflow.read_text(encoding="utf-8")
            self.assertEqual(source.count("      contents: write\n"), 1)
            workflow.write_text(
                source.replace("      contents: write\n", "      contents: read\n", 1),
                encoding="utf-8",
            )
            marker = candidate / "candidate-checker-ran"
            fake = candidate / "scripts/check-release-policy.py"
            fake.write_text(
                "from pathlib import Path\n"
                f"Path({str(marker)!r}).touch()\n"
                "raise SystemExit(0)\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                checker.ReleasePolicyError, "exact reviewed source identity"
            ):
                checker.check(workflow, candidate)
            self.assertFalse(marker.exists(), "candidate checker authorized tamper")

    def test_governed_release_input_tamper_is_rejected(self) -> None:
        for relative in [
            "scripts/build-release-archive.py",
            "distribution/release-payload.txt",
        ]:
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as directory:
                candidate = Path(directory)
                self.copied_candidate(candidate)
                governed = candidate / relative
                governed.write_text(
                    governed.read_text(encoding="utf-8") + "\n# candidate tamper\n",
                    encoding="utf-8",
                )
                with self.assertRaisesRegex(
                    checker.ReleasePolicyError,
                    "governed release file content drifted",
                ):
                    checker.check(candidate / ".github/workflows/release.yml", candidate)

    def test_candidate_checkers_are_never_executed_or_admitted(self) -> None:
        for relative, reason in [
            (
                "scripts/check-release-policy.py",
                "candidate release policy checker differs",
            ),
            (
                "scripts/check-release-locking.py",
                "candidate release lock checker differs",
            ),
        ]:
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as directory:
                candidate = Path(directory)
                self.copied_candidate(candidate)
                marker = candidate / "candidate-checker-ran"
                fake = candidate / relative
                fake.write_text(
                    "from pathlib import Path\n"
                    f"Path({str(marker)!r}).touch()\n"
                    "raise SystemExit(0)\n",
                    encoding="utf-8",
                )
                with self.assertRaisesRegex(checker.ReleasePolicyError, reason):
                    checker.check(candidate / ".github/workflows/release.yml", candidate)
                self.assertFalse(marker.exists(), "candidate checker was executed")

    def test_policy_deletion_and_noncanonical_alias_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory)
            self.copied_candidate(candidate)
            policy = candidate / ".github/workflows/release-policy.yml"
            policy.unlink()
            with self.assertRaisesRegex(checker.ReleasePolicyError, "required release policy"):
                checker.check(candidate / ".github/workflows/release.yml", candidate)

            alias = candidate / ".github/workflows/release-policy-renamed.yml"
            shutil.copy2(POLICY_WORKFLOW, alias)
            with self.assertRaisesRegex(checker.ReleasePolicyError, "canonical policy"):
                checker.check(
                    candidate / ".github/workflows/release.yml",
                    candidate,
                    alias,
                )


if __name__ == "__main__":
    unittest.main()

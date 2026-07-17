#!/usr/bin/env python3
"""Adversarial and real-toolchain tests for the fail-closed MSRV lane."""

from __future__ import annotations

import importlib.util
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/ci.yml"
FIXTURE = ROOT / "contracts/fixtures/msrv/post-1.85-language"


def load_checker():
    path = ROOT / "scripts/check-msrv.py"
    spec = importlib.util.spec_from_file_location("forge_msrv_checker", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load_checker()


class MsrvContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = WORKFLOW.read_text(encoding="utf-8")

    def replace_once(self, old: str, new: str) -> str:
        self.assertEqual(self.source.count(old), 1, old)
        return self.source.replace(old, new, 1)

    def assert_workflow_rejected(self, old: str, new: str) -> None:
        with self.assertRaises(checker.MsrvCheckError):
            checker.check_workflow_source(self.replace_once(old, new))

    def copied_manifests(self, destination: Path) -> None:
        shutil.copy2(ROOT / "Cargo.toml", destination / "Cargo.toml")
        shutil.copytree(ROOT / "crates", destination / "crates")

    def test_repository_contract_is_complete(self) -> None:
        packages = checker.check()
        self.assertEqual(len(packages), 23)
        self.assertEqual(len(packages), len(set(packages)))

    def test_rejects_newer_or_unpinned_toolchains(self) -> None:
        for replacement in ("1.85", "1.86.0", "stable"):
            with self.subTest(toolchain=replacement):
                mutated = self.source.replace("toolchain: 1.85.1", f"toolchain: {replacement}", 1)
                with self.assertRaises(checker.MsrvCheckError):
                    checker.check_workflow_source(mutated)

    def test_rejects_every_omitted_cargo_dimension(self) -> None:
        for flag in ("--locked", "--workspace", "--all-targets", "--all-features"):
            with self.subTest(flag=flag):
                mutated = self.source.replace(f" {flag}", "", 1)
                with self.assertRaises(checker.MsrvCheckError):
                    checker.check_workflow_source(mutated)

    def test_rejects_toolchain_command_bypass(self) -> None:
        self.assert_workflow_rejected("cargo +1.85.1 check", "cargo check")

    def test_rejects_job_trigger_or_dependency_bypass(self) -> None:
        dependency = (
            "  msrv:\n    name: Rust 1.85 minimum supported version\n"
            "    needs: static_docs\n"
        )
        self.assert_workflow_rejected(
            dependency, dependency.replace("static_docs", "focused"),
        )
        runner = dependency + "    runs-on: ubuntu-latest\n    timeout-minutes: 35\n"
        self.assert_workflow_rejected(
            runner,
            runner.replace("    runs-on:", "    if: false\n    runs-on:", 1),
        )
        self.assert_workflow_rejected("  pull_request:\n", "  workflow_dispatch:\n")

    def test_rejects_cache_restore_or_save(self) -> None:
        self.assert_workflow_rejected(
            "      - name: Install exact MSRV toolchain\n",
            "      - name: Cache Rust artifacts\n"
            "        uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32\n"
            "      - name: Install exact MSRV toolchain\n",
        )

    def test_rejects_missing_or_weakened_timing_artifact(self) -> None:
        upload = "      - name: Upload MSRV timing reports\n        if: always()\n"
        self.assert_workflow_rejected(upload, upload.replace("always()", "success()"))
        self.assert_workflow_rejected("          retention-days: 14\n", "          retention-days: 1\n")
        self.assert_workflow_rejected(
            "--budget-seconds 1800 --report target/ci-timing/msrv-workspace.json",
            "--budget-seconds 99999 --report target/ci-timing/msrv-workspace.json",
        )

    def test_rejects_duplicate_or_ambiguous_yaml(self) -> None:
        dependency = (
            "  msrv:\n    name: Rust 1.85 minimum supported version\n"
            "    needs: static_docs\n"
        )
        self.assert_workflow_rejected(
            dependency, dependency + "    needs: static_docs\n",
        )
        self.assert_workflow_rejected(
            dependency, dependency.replace("needs: static_docs", "needs: &dependency static_docs"),
        )

    def test_rejects_workspace_member_omission_and_undeclared_crate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            manifest = root / "Cargo.toml"
            text = manifest.read_text(encoding="utf-8")
            text = text.replace('  "crates/forge-core-research",\n', "", 1)
            manifest.write_text(text, encoding="utf-8")
            with self.assertRaises(checker.MsrvCheckError):
                checker.check_manifests(root)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            ghost = root / "crates/undeclared/src"
            ghost.mkdir(parents=True)
            (ghost.parent / "Cargo.toml").write_text(
                '[package]\nname = "undeclared"\nversion = "0.1.0"\nedition = "2021"\n',
                encoding="utf-8",
            )
            with self.assertRaises(checker.MsrvCheckError):
                checker.check_manifests(root)

    def test_rejects_manifest_parse_and_msrv_override_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            member = root / "crates/forge-core-research/Cargo.toml"
            text = member.read_text(encoding="utf-8")
            member.write_text(text.replace("edition.workspace = true", 'edition.workspace = true\nrust-version = "1.86"', 1), encoding="utf-8")
            with self.assertRaises(checker.MsrvCheckError):
                checker.check_manifests(root)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copied_manifests(root)
            member = root / "crates/forge-core-research/Cargo.toml"
            member.write_text("not valid [toml", encoding="utf-8")
            with self.assertRaises(checker.MsrvCheckError):
                checker.check_manifests(root)

    def test_real_post_185_fixture_fails_for_intended_language_gate(self) -> None:
        version = subprocess.run(
            ["rustc", "+1.85.1", "--version"],
            text=True,
            capture_output=True,
            timeout=60,
            check=False,
        )
        self.assertEqual(version.returncode, 0, f"missing exact toolchain: {version.stderr}")
        self.assertRegex(version.stdout, r"^rustc 1\.85\.1 ")
        with tempfile.TemporaryDirectory() as target:
            result = subprocess.run(
                [
                    "cargo", "+1.85.1", "check", "--manifest-path",
                    str(FIXTURE / "Cargo.toml"), "--locked", "--target-dir", target,
                ],
                text=True,
                capture_output=True,
                timeout=120,
                check=False,
            )
        output = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, output)
        self.assertIn("E0658", output)
        self.assertRegex(
            output, re.compile(r"`let` expressions? in this position (?:are|is) unstable")
        )
        self.assertNotIn("toolchain", output.casefold().split("error[e0658]", 1)[0])

        current = subprocess.run(
            ["rustc", "--version"], text=True, capture_output=True, timeout=30, check=True
        ).stdout
        match = re.match(r"rustc (\d+)\.(\d+)\.(\d+)", current)
        self.assertIsNotNone(match, current)
        assert match is not None
        if tuple(map(int, match.groups())) > (1, 85, 1):
            with tempfile.TemporaryDirectory() as target:
                accepted = subprocess.run(
                    [
                        "cargo", "check", "--manifest-path", str(FIXTURE / "Cargo.toml"),
                        "--locked", "--target-dir", target,
                    ],
                    text=True,
                    capture_output=True,
                    timeout=120,
                    check=False,
                )
            self.assertEqual(accepted.returncode, 0, accepted.stdout + accepted.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)

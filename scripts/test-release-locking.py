#!/usr/bin/env python3
"""Static and real-Cargo regressions for release lock enforcement."""

from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "contracts/fixtures/release-lock/manifest-drift"


def load_checker():
    path = ROOT / "scripts/check-release-locking.py"
    spec = importlib.util.spec_from_file_location("forge_release_lock_checker", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load_checker()


class ReleaseLockingTests(unittest.TestCase):
    def test_real_release_topology_is_exact_and_locked(self) -> None:
        workflow = ROOT / ".github/workflows/release.yml"
        invocations = checker.check(workflow)
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

    def test_removing_any_release_locked_flag_is_rejected(self) -> None:
        source = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        offsets = []
        start = 0
        while True:
            offset = source.find("--locked", start)
            if offset < 0:
                break
            offsets.append(offset)
            start = offset + 1
        self.assertEqual(len(offsets), 4)

        for number, offset in enumerate(offsets, 1):
            with self.subTest(invocation=number), tempfile.TemporaryDirectory() as directory:
                mutated = source[:offset] + source[offset + len("--locked") :]
                workflow = Path(directory) / "release.yml"
                workflow.write_text(mutated, encoding="utf-8")
                with self.assertRaisesRegex(
                    checker.ReleaseLockError, "must include a literal --locked"
                ):
                    checker.check(workflow)

    def test_sbom_metadata_shim_lock_removal_is_rejected(self) -> None:
        workflow = ROOT / ".github/workflows/release.yml"
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        source = runner.read_text(encoding="utf-8")
        mutated = source.replace(
            'return [real_cargo, "metadata", "--locked", *arguments[1:]]',
            'return [real_cargo, "metadata", *arguments[1:]]',
            1,
        )
        self.assertNotEqual(mutated, source)
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / runner.name
            candidate.write_text(mutated, encoding="utf-8")
            with self.assertRaisesRegex(
                checker.ReleaseLockError, "metadata shim contract drifted"
            ):
                checker.check(workflow, candidate)


    def test_multiline_and_chained_commands_are_independently_checked(self) -> None:
        source = """steps:
  - run: |
      cargo build \\
        --locked --release
      cross build --locked && cargo test --workspace
"""
        invocations = checker.find_invocations(source)
        self.assertEqual(len(invocations), 3)
        self.assertIn("--locked", invocations[0].command)
        self.assertIn("--locked", invocations[1].command)
        self.assertNotIn("--locked", invocations[2].command)

        with tempfile.TemporaryDirectory() as directory:
            workflow = Path(directory) / "release.yml"
            workflow.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(checker.ReleaseLockError, "cargo test"):
                checker.check(workflow)

    def test_sbom_metadata_shim_rejects_manifest_lock_drift(self) -> None:
        cargo = shutil.which("cargo")
        self.assertIsNotNone(cargo, "Cargo is required to prove release lock enforcement")
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "fixture"
            shutil.copytree(FIXTURE, root)
            original_lock = (root / "Cargo.lock").read_bytes()
            environment = os.environ.copy()
            environment["FORGE_RELEASE_LOCKED_CARGO_SHIM"] = "1"
            environment["FORGE_RELEASE_REAL_CARGO"] = str(cargo)
            completed = subprocess.run(
                [
                    sys.executable,
                    str(runner),
                    "metadata",
                    "--format-version",
                    "1",
                    "--manifest-path",
                    str(root / "Cargo.toml"),
                ],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertIn("--locked", completed.stderr)
            self.assertEqual((root / "Cargo.lock").read_bytes(), original_lock)
    def test_manifest_lock_drift_fails_before_packaging(self) -> None:
        cargo = shutil.which("cargo")
        self.assertIsNotNone(cargo, "Cargo is required to prove release lock enforcement")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "fixture"
            shutil.copytree(FIXTURE, root)
            original_lock = (root / "Cargo.lock").read_bytes()
            environment = os.environ.copy()
            environment["CARGO_HOME"] = str(Path(directory) / "cargo-home")
            environment["CARGO_TARGET_DIR"] = str(Path(directory) / "target")
            completed = subprocess.run(
                [
                    str(cargo),
                    "package",
                    "--locked",
                    "--allow-dirty",
                    "--no-verify",
                    "--manifest-path",
                    str(root / "Cargo.toml"),
                ],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertIn("lock file", completed.stderr)
            self.assertIn("--locked", completed.stderr)
            self.assertEqual((root / "Cargo.lock").read_bytes(), original_lock)
            self.assertEqual(list(Path(directory).rglob("*.crate")), [])
            self.assertFalse((Path(directory) / "target/package").exists())

            generated = subprocess.run(
                [
                    str(cargo),
                    "generate-lockfile",
                    "--offline",
                    "--manifest-path",
                    str(root / "Cargo.toml"),
                ],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=30,
            )
            self.assertEqual(generated.returncode, 0, generated.stderr)
            repaired_lock = (root / "Cargo.lock").read_text(encoding="utf-8")
            self.assertNotEqual(repaired_lock.encode(), original_lock)
            self.assertIn('version = "0.2.0"', repaired_lock)


if __name__ == "__main__":
    unittest.main()

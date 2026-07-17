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
WORKFLOW = ROOT / ".github/workflows/release.yml"
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
    def assert_mutation_rejected(self, mutated: str) -> None:
        original = WORKFLOW.read_text(encoding="utf-8")
        self.assertNotEqual(mutated, original)
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / "release.yml"
            candidate.write_text(mutated, encoding="utf-8")
            with self.assertRaises(checker.ReleaseLockError):
                checker.check(candidate, repo_root=ROOT)

    def replace_once(self, old: str, new: str) -> str:
        source = WORKFLOW.read_text(encoding="utf-8")
        self.assertEqual(source.count(old), 1, old)
        return source.replace(old, new, 1)

    def test_real_release_topology_is_exact_and_locked(self) -> None:
        invocations = checker.check(WORKFLOW)
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
        source = WORKFLOW.read_text(encoding="utf-8")
        offsets: list[int] = []
        start = 0
        while (offset := source.find("--locked", start)) >= 0:
            offsets.append(offset)
            start = offset + 1
        self.assertEqual(len(offsets), 4)
        for number, offset in enumerate(offsets, 1):
            with self.subTest(invocation=number):
                self.assert_mutation_rejected(
                    source[:offset] + source[offset + len("--locked") :]
                )

    def test_reviewer_workflow_bypasses_are_rejected(self) -> None:
        native = "run: cargo build --locked --release --target ${{ matrix.target }} -p forge-core-cli"
        mutations = {
            "path-qualified": self.replace_once(native, native.replace("cargo", "/usr/bin/cargo", 1)),
            "toolchain": self.replace_once(native, native.replace("cargo build", "cargo +1.97.0 build", 1)),
            "global-config": self.replace_once(native, native.replace("cargo build", "cargo --config net.retry=2 build", 1)),
            "quoted-cargo-env": self.replace_once(native, native.replace("cargo", '"$CARGO"', 1)),
            "shell-variable": self.replace_once(native, "run: |\n          tool=cargo\n          \"$tool\" build --locked --release"),
            "alias": self.replace_once(native, "run: |\n          alias builder='cargo'\n          builder build --locked --release"),
            "function": self.replace_once(native, "run: |\n          builder() { cargo \"$@\"; }\n          builder build --locked --release"),
            "payload-locked": self.replace_once(native, "run: cargo run --release -- --locked"),
            "called-python": self.replace_once(native, "run: python scripts/package.py"),
            "called-shell": self.replace_once(native, "run: bash scripts/package.sh"),
        }
        for name, mutated in mutations.items():
            with self.subTest(name=name):
                self.assert_mutation_rejected(mutated)

    def test_echoed_wrapper_plus_direct_plugin_is_rejected(self) -> None:
        command = """          python scripts/run-release-locked-sbom.py \\
            --lockfile Cargo.lock \\
            -- \\
            --format json \\
            --manifest-path crates/forge-core-cli/Cargo.toml \\
            --override-filename \"forge-core-$VERSION.cdx\""""
        bypass = """          echo \"python scripts/run-release-locked-sbom.py\"
          cargo-cyclonedx cyclonedx --format json --manifest-path crates/forge-core-cli/Cargo.toml"""
        self.assert_mutation_rejected(self.replace_once(command, bypass))

    def test_yaml_alias_run_is_rejected_as_yaml_not_as_text(self) -> None:
        source = self.replace_once(
            "run: cargo build --locked --release --target ${{ matrix.target }} -p forge-core-cli",
            "run: *native_build",
        )
        source = source.replace("jobs:\n", "jobs:\n  native_template: &native_build cargo build --locked\n", 1)
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / "release.yml"
            candidate.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(checker.ReleaseLockError, "unsupported YAML|anchors, aliases"):
                checker.check(candidate, repo_root=ROOT)

    def test_inline_literal_and_folded_run_scalars_are_semantic(self) -> None:
        source = """jobs:
  audit:
    steps:
      - name: Inline
        run: 'cargo build --locked'
      - name: Literal
        run: |
          cargo build \\
            --locked --release
      - name: Folded
        run: >
          cross build
          --locked --release
"""
        steps = checker.parse_workflow(source)
        self.assertEqual([step.name for step in steps], ["Inline", "Literal", "Folded"])
        parsed = [checker.find_invocations(step.run, step.line)[0] for step in steps]
        self.assertEqual(
            [(item.tool, item.subcommand) for item in parsed],
            [("cargo", "build"), ("cargo", "build"), ("cross", "build")],
        )

    def test_multiline_and_chained_commands_are_independently_checked(self) -> None:
        body = """cargo build \\
  --locked --release
cross build --locked && cargo test --workspace
"""
        with self.assertRaisesRegex(checker.ReleaseLockError, "cargo test"):
            checker.find_invocations(body)
        positive = """cargo build \\
  --locked --release
cross build \\
  --locked --release
"""
        invocations = checker.find_invocations(positive)
        self.assertEqual(len(invocations), 2)

    def test_cargo_parser_rejects_all_executable_spellings_and_payload_lock(self) -> None:
        negatives = {
            "path": "/usr/bin/cargo package",
            "toolchain": "cargo +1.97.0 package",
            "global-option": "cargo --config net.retry=2 package",
            "quoted-env": '"$CARGO" package --locked',
            "assignment": "tool=cargo\n\"$tool\" package --locked",
            "alias": "alias c=cargo; c package --locked",
            "function": "function c { cargo package --locked; }; c",
            "plugin": "cargo-cyclonedx cyclonedx --locked",
            "cargo-plugin": "cargo cyclonedx --locked",
            "payload": "cargo run --release -- --locked",
        }
        for name, body in negatives.items():
            with self.subTest(name=name), self.assertRaises(checker.ReleaseLockError):
                checker.find_invocations(body)
        accepted = checker.find_invocations(
            "cargo +1.97.0 --config net.retry=2 package --locked --allow-dirty"
        )
        self.assertEqual([(item.tool, item.subcommand) for item in accepted], [("cargo", "package")])

    def test_sbom_runner_content_drift_is_rejected(self) -> None:
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        source = runner.read_text(encoding="utf-8")
        mutated = source.replace(
            'return [real_cargo, "metadata", "--locked", *arguments[1:]]',
            'return [real_cargo, "metadata", *arguments[1:]]',
            1,
        )
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / runner.name
            candidate.write_text(mutated, encoding="utf-8")
            with self.assertRaisesRegex(checker.ReleaseLockError, "outside the governed"):
                checker.check(WORKFLOW, candidate, ROOT)

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
                [sys.executable, str(runner), "metadata", "--format-version", "1", "--manifest-path", str(root / "Cargo.toml")],
                env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertIn("--locked", completed.stderr)
            self.assertEqual((root / "Cargo.lock").read_bytes(), original_lock)

    def test_sbom_shim_preserves_cargo_failure_status(self) -> None:
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            capture = root / "argv"
            fake = root / "cargo"
            fake.write_text(
                f"#!{sys.executable}\nimport pathlib,sys\npathlib.Path({str(capture)!r}).write_text('\\n'.join(sys.argv[1:]))\nraise SystemExit(47)\n",
                encoding="utf-8",
            )
            fake.chmod(0o755)
            environment = os.environ.copy()
            environment["FORGE_RELEASE_LOCKED_CARGO_SHIM"] = "1"
            environment["FORGE_RELEASE_REAL_CARGO"] = str(fake)
            completed = subprocess.run(
                [sys.executable, str(runner), "metadata", "--format-version", "1"],
                env=environment, text=True, capture_output=True, check=False, timeout=10,
            )
            self.assertEqual(completed.returncode, 47)
            self.assertEqual(capture.read_text(encoding="utf-8").splitlines()[:2], ["metadata", "--locked"])

    def test_real_cargo_cyclonedx_stale_lock_has_no_output(self) -> None:
        if shutil.which("cargo-cyclonedx") is None:
            self.skipTest("real cargo-cyclonedx 0.5.9 probe requires installed plugin")
        runner = ROOT / "scripts/run-release-locked-sbom.py"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "fixture"
            shutil.copytree(FIXTURE, root)
            original_lock = (root / "Cargo.lock").read_bytes()
            completed = subprocess.run(
                [sys.executable, str(runner), "--lockfile", str(root / "Cargo.lock"), "--", "--format", "json", "--manifest-path", str(root / "Cargo.toml"), "--override-filename", "stale-proof.cdx"],
                cwd=root, text=True, capture_output=True, check=False, timeout=60,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertIn("--locked", completed.stderr)
            self.assertEqual((root / "Cargo.lock").read_bytes(), original_lock)
            self.assertEqual(list(root.rglob("*.cdx.json")), [])

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
                [str(cargo), "package", "--locked", "--allow-dirty", "--no-verify", "--manifest-path", str(root / "Cargo.toml")],
                env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertNotEqual(completed.returncode, 0, completed.stdout)
            self.assertIn("lock file", completed.stderr)
            self.assertIn("--locked", completed.stderr)
            self.assertEqual((root / "Cargo.lock").read_bytes(), original_lock)
            self.assertEqual(list(Path(directory).rglob("*.crate")), [])
            self.assertFalse((Path(directory) / "target/package").exists())

            generated = subprocess.run(
                [str(cargo), "generate-lockfile", "--offline", "--manifest-path", str(root / "Cargo.toml")],
                env=environment, text=True, capture_output=True, check=False, timeout=30,
            )
            self.assertEqual(generated.returncode, 0, generated.stderr)
            repaired_lock = (root / "Cargo.lock").read_text(encoding="utf-8")
            self.assertNotEqual(repaired_lock.encode(), original_lock)
            self.assertIn('version = "0.2.0"', repaired_lock)


if __name__ == "__main__":
    unittest.main()

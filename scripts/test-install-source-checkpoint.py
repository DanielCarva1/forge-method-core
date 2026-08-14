#!/usr/bin/env python3
"""Public-command tests for the source-checkpoint installer."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("install-source-checkpoint.py")
VERSION = "0.12.0-alpha.17"


def run(
    command: list[str], *, cwd: Path, env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command, cwd=cwd, text=True, capture_output=True, check=False, env=env
    )


class SourceCheckpointInstallerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="forge-source-install-test-")
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        self.install_root = self.root / "install"
        self.target_dir = self.root / "target"
        crate = self.repo / "crates" / "forge-core-cli"
        (crate / "src").mkdir(parents=True)
        (self.repo / "Cargo.toml").write_text(
            "[workspace]\n"
            'members = ["crates/forge-core-cli"]\n'
            'resolver = "2"\n\n'
            "[workspace.package]\n"
            f'version = "{VERSION}"\n'
            'edition = "2021"\n',
            encoding="utf-8",
        )
        (crate / "Cargo.toml").write_text(
            "[package]\n"
            'name = "forge-core-cli"\n'
            "version.workspace = true\n"
            "edition.workspace = true\n\n"
            "[[bin]]\n"
            'name = "forge-core"\n'
            'path = "src/main.rs"\n',
            encoding="utf-8",
        )
        self.write_program("one")
        (self.repo / "product.txt").write_text("checkpoint one\n", encoding="utf-8")
        generated = run(["cargo", "generate-lockfile"], cwd=self.repo)
        self.assertEqual(generated.returncode, 0, generated.stderr)
        self.git("init")
        self.git("config", "user.name", "Forge Test")
        self.git("config", "user.email", "forge-test@example.invalid")
        self.git("add", ".")
        self.git("commit", "-m", "checkpoint one")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *args: str) -> str:
        result = run(["git", *args], cwd=self.repo)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def write_program(self, marker: str, *, reported_version: str | None = None) -> None:
        version_expression = (
            f'"{reported_version}"' if reported_version else 'env!("CARGO_PKG_VERSION")'
        )
        (self.repo / "crates" / "forge-core-cli" / "src" / "main.rs").write_text(
            "fn main() {\n"
            "    if std::env::args().any(|arg| arg == \"--version\") {\n"
            f'        println!("forge-core {{}}", {version_expression});\n'
            "    } else {\n"
            f'        println!("{marker}");\n'
            "    }\n"
            "}\n",
            encoding="utf-8",
        )

    def install(
        self, *extra: str, env: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        return run(
            [
                sys.executable,
                str(SCRIPT),
                "install",
                "--repo-root",
                str(self.repo),
                "--install-root",
                str(self.install_root),
                "--target-dir",
                str(self.target_dir),
                *extra,
            ],
            cwd=self.repo,
            env=env,
        )

    def commit_program(self, marker: str, number: int) -> str:
        self.write_program(marker)
        self.git("add", ".")
        self.git("commit", "-m", f"checkpoint {number}")
        return self.git("rev-parse", "HEAD")

    def commit_product_only(self, number: int) -> str:
        (self.repo / "product.txt").write_text(
            f"checkpoint {number}\n", encoding="utf-8"
        )
        self.git("add", "product.txt")
        self.git("commit", "-m", f"checkpoint {number}")
        return self.git("rev-parse", "HEAD")

    def binary(self, *, rollback: bool = False) -> Path:
        name = "forge-core.exe" if os.name == "nt" else "forge-core"
        base = (
            self.install_root / "source-install" / "rollback"
            if rollback
            else self.install_root / "bin"
        )
        return base / name

    def marker(self, path: Path) -> str:
        result = run([str(path)], cwd=self.repo)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def receipts(self) -> list[Path]:
        return sorted((self.install_root / "source-install" / "receipts").glob("*.json"))

    def test_three_same_version_checkpoints_leave_current_and_one_rollback(self) -> None:
        commits = [self.git("rev-parse", "HEAD")]
        self.assertEqual(self.install().returncode, 0)
        commits.append(self.commit_program("two", 2))
        self.assertEqual(self.install().returncode, 0)
        commits.append(self.commit_program("three", 3))
        third = self.install()
        self.assertEqual(third.returncode, 0, third.stderr)

        report = json.loads(third.stdout)
        self.assertEqual(report["source_commit"], commits[2])
        self.assertEqual(self.marker(self.binary()), "three")
        self.assertEqual(self.marker(self.binary(rollback=True)), "two")
        self.assertEqual(len(self.receipts()), 2)
        self.assertEqual(
            {
                json.loads(path.read_text(encoding="ascii"))["source_commit"]
                for path in self.receipts()
            },
            {commits[1], commits[2]},
        )
        self.assertEqual(
            list((self.install_root / "source-install" / "staging").iterdir()), []
        )

    def test_exact_checkpoint_retry_is_idempotent(self) -> None:
        self.assertEqual(self.install().returncode, 0)
        retry = self.install()
        self.assertEqual(retry.returncode, 0, retry.stderr)
        self.assertEqual(json.loads(retry.stdout)["status"], "already_installed")
        self.assertEqual(len(self.receipts()), 1)

    def test_different_commits_with_identical_binary_are_distinguished(self) -> None:
        first_commit = self.git("rev-parse", "HEAD")
        first = self.install()
        self.assertEqual(first.returncode, 0, first.stderr)
        first_hash = json.loads(first.stdout)["binary_sha256"]

        second_commit = self.commit_product_only(2)
        second = self.install()
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(json.loads(second.stdout)["binary_sha256"], first_hash)
        self.assertEqual(len(self.receipts()), 2)
        state = json.loads(
            (self.install_root / "source-install" / "state.json").read_text(
                encoding="ascii"
            )
        )
        self.assertTrue(state["active_receipt_id"].startswith(second_commit))
        self.assertTrue(state["rollback_receipt_id"].startswith(first_commit))

    def test_dirty_checkout_is_rejected_before_installation(self) -> None:
        (self.repo / "product.txt").write_text("uncommitted\n", encoding="utf-8")
        result = self.install()
        self.assertEqual(result.returncode, 2)
        self.assertIn("source checkout is dirty", result.stderr)
        self.assertFalse((self.install_root / "bin").exists())

    def test_built_candidate_version_mismatch_preserves_current_binary(self) -> None:
        self.assertEqual(self.install().returncode, 0)
        before = self.binary().read_bytes()
        self.write_program("wrong", reported_version="9.9.9")
        self.git("add", ".")
        self.git("commit", "-m", "wrong version")
        rejected = self.install()
        self.assertEqual(rejected.returncode, 2)
        self.assertIn("candidate version mismatch", rejected.stderr)
        self.assertEqual(self.binary().read_bytes(), before)
        self.assertEqual(len(self.receipts()), 1)

    def test_active_replace_failure_preserves_active_rollback_and_receipts(self) -> None:
        self.assertEqual(self.install().returncode, 0)
        self.commit_program("two", 2)
        self.assertEqual(self.install().returncode, 0)
        self.assertEqual(self.marker(self.binary()), "two")
        self.assertEqual(self.marker(self.binary(rollback=True)), "one")

        self.commit_program("three", 3)
        env = dict(os.environ)
        env["FORGE_SOURCE_INSTALL_TEST_FAIL_ACTIVE_REPLACE"] = "1"
        rejected = self.install(env=env)
        self.assertEqual(rejected.returncode, 2)
        self.assertEqual(self.marker(self.binary()), "two")
        self.assertEqual(self.marker(self.binary(rollback=True)), "one")
        self.assertEqual(len(self.receipts()), 2)

    def test_post_replace_failure_reconciles_active_rollback_and_state(self) -> None:
        self.assertEqual(self.install().returncode, 0)
        second_commit = self.commit_program("two", 2)
        self.assertEqual(self.install().returncode, 0)
        third_commit = self.commit_program("three", 3)

        env = dict(os.environ)
        env["FORGE_SOURCE_INSTALL_TEST_FAIL_ROLLBACK_REPLACE"] = "1"
        rejected = self.install(env=env)
        self.assertEqual(rejected.returncode, 2)
        self.assertEqual(self.marker(self.binary()), "three")
        self.assertEqual(self.marker(self.binary(rollback=True)), "two")
        self.assertEqual(len(self.receipts()), 2)
        state = json.loads(
            (self.install_root / "source-install" / "state.json").read_text(
                encoding="ascii"
            )
        )
        self.assertTrue(state["active_receipt_id"].startswith(third_commit))
        self.assertTrue(state["rollback_receipt_id"].startswith(second_commit))
        self.assertFalse((self.install_root / "source-install" / "pending.json").exists())

        retry = self.install()
        self.assertEqual(retry.returncode, 0, retry.stderr)
        self.assertEqual(json.loads(retry.stdout)["status"], "already_installed")

    def test_crash_left_atomic_temps_are_cleaned_without_growth(self) -> None:
        source_root = self.install_root / "source-install"
        receipts = source_root / "receipts"
        receipts.mkdir(parents=True)
        state_temp = source_root / ".state.json.123.tmp"
        receipt_temp = receipts / (
            "." + "a" * 40 + "-" + "b" * 64 + ".json.456.tmp"
        )
        state_temp.write_text("partial", encoding="ascii")
        receipt_temp.write_text("partial", encoding="ascii")

        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(state_temp.exists())
        self.assertFalse(receipt_temp.exists())
        self.assertEqual(len(self.receipts()), 1)

    def test_missing_active_with_retained_rollback_fails_closed(self) -> None:
        self.assertEqual(self.install().returncode, 0)
        self.commit_program("two", 2)
        self.assertEqual(self.install().returncode, 0)
        rollback_before = self.binary(rollback=True).read_bytes()
        state_before = (self.install_root / "source-install" / "state.json").read_bytes()
        self.binary().unlink()

        rejected = self.install()
        self.assertEqual(rejected.returncode, 2)
        self.assertIn("installed binary is missing", rejected.stderr)
        self.assertFalse(self.binary().exists())
        self.assertEqual(self.binary(rollback=True).read_bytes(), rollback_before)
        self.assertEqual(
            (self.install_root / "source-install" / "state.json").read_bytes(),
            state_before,
        )

    def test_matching_unmanaged_binary_can_be_adopted_without_duplicate_rollback(self) -> None:
        built = run(
            [
                "cargo",
                "build",
                "--locked",
                "--release",
                "--package",
                "forge-core-cli",
                "--bin",
                "forge-core",
                "--target-dir",
                str(self.target_dir),
            ],
            cwd=self.repo,
        )
        self.assertEqual(built.returncode, 0, built.stderr)
        self.binary().parent.mkdir(parents=True)
        shutil.copy2(
            self.target_dir
            / "release"
            / ("forge-core.exe" if os.name == "nt" else "forge-core"),
            self.binary(),
        )

        result = self.install("--adopt-current")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["status"], "adopted_current")
        self.assertEqual(len(self.receipts()), 1)
        self.assertFalse(self.binary(rollback=True).exists())


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Focused standard-library tests for deterministic release archive assembly."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parent


def load_module(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / filename)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {filename}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


builder = load_module("forge_release_builder", "build-release-archive.py")
checker = load_module("forge_release_checker", "check-release-archive.py")


class Args:
    pass


class ReleaseArchiveTests(unittest.TestCase):
    def fixture(self, root: Path, suffix: str) -> Args:
        (root / "distribution").mkdir()
        (root / "skill/start-forge").mkdir(parents=True)
        (root / "docs").mkdir()
        (root / "bin").mkdir()
        (root / "distribution/release-payload.txt").write_text(
            "skill/start-forge/SKILL.md\ndocs/getting-started.md\n", encoding="utf-8"
        )
        (root / "skill/start-forge/SKILL.md").write_text("skill\n", encoding="utf-8")
        (root / "docs/getting-started.md").write_text(
            "[start skill](../skill/start-forge/SKILL.md)\n", encoding="utf-8"
        )
        binary_name = "forge-core.exe" if suffix == ".zip" else "forge-core"
        wrapper_name = "forge.cmd" if suffix == ".zip" else "forge"
        binary = root / "bin" / binary_name
        wrapper = root / "distribution" / wrapper_name
        binary.write_bytes(b"binary-v0.9.0\n")
        wrapper.write_bytes(b"wrapper\n")
        args = Args()
        args.repo_root = root
        args.payload_manifest = Path("distribution/release-payload.txt")
        args.binary = binary
        args.binary_name = binary_name
        args.wrapper = wrapper
        args.wrapper_name = wrapper_name
        args.archive = root / f"forge-core-test{suffix}"
        args.version = "0.9.0"
        args.source_date_epoch = 1_700_000_000
        return args

    def verify(self, args: Args) -> None:
        check_args = Args()
        check_args.archive = args.archive
        check_args.binary_name = args.binary_name
        check_args.wrapper_name = args.wrapper_name
        check_args.version = args.version
        check_args.payload_manifest = args.repo_root / args.payload_manifest
        check_args.require_checksum = True
        checker.check(check_args)

    def test_tar_and_zip_are_reproducible_and_exact(self) -> None:
        for suffix in [".tar.gz", ".zip"]:
            with self.subTest(suffix=suffix), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                args = self.fixture(root, suffix)
                builder.build(args)
                first = hashlib.sha256(args.archive.read_bytes()).digest()
                self.verify(args)
                builder.build(args)
                second = hashlib.sha256(args.archive.read_bytes()).digest()
                self.verify(args)
                self.assertEqual(first, second)

    def test_checker_rejects_wrong_version(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            args = self.fixture(Path(directory), ".zip")
            builder.build(args)
            args.version = "0.9.1"
            with self.assertRaises(checker.ArchiveCheckError):
                self.verify(args)

    def test_payload_rejects_traversal_and_duplicates(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            args = self.fixture(root, ".tar.gz")
            manifest = root / args.payload_manifest
            manifest.write_text("../escape\n", encoding="utf-8")
            with self.assertRaises(builder.ReleaseArchiveError):
                builder.build(args)
            manifest.write_text(
                "docs/getting-started.md\ndocs/getting-started.md\n", encoding="utf-8"
            )
            with self.assertRaises(builder.ReleaseArchiveError):
                builder.build(args)

    def test_builder_and_checker_reject_noncanonical_paths(self) -> None:
        invalid = ["C:/escape", "./docs/file", "docs//file", "docs\\file"]
        for value in invalid:
            with self.subTest(value=value):
                with self.assertRaises(builder.ReleaseArchiveError):
                    builder.normalized_archive_path(value)
                with self.assertRaises(checker.ArchiveCheckError):
                    checker.normalized(value)

    def test_checker_rejects_broken_local_markdown_link(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            args = self.fixture(root, ".tar.gz")
            (root / "docs/getting-started.md").write_text(
                "[missing](not-in-the-archive.md)\n", encoding="utf-8"
            )
            builder.build(args)
            with self.assertRaisesRegex(
                checker.ArchiveCheckError, "broken local Markdown link"
            ):
                self.verify(args)


if __name__ == "__main__":
    unittest.main()

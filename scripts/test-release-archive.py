#!/usr/bin/env python3
"""Focused standard-library tests for deterministic release archive assembly."""

from __future__ import annotations

import errno
import hashlib
import importlib.util
import os
import shutil
import subprocess
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
        args.release_tag = "v0.9.0"
        args.source_commit = "0123456789abcdef0123456789abcdef01234567"
        args.source_date_epoch = 1_700_000_000
        return args

    def checker_args(self, args: Args) -> Args:
        check_args = Args()
        check_args.archive = args.archive
        check_args.binary_name = args.binary_name
        check_args.wrapper_name = args.wrapper_name
        check_args.version = args.version
        check_args.expected_release_tag = args.release_tag
        check_args.expected_source_commit = args.source_commit
        check_args.payload_manifest = args.repo_root / args.payload_manifest
        check_args.require_checksum = True
        return check_args

    def verify(self, args: Args) -> None:
        checker.check(self.checker_args(args))

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

    def test_checker_rejects_expected_release_tag_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            args = self.fixture(Path(directory), ".tar.gz")
            builder.build(args)
            check_args = self.checker_args(args)
            check_args.expected_release_tag = "v0.9.1"
            with self.assertRaisesRegex(checker.ArchiveCheckError, "!= expected"):
                checker.check(check_args)

    def test_checker_rejects_expected_source_commit_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            args = self.fixture(Path(directory), ".zip")
            builder.build(args)
            check_args = self.checker_args(args)
            check_args.expected_source_commit = "f" * 40
            with self.assertRaisesRegex(checker.ArchiveCheckError, "!= expected"):
                checker.check(check_args)

    def test_builder_rejects_release_identity_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            args = self.fixture(Path(directory), ".tar.gz")
            args.release_tag = "v0.9.1"
            with self.assertRaisesRegex(builder.ReleaseArchiveError, "release-tag"):
                builder.build(args)
            args.release_tag = "v0.9.0"
            args.source_commit = "not-a-full-commit"
            with self.assertRaisesRegex(builder.ReleaseArchiveError, "source-commit"):
                builder.build(args)

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


    def wrapper_fixture(self, root: Path) -> tuple[Path, Path]:
        package = root / "relocated package\nwith spaces\n"
        package.mkdir()
        wrapper = package / "forge"
        shutil.copyfile(SCRIPTS.parent / "distribution/forge", wrapper)
        wrapper.chmod(0o755)
        binary = package / "forge-core"
        binary.write_text(
            "#!/bin/sh\n"
            "printf '%s\\n' real-core\n"
            "printf '%s\\n' \"$@\"\n",
            encoding="utf-8",
        )
        binary.chmod(0o755)
        return package, wrapper

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_bare_path_lookup_handles_newlines_and_selects_executable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, wrapper = self.wrapper_fixture(root)
            non_executable = root / "not executable\nfirst"
            non_executable.mkdir()
            shadow = non_executable / "forge"
            shutil.copyfile(wrapper, shadow)
            shadow.chmod(0o644)
            path = os.pathsep.join([str(non_executable), str(package), "/usr/bin", "/bin"])
            argument = "argument with spaces\nand newline"
            environment = os.environ.copy()
            bare_command = subprocess.run(
                [
                    "/bin/sh",
                    "-c",
                    'PATH="$1"; exec forge "$2"',
                    "bare-command",
                    path,
                    argument,
                ],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            sourced_bare = subprocess.run(
                [
                    "/bin/sh",
                    "-c",
                    'PATH="$1"; wrapper=$2; argument=$3; set -- "$argument"; . "$wrapper"',
                    "forge",
                    path,
                    str(wrapper),
                    argument,
                ],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            expected = f"real-core\n{argument}\n"
            for completed in (bare_command, sourced_bare):
                self.assertEqual(completed.returncode, 0, completed.stderr)
                self.assertEqual(completed.stdout, expected)
                self.assertEqual(completed.stderr, "")

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_ignores_path_controlled_readlink(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, wrapper = self.wrapper_fixture(root)
            aliases = root / "aliases\nwith spaces"
            aliases.mkdir()
            alias = aliases / "forge"
            alias.symlink_to(os.path.relpath(wrapper, aliases))
            fake_bin = root / "fake-bin"
            fake_bin.mkdir()
            marker = root / "fake-readlink-called"
            fake_readlink = fake_bin / "readlink"
            fake_readlink.write_text(
                "#!/bin/sh\n"
                ": > \"$FAKE_READLINK_MARKER\"\n"
                "printf '%s\\n' /tmp/redirected\n",
                encoding="utf-8",
            )
            fake_readlink.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = os.pathsep.join(
                [str(fake_bin), "/usr/bin", "/bin"]
            )
            environment["FAKE_READLINK_MARKER"] = str(marker)
            completed = subprocess.run(
                [str(alias), "not redirected"],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(completed.stdout, "real-core\nnot redirected\n")
            self.assertEqual(completed.stderr, "")
            self.assertFalse(marker.exists(), completed.stdout)

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_resolver_fails_closed_at_symlink_bound(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, wrapper = self.wrapper_fixture(root)
            loop_dir = root / "resolver loop"
            loop_dir.mkdir()
            links = [loop_dir / f"link-{index}" for index in range(41)]
            for index, link in enumerate(links):
                link.symlink_to(links[(index + 1) % len(links)].name)
            completed = subprocess.run(
                [
                    "/bin/sh",
                    "-c",
                    '. "$1"',
                    str(links[0]),
                    str(wrapper),
                ],
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(completed.returncode, 127)
            self.assertIn("symlink loop", completed.stderr)
            self.assertEqual(completed.stdout, "")

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_bare_lookup_fails_closed_when_path_is_unset(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            _, wrapper = self.wrapper_fixture(Path(directory))
            environment = os.environ.copy()
            environment.pop("PATH", None)
            completed = subprocess.run(
                [
                    "/bin/sh",
                    "-c",
                    'unset PATH; . "$1"',
                    "forge",
                    str(wrapper),
                ],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(completed.returncode, 127)
            self.assertIn("unset or empty PATH", completed.stderr)
            self.assertNotIn("parameter not set", completed.stderr)

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_resolves_relocated_relative_newline_path_and_path_shadow(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, wrapper = self.wrapper_fixture(root)
            shadow = root / "path shadow"
            shadow.mkdir()
            shadow_binary = shadow / "forge-core"
            shadow_binary.write_text(
                "#!/bin/sh\n" "printf '%s\\n' shadow-core\n", encoding="utf-8"
            )
            shadow_binary.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = os.pathsep.join(
                [str(shadow), environment.get("PATH", "")]
            )
            relative_wrapper = os.path.relpath(wrapper, package.parent)
            completed = subprocess.run(
                [relative_wrapper, "argument with spaces"],
                cwd=package.parent,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(completed.stdout, "real-core\nargument with spaces\n")
            self.assertEqual(completed.stderr, "")

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_follows_relative_symlink_but_rejects_binary_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, wrapper = self.wrapper_fixture(root)
            aliases = root / "aliases\nwith spaces"
            aliases.mkdir()
            alias = aliases / "forge"
            alias.symlink_to(os.path.relpath(wrapper, aliases))
            followed = subprocess.run(
                [str(alias), "via-symlink"],
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(followed.returncode, 0, followed.stderr)
            self.assertEqual(followed.stdout, "real-core\nvia-symlink\n")

            outside = root / "outside-core"
            outside.write_text("#!/bin/sh\nprintf '%s\\n' escaped\n", encoding="utf-8")
            outside.chmod(0o755)
            binary = package / "forge-core"
            binary.unlink()
            binary.symlink_to(outside)
            rejected = subprocess.run(
                [str(wrapper)], text=True, capture_output=True, check=False, timeout=5
            )
            self.assertEqual(rejected.returncode, 127)
            self.assertIn("missing or unsafe packaged binary", rejected.stderr)
            self.assertNotIn("escaped", rejected.stdout)

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_fails_closed_for_missing_binary_and_symlink_loop(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, wrapper = self.wrapper_fixture(root)
            (wrapper.parent / "forge-core").unlink()
            missing = subprocess.run(
                [str(wrapper)], text=True, capture_output=True, check=False, timeout=5
            )
            self.assertEqual(missing.returncode, 127)
            self.assertIn("missing or unsafe packaged binary", missing.stderr)

            loop_dir = root / "symlink loop"
            loop_dir.mkdir()
            first = loop_dir / "first"
            second = loop_dir / "second"
            first.symlink_to(second.name)
            second.symlink_to(first.name)
            try:
                loop = subprocess.run(
                    [str(first)],
                    text=True,
                    capture_output=True,
                    check=False,
                    timeout=5,
                )
            except OSError as error:
                # The kernel may reject a top-level symlink loop before /bin/sh
                # can enter the wrapper; that is also a fail-closed result.
                self.assertEqual(error.errno, errno.ELOOP)
            else:
                self.assertEqual(loop.returncode, 127)
                self.assertIn("symlink loop", loop.stderr)

    def test_wrapper_does_not_use_gnu_readlink_f(self) -> None:
        source = (SCRIPTS.parent / "distribution/forge").read_text(encoding="utf-8")
        self.assertNotRegex(source, r"readlink\s+-f")

if __name__ == "__main__":
    unittest.main()

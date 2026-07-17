#!/usr/bin/env python3
"""Focused standard-library tests for deterministic release archive assembly."""

from __future__ import annotations

import errno
import hashlib
import importlib.util
import os
import shutil
import subprocess
import sys
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

    def install_marker_core(self, package: Path) -> Path:
        binary = package / "forge-core"
        binary.write_text(
            f"#!{Path(sys.executable).resolve()}\n"
            "import os\n"
            "import sys\n"
            "from pathlib import Path\n"
            "Path(os.environ['REAL_CORE_MARKER']).write_text("
            "'executed\\n', encoding='utf-8')\n"
            "print('real-core')\n"
            "for argument in sys.argv[1:]:\n"
            "    print(argument)\n",
            encoding="utf-8",
        )
        binary.chmod(0o755)
        return binary

    def require_bash_5(self) -> Path:
        bash = shutil.which("bash")
        if bash is None:
            self.skipTest("Bash 5 poisoning regression requires bash")
        completed = subprocess.run(
            [bash, "--version"], text=True, capture_output=True, check=False, timeout=5
        )
        if completed.returncode != 0 or not completed.stdout.startswith("GNU bash, version 5."):
            self.skipTest("Bash 5 poisoning regression requires Bash major version 5")
        return Path(bash).resolve()

    def simulated_platform_wrapper_fixture(
        self, root: Path, system: str, machine: str, libc: str | None
    ) -> tuple[Path, Path, Path]:
        package, wrapper = self.wrapper_fixture(root)
        tools = root / "trusted-tools"
        tools.mkdir()
        uname = tools / "uname"
        uname.write_text(
            "#!/bin/sh\n"
            "case ${1-} in\n"
            f"  -s) printf '%s\\n' '{system}' ;;\n"
            f"  -m) printf '%s\\n' '{machine}' ;;\n"
            "  *) exit 2 ;;\n"
            "esac\n",
            encoding="utf-8",
        )
        uname.chmod(0o755)
        getconf = tools / "getconf"
        if libc is None:
            getconf.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
        else:
            getconf.write_text(
                f"#!/bin/sh\nprintf '%s\\n' '{libc}'\n", encoding="utf-8"
            )
        getconf.chmod(0o755)
        marker = root / "trusted-readlink-called"
        readlink = tools / "readlink"
        readlink.write_text(
            "#!/bin/sh\n"
            ": > \"$TRUSTED_READLINK_MARKER\"\n"
            "exec /usr/bin/readlink \"$@\"\n",
            encoding="utf-8",
        )
        readlink.chmod(0o755)

        source = wrapper.read_text(encoding="utf-8")
        for production, simulated in (
            ("/usr/bin/uname", uname),
            ("/bin/uname", uname),
            ("/usr/bin/getconf", getconf),
            ("/bin/getconf", getconf),
            ("/usr/bin/readlink", readlink),
            ("/bin/readlink", readlink),
        ):
            source = source.replace(production, str(simulated))
        wrapper.write_text(source, encoding="utf-8")
        wrapper.chmod(0o755)
        return package, wrapper, marker

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_blocks_exact_exported_bash_pwd_function_attack(self) -> None:
        bash = self.require_bash_5()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, wrapper = self.wrapper_fixture(root)
            self.install_marker_core(package)
            poison_marker = root / "pwd-function-called"
            core_marker = root / "real-core-called"
            environment = os.environ.copy()
            environment["POISON_MARKER"] = str(poison_marker)
            environment["REAL_CORE_MARKER"] = str(core_marker)
            environment["BASH_FUNC_pwd%%"] = (
                "() { printf '%s\\n' /tmp; : > \"$POISON_MARKER\"; }"
            )
            completed = subprocess.run(
                [str(bash), "--posix", str(wrapper), "pwd-attack"],
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(completed.stdout, "real-core\npwd-attack\n")
            self.assertEqual(completed.stderr, "")
            self.assertTrue(core_marker.exists())
            self.assertFalse(poison_marker.exists())

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_blocks_all_poisonable_command_functions_in_bash_modes(self) -> None:
        bash = self.require_bash_5()
        poisoned_names = (
            "command",
            "cd",
            "pwd",
            "printf",
            "test",
            "uname",
            "getconf",
            "readlink",
            "fail",
            "select_readlink",
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, wrapper = self.wrapper_fixture(root)
            self.install_marker_core(package)
            alias = root / "forge-alias"
            alias.symlink_to(os.path.relpath(wrapper, root))
            poison_markers = root / "poison-markers"
            poison_markers.mkdir()
            for mode in ("posix", "sh"):
                with self.subTest(mode=mode):
                    core_marker = root / f"real-core-called-{mode}"
                    environment = os.environ.copy()
                    environment["REAL_CORE_MARKER"] = str(core_marker)
                    for name in poisoned_names:
                        environment[f"BASH_FUNC_{name}%%"] = (
                            f"() {{ : > \"{poison_markers}/{name}\"; return 93; }}"
                        )
                    if mode == "posix":
                        argv = [str(bash), "--posix", str(alias), mode]
                        executable = None
                    else:
                        argv = ["sh", str(alias), mode]
                        executable = str(bash)
                    completed = subprocess.run(
                        argv,
                        executable=executable,
                        env=environment,
                        text=True,
                        capture_output=True,
                        check=False,
                        timeout=5,
                    )
                    self.assertEqual(completed.returncode, 0, completed.stderr)
                    self.assertEqual(completed.stdout, f"real-core\n{mode}\n")
                    self.assertEqual(completed.stderr, "")
                    self.assertTrue(core_marker.exists())
                    self.assertEqual(list(poison_markers.iterdir()), [])

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_bash_rejects_special_or_nonidentifier_function_poisoning(self) -> None:
        bash = self.require_bash_5()
        rejected_names = ("[", ":", "break", "exec", "exit", "set", "unset")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, wrapper = self.wrapper_fixture(root)
            self.install_marker_core(package)
            for name in rejected_names:
                with self.subTest(name=name):
                    poison_marker = root / f"poison-{name}"
                    core_marker = root / f"real-core-{name}"
                    environment = os.environ.copy()
                    environment["REAL_CORE_MARKER"] = str(core_marker)
                    environment[f"BASH_FUNC_{name}%%"] = (
                        f"() {{ : > \"{poison_marker}\"; return 93; }}"
                    )
                    completed = subprocess.run(
                        [str(bash), "--posix", str(wrapper), name],
                        env=environment,
                        text=True,
                        capture_output=True,
                        check=False,
                        timeout=5,
                    )
                    if name in ("break", "exec", "exit", "set", "unset"):
                        self.assertEqual(completed.returncode, 2, completed.stderr)
                        self.assertEqual(completed.stdout, "")
                        self.assertFalse(core_marker.exists())
                    else:
                        self.assertEqual(completed.returncode, 0, completed.stderr)
                        self.assertEqual(completed.stdout, f"real-core\n{name}\n")
                        self.assertTrue(core_marker.exists())
                    self.assertFalse(poison_marker.exists())

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
            completed = subprocess.run(
                [
                    "/bin/sh",
                    "-c",
                    'PATH="$1"; exec forge "$2"',
                    "bare-command",
                    path,
                    argument,
                ],
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(completed.stderr, "")
            self.assertEqual(completed.stdout, f"real-core\n{argument}\n")

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_executes_bare_from_each_empty_path_component(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, _ = self.wrapper_fixture(root)
            missing_a = root / "missing-a"
            missing_b = root / "missing-b"
            paths = {
                "empty": "",
                "leading-empty": f":{missing_a}",
                "interior-empty": f"{missing_a}::{missing_b}",
                "trailing-empty": f"{missing_a}:",
            }
            for name, path in paths.items():
                with self.subTest(name=name):
                    argument = f"{name} argument\nwith newline"
                    completed = subprocess.run(
                        [
                            "/bin/sh",
                            "-c",
                            'PATH=$1; exec forge "$2"',
                            "bare-command",
                            path,
                            argument,
                        ],
                        cwd=package,
                        text=True,
                        capture_output=True,
                        check=False,
                        timeout=5,
                    )
                    self.assertEqual(completed.returncode, 0, completed.stderr)
                    self.assertEqual(completed.stdout, f"real-core\n{argument}\n")
                    self.assertEqual(completed.stderr, "")

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_fixed_interpreter_and_tools_ignore_attacker_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, wrapper = self.wrapper_fixture(root)
            self.assertEqual(wrapper.read_bytes().splitlines()[0], b"#!/bin/sh")
            aliases = root / "aliases\nwith spaces"
            aliases.mkdir()
            alias = aliases / "forge"
            alias.symlink_to(os.path.relpath(wrapper, aliases))
            fake_bin = root / "fake-bin"
            fake_bin.mkdir()
            marker = root / "fake-program-called"
            for name in ("sh", "uname", "getconf", "readlink"):
                fake_tool = fake_bin / name
                fake_tool.write_text(
                    "#!/bin/sh\n"
                    f"printf '%s\\n' '{name}' >> \"$FAKE_PROGRAM_MARKER\"\n"
                    "exit 91\n",
                    encoding="utf-8",
                )
                fake_tool.chmod(0o755)
            environment = os.environ.copy()
            environment["PATH"] = os.pathsep.join([str(fake_bin), "/usr/bin", "/bin"])
            environment["FAKE_PROGRAM_MARKER"] = str(marker)
            invocations = {"direct": wrapper, "symlink": alias}
            for name, invoked in invocations.items():
                with self.subTest(name=name):
                    marker.unlink(missing_ok=True)
                    argument = f"{name} argument\nwith newline"
                    completed = subprocess.run(
                        [str(invoked), argument],
                        env=environment,
                        text=True,
                        capture_output=True,
                        check=False,
                        timeout=5,
                    )
                    self.assertEqual(completed.returncode, 0, completed.stderr)
                    self.assertEqual(completed.stdout, f"real-core\n{argument}\n")
                    self.assertEqual(completed.stderr, "")
                    self.assertFalse(marker.exists(), completed.stdout)
            self.assertEqual(package, wrapper.parent)

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_resolver_accepts_declared_package_targets(self) -> None:
        targets = (
            ("Linux", "x86_64", "glibc 2.17"),
            ("Linux", "aarch64", "glibc 2.17"),
            ("Darwin", "x86_64", None),
            ("Darwin", "arm64", None),
        )
        for system, machine, libc in targets:
            with self.subTest(system=system, machine=machine):
                with tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    package, wrapper, marker = self.simulated_platform_wrapper_fixture(
                        root, system, machine, libc
                    )
                    alias = root / "forge-alias"
                    alias.symlink_to(os.path.relpath(wrapper, root))
                    environment = os.environ.copy()
                    environment["TRUSTED_READLINK_MARKER"] = str(marker)
                    completed = subprocess.run(
                        [str(alias), "supported"],
                        env=environment,
                        text=True,
                        capture_output=True,
                        check=False,
                        timeout=5,
                    )
                    self.assertEqual(completed.returncode, 0, completed.stderr)
                    self.assertEqual(completed.stdout, "real-core\nsupported\n")
                    self.assertEqual(completed.stderr, "")
                    self.assertTrue(marker.exists())
                    self.assertEqual(package, wrapper.parent)

    @unittest.skipUnless(os.name == "posix", "POSIX wrapper tests require a POSIX shell")
    def test_wrapper_resolver_rejects_outside_declared_package_targets(self) -> None:
        targets = (
            ("Linux", "x86_64", None),
            ("Linux", "armv7l", "glibc 2.17"),
            ("Darwin", "powerpc", None),
            ("FreeBSD", "x86_64", None),
        )
        for system, machine, libc in targets:
            with self.subTest(system=system, machine=machine, libc=libc):
                with tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    _, wrapper, marker = self.simulated_platform_wrapper_fixture(
                        root, system, machine, libc
                    )
                    alias = root / "forge-alias"
                    alias.symlink_to(os.path.relpath(wrapper, root))
                    environment = os.environ.copy()
                    environment["TRUSTED_READLINK_MARKER"] = str(marker)
                    completed = subprocess.run(
                        [str(alias)],
                        env=environment,
                        text=True,
                        capture_output=True,
                        check=False,
                        timeout=5,
                    )
                    self.assertEqual(completed.returncode, 127)
                    self.assertIn("unsupported symlink resolver platform", completed.stderr)
                    self.assertEqual(completed.stdout, "")
                    self.assertFalse(marker.exists())

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
    def test_wrapper_execve_fails_closed_when_path_is_unset(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package, _ = self.wrapper_fixture(root)
            environment = os.environ.copy()
            environment.pop("PATH", None)
            argument = "unset PATH argument\nwith newline"
            completed = subprocess.run(
                [
                    str(Path(sys.executable).resolve()),
                    "-c",
                    "import os, sys; os.execve('forge', ['forge', sys.argv[1]], os.environ)",
                    argument,
                ],
                cwd=package,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(completed.returncode, 127)
            self.assertEqual(completed.stdout, "")
            # /bin/sh may initialize its implementation-default PATH when execve
            # receives none. Either way, the production wrapper itself fails closed.
            self.assertRegex(
                completed.stderr,
                r"forge wrapper: cannot locate wrapper forge through (?:an unset )?PATH",
            )
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

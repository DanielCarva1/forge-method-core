#!/usr/bin/env python3
"""Extract a release archive and smoke its packaged native CLI and wrapper."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import tempfile


class InstallSmokeError(RuntimeError):
    """The packaged release cannot complete the native installation journey."""


def load_checker():
    script = Path(__file__).resolve().with_name("check-release-archive.py")
    spec = importlib.util.spec_from_file_location("forge_release_checker_for_smoke", script)
    if spec is None or spec.loader is None:
        raise InstallSmokeError(f"cannot load archive reader from {script}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def extract_checked_members(archive: Path, destination: Path) -> dict[str, Path]:
    """Extract only regular, canonical members accepted by the archive checker."""

    checker = load_checker()
    try:
        members = checker.read_members(archive)
    except (OSError, checker.ArchiveCheckError) as error:
        raise InstallSmokeError(f"unsafe or unreadable archive: {error}") from error

    extracted: dict[str, Path] = {}
    for archive_path, (content, mode) in members.items():
        target = destination.joinpath(*PurePosixPath(archive_path).parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            with target.open("xb") as stream:
                stream.write(content)
            target.chmod(stat.S_IMODE(mode))
        except OSError as error:
            raise InstallSmokeError(f"extract {archive_path}: {error}") from error
        extracted[archive_path] = target
    return extracted


def run(command: list[str], label: str) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(command, text=True, capture_output=True, check=False)
    if completed.returncode != 0:
        raise InstallSmokeError(
            f"{label} failed with exit {completed.returncode}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def wrapper_command(wrapper: Path, arguments: list[str]) -> list[str]:
    if os.name != "nt":
        return [str(wrapper), *arguments]
    # `call` preserves the batch wrapper's exit status and handles a quoted
    # extraction path without asking Python to reinterpret command arguments.
    command_line = "call " + subprocess.list2cmdline([str(wrapper), *arguments])
    return ["cmd.exe", "/d", "/s", "/c", command_line]


def require_version(command: list[str], expected: str, label: str) -> None:
    actual = run(command, label).stdout.strip()
    if actual != expected:
        raise InstallSmokeError(f"{label}: expected {expected!r}, got {actual!r}")


def require_ok_json(command: list[str], label: str) -> dict:
    completed = run(command, label)
    try:
        envelope = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise InstallSmokeError(f"{label} did not emit JSON: {error}") from error
    if not isinstance(envelope, dict) or envelope.get("ok") is not True:
        raise InstallSmokeError(f"{label} did not emit an ok envelope: {envelope!r}")
    return envelope


def smoke(args: argparse.Namespace) -> None:
    expected_version = f"forge-core {args.version}"
    with tempfile.TemporaryDirectory(prefix="forge-release-install-") as directory:
        root = Path(directory)
        install_root = root / "installed"
        install_root.mkdir()
        extracted = extract_checked_members(args.archive, install_root)

        binary = extracted.get(args.binary_name)
        wrapper = extracted.get(args.wrapper_name)
        if binary is None or wrapper is None:
            raise InstallSmokeError(
                "archive lacks requested executable members: "
                f"binary={args.binary_name!r}, wrapper={args.wrapper_name!r}"
            )
        require_version([str(binary), "--version"], expected_version, "packaged binary --version")
        require_version(
            wrapper_command(wrapper, ["--version"]), expected_version, "packaged wrapper --version"
        )

        project = root / "consumer project"
        project.mkdir()
        root_args = ["--root", str(project), "--json"]
        journey = [
            (["start", *root_args], "start"),
            (["workflow", "init", *root_args], "workflow init"),
            (["workflow", "resume", *root_args], "workflow resume"),
            (["workflow", "release-status", *root_args], "workflow release-status"),
            (["workflow", "next", *root_args], "workflow next"),
        ]
        for command_args, label in journey:
            require_ok_json(wrapper_command(wrapper, command_args), label)

    print(
        f"smoked extracted {args.archive}: binary and wrapper version plus "
        "start/init/resume/release-status/next"
    )


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--archive", type=Path, required=True)
    result.add_argument("--binary-name", required=True)
    result.add_argument("--wrapper-name", required=True)
    result.add_argument("--version", required=True)
    return result


if __name__ == "__main__":
    try:
        smoke(parser().parse_args())
    except (OSError, InstallSmokeError) as error:
        raise SystemExit(f"release install smoke failed: {error}") from error

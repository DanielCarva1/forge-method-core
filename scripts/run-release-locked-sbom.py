#!/usr/bin/env python3
"""Run cargo-cyclonedx with every generated Cargo metadata call locked."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile


SHIM_MARKER = "FORGE_RELEASE_LOCKED_CARGO_SHIM"
REAL_CARGO = "FORGE_RELEASE_REAL_CARGO"


def locked_metadata_command(real_cargo: str, arguments: list[str]) -> list[str]:
    """Allow only cargo-cyclonedx's metadata query and inject lock enforcement."""
    if not arguments or arguments[0] != "metadata":
        raise ValueError(
            "release Cargo shim permits only cargo metadata, got "
            + repr(arguments)
        )
    return [real_cargo, "metadata", "--locked", *arguments[1:]]


def cargo_shim() -> int:
    real_cargo = os.environ.get(REAL_CARGO)
    if not real_cargo:
        raise RuntimeError(f"{REAL_CARGO} is required in Cargo shim mode")
    command = locked_metadata_command(real_cargo, sys.argv[1:])
    os.execv(real_cargo, command)


def _identity(metadata: os.stat_result) -> tuple[int, int, int, int, int]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _read_descriptor(fd: int) -> tuple[bytes, tuple[int, int, int, int, int]]:
    """Read twice from a retained descriptor and require a stable file identity."""
    before = os.fstat(fd)
    if not stat.S_ISREG(before.st_mode):
        raise RuntimeError("release lockfile descriptor is not a regular file")
    os.lseek(fd, 0, os.SEEK_SET)
    first = b""
    while chunk := os.read(fd, 1024 * 1024):
        first += chunk
    middle = os.fstat(fd)
    os.lseek(fd, 0, os.SEEK_SET)
    second = b""
    while chunk := os.read(fd, 1024 * 1024):
        second += chunk
    after = os.fstat(fd)
    if _identity(before) != _identity(middle) or _identity(middle) != _identity(after):
        raise RuntimeError("release lockfile changed while its descriptor was read")
    if first != second:
        raise RuntimeError("release lockfile bytes were unstable while read")
    return first, _identity(after)


def _inside(path: Path, root: Path, label: str) -> Path:
    resolved = path.resolve(strict=True)
    try:
        return resolved.relative_to(root)
    except ValueError as error:
        raise RuntimeError(f"{label} must remain inside checked repository {root}: {resolved}") from error


def _open_bound_lockfile(candidate: Path, repository: Path) -> tuple[int, bytes, tuple[int, int, int, int, int], Path, Path]:
    """Open the lockfile without following its leaf and bind its Linux descriptor path.

    The release workflow runs on Ubuntu. We fail closed elsewhere because Python
    has no portable equivalent to Linux /proc/self/fd for proving the opened
    descriptor still names the reviewed repository path.
    """
    if not hasattr(os, "O_NOFOLLOW") or not Path("/proc/self/fd").is_dir():
        raise RuntimeError(
            "race-free release lockfile binding requires O_NOFOLLOW and Linux /proc/self/fd"
        )
    lexical = Path(os.path.abspath(candidate))
    initial = os.lstat(lexical)
    if not stat.S_ISREG(initial.st_mode):
        raise RuntimeError(f"release lockfile is missing or unsafe: {lexical}")
    resolved = lexical.resolve(strict=True)
    relative = _inside(lexical, repository, "release lockfile")
    fd = os.open(lexical, os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_BINARY", 0))
    try:
        opened_path = Path(f"/proc/self/fd/{fd}").resolve(strict=True)
        if opened_path != resolved:
            raise RuntimeError(
                f"release lockfile path changed while opened: expected {resolved}, got {opened_path}"
            )
        _inside(opened_path, repository, "opened release lockfile descriptor")
        data, identity = _read_descriptor(fd)
        current = os.stat(lexical, follow_symlinks=False)
        if _identity(current) != identity:
            raise RuntimeError("release lockfile identity changed while opened")
        return fd, data, identity, resolved, relative
    except BaseException:
        os.close(fd)
        raise


def _verify_bound_lockfile(
    fd: int,
    candidate: Path,
    repository: Path,
    expected_bytes: bytes,
    expected_identity: tuple[int, int, int, int, int],
    expected_path: Path,
) -> None:
    data, identity = _read_descriptor(fd)
    if identity != expected_identity or hashlib.sha256(data).digest() != hashlib.sha256(expected_bytes).digest():
        raise RuntimeError("release lockfile descriptor identity or bytes changed during SBOM generation")
    lexical = Path(os.path.abspath(candidate))
    current = os.lstat(lexical)
    if not stat.S_ISREG(current.st_mode):
        raise RuntimeError("release lockfile path was replaced during SBOM generation")
    if lexical.resolve(strict=True) != expected_path:
        raise RuntimeError("release lockfile resolved path changed during SBOM generation")
    _inside(lexical, repository, "release lockfile after SBOM generation")
    if _identity(current) != expected_identity:
        raise RuntimeError("release lockfile path identity changed during SBOM generation")


def _argument(forwarded: list[str], option: str) -> tuple[int, str]:
    indexes = [index for index, item in enumerate(forwarded) if item == option]
    if len(indexes) != 1 or indexes[0] + 1 >= len(forwarded):
        raise RuntimeError(f"cargo-cyclonedx requires exactly one {option} value")
    return indexes[0] + 1, forwarded[indexes[0] + 1]


def _copy_repository(repository: Path, destination: Path) -> None:
    shutil.copytree(
        repository,
        destination,
        symlinks=True,
        ignore=shutil.ignore_patterns(".git", "target", "release-assets", "*.cdx.json"),
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lockfile", type=Path, required=True)
    parser.add_argument("cyclonedx_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    forwarded = args.cyclonedx_args
    if forwarded[:1] == ["--"]:
        forwarded = forwarded[1:]
    if not forwarded:
        parser.error("cargo-cyclonedx arguments are required after --")

    repository = Path(__file__).resolve().parents[1]
    candidate = args.lockfile if args.lockfile.is_absolute() else Path.cwd() / args.lockfile
    fd, lock_bytes, lock_identity, lock_path, lock_relative = _open_bound_lockfile(
        candidate, repository
    )
    try:
        cargo = shutil.which("cargo")
        cyclonedx = shutil.which("cargo-cyclonedx")
        if cargo is None:
            raise RuntimeError("cargo is not available on PATH")
        if cyclonedx is None:
            raise RuntimeError("cargo-cyclonedx is not available on PATH")

        manifest_index, manifest_value = _argument(forwarded, "--manifest-path")
        output_index, output_value = _argument(forwarded, "--override-filename")
        if Path(output_value).name != output_value:
            raise RuntimeError("cargo-cyclonedx override filename must be a basename")
        manifest = Path(manifest_value)
        if not manifest.is_absolute():
            manifest = Path.cwd() / manifest
        manifest_relative = _inside(manifest, repository, "cargo-cyclonedx manifest")
        cwd_relative = _inside(Path.cwd(), repository, "cargo-cyclonedx working directory")
        output_name = output_value if output_value.endswith(".json") else output_value + ".json"

        with tempfile.TemporaryDirectory(prefix="forge-release-sbom-") as temporary:
            staged_repository = Path(temporary) / "repository"
            _copy_repository(repository, staged_repository)
            staged_lock = staged_repository / lock_relative
            staged_lock.parent.mkdir(parents=True, exist_ok=True)
            staged_lock.write_bytes(lock_bytes)
            staged_manifest = staged_repository / manifest_relative
            staged_output = staged_manifest.parent / output_name
            staged_output.unlink(missing_ok=True)
            staged_forwarded = list(forwarded)
            staged_forwarded[manifest_index] = str(staged_manifest)
            staged_forwarded[output_index] = output_value

            environment = os.environ.copy()
            environment["CARGO"] = str(Path(__file__).resolve())
            environment[SHIM_MARKER] = "1"
            environment[REAL_CARGO] = cargo
            completed = subprocess.run(
                [cyclonedx, "cyclonedx", *staged_forwarded],
                cwd=staged_repository / cwd_relative,
                env=environment,
                check=False,
            )
            _verify_bound_lockfile(
                fd, candidate, repository, lock_bytes, lock_identity, lock_path
            )
            if completed.returncode != 0:
                return completed.returncode
            metadata = os.lstat(staged_output)
            if not stat.S_ISREG(metadata.st_mode):
                raise RuntimeError(f"cargo-cyclonedx did not create a safe SBOM at {staged_output}")
            output_bytes = staged_output.read_bytes()
            destination = repository / manifest_relative.parent / output_name
            destination.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as stream:
                temporary_output = Path(stream.name)
                stream.write(output_bytes)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary_output, destination)
            return 0
    finally:
        os.close(fd)


if __name__ == "__main__":
    try:
        if os.environ.get(SHIM_MARKER) == "1":
            raise SystemExit(cargo_shim())
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(f"locked release SBOM failed: {error}") from error

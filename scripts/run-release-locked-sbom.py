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


def digest(path: Path) -> bytes:
    return hashlib.sha256(path.read_bytes()).digest()


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
    metadata = os.lstat(candidate)
    if not stat.S_ISREG(metadata.st_mode):
        raise RuntimeError(f"release lockfile is missing or unsafe: {candidate}")
    lockfile = candidate.resolve(strict=True)
    try:
        lockfile.relative_to(repository)
    except ValueError as error:
        raise RuntimeError(
            f"release lockfile must remain inside checked repository {repository}: {lockfile}"
        ) from error
    before = digest(lockfile)

    cargo = shutil.which("cargo")
    cyclonedx = shutil.which("cargo-cyclonedx")
    if cargo is None:
        raise RuntimeError("cargo is not available on PATH")
    if cyclonedx is None:
        raise RuntimeError("cargo-cyclonedx is not available on PATH")

    environment = os.environ.copy()
    environment["CARGO"] = str(Path(__file__).resolve())
    environment[SHIM_MARKER] = "1"
    environment[REAL_CARGO] = cargo
    completed = subprocess.run(
        [cyclonedx, "cyclonedx", *forwarded],
        env=environment,
        check=False,
    )
    if digest(lockfile) != before:
        raise RuntimeError(f"cargo-cyclonedx modified release lockfile {lockfile}")
    return completed.returncode


if __name__ == "__main__":
    try:
        if os.environ.get(SHIM_MARKER) == "1":
            raise SystemExit(cargo_shim())
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(f"locked release SBOM failed: {error}") from error

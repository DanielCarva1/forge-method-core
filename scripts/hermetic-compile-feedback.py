#!/usr/bin/env python3
"""Run the reviewed Cargo compile-feedback grammar in a closed environment."""

from __future__ import annotations

import os
import re
import stat
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TRUSTED_TOOLCHAIN_ROOT = "/opt/forge-method/rust-1.85.1"
TRUSTED_CARGO = f"{TRUSTED_TOOLCHAIN_ROOT}/bin/cargo"
TRUSTED_RUSTC = f"{TRUSTED_TOOLCHAIN_ROOT}/bin/rustc"
TRUSTED_RUSTDOC = f"{TRUSTED_TOOLCHAIN_ROOT}/bin/rustdoc"
TRUSTED_PATH = "/usr/bin"
TRUSTED_EXECUTABLES = (TRUSTED_CARGO, TRUSTED_RUSTC, TRUSTED_RUSTDOC)
PACKAGE_NAME = re.compile(r"[A-Za-z0-9_.-]+")
SAFE_CHECK_SWITCHES = {
    "--all-targets",
    "--all-features",
    "--no-default-features",
    "--lib",
    "--bins",
    "--examples",
    "--tests",
    "--benches",
}
CHECK_VALUE_FLAGS = {"--features", "--bin", "--example", "--test", "--bench"}


def _mapping(value: object) -> dict[str, object] | None:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        return None
    return value


def _string_list(value: object) -> list[str] | None:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        return None
    return value


def workspace_package_names() -> set[str] | None:
    root_manifest = ROOT / "Cargo.toml"
    try:
        if not root_manifest.is_file() or root_manifest.is_symlink():
            return None
        root_document = tomllib.loads(root_manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError):
        return None
    workspace = _mapping(root_document.get("workspace"))
    members = _string_list(workspace.get("members")) if workspace else None
    if members is None or not members or len(members) != len(set(members)):
        return None
    names: set[str] = set()
    for member in members:
        member_path = Path(member)
        if member_path.is_absolute() or ".." in member_path.parts:
            return None
        manifest = ROOT / member_path / "Cargo.toml"
        try:
            if not manifest.is_file() or manifest.is_symlink():
                return None
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, tomllib.TOMLDecodeError):
            return None
        package = _mapping(document.get("package"))
        name = package.get("name") if package else None
        if (
            not isinstance(name, str)
            or PACKAGE_NAME.fullmatch(name) is None
            or name.startswith("-")
            or name in names
        ):
            return None
        names.add(name)
    return names


def _package_value_valid(value: str, package_names: set[str]) -> bool:
    return bool(
        value
        and not value.startswith("-")
        and PACKAGE_NAME.fullmatch(value) is not None
        and value in package_names
    )


def cargo_check_args_valid(args: list[str], package_names: set[str]) -> bool:
    if len(args) < 3 or args[0] != "check":
        return False
    lock_modes = 0
    workspace_scopes = 0
    package_scopes = 0
    index = 1
    while index < len(args):
        token = args[index]
        if token in {"--locked", "--frozen"}:
            lock_modes += 1
            index += 1
            continue
        if token == "--workspace":
            workspace_scopes += 1
            index += 1
            continue
        if token in {"-p", "--package"}:
            if index + 1 >= len(args) or not _package_value_valid(args[index + 1], package_names):
                return False
            package_scopes += 1
            index += 2
            continue
        if token.startswith("--package="):
            if not _package_value_valid(token.split("=", 1)[1], package_names):
                return False
            package_scopes += 1
            index += 1
            continue
        if token in SAFE_CHECK_SWITCHES:
            index += 1
            continue
        if token in CHECK_VALUE_FLAGS:
            if index + 1 >= len(args) or not args[index + 1] or args[index + 1].startswith("-"):
                return False
            index += 2
            continue
        if token.startswith("--features="):
            if not token.split("=", 1)[1]:
                return False
            index += 1
            continue
        return False
    if lock_modes != 1:
        return False
    return (workspace_scopes == 1 and package_scopes == 0) or (
        workspace_scopes == 0 and package_scopes == 1
    )


def cargo_metadata_args_valid(args: list[str]) -> bool:
    if len(args) < 4 or args[0] != "metadata":
        return False
    lock_modes = 0
    no_deps = 0
    format_version = 0
    index = 1
    while index < len(args):
        token = args[index]
        if token in {"--locked", "--frozen"}:
            lock_modes += 1
            index += 1
            continue
        if token == "--no-deps":
            no_deps += 1
            index += 1
            continue
        if token == "--format-version":
            if index + 1 >= len(args) or args[index + 1] != "1":
                return False
            format_version += 1
            index += 2
            continue
        if token == "--format-version=1":
            format_version += 1
            index += 1
            continue
        return False
    return lock_modes == 1 and no_deps == 1 and format_version == 1


def compile_feedback_args_valid(args: list[str]) -> bool:
    package_names = workspace_package_names()
    if package_names is None:
        return False
    return cargo_check_args_valid(args, package_names) or cargo_metadata_args_valid(args)


def cargo_config_is_closed() -> bool:
    candidates = {
        Path("/home/user/.cargo/config"),
        Path("/home/user/.cargo/config.toml"),
    }
    for directory in (ROOT, *ROOT.parents):
        candidates.add(directory / ".cargo/config")
        candidates.add(directory / ".cargo/config.toml")
    return all(not path.exists() and not path.is_symlink() for path in candidates)


def closed_environment() -> dict[str, str]:
    return {
        "HOME": "/nonexistent",
        "CARGO_HOME": "/home/user/.cargo",
        "PATH": TRUSTED_PATH,
        "RUSTC": TRUSTED_RUSTC,
        "RUSTDOC": TRUSTED_RUSTDOC,
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
    }


def trusted_path_chain(path: str, *, executable: bool, invoking_uid: int | None = None) -> bool:
    """Require a root-owned, non-symlink chain not writable by the caller."""
    candidate = Path(path)
    if not candidate.is_absolute() or candidate.as_posix() != path:
        return False
    uid = os.geteuid() if invoking_uid is None else invoking_uid
    current = Path("/")
    try:
        parts = candidate.parts[1:]
        for index, part in enumerate(parts):
            current /= part
            metadata = os.lstat(current)
            mode = metadata.st_mode
            final = index == len(parts) - 1
            if stat.S_ISLNK(mode) or metadata.st_uid != 0:
                return False
            if mode & (stat.S_IWGRP | stat.S_IWOTH):
                return False
            if metadata.st_uid == uid and mode & stat.S_IWUSR:
                return False
            if final and executable:
                if not stat.S_ISREG(mode) or mode & 0o111 == 0:
                    return False
            elif not stat.S_ISDIR(mode):
                return False
    except OSError:
        return False
    return True


def trusted_execution_environment_valid(invoking_uid: int | None = None) -> bool:
    path_components = TRUSTED_PATH.split(os.pathsep)
    return bool(
        path_components
        and all(
            component
            and trusted_path_chain(component, executable=False, invoking_uid=invoking_uid)
            for component in path_components
        )
        and all(
            trusted_path_chain(path, executable=True, invoking_uid=invoking_uid)
            for path in TRUSTED_EXECUTABLES
        )
    )


def main() -> int:
    args = sys.argv[1:]
    if not compile_feedback_args_valid(args):
        print(
            "hermetic compile feedback requires exactly one locked/frozen package or workspace "
            "cargo check, or bounded locked/frozen metadata",
            file=sys.stderr,
        )
        return 2
    if not cargo_config_is_closed():
        print("hermetic compile feedback rejects discovered Cargo configuration files", file=sys.stderr)
        return 2
    if not trusted_execution_environment_valid():
        print(
            "hermetic compile feedback requires the reviewed root-owned non-symlink Rust toolchain "
            "and native-tool PATH, with no component writable by the invoking uid",
            file=sys.stderr,
        )
        return 126
    try:
        os.chdir(ROOT)
        os.execve(TRUSTED_CARGO, [TRUSTED_CARGO, *args], closed_environment())
    except OSError as error:
        print(f"cannot launch trusted Cargo: {error}", file=sys.stderr)
        return 126


if __name__ == "__main__":
    raise SystemExit(main())

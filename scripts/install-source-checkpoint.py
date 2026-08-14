#!/usr/bin/env python3
"""Build and install one clean Forge source checkpoint with one rollback."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import tomllib
from typing import Iterator


RECEIPT_SCHEMA = "forge_source_install_receipt_v1"
STATE_SCHEMA = "forge_source_install_state_v1"
PENDING_SCHEMA = "forge_source_install_pending_v1"
SHA256 = re.compile(r"^[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
RECEIPT_ID = re.compile(r"^(?:[0-9a-f]{40}|adopted)-[0-9a-f]{64}$")
KNOWN_BINARY_NAMES = ("forge-core.exe", "forge-core", "forge-core.cmd")


class InstallError(RuntimeError):
    """A safe, user-actionable source-install rejection."""


def default_install_root() -> Path:
    if os.name == "nt":
        local_app_data = os.environ.get("LOCALAPPDATA")
        if local_app_data:
            return Path(local_app_data) / "Programs" / "forge-core"
    return Path.home() / ".local" / "share" / "forge-core"


def command(args: list[str], *, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, cwd=cwd, text=True, capture_output=True, check=False)


def git(repo: Path, *args: str) -> str:
    result = command(["git", *args], cwd=repo)
    if result.returncode != 0:
        raise InstallError(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout.strip()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def is_reparse_point(path: Path) -> bool:
    attributes = getattr(path.lstat(), "st_file_attributes", 0)
    flag = getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400)
    return bool(attributes & flag)


def reject_link_components(path: Path) -> None:
    absolute = path.absolute()
    current = Path(absolute.anchor)
    for part in absolute.parts[1:]:
        current /= part
        if not current.exists():
            continue
        if current.is_symlink() or is_reparse_point(current):
            raise InstallError(f"source install path contains a link or reparse point: {current}")


def require_regular_file(path: Path, label: str) -> Path:
    resolved = path.absolute()
    if not resolved.exists() or resolved.is_symlink() or is_reparse_point(resolved):
        raise InstallError(f"{label} must be an existing regular non-link file: {resolved}")
    if not resolved.is_file():
        raise InstallError(f"{label} must be a regular file: {resolved}")
    return resolved


def workspace_version(repo: Path) -> str:
    try:
        document = tomllib.loads((repo / "Cargo.toml").read_text(encoding="utf-8"))
        version = document["workspace"]["package"]["version"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
        raise InstallError(f"cannot read workspace package version: {error}") from error
    if not isinstance(version, str) or not version.strip():
        raise InstallError("workspace package version must be a non-empty string")
    return version


def clean_checkpoint(repo_value: Path) -> tuple[Path, str, str]:
    repo = repo_value.resolve(strict=True)
    top = Path(git(repo, "rev-parse", "--show-toplevel")).resolve(strict=True)
    if os.path.normcase(str(top)) != os.path.normcase(str(repo)):
        raise InstallError(f"--repo-root must be the exact Git root: {top}")
    status = git(repo, "status", "--porcelain", "--untracked-files=normal")
    if status:
        raise InstallError("source checkout is dirty; commit or remove changes before installation")
    commit = git(repo, "rev-parse", "HEAD")
    if COMMIT.fullmatch(commit) is None:
        raise InstallError("Git did not return one full SHA-1 source commit")
    return repo, commit, workspace_version(repo)


def run_candidate_version(candidate: Path) -> str:
    if os.name == "nt" and candidate.suffix.lower() in {".cmd", ".bat"}:
        comspec = os.environ.get("COMSPEC", "cmd.exe")
        result = subprocess.run(
            [comspec, "/d", "/c", "call", str(candidate), "--version"],
            text=True,
            capture_output=True,
            check=False,
        )
    else:
        result = subprocess.run(
            [str(candidate), "--version"], text=True, capture_output=True, check=False
        )
    if result.returncode != 0:
        raise InstallError(
            f"candidate --version failed: {result.stderr.strip() or result.stdout.strip()}"
        )
    return result.stdout.strip()


def binary_name(candidate: Path) -> str:
    suffix = candidate.suffix.lower()
    if suffix == ".exe":
        return "forge-core.exe"
    if suffix in {".cmd", ".bat"}:
        return "forge-core.cmd"
    return "forge-core"


def build_candidate(repo: Path, target_dir: Path) -> Path:
    reject_link_components(target_dir)
    target_dir.mkdir(parents=True, exist_ok=True)
    result = command(
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
            str(target_dir),
        ],
        cwd=repo,
    )
    if result.returncode != 0:
        raise InstallError(result.stderr.strip() or "cargo release build failed")
    name = "forge-core.exe" if os.name == "nt" else "forge-core"
    return require_regular_file(target_dir / "release" / name, "built candidate")


def fsync_file(path: Path) -> None:
    with path.open("r+b") as handle:
        os.fsync(handle.fileno())


def atomic_json(path: Path, document: dict[str, object]) -> None:
    payload = json.dumps(document, indent=2, sort_keys=True, ensure_ascii=True) + "\n"
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    try:
        with temporary.open("x", encoding="ascii", newline="\n") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def clean_atomic_temps(source_root: Path, receipts: Path) -> None:
    locations = (
        (source_root, re.compile(r"^\.(?:state|pending)\.json\.\d+\.tmp$")),
        (
            receipts,
            re.compile(
                r"^\.(?:[0-9a-f]{40}|adopted)-[0-9a-f]{64}\.json\.\d+\.tmp$"
            ),
        ),
    )
    for directory, pattern in locations:
        for path in directory.iterdir():
            if pattern.fullmatch(path.name) is None:
                continue
            if path.is_symlink() or is_reparse_point(path) or not path.is_file():
                raise InstallError(f"unsafe source-install temporary: {path}")
            path.unlink()


def copy_synced(source: Path, destination: Path) -> None:
    shutil.copyfile(source, destination)
    shutil.copymode(source, destination)
    fsync_file(destination)


@contextmanager
def install_lock(path: Path) -> Iterator[None]:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+b") as handle:
        handle.seek(0, os.SEEK_END)
        if handle.tell() == 0:
            handle.write(b"\0")
            handle.flush()
        handle.seek(0)
        try:
            if os.name == "nt":
                import msvcrt

                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl

                fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            raise InstallError("another source-checkpoint installation is running") from error
        try:
            yield
        finally:
            handle.seek(0)
            if os.name == "nt":
                import msvcrt

                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl

                fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def clean_staging(staging: Path) -> None:
    staging.mkdir(parents=True, exist_ok=True)
    for child in staging.iterdir():
        if child.is_symlink() or is_reparse_point(child) or not child.is_file():
            raise InstallError(f"unsafe or unknown source-install staging entry: {child}")
        if not child.name.startswith(("candidate-", "rollback-")):
            raise InstallError(f"unknown source-install staging entry: {child}")
        child.unlink()


def receipt_id(commit: str | None, digest: str) -> str:
    identity = commit if commit is not None else "adopted"
    return f"{identity}-{digest}"


def load_receipt(receipts: Path, identity: str) -> dict[str, object] | None:
    if RECEIPT_ID.fullmatch(identity) is None:
        raise InstallError("source-install receipt identity is invalid")
    path = receipts / f"{identity}.json"
    if not path.exists():
        return None
    if path.is_symlink() or is_reparse_point(path) or not path.is_file():
        raise InstallError(f"unsafe source-install receipt: {path}")
    try:
        document = json.loads(path.read_text(encoding="ascii"))
    except (OSError, json.JSONDecodeError) as error:
        raise InstallError(f"invalid source-install receipt {path}: {error}") from error
    if (
        not isinstance(document, dict)
        or document.get("schema_version") != RECEIPT_SCHEMA
        or receipt_id(
            document.get("source_commit")
            if isinstance(document.get("source_commit"), str)
            else None,
            document.get("binary_sha256", ""),
        )
        != identity
    ):
        raise InstallError(f"source-install receipt does not bind its filename: {path}")
    return document


def receipt_for(
    *, commit: str | None, version: str, digest: str, size: int, name: str, provenance: str
) -> dict[str, object]:
    return {
        "schema_version": RECEIPT_SCHEMA,
        "source_commit": commit,
        "source_checkout_clean": commit is not None,
        "package_version": version,
        "binary_name": name,
        "binary_sha256": digest,
        "binary_size": size,
        "provenance": provenance,
        "installed_at_unix": int(time.time()),
    }


def write_receipt(receipts: Path, document: dict[str, object]) -> tuple[str, bool]:
    digest = document["binary_sha256"]
    if not isinstance(digest, str) or SHA256.fullmatch(digest) is None:
        raise InstallError("receipt binary digest is invalid")
    commit = document.get("source_commit")
    if commit is not None and (not isinstance(commit, str) or COMMIT.fullmatch(commit) is None):
        raise InstallError("receipt source commit is invalid")
    identity = receipt_id(commit if isinstance(commit, str) else None, digest)
    path = receipts / f"{identity}.json"
    if path.exists():
        existing = load_receipt(receipts, identity)
        if existing != document:
            stable_existing = dict(existing or {})
            stable_new = dict(document)
            stable_existing.pop("installed_at_unix", None)
            stable_new.pop("installed_at_unix", None)
            if stable_existing != stable_new:
                raise InstallError("receipt identity has conflicting source-install metadata")
        return identity, False
    atomic_json(path, document)
    return identity, True


def load_state(source_root: Path, receipts: Path) -> dict[str, object] | None:
    path = source_root / "state.json"
    if not path.exists():
        return None
    if path.is_symlink() or is_reparse_point(path) or not path.is_file():
        raise InstallError(f"unsafe source-install state: {path}")
    try:
        document = json.loads(path.read_text(encoding="ascii"))
    except (OSError, json.JSONDecodeError) as error:
        raise InstallError(f"invalid source-install state: {error}") from error
    if not isinstance(document, dict) or document.get("schema_version") != STATE_SCHEMA:
        raise InstallError("invalid source-install state schema")
    for field in ("active_receipt_id", "rollback_receipt_id"):
        value = document.get(field)
        if value is not None and (not isinstance(value, str) or load_receipt(receipts, value) is None):
            raise InstallError(f"source-install state has an invalid {field}")
    return document


def write_state(
    source_root: Path, *, active_receipt_id: str, rollback_receipt_id: str | None
) -> None:
    atomic_json(
        source_root / "state.json",
        {
            "schema_version": STATE_SCHEMA,
            "active_receipt_id": active_receipt_id,
            "rollback_receipt_id": rollback_receipt_id,
        },
    )


def load_pending(source_root: Path, receipts: Path) -> dict[str, object] | None:
    path = source_root / "pending.json"
    if not path.exists():
        return None
    if path.is_symlink() or is_reparse_point(path) or not path.is_file():
        raise InstallError(f"unsafe source-install pending transaction: {path}")
    try:
        document = json.loads(path.read_text(encoding="ascii"))
    except (OSError, json.JSONDecodeError) as error:
        raise InstallError(f"invalid source-install pending transaction: {error}") from error
    if not isinstance(document, dict) or document.get("schema_version") != PENDING_SCHEMA:
        raise InstallError("invalid source-install pending transaction schema")
    candidate_id = document.get("candidate_receipt_id")
    prior_id = document.get("prior_active_receipt_id")
    name = document.get("binary_name")
    staged_rollback = document.get("staged_rollback_name")
    if (
        not isinstance(candidate_id, str)
        or load_receipt(receipts, candidate_id) is None
        or (prior_id is not None and (not isinstance(prior_id, str) or load_receipt(receipts, prior_id) is None))
        or name not in KNOWN_BINARY_NAMES
        or (
            staged_rollback is not None
            and (
                not isinstance(staged_rollback, str)
                or re.fullmatch(r"rollback-\d+-\d+", staged_rollback) is None
            )
        )
    ):
        raise InstallError("invalid source-install pending transaction fields")
    return document


def recover_pending(
    source_root: Path,
    bin_dir: Path,
    rollback_dir: Path,
    receipts: Path,
    staging: Path,
) -> None:
    pending = load_pending(source_root, receipts)
    if pending is None:
        return
    candidate_id = pending["candidate_receipt_id"]
    prior_id = pending.get("prior_active_receipt_id")
    name = pending["binary_name"]
    candidate_receipt = load_receipt(receipts, candidate_id)
    prior_receipt = load_receipt(receipts, prior_id) if isinstance(prior_id, str) else None
    if candidate_receipt is None:
        raise InstallError("pending candidate receipt is missing")

    active = current_binary(bin_dir)
    active_digest = sha256(active) if active is not None else None
    candidate_digest = candidate_receipt["binary_sha256"]
    prior_digest = prior_receipt["binary_sha256"] if prior_receipt is not None else None
    pending_path = source_root / "pending.json"

    if active_digest == candidate_digest:
        rollback_id: str | None = prior_id if isinstance(prior_id, str) else None
        if prior_receipt is not None:
            staged_name = pending.get("staged_rollback_name")
            staged_path = staging / staged_name if isinstance(staged_name, str) else None
            rollback_path = rollback_dir / name
            if staged_path is not None and staged_path.exists():
                require_regular_file(staged_path, "staged rollback")
                if sha256(staged_path) != prior_digest:
                    raise InstallError("pending staged rollback digest is invalid")
                os.replace(staged_path, rollback_path)
            rollback = require_regular_file(rollback_path, "rollback binary")
            if sha256(rollback) != prior_digest:
                raise InstallError("pending rollback does not match prior active receipt")
        write_state(
            source_root,
            active_receipt_id=candidate_id,
            rollback_receipt_id=rollback_id,
        )
        retained = {candidate_id}
        if rollback_id is not None:
            retained.add(rollback_id)
        prune_receipts(receipts, retained)
        pending_path.unlink()
        return

    if active_digest == prior_digest or (active is None and prior_receipt is None):
        state = load_state(source_root, receipts)
        state_ids = {
            value
            for value in (
                state.get("active_receipt_id") if state else None,
                state.get("rollback_receipt_id") if state else None,
            )
            if isinstance(value, str)
        }
        if candidate_id not in state_ids:
            (receipts / f"{candidate_id}.json").unlink()
        pending_path.unlink()
        return

    raise InstallError("cannot safely reconcile interrupted source installation")


def current_binary(bin_dir: Path) -> Path | None:
    found = [bin_dir / name for name in KNOWN_BINARY_NAMES if (bin_dir / name).exists()]
    if len(found) > 1:
        raise InstallError("install root contains more than one forge-core binary name")
    if not found:
        return None
    return require_regular_file(found[0], "installed binary")


def prune_receipts(receipts: Path, retained: set[str]) -> None:
    for path in receipts.iterdir():
        if path.is_symlink() or is_reparse_point(path) or not path.is_file():
            raise InstallError(f"unsafe source-install receipt entry: {path}")
        match = re.fullmatch(r"((?:[0-9a-f]{40}|adopted)-[0-9a-f]{64})\.json", path.name)
        if match is None:
            raise InstallError(f"unknown source-install receipt entry: {path}")
        load_receipt(receipts, match.group(1))
        if match.group(1) not in retained:
            path.unlink()


def install(args: argparse.Namespace) -> dict[str, object]:
    repo, commit, version = clean_checkpoint(args.repo_root)
    install_root = args.install_root.absolute()
    reject_link_components(install_root)
    if install_root.exists() and not install_root.is_dir():
        raise InstallError("install root must be a directory")
    install_root.mkdir(parents=True, exist_ok=True)
    reject_link_components(install_root)

    source_root = install_root / "source-install"
    bin_dir = install_root / "bin"
    rollback_dir = source_root / "rollback"
    receipts = source_root / "receipts"
    staging = source_root / "staging"
    for directory in (source_root, bin_dir, rollback_dir, receipts, staging):
        directory.mkdir(parents=True, exist_ok=True)
        reject_link_components(directory)

    with install_lock(source_root / ".install.lock"):
        clean_atomic_temps(source_root, receipts)
        recover_pending(source_root, bin_dir, rollback_dir, receipts, staging)
        clean_staging(staging)
        reconciled_state = load_state(source_root, receipts)
        retained_receipts = {
            value
            for value in (
                reconciled_state.get("active_receipt_id") if reconciled_state else None,
                reconciled_state.get("rollback_receipt_id") if reconciled_state else None,
            )
            if isinstance(value, str)
        }
        prune_receipts(receipts, retained_receipts)
        configured_target = args.target_dir
        if configured_target is None and os.environ.get("CARGO_TARGET_DIR"):
            configured_target = Path(os.environ["CARGO_TARGET_DIR"])
        target_dir = (configured_target or (repo / "target")).absolute()
        candidate = build_candidate(repo, target_dir)

        observed_version = run_candidate_version(candidate)
        expected_version = f"forge-core {version}"
        if observed_version != expected_version:
            raise InstallError(
                f"candidate version mismatch: expected {expected_version!r}, got {observed_version!r}"
            )
        # The checkout must still be the exact clean checkpoint observed before a potentially long build.
        _, observed_commit, observed_package = clean_checkpoint(repo)
        if observed_commit != commit or observed_package != version:
            raise InstallError("source checkpoint changed during build")

        name = binary_name(candidate)
        active = current_binary(bin_dir)
        if active is not None and active.name != name:
            raise InstallError(
                f"installed binary name {active.name!r} differs from candidate name {name!r}"
            )
        candidate_digest = sha256(candidate)
        candidate_receipt = receipt_for(
            commit=commit,
            version=version,
            digest=candidate_digest,
            size=candidate.stat().st_size,
            name=name,
            provenance="repo_owned_release_build",
        )

        state = load_state(source_root, receipts)
        active_receipt_id: str | None = None

        if active is None:
            stale_rollbacks = [
                rollback_dir / known_name
                for known_name in KNOWN_BINARY_NAMES
                if (rollback_dir / known_name).exists()
            ]
            if state is not None or stale_rollbacks:
                raise InstallError(
                    "installed binary is missing while source-install state or rollback remains"
                )

        if active is not None:
            active_digest = sha256(active)
            active_receipt_id = (
                state.get("active_receipt_id") if state is not None else None
            )
            active_receipt = (
                load_receipt(receipts, active_receipt_id)
                if isinstance(active_receipt_id, str)
                else None
            )
            if active_receipt is not None and active_receipt.get("binary_sha256") != active_digest:
                raise InstallError("installed binary does not match source-install state")
            if active_receipt is None and not args.adopt_current:
                raise InstallError(
                    "installed binary is unmanaged; rerun once with --adopt-current to preserve it as rollback"
                )
            if active_receipt is None:
                if active_digest == candidate_digest:
                    candidate_id, _ = write_receipt(receipts, candidate_receipt)
                    write_state(
                        source_root,
                        active_receipt_id=candidate_id,
                        rollback_receipt_id=None,
                    )
                    prune_receipts(receipts, {candidate_id})
                    return {
                        "schema_version": RECEIPT_SCHEMA,
                        "status": "adopted_current",
                        "source_commit": commit,
                        "package_version": version,
                        "binary_path": str(active),
                        "binary_sha256": active_digest,
                        "rollback_available": False,
                    }
                active_version = run_candidate_version(active)
                prefix = "forge-core "
                if not active_version.startswith(prefix):
                    raise InstallError("unmanaged installed binary has an invalid version response")
                active_receipt = receipt_for(
                    commit=None,
                    version=active_version[len(prefix) :],
                    digest=active_digest,
                    size=active.stat().st_size,
                    name=active.name,
                    provenance="adopted_unmanaged_binary",
                )
                active_receipt_id, _ = write_receipt(receipts, active_receipt)
            if active_digest == candidate_digest and active_receipt.get("source_commit") == commit:
                if not isinstance(active_receipt_id, str):
                    raise InstallError("installed checkpoint is missing its receipt identity")
                retained = {active_receipt_id}
                rollback_path = rollback_dir / name
                rollback_available = rollback_path.exists()
                if rollback_available:
                    rollback_file = require_regular_file(rollback_path, "rollback binary")
                    rollback_digest = sha256(rollback_file)
                    rollback_receipt_id = (
                        state.get("rollback_receipt_id") if state is not None else None
                    )
                    rollback_receipt = (
                        load_receipt(receipts, rollback_receipt_id)
                        if isinstance(rollback_receipt_id, str)
                        else None
                    )
                    if rollback_receipt is None or rollback_receipt.get("binary_sha256") != rollback_digest:
                        raise InstallError("rollback binary has no matching source-install receipt")
                    retained.add(rollback_receipt_id)
                prune_receipts(receipts, retained)
                return {
                    "schema_version": RECEIPT_SCHEMA,
                    "status": "already_installed",
                    "source_commit": commit,
                    "package_version": version,
                    "binary_path": str(active),
                    "binary_sha256": active_digest,
                    "rollback_available": rollback_available,
                }
        else:
            active_digest = None

        staged_candidate = staging / f"candidate-{os.getpid()}-{time.time_ns()}"
        staged_rollback = staging / f"rollback-{os.getpid()}-{time.time_ns()}"
        pending_path = source_root / "pending.json"
        candidate_id: str | None = None
        try:
            copy_synced(candidate, staged_candidate)
            if sha256(staged_candidate) != candidate_digest:
                raise InstallError("staged candidate digest changed during copy")

            if active is not None and active_digest is not None:
                rollback_path = rollback_dir / name
                if rollback_path.exists():
                    require_regular_file(rollback_path, "rollback binary")
                copy_synced(active, staged_rollback)
                if sha256(staged_rollback) != active_digest:
                    raise InstallError("staged rollback digest changed during copy")

            candidate_id, _ = write_receipt(receipts, candidate_receipt)
            atomic_json(
                pending_path,
                {
                    "schema_version": PENDING_SCHEMA,
                    "candidate_receipt_id": candidate_id,
                    "prior_active_receipt_id": active_receipt_id,
                    "binary_name": name,
                    "staged_rollback_name": staged_rollback.name if active is not None else None,
                },
            )

            destination = bin_dir / name
            if os.environ.get("FORGE_SOURCE_INSTALL_TEST_FAIL_ACTIVE_REPLACE") == "1":
                raise OSError("injected active replacement failure")
            os.replace(staged_candidate, destination)
            if sha256(destination) != candidate_digest:
                raise InstallError("installed binary digest differs after atomic replacement")

            rollback_receipt_id = active_receipt_id if active is not None else None
            rollback_path = rollback_dir / name
            if active is not None and active_digest is not None:
                if os.environ.get("FORGE_SOURCE_INSTALL_TEST_FAIL_ROLLBACK_REPLACE") == "1":
                    raise OSError("injected rollback replacement failure")
                os.replace(staged_rollback, rollback_path)

            write_state(
                source_root,
                active_receipt_id=candidate_id,
                rollback_receipt_id=rollback_receipt_id,
            )
            retained = {candidate_id}
            if isinstance(rollback_receipt_id, str):
                retained.add(rollback_receipt_id)
            prune_receipts(receipts, retained)
            pending_path.unlink()
            return {
                "schema_version": RECEIPT_SCHEMA,
                "status": "installed",
                "source_commit": commit,
                "package_version": version,
                "binary_path": str(destination),
                "binary_sha256": candidate_digest,
                "rollback_available": active_digest is not None,
            }
        except Exception:
            recover_pending(source_root, bin_dir, rollback_dir, receipts, staging)
            raise
        finally:
            if not pending_path.exists():
                for temporary in (staged_candidate, staged_rollback):
                    if temporary.exists() and temporary.is_file() and not temporary.is_symlink():
                        temporary.unlink()


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    subcommands = result.add_subparsers(dest="command", required=True)
    install_command = subcommands.add_parser("install", help="build and install one clean checkpoint")
    install_command.add_argument("--repo-root", type=Path, default=Path.cwd())
    install_command.add_argument("--install-root", type=Path, default=default_install_root())
    install_command.add_argument("--target-dir", type=Path)
    install_command.add_argument("--adopt-current", action="store_true")
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        if args.command == "install":
            report = install(args)
        else:
            raise InstallError(f"unsupported command: {args.command}")
    except (InstallError, OSError) as error:
        if os.environ.get("FORGE_SOURCE_INSTALL_DEBUG") == "1":
            raise
        print(f"source checkpoint install rejected: {error}", file=sys.stderr)
        return 2
    print(json.dumps(report, indent=2, sort_keys=True, ensure_ascii=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

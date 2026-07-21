#!/usr/bin/env python3
"""Build or verify the deterministic, closed Forge release-set manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tempfile


FORMAT = "forge-release-set-manifest-v1"
MANIFEST_NAME = "forge-core-release-set.json"
CHECKSUM_NAME = f"{MANIFEST_NAME}.sha256"
CANONICAL_ARCHIVES = (
    "forge-core-x86_64-linux.tar.gz",
    "forge-core-aarch64-linux.tar.gz",
    "forge-core-x86_64-macos.tar.gz",
    "forge-core-aarch64-macos.tar.gz",
    "forge-core-x86_64-windows.zip",
)
SHA256 = re.compile(r"[0-9a-f]{64}")
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?")
COMMIT = re.compile(r"[0-9a-f]{40}")


class ReleaseSetError(RuntimeError):
    """Release-set authority is incomplete, ambiguous, or inconsistent."""


def _read_regular(path: Path, label: str) -> bytes:
    try:
        before = path.lstat()
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise ReleaseSetError(f"{label} is not a safe regular file: {path}")
        with path.open("rb") as stream:
            data = stream.read()
            after = os.fstat(stream.fileno())
    except OSError as error:
        raise ReleaseSetError(f"cannot read {label} {path}: {error}") from error
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    if identity(before) != identity(after):
        raise ReleaseSetError(f"{label} changed while read: {path}")
    if not data:
        raise ReleaseSetError(f"{label} is empty: {path}")
    return data


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _sbom_name(version: str) -> str:
    return f"forge-core-{version}.cdx.json"


def _validate_identity(version: str, tag: str, source_commit: str) -> None:
    if VERSION.fullmatch(version) is None:
        raise ReleaseSetError(f"version is not exact SemVer: {version!r}")
    if tag != f"v{version}":
        raise ReleaseSetError(f"tag {tag!r} does not bind version {version!r}")
    if COMMIT.fullmatch(source_commit) is None:
        raise ReleaseSetError("source commit must be exactly 40 lowercase hexadecimal characters")


def _expected_assets(version: str, *, final: bool) -> set[str]:
    archives = set(CANONICAL_ARCHIVES)
    expected = archives | {f"{name}.sha256" for name in archives} | {
        f"{name}.sigstore" for name in archives
    }
    expected.add(_sbom_name(version))
    if final:
        expected.update({MANIFEST_NAME, CHECKSUM_NAME})
    return expected


def _require_closed_assets(root: Path, version: str, *, final: bool) -> None:
    try:
        actual = {path.name for path in root.iterdir()}
    except OSError as error:
        raise ReleaseSetError(f"cannot enumerate release asset directory {root}: {error}") from error
    expected = _expected_assets(version, final=final)
    if actual != expected:
        raise ReleaseSetError(
            "release asset set mismatch; "
            f"missing={sorted(expected - actual)!r}, unexpected={sorted(actual - expected)!r}"
        )
    for name in sorted(expected):
        _read_regular(root / name, "release asset")


def build_document(
    root: Path,
    *,
    version: str,
    tag: str,
    source_commit: str,
    archives: tuple[str, ...],
) -> dict[str, object]:
    _validate_identity(version, tag, source_commit)
    if archives != CANONICAL_ARCHIVES:
        raise ReleaseSetError(
            "archives must be the exact ordered canonical release set; "
            f"expected={CANONICAL_ARCHIVES!r}, actual={archives!r}"
        )
    _require_closed_assets(root, version, final=False)
    return {
        "format": FORMAT,
        "version": version,
        "tag": tag,
        "source_commit": source_commit,
        "archives": [
            {"filename": name, "sha256": _digest(_read_regular(root / name, "archive"))}
            for name in archives
        ],
        "sbom": {
            "filename": _sbom_name(version),
            "sha256": _digest(_read_regular(root / _sbom_name(version), "CycloneDX SBOM")),
        },
    }


def encode_document(document: dict[str, object]) -> bytes:
    return (json.dumps(document, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def _atomic_write(path: Path, data: bytes) -> None:
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary_path = Path(temporary)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary_path, path)
    finally:
        try:
            temporary_path.unlink()
        except FileNotFoundError:
            pass


def build(
    root: Path,
    *,
    version: str,
    tag: str,
    source_commit: str,
    archives: tuple[str, ...],
) -> None:
    document_bytes = encode_document(
        build_document(
            root,
            version=version,
            tag=tag,
            source_commit=source_commit,
            archives=archives,
        )
    )
    checksum_bytes = f"{_digest(document_bytes)}  {MANIFEST_NAME}\n".encode("ascii")
    _atomic_write(root / MANIFEST_NAME, document_bytes)
    _atomic_write(root / CHECKSUM_NAME, checksum_bytes)
    verify(
        root,
        version=version,
        tag=tag,
        source_commit=source_commit,
        archives=archives,
    )


def verify(
    root: Path,
    *,
    version: str,
    tag: str,
    source_commit: str,
    archives: tuple[str, ...],
) -> None:
    _validate_identity(version, tag, source_commit)
    if archives != CANONICAL_ARCHIVES:
        raise ReleaseSetError("manifest verification requires the exact ordered canonical archives")
    _require_closed_assets(root, version, final=True)
    manifest_bytes = _read_regular(root / MANIFEST_NAME, "release-set manifest")
    checksum_bytes = _read_regular(root / CHECKSUM_NAME, "release-set checksum")
    expected_checksum = f"{_digest(manifest_bytes)}  {MANIFEST_NAME}\n".encode("ascii")
    if checksum_bytes != expected_checksum:
        raise ReleaseSetError("release-set checksum sidecar does not exactly bind the manifest")
    try:
        document = json.loads(manifest_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseSetError(f"release-set manifest is not valid UTF-8 JSON: {error}") from error
    expected = build_document_from_final_assets(
        root,
        version=version,
        tag=tag,
        source_commit=source_commit,
        archives=archives,
    )
    if document != expected:
        raise ReleaseSetError("release-set manifest metadata or member authority does not match assets")
    if manifest_bytes != encode_document(expected):
        raise ReleaseSetError("release-set manifest JSON is not in deterministic canonical form")
    for member in document["archives"]:
        if SHA256.fullmatch(member["sha256"]) is None:
            raise ReleaseSetError("archive digest is not lowercase SHA-256")
    if SHA256.fullmatch(document["sbom"]["sha256"]) is None:
        raise ReleaseSetError("SBOM digest is not lowercase SHA-256")


def build_document_from_final_assets(
    root: Path,
    *,
    version: str,
    tag: str,
    source_commit: str,
    archives: tuple[str, ...],
) -> dict[str, object]:
    """Recompute authority after the manifest files have joined the closed set."""
    _validate_identity(version, tag, source_commit)
    if archives != CANONICAL_ARCHIVES:
        raise ReleaseSetError("archives are not the exact ordered canonical release set")
    return {
        "format": FORMAT,
        "version": version,
        "tag": tag,
        "source_commit": source_commit,
        "archives": [
            {"filename": name, "sha256": _digest(_read_regular(root / name, "archive"))}
            for name in archives
        ],
        "sbom": {
            "filename": _sbom_name(version),
            "sha256": _digest(_read_regular(root / _sbom_name(version), "CycloneDX SBOM")),
        },
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("operation", choices=("build", "verify"))
    result.add_argument("--assets-dir", type=Path, required=True)
    result.add_argument("--version", required=True)
    result.add_argument("--tag", required=True)
    result.add_argument("--source-commit", required=True)
    result.add_argument("--archive", action="append", required=True, dest="archives")
    return result


def main() -> int:
    args = parser().parse_args()
    operation = build if args.operation == "build" else verify
    operation(
        args.assets_dir,
        version=args.version,
        tag=args.tag,
        source_commit=args.source_commit,
        archives=tuple(args.archives),
    )
    print(f"{args.operation}ed deterministic release set at {args.assets_dir / MANIFEST_NAME}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ReleaseSetError) as error:
        raise SystemExit(f"release-set manifest failed: {error}") from error

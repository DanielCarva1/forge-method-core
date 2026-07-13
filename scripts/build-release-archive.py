#!/usr/bin/env python3
"""Build a deterministic Forge release archive and embedded payload manifest."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import tarfile
import zipfile


MANIFEST_NAME = "RELEASE-MANIFEST.json"
SCHEMA_VERSION = "forge_release_manifest_v1"
SEMVER = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:[-+][0-9A-Za-z.-]+)?$")


class ReleaseArchiveError(ValueError):
    """The requested archive cannot be assembled safely."""


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def normalized_archive_path(value: str) -> str:
    if "\\" in value or re.match(r"^[A-Za-z]:", value):
        raise ReleaseArchiveError(f"archive path must use canonical POSIX syntax: {value!r}")
    raw_parts = value.split("/")
    candidate = PurePosixPath(value)
    if candidate.is_absolute() or not candidate.parts:
        raise ReleaseArchiveError(f"archive path must be relative: {value!r}")
    if any(part in {"", ".", ".."} for part in raw_parts):
        raise ReleaseArchiveError(f"archive path contains traversal: {value!r}")
    normalized = candidate.as_posix()
    if normalized != value:
        raise ReleaseArchiveError(f"archive path is not canonical: {value!r}")
    return normalized


def load_payload_paths(repo_root: Path, payload_manifest: Path) -> list[tuple[str, bytes]]:
    try:
        lines = payload_manifest.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise ReleaseArchiveError(f"read payload manifest {payload_manifest}: {error}") from error

    entries: list[tuple[str, bytes]] = []
    seen: set[str] = set()
    for line_number, raw_line in enumerate(lines, start=1):
        value = raw_line.strip()
        if not value or value.startswith("#"):
            continue
        archive_path = normalized_archive_path(value)
        if archive_path in seen:
            raise ReleaseArchiveError(
                f"duplicate payload path {archive_path!r} at {payload_manifest}:{line_number}"
            )
        source = repo_root.joinpath(*PurePosixPath(archive_path).parts)
        if source.is_symlink() or not source.is_file():
            raise ReleaseArchiveError(
                f"payload path must be an existing regular non-symlink file: {source}"
            )
        try:
            source.resolve(strict=True).relative_to(repo_root.resolve(strict=True))
        except (OSError, ValueError) as error:
            raise ReleaseArchiveError(f"payload escapes repository root: {source}") from error
        entries.append((archive_path, source.read_bytes()))
        seen.add(archive_path)
    if not entries:
        raise ReleaseArchiveError("release payload manifest must not be empty")
    return entries


def release_entries(args: argparse.Namespace) -> tuple[list[tuple[str, bytes, int]], bytes]:
    repo_root = args.repo_root.resolve(strict=True)
    if args.binary.is_symlink() or not args.binary.is_file():
        raise ReleaseArchiveError(f"binary must be a regular non-symlink file: {args.binary}")
    if args.wrapper.is_symlink() or not args.wrapper.is_file():
        raise ReleaseArchiveError(f"wrapper must be a regular non-symlink file: {args.wrapper}")
    binary = args.binary.resolve(strict=True)
    wrapper = args.wrapper.resolve(strict=True)

    binary_name = normalized_archive_path(args.binary_name)
    wrapper_name = normalized_archive_path(args.wrapper_name)
    reserved = {binary_name, wrapper_name, MANIFEST_NAME}
    if len(reserved) != 3:
        raise ReleaseArchiveError("binary, wrapper, and manifest paths must be distinct")

    entries: list[tuple[str, bytes, int]] = [
        (binary_name, binary.read_bytes(), 0o755),
        (wrapper_name, wrapper.read_bytes(), 0o755),
    ]
    payload_manifest = args.payload_manifest
    if not payload_manifest.is_absolute():
        payload_manifest = repo_root / payload_manifest
    for path, content in load_payload_paths(repo_root, payload_manifest):
        if path in reserved:
            raise ReleaseArchiveError(f"static payload collides with reserved archive path: {path}")
        entries.append((path, content, 0o644))

    entries.sort(key=lambda item: item[0])
    file_rows = [
        {
            "path": path,
            "sha256": sha256(content),
            "size": len(content),
            "mode": f"{mode:04o}",
        }
        for path, content, mode in entries
    ]
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "product": "forge-method-core",
        "version": args.version,
        "source_date_epoch": args.source_date_epoch,
        "coverage": f"all archive members except {MANIFEST_NAME}",
        "files": file_rows,
    }
    manifest_bytes = (
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=True).encode("utf-8") + b"\n"
    )
    return entries, manifest_bytes


def tar_info(path: str, content: bytes, mode: int, epoch: int) -> tarfile.TarInfo:
    info = tarfile.TarInfo(path)
    info.size = len(content)
    info.mode = mode
    info.mtime = epoch
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "root"
    info.type = tarfile.REGTYPE
    return info


def build_tar_gz(
    archive: Path, entries: list[tuple[str, bytes, int]], manifest: bytes, epoch: int
) -> None:
    with archive.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=epoch, compresslevel=9) as zipped:
            with tarfile.open(fileobj=zipped, mode="w", format=tarfile.PAX_FORMAT) as tar:
                for path, content, mode in [*entries, (MANIFEST_NAME, manifest, 0o644)]:
                    tar.addfile(tar_info(path, content, mode, epoch), io.BytesIO(content))


def zip_datetime(epoch: int) -> tuple[int, int, int, int, int, int]:
    # ZIP timestamps have no timezone and cannot represent dates before 1980.
    import datetime

    value = datetime.datetime.fromtimestamp(max(epoch, 315532800), datetime.UTC)
    return (value.year, value.month, value.day, value.hour, value.minute, value.second)


def build_zip(
    archive: Path, entries: list[tuple[str, bytes, int]], manifest: bytes, epoch: int
) -> None:
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zipped:
        for path, content, mode in [*entries, (MANIFEST_NAME, manifest, 0o644)]:
            info = zipfile.ZipInfo(path, date_time=zip_datetime(epoch))
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | mode) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            zipped.writestr(info, content, compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def write_checksum(archive: Path) -> None:
    checksum = sha256(archive.read_bytes())
    archive.with_name(f"{archive.name}.sha256").write_text(
        f"{checksum}  {archive.name}\n", encoding="ascii", newline="\n"
    )


def build(args: argparse.Namespace) -> None:
    if not SEMVER.fullmatch(args.version):
        raise ReleaseArchiveError(f"version must be SemVer without a v prefix: {args.version!r}")
    if args.source_date_epoch < 0:
        raise ReleaseArchiveError("source-date-epoch must be non-negative")
    args.archive.parent.mkdir(parents=True, exist_ok=True)
    entries, manifest = release_entries(args)
    archive_name = args.archive.name.lower()
    if archive_name.endswith(".tar.gz"):
        build_tar_gz(args.archive, entries, manifest, args.source_date_epoch)
    elif archive_name.endswith(".zip"):
        build_zip(args.archive, entries, manifest, args.source_date_epoch)
    else:
        raise ReleaseArchiveError("archive must end in .tar.gz or .zip")
    write_checksum(args.archive)
    print(f"built {args.archive} with {len(entries) + 1} members")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--repo-root", type=Path, default=Path.cwd())
    result.add_argument("--payload-manifest", type=Path, default=Path("distribution/release-payload.txt"))
    result.add_argument("--binary", type=Path, required=True)
    result.add_argument("--binary-name", required=True)
    result.add_argument("--wrapper", type=Path, required=True)
    result.add_argument("--wrapper-name", required=True)
    result.add_argument("--archive", type=Path, required=True)
    result.add_argument("--version", required=True)
    result.add_argument("--source-date-epoch", type=int, required=True)
    return result


if __name__ == "__main__":
    try:
        build(parser().parse_args())
    except (OSError, ReleaseArchiveError) as error:
        raise SystemExit(f"release archive rejected: {error}") from error

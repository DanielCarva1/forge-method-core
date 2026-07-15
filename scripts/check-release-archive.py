#!/usr/bin/env python3
"""Fail closed unless a Forge release archive exactly matches its manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import posixpath
from pathlib import Path, PurePosixPath
import re
import tarfile
from urllib.parse import unquote, urlsplit
import zipfile


MANIFEST_NAME = "RELEASE-MANIFEST.json"
SCHEMA_VERSION = "forge_release_manifest_v1"
SHA256 = re.compile(r"^[0-9a-f]{64}$")
SOURCE_COMMIT = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
MARKDOWN_LINK = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")


class ArchiveCheckError(ValueError):
    """The archive is missing, malformed, or does not match its manifest."""


def normalized(path: str) -> str:
    if "\\" in path or re.match(r"^[A-Za-z]:", path):
        raise ArchiveCheckError(f"archive member must use canonical POSIX syntax: {path!r}")
    raw_parts = path.split("/")
    candidate = PurePosixPath(path)
    if candidate.is_absolute() or not candidate.parts:
        raise ArchiveCheckError(f"archive member must be relative: {path!r}")
    if any(part in {"", ".", ".."} for part in raw_parts):
        raise ArchiveCheckError(f"archive member contains traversal: {path!r}")
    normalized_path = candidate.as_posix()
    if normalized_path != path:
        raise ArchiveCheckError(f"archive member is not canonical: {path!r}")
    return normalized_path


def read_members(archive: Path) -> dict[str, tuple[bytes, int]]:
    members: dict[str, tuple[bytes, int]] = {}
    lower = archive.name.lower()
    if lower.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as opened:
            for member in opened.getmembers():
                path = normalized(member.name)
                if path in members:
                    raise ArchiveCheckError(f"duplicate archive member: {path}")
                if not member.isfile():
                    raise ArchiveCheckError(f"archive member is not a regular file: {path}")
                stream = opened.extractfile(member)
                if stream is None:
                    raise ArchiveCheckError(f"cannot read archive member: {path}")
                members[path] = (stream.read(), member.mode & 0o7777)
    elif lower.endswith(".zip"):
        with zipfile.ZipFile(archive, "r") as opened:
            for member in opened.infolist():
                path = normalized(member.filename)
                if path in members:
                    raise ArchiveCheckError(f"duplicate archive member: {path}")
                mode = (member.external_attr >> 16) & 0o7777
                file_type = (member.external_attr >> 16) & 0o170000
                if member.is_dir() or file_type not in {0, 0o100000}:
                    raise ArchiveCheckError(f"archive member is not a regular file: {path}")
                members[path] = (opened.read(member), mode)
    else:
        raise ArchiveCheckError("archive must end in .tar.gz or .zip")
    return members


def expected_static_paths(payload_manifest: Path) -> set[str]:
    result: set[str] = set()
    for line_number, raw_line in enumerate(
        payload_manifest.read_text(encoding="utf-8").splitlines(), start=1
    ):
        value = raw_line.strip()
        if not value or value.startswith("#"):
            continue
        path = normalized(value)
        if path in result:
            raise ArchiveCheckError(
                f"duplicate payload path {path!r} at {payload_manifest}:{line_number}"
            )
        result.add(path)
    if not result:
        raise ArchiveCheckError("release payload manifest must not be empty")
    return result


def check_markdown_link_closure(members: dict[str, tuple[bytes, int]]) -> None:
    """Require every relative Markdown link to resolve inside the archive."""

    member_paths = set(members)
    for source, (raw_content, _) in members.items():
        if not source.lower().endswith(".md"):
            continue
        try:
            content = raw_content.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ArchiveCheckError(f"Markdown member is not UTF-8: {source}") from error
        for match in MARKDOWN_LINK.finditer(content):
            destination = match.group(1).strip()
            if destination.startswith("<") and ">" in destination:
                destination = destination[1 : destination.index(">")]
            else:
                # Markdown permits an optional title after the destination.
                destination = destination.split(maxsplit=1)[0]
            if not destination or destination.startswith("#"):
                continue
            parsed = urlsplit(destination)
            if parsed.scheme or parsed.netloc:
                continue
            link_path = unquote(parsed.path).replace("\\", "/")
            if not link_path:
                continue
            if link_path.startswith("/"):
                resolved = posixpath.normpath(link_path.lstrip("/"))
            else:
                resolved = posixpath.normpath(
                    posixpath.join(PurePosixPath(source).parent.as_posix(), link_path)
                )
            try:
                resolved = normalized(resolved)
            except ArchiveCheckError as error:
                raise ArchiveCheckError(
                    f"unsafe local Markdown link in {source}: {destination!r}"
                ) from error
            if resolved not in member_paths:
                raise ArchiveCheckError(
                    f"broken local Markdown link in {source}: {destination!r} "
                    f"resolves to absent member {resolved!r}"
                )


def check(args: argparse.Namespace) -> None:
    if not args.archive.is_file():
        raise ArchiveCheckError(f"archive does not exist: {args.archive}")
    members = read_members(args.archive)
    manifest_member = members.get(MANIFEST_NAME)
    if manifest_member is None:
        raise ArchiveCheckError(f"archive is missing {MANIFEST_NAME}")
    try:
        manifest = json.loads(manifest_member[0])
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ArchiveCheckError(f"invalid {MANIFEST_NAME}: {error}") from error
    if not isinstance(manifest, dict):
        raise ArchiveCheckError("release manifest must be a JSON object")
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise ArchiveCheckError("unsupported release manifest schema")
    if manifest.get("product") != "forge-method-core":
        raise ArchiveCheckError("release manifest product mismatch")
    if manifest.get("version") != args.version:
        raise ArchiveCheckError(
            f"release manifest version {manifest.get('version')!r} != {args.version!r}"
        )

    release_tag = manifest.get("release_tag")
    if release_tag != f"v{args.version}":
        raise ArchiveCheckError(
            f"release manifest tag {release_tag!r} does not bind version v{args.version}"
        )
    source_commit = manifest.get("source_commit")
    if not isinstance(source_commit, str) or not SOURCE_COMMIT.fullmatch(source_commit):
        raise ArchiveCheckError("release manifest source_commit is not a full Git object id")
    expected_release_tag = getattr(args, "expected_release_tag", None)
    expected_source_commit = getattr(args, "expected_source_commit", None)
    if expected_release_tag is not None and release_tag != expected_release_tag:
        raise ArchiveCheckError(
            f"release manifest tag {release_tag!r} != expected {expected_release_tag!r}"
        )
    if expected_source_commit is not None and source_commit != expected_source_commit:
        raise ArchiveCheckError(
            f"release manifest source_commit {source_commit!r} "
            f"!= expected {expected_source_commit!r}"
        )

    rows = manifest.get("files")
    if not isinstance(rows, list) or not rows:
        raise ArchiveCheckError("release manifest files must be a non-empty array")

    expected_paths = expected_static_paths(args.payload_manifest)
    expected_paths.update({normalized(args.binary_name), normalized(args.wrapper_name)})
    manifest_paths: set[str] = set()
    for row in rows:
        if not isinstance(row, dict):
            raise ArchiveCheckError("release manifest file row must be an object")
        path = normalized(str(row.get("path", "")))
        if path in manifest_paths:
            raise ArchiveCheckError(f"duplicate release manifest path: {path}")
        manifest_paths.add(path)
        member = members.get(path)
        if member is None:
            raise ArchiveCheckError(f"manifested file is absent from archive: {path}")
        digest = hashlib.sha256(member[0]).hexdigest()
        if not SHA256.fullmatch(str(row.get("sha256", ""))) or row["sha256"] != digest:
            raise ArchiveCheckError(f"sha256 mismatch for {path}")
        if row.get("size") != len(member[0]):
            raise ArchiveCheckError(f"size mismatch for {path}")
        expected_mode = f"{member[1]:04o}"
        if row.get("mode") != expected_mode:
            raise ArchiveCheckError(
                f"mode mismatch for {path}: manifest={row.get('mode')!r} archive={expected_mode}"
            )

    if manifest_paths != expected_paths:
        missing = sorted(expected_paths - manifest_paths)
        unexpected = sorted(manifest_paths - expected_paths)
        raise ArchiveCheckError(
            f"manifest payload mismatch; missing={missing}, unexpected={unexpected}"
        )
    expected_members = expected_paths | {MANIFEST_NAME}
    if set(members) != expected_members:
        raise ArchiveCheckError(
            "archive contains unmanifested or missing members; "
            f"missing={sorted(expected_members - set(members))}, "
            f"unexpected={sorted(set(members) - expected_members)}"
        )
    check_markdown_link_closure(members)

    checksum_path = args.archive.with_name(f"{args.archive.name}.sha256")
    if args.require_checksum:
        try:
            checksum_line = checksum_path.read_text(encoding="ascii").strip()
        except OSError as error:
            raise ArchiveCheckError(f"read checksum {checksum_path}: {error}") from error
        expected_checksum = f"{hashlib.sha256(args.archive.read_bytes()).hexdigest()}  {args.archive.name}"
        if checksum_line != expected_checksum:
            raise ArchiveCheckError(f"checksum sidecar mismatch for {args.archive.name}")
    print(
        f"verified {args.archive}: {len(members)} exact members, "
        f"version {args.version}, tag {release_tag}, commit {source_commit}"
    )


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--archive", type=Path, required=True)
    result.add_argument("--binary-name", required=True)
    result.add_argument("--wrapper-name", required=True)
    result.add_argument("--version", required=True)
    result.add_argument("--expected-release-tag")
    result.add_argument("--expected-source-commit")
    result.add_argument("--payload-manifest", type=Path, default=Path("distribution/release-payload.txt"))
    result.add_argument("--require-checksum", action="store_true")
    return result


if __name__ == "__main__":
    try:
        check(parser().parse_args())
    except (OSError, ArchiveCheckError) as error:
        raise SystemExit(f"release archive verification failed: {error}") from error

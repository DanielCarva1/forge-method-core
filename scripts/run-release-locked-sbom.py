#!/usr/bin/env python3
"""Run cargo-cyclonedx against immutable HEAD with locked Cargo metadata."""

from __future__ import annotations

import argparse
import hashlib
import io
import os
from pathlib import Path, PurePosixPath
import secrets
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile


SHIM_MARKER = "FORGE_RELEASE_LOCKED_CARGO_SHIM"
REAL_CARGO = "FORGE_RELEASE_REAL_CARGO"


def locked_metadata_command(real_cargo: str, arguments: list[str]) -> list[str]:
    """Allow only cargo-cyclonedx's metadata query and inject lock enforcement."""
    if not arguments or arguments[0] != "metadata":
        raise ValueError(
            "release Cargo shim permits only cargo metadata, got " + repr(arguments)
        )
    return [real_cargo, "metadata", "--locked", *arguments[1:]]


def cargo_shim() -> int:
    real_cargo = os.environ.get(REAL_CARGO)
    if not real_cargo:
        raise RuntimeError(f"{REAL_CARGO} is required in Cargo shim mode")
    os.execv(real_cargo, locked_metadata_command(real_cargo, sys.argv[1:]))


def _identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_nlink,
        metadata.st_size, metadata.st_mtime_ns, metadata.st_ctime_ns,
    )


def _require_linux() -> None:
    if not hasattr(os, "O_NOFOLLOW") or not Path("/proc/self/fd").is_dir():
        raise RuntimeError(
            "race-free release SBOM staging requires Linux O_NOFOLLOW and /proc/self/fd"
        )


def _safe_parts(relative: str | Path, label: str) -> tuple[str, ...]:
    text = relative.as_posix() if isinstance(relative, Path) else relative
    pure = PurePosixPath(text)
    if (
        pure.is_absolute() or not pure.parts or "\\" in text
        or pure.as_posix() != text
        or any(part in {"", ".", ".."} for part in pure.parts)
    ):
        raise RuntimeError(f"{label} has an unsafe relative path: {text!r}")
    return pure.parts


def _directory_parts(relative: Path, label: str) -> tuple[str, ...]:
    if relative.as_posix() in {"", "."}:
        return ()
    return _safe_parts(relative, label)


def _relative_lexical(path: Path, root: Path, label: str) -> Path:
    absolute = Path(os.path.abspath(path))
    try:
        relative = absolute.relative_to(root)
    except ValueError as error:
        raise RuntimeError(
            f"{label} must remain lexically inside checked repository {root}: {absolute}"
        ) from error
    if relative != Path("."):
        _safe_parts(relative, label)
    return relative


def _open_absolute_directory(path: Path, label: str) -> int:
    _require_linux()
    absolute = Path(os.path.abspath(path))
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in absolute.parts[1:]:
            if part in {"", ".", ".."}:
                raise RuntimeError(f"{label} has an unsafe path component: {absolute}")
            next_fd = os.open(
                part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd
            )
            os.close(fd)
            fd = next_fd
        return fd
    except BaseException:
        os.close(fd)
        raise


def _open_directory(root_fd: int, parts: tuple[str, ...], *, create: bool = False) -> int:
    fd = os.dup(root_fd)
    try:
        for part in parts:
            if create:
                try:
                    os.mkdir(part, mode=0o700, dir_fd=fd)
                except FileExistsError:
                    pass
            next_fd = os.open(
                part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd
            )
            os.close(fd)
            fd = next_fd
        return fd
    except BaseException:
        os.close(fd)
        raise


def _read_fd(fd: int, label: str) -> tuple[bytes, tuple[int, ...]]:
    before = os.fstat(fd)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise RuntimeError(f"{label} is not a safe unlinked regular file")
    os.lseek(fd, 0, os.SEEK_SET)
    chunks: list[bytes] = []
    while chunk := os.read(fd, 1024 * 1024):
        chunks.append(chunk)
    after = os.fstat(fd)
    if _identity(before) != _identity(after):
        raise RuntimeError(f"{label} changed while read")
    return b"".join(chunks), _identity(after)


def _open_regular(root_fd: int, relative: Path, label: str) -> tuple[int, bytes, tuple[int, ...]]:
    parts = _safe_parts(relative, label)
    parent_fd = _open_directory(root_fd, parts[:-1])
    try:
        fd = os.open(
            parts[-1], os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_BINARY", 0),
            dir_fd=parent_fd,
        )
    finally:
        os.close(parent_fd)
    try:
        data, identity = _read_fd(fd, label)
        return fd, data, identity
    except BaseException:
        os.close(fd)
        raise


def _verify_regular(
    root_fd: int, relative: Path, retained_fd: int, expected: bytes,
    expected_identity: tuple[int, ...], label: str,
) -> None:
    data, identity = _read_fd(retained_fd, label)
    if identity != expected_identity or data != expected:
        raise RuntimeError(f"{label} descriptor identity or bytes changed")
    fresh_fd, fresh_data, fresh_identity = _open_regular(root_fd, relative, label)
    try:
        if fresh_identity != expected_identity or fresh_data != expected:
            raise RuntimeError(f"{label} path was replaced")
    finally:
        os.close(fresh_fd)


def _directory_identity(fd: int) -> tuple[int, int]:
    metadata = os.fstat(fd)
    if not stat.S_ISDIR(metadata.st_mode):
        raise RuntimeError("bound output parent is not a directory")
    return metadata.st_dev, metadata.st_ino


def _verify_directory(root_fd: int, parts: tuple[str, ...], expected: tuple[int, int], label: str) -> None:
    try:
        fresh = _open_directory(root_fd, parts)
    except OSError as error:
        raise RuntimeError(f"{label} path is missing or unsafe") from error
    try:
        if _directory_identity(fresh) != expected:
            raise RuntimeError(f"{label} path no longer names the retained directory")
    finally:
        os.close(fresh)


def _verify_repository_path(repository: Path, expected: tuple[int, int]) -> None:
    fresh = _open_absolute_directory(repository, "checked repository")
    try:
        if _directory_identity(fresh) != expected:
            raise RuntimeError("checked repository lexical path changed")
    finally:
        os.close(fresh)


def _argument(forwarded: list[str], option: str) -> tuple[int, str]:
    indexes = [index for index, item in enumerate(forwarded) if item == option]
    if len(indexes) != 1 or indexes[0] + 1 >= len(forwarded):
        raise RuntimeError(f"cargo-cyclonedx requires exactly one {option} value")
    return indexes[0] + 1, forwarded[indexes[0] + 1]


def _git(repo_fd: int, arguments: list[str]) -> bytes:
    completed = subprocess.run(
        ["git", *arguments], cwd=f"/proc/self/fd/{repo_fd}",
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(arguments)} failed: {completed.stderr.decode('utf-8', 'replace').strip()}"
        )
    return completed.stdout


def _git_head_snapshot(repo_fd: int) -> tuple[str, dict[str, tuple[bytes, int]]]:
    """Read an exact HEAD tree/archive and bind every staged byte to its blob OID."""
    commit = _git(repo_fd, ["rev-parse", "--verify", "HEAD^{commit}"]).decode("ascii").strip()
    object_format = _git(repo_fd, ["rev-parse", "--show-object-format"]).decode("ascii").strip()
    if object_format not in {"sha1", "sha256"}:
        raise RuntimeError(f"unsupported Git object format: {object_format!r}")
    expected: dict[str, tuple[str, int]] = {}
    for record in _git(repo_fd, ["ls-tree", "-rz", "--full-tree", "-r", commit]).split(b"\0"):
        if not record:
            continue
        try:
            header, raw_path = record.split(b"\t", 1)
            mode_raw, kind_raw, oid_raw = header.split(b" ", 2)
            path = raw_path.decode("utf-8", "strict")
            mode = int(mode_raw, 8)
            kind = kind_raw.decode("ascii")
            oid = oid_raw.decode("ascii")
        except (ValueError, UnicodeDecodeError) as error:
            raise RuntimeError("HEAD contains an unsafe or malformed tree entry") from error
        parts = _safe_parts(path, "Git tree entry")
        if parts[0] == ".git" or path in expected:
            raise RuntimeError(f"HEAD contains an unsafe Git tree path: {path!r}")
        if kind != "blob" or mode not in {0o100644, 0o100755}:
            raise RuntimeError(
                f"HEAD entry must be a regular non-symlink blob: {path!r} mode={mode_raw!r} kind={kind!r}"
            )
        expected[path] = (oid, mode)
    archive = _git(repo_fd, ["archive", "--format=tar", commit])
    actual: dict[str, tuple[bytes, int]] = {}
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as opened:
        for member in opened.getmembers():
            name = member.name.removesuffix("/")
            if member.isdir():
                if name:
                    _safe_parts(name, "Git archive directory")
                continue
            if not member.isreg() or member.islnk() or member.issym():
                raise RuntimeError(f"Git archive contains a non-regular member: {member.name!r}")
            _safe_parts(name, "Git archive member")
            if name in actual or name not in expected:
                raise RuntimeError(f"Git archive contains an unexpected/duplicate member: {name!r}")
            stream = opened.extractfile(member)
            if stream is None:
                raise RuntimeError(f"cannot read Git archive member: {name!r}")
            data = stream.read()
            oid, mode = expected[name]
            digest = hashlib.new(object_format)
            digest.update(f"blob {len(data)}\0".encode("ascii"))
            digest.update(data)
            if digest.hexdigest() != oid:
                raise RuntimeError(f"Git archive bytes do not match blob identity: {name!r}")
            actual[name] = (data, mode)
    if set(actual) != set(expected):
        raise RuntimeError("Git archive does not exactly cover the immutable HEAD tree")
    return commit, actual


def _write_exclusive(root_fd: int, relative: Path, data: bytes, mode: int) -> None:
    parts = _safe_parts(relative, "staged file")
    parent_fd = _open_directory(root_fd, parts[:-1], create=True)
    try:
        fd = os.open(
            parts[-1], os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            mode & 0o777, dir_fd=parent_fd,
        )
        try:
            view = memoryview(data)
            while view:
                written = os.write(fd, view)
                if written <= 0:
                    raise RuntimeError(f"short write staging {relative}")
                view = view[written:]
        finally:
            os.close(fd)
    finally:
        os.close(parent_fd)


def _stage_snapshot(
    stage_fd: int, files: dict[str, tuple[bytes, int]], lock_relative: Path, lock_bytes: bytes,
) -> None:
    lock_key = lock_relative.as_posix()
    committed = files.get(lock_key)
    if committed is None or committed[0] != lock_bytes:
        raise RuntimeError("retained Cargo.lock bytes differ from immutable HEAD Cargo.lock")
    for path, (data, mode) in sorted(files.items()):
        if path != lock_key:
            _write_exclusive(stage_fd, Path(path), data, mode)
    # The retained, committed-equal descriptor bytes are created as a fresh leaf;
    # no copied symlink or hardlink can be followed by cargo-cyclonedx.
    _write_exclusive(stage_fd, lock_relative, lock_bytes, committed[1])


def _unlink_relative(root_fd: int, relative: Path) -> None:
    parts = _safe_parts(relative, "staged output")
    parent_fd = _open_directory(root_fd, parts[:-1])
    try:
        try:
            os.unlink(parts[-1], dir_fd=parent_fd)
        except FileNotFoundError:
            pass
    finally:
        os.close(parent_fd)


def _read_output(root_fd: int, relative: Path) -> bytes:
    fd, data, _ = _open_regular(root_fd, relative, "cargo-cyclonedx output")
    os.close(fd)
    return data


def _publish_output(
    destination_fd: int, output_name: str, output_bytes: bytes,
    verify_bound_path,
) -> None:
    """Fsync and atomically replace relative to a retained destination dir fd."""
    temporary_name = f".forge-sbom-{secrets.token_hex(16)}.tmp"
    replaced = False
    try:
        verify_bound_path()
        temporary_fd = os.open(
            temporary_name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o600, dir_fd=destination_fd,
        )
        try:
            view = memoryview(output_bytes)
            while view:
                written = os.write(temporary_fd, view)
                if written <= 0:
                    raise RuntimeError("short write publishing CycloneDX SBOM")
                view = view[written:]
            os.fsync(temporary_fd)
        finally:
            os.close(temporary_fd)
        verify_bound_path()
        os.replace(
            temporary_name, output_name,
            src_dir_fd=destination_fd, dst_dir_fd=destination_fd,
        )
        replaced = True
        os.fsync(destination_fd)
        verify_bound_path()
    except BaseException:
        if replaced:
            try:
                os.unlink(output_name, dir_fd=destination_fd)
                os.fsync(destination_fd)
            except FileNotFoundError:
                pass
        raise
    finally:
        try:
            os.unlink(temporary_name, dir_fd=destination_fd)
        except FileNotFoundError:
            pass


def _make_stage() -> tuple[int, int, str]:
    parent = Path(os.path.abspath(tempfile.gettempdir()))
    parent_fd = _open_absolute_directory(parent, "trusted temporary parent")
    for _ in range(100):
        name = f"forge-release-sbom-{secrets.token_hex(16)}"
        try:
            os.mkdir(name, 0o700, dir_fd=parent_fd)
            stage_fd = os.open(
                name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd
            )
            return parent_fd, stage_fd, name
        except FileExistsError:
            continue
    os.close(parent_fd)
    raise RuntimeError("cannot allocate trusted release staging directory")


def _remove_tree(fd: int) -> None:
    for name in os.listdir(fd):
        metadata = os.stat(name, dir_fd=fd, follow_symlinks=False)
        if stat.S_ISDIR(metadata.st_mode):
            child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            try:
                _remove_tree(child)
            finally:
                os.close(child)
            os.rmdir(name, dir_fd=fd)
        else:
            os.unlink(name, dir_fd=fd)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lockfile", type=Path, required=True)
    parser.add_argument("cyclonedx_args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    forwarded = args.cyclonedx_args[1:] if args.cyclonedx_args[:1] == ["--"] else args.cyclonedx_args
    if not forwarded:
        parser.error("cargo-cyclonedx arguments are required after --")

    repository = Path(os.path.abspath(__file__)).parents[1]
    repo_fd = _open_absolute_directory(repository, "checked repository")
    repo_identity = _directory_identity(repo_fd)
    candidate = args.lockfile if args.lockfile.is_absolute() else Path.cwd() / args.lockfile
    lock_relative = _relative_lexical(candidate, repository, "release lockfile")
    if lock_relative.name != "Cargo.lock":
        raise RuntimeError("release lockfile must be a repository Cargo.lock")
    lock_fd, lock_bytes, lock_identity = _open_regular(
        repo_fd, lock_relative, "release Cargo.lock"
    )
    destination_fd = -1
    script_fd = -1
    stage_fd = -1
    temp_parent_fd = -1
    stage_name = ""
    cwd_fd = -1
    try:
        cargo = shutil.which("cargo")
        cyclonedx = shutil.which("cargo-cyclonedx")
        if cargo is None:
            raise RuntimeError("cargo is not available on PATH")
        if cyclonedx is None:
            raise RuntimeError("cargo-cyclonedx is not available on PATH")

        manifest_index, manifest_value = _argument(forwarded, "--manifest-path")
        output_index, output_value = _argument(forwarded, "--override-filename")
        if output_value in {"", ".", ".."} or Path(output_value).name != output_value or "\\" in output_value:
            raise RuntimeError("cargo-cyclonedx override filename must be a safe basename")
        manifest = Path(manifest_value)
        if not manifest.is_absolute():
            manifest = Path.cwd() / manifest
        manifest_relative = _relative_lexical(manifest, repository, "cargo-cyclonedx manifest")
        cwd_relative = _relative_lexical(Path.cwd(), repository, "cargo-cyclonedx working directory")
        output_name = output_value if output_value.endswith(".json") else output_value + ".json"
        output_relative = manifest_relative.parent / output_name

        destination_parts = _safe_parts(manifest_relative.parent, "SBOM destination parent")
        destination_fd = _open_directory(repo_fd, destination_parts)
        destination_identity = _directory_identity(destination_fd)
        script_fd, _, _ = _open_regular(
            repo_fd, Path("scripts/run-release-locked-sbom.py"), "release SBOM runner"
        )

        commit, files = _git_head_snapshot(repo_fd)
        if manifest_relative.as_posix() not in files:
            raise RuntimeError("cargo-cyclonedx manifest is not a regular file in immutable HEAD")
        temp_parent_fd, stage_fd, stage_name = _make_stage()
        _stage_snapshot(stage_fd, files, lock_relative, lock_bytes)
        _unlink_relative(stage_fd, output_relative)
        cwd_fd = _open_directory(
            stage_fd, _directory_parts(cwd_relative, "staged working directory")
        )

        pid = os.getpid()
        staged_manifest = f"/proc/{pid}/fd/{stage_fd}/{manifest_relative.as_posix()}"
        staged_forwarded = list(forwarded)
        staged_forwarded[manifest_index] = staged_manifest
        staged_forwarded[output_index] = output_value
        environment = os.environ.copy()
        environment["CARGO"] = f"/proc/{pid}/fd/{script_fd}"
        environment[SHIM_MARKER] = "1"
        environment[REAL_CARGO] = cargo
        completed = subprocess.run(
            [cyclonedx, "cyclonedx", *staged_forwarded],
            cwd=f"/proc/{pid}/fd/{cwd_fd}", env=environment, check=False,
        )
        _verify_regular(
            repo_fd, lock_relative, lock_fd, lock_bytes, lock_identity, "release Cargo.lock"
        )
        if _git(repo_fd, ["rev-parse", "--verify", "HEAD^{commit}"]).decode("ascii").strip() != commit:
            raise RuntimeError("repository HEAD identity changed during SBOM generation")
        if completed.returncode != 0:
            return completed.returncode
        output_bytes = _read_output(stage_fd, output_relative)

        def verify_destination() -> None:
            _verify_repository_path(repository, repo_identity)
            _verify_directory(
                repo_fd, destination_parts, destination_identity, "SBOM destination parent"
            )

        _publish_output(destination_fd, output_name, output_bytes, verify_destination)
        return 0
    finally:
        if stage_fd >= 0:
            try:
                _remove_tree(stage_fd)
            except OSError:
                pass
            try:
                os.close(stage_fd)
            except OSError:
                pass
        if temp_parent_fd >= 0 and stage_name:
            try:
                os.rmdir(stage_name, dir_fd=temp_parent_fd)
            except OSError:
                pass
        for fd in (cwd_fd, script_fd, destination_fd, lock_fd, repo_fd, temp_parent_fd):
            if fd >= 0:
                try:
                    os.close(fd)
                except OSError:
                    pass


if __name__ == "__main__":
    try:
        if os.environ.get(SHIM_MARKER) == "1":
            raise SystemExit(cargo_shim())
        raise SystemExit(main())
    except (OSError, RuntimeError, ValueError, tarfile.TarError) as error:
        raise SystemExit(f"locked release SBOM failed: {error}") from error

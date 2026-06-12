#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


PLUGIN_NAME = "forge-method-core"
USER_ENTRYPOINTS = {"start", "preflight", "guide", "resume", "reload"}
SKIP_VALUES = {"1", "true", "yes", "on"}
POLICIES = {"auto", "notify", "off"}


def eprint(message: str) -> None:
    print(message, file=sys.stderr)


def parse_semver(value: str) -> tuple[int, int, int]:
    parts = value.strip().split(".")
    if len(parts) != 3:
        return (0, 0, 0)
    try:
        return tuple(int(part.split("-", 1)[0]) for part in parts)  # type: ignore[return-value]
    except ValueError:
        return (0, 0, 0)


def is_newer_version(new: str, old: str) -> bool:
    return parse_semver(new) > parse_semver(old)


def should_run_for_args(args: list[str]) -> bool:
    if not args:
        return True
    first = args[0]
    return first in USER_ENTRYPOINTS


def read_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def write_json(path: Path, payload: dict[str, Any]) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    except OSError:
        pass


def find_repo_root(skill_dir: Path) -> Path:
    current = skill_dir.resolve()
    for candidate in [current, *current.parents]:
        if (candidate / ".codex-plugin" / "plugin.json").exists():
            return candidate
        if (candidate / ".codex-marketplace-install.json").exists():
            return candidate
    return current


def read_version(repo_root: Path) -> str:
    manifest = read_json(repo_root / ".codex-plugin" / "plugin.json")
    if manifest.get("version"):
        return str(manifest["version"])
    version_file = repo_root / "VERSION"
    if version_file.exists():
        try:
            return version_file.read_text(encoding="utf-8").strip()
        except OSError:
            return ""
    return ""


def file_hash(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return ""


def marketplace_metadata(repo_root: Path) -> dict[str, Any]:
    return read_json(repo_root / ".codex-marketplace-install.json")


def marketplace_name(repo_root: Path) -> str:
    return repo_root.name or PLUGIN_NAME


def is_git_marketplace(repo_root: Path) -> bool:
    metadata = marketplace_metadata(repo_root)
    return metadata.get("source_type") == "git" and bool(metadata.get("source"))


def state_path() -> Path:
    override = os.environ.get("FORGE_METHOD_UPDATE_STATE")
    if override:
        return Path(override).expanduser()
    return Path.home() / ".forge-method" / "update-state.json"


def load_state() -> dict[str, Any]:
    return read_json(state_path())


def save_state(payload: dict[str, Any]) -> None:
    write_json(state_path(), payload)


def effective_policy(repo_root: Path) -> str:
    raw = os.environ.get("FORGE_METHOD_UPDATE_POLICY", "").strip().lower()
    if raw:
        return raw if raw in POLICIES else "off"
    return "auto" if is_git_marketplace(repo_root) else "notify"


def command_from_env(name: str) -> list[str] | None:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return None
    return shlex.split(raw, posix=(os.name != "nt"))


def codex_command() -> list[str] | None:
    override = command_from_env("FORGE_METHOD_CODEX")
    if override:
        return override
    path = shutil.which("codex")
    return [path] if path else None


def run_with_timeout(command: list[str], timeout: float) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def update_timeout() -> float:
    raw = os.environ.get("FORGE_METHOD_UPDATE_TIMEOUT", "").strip()
    if not raw:
        return 12.0
    try:
        return max(1.0, float(raw))
    except ValueError:
        return 12.0


def read_release_notes(repo_root: Path, version: str) -> dict[str, Any]:
    latest = read_json(repo_root / "release-notes" / "latest.json")
    if latest.get("version") == version:
        return latest
    return {
        "version": version,
        "title": f"Forge Method {version}",
        "summary": "Forge Method was updated.",
        "highlights": [],
        "full_notes_url": "https://github.com/DanielCarva1/forge-method-core",
    }


def print_patch_notes(old_version: str, new_version: str, notes: dict[str, Any], *, skill_changed: bool) -> None:
    eprint(f"Forge Method updated: {old_version or '<unknown>'} -> {new_version}")
    summary = str(notes.get("summary") or "").strip()
    if summary:
        eprint(summary)
    highlights = notes.get("highlights") or []
    for item in [str(item).strip() for item in highlights if str(item).strip()][:4]:
        eprint(f"- {item}")
    url = str(notes.get("full_notes_url") or "").strip()
    if url:
        eprint(f"Full notes: {url}")
    if skill_changed:
        eprint(
            "Skill instructions changed in this update. This start continues with the updated runtime; "
            "open a new thread later only if you need the refreshed skill text fully loaded."
        )


def maybe_print_legacy_hint(repo_root: Path, current_version: str) -> None:
    if is_git_marketplace(repo_root):
        return
    state = load_state()
    key = f"legacy_hint:{current_version or 'unknown'}"
    if state.get("last_legacy_hint") == key:
        return
    eprint(
        "Forge Method automatic updates require the Git marketplace install. "
        "Recommended: codex plugin marketplace add DanielCarva1/forge-method-core --ref main"
    )
    state["last_legacy_hint"] = key
    save_state(state)


def notify_available(repo_root: Path, timeout: float) -> None:
    metadata = marketplace_metadata(repo_root)
    source = str(metadata.get("source") or "")
    ref_name = str(metadata.get("ref_name") or metadata.get("ref") or "main")
    current_revision = str(metadata.get("revision") or "")
    git_cmd = command_from_env("FORGE_METHOD_GIT") or ([shutil.which("git")] if shutil.which("git") else None)
    if not source or not git_cmd:
        eprint("Forge Method update check unavailable; continuing with the local runtime.")
        return
    try:
        result = run_with_timeout([*git_cmd, "ls-remote", source, ref_name], timeout)
    except (OSError, subprocess.TimeoutExpired):
        eprint("Forge Method update check timed out or failed; continuing with the local runtime.")
        return
    if result.returncode != 0:
        eprint("Forge Method update check failed; continuing with the local runtime.")
        return
    remote_revision = result.stdout.strip().split()[0] if result.stdout.strip() else ""
    if remote_revision and remote_revision != current_revision:
        eprint("Forge Method update available. Set FORGE_METHOD_UPDATE_POLICY=auto or run:")
        eprint(f"codex plugin marketplace upgrade {marketplace_name(repo_root)}")


def auto_update(repo_root: Path, timeout: float) -> None:
    command = codex_command()
    old_version = read_version(repo_root)
    old_metadata = marketplace_metadata(repo_root)
    old_revision = str(old_metadata.get("revision") or "")
    watched = [
        repo_root / "skills" / "forge-method" / "SKILL.md",
        repo_root / ".codex-plugin" / "plugin.json",
    ]
    old_hashes = {str(path): file_hash(path) for path in watched}
    if not command:
        eprint("Forge Method update skipped: Codex CLI not found. Continuing with the local runtime.")
        return
    try:
        result = run_with_timeout([*command, "plugin", "marketplace", "upgrade", marketplace_name(repo_root)], timeout)
    except subprocess.TimeoutExpired:
        eprint("Forge Method update timed out; continuing with the local runtime.")
        return
    except OSError:
        eprint("Forge Method update failed to start; continuing with the local runtime.")
        return
    if result.returncode != 0:
        eprint("Forge Method update failed; continuing with the local runtime.")
        return

    new_version = read_version(repo_root)
    new_metadata = marketplace_metadata(repo_root)
    new_revision = str(new_metadata.get("revision") or "")
    if not new_version:
        return
    updated = is_newer_version(new_version, old_version) or (bool(new_revision) and new_revision != old_revision)
    if not updated or not is_newer_version(new_version, old_version):
        return

    state = load_state()
    if state.get("last_announced_version") == new_version:
        return
    skill_changed = any(file_hash(path) != old_hashes.get(str(path), "") for path in watched)
    print_patch_notes(old_version, new_version, read_release_notes(repo_root, new_version), skill_changed=skill_changed)
    state["last_announced_version"] = new_version
    state["last_seen_revision"] = new_revision
    save_state(state)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Forge Method self-update helper")
    parser.add_argument("--skill-dir", required=True)
    parser.add_argument("runtime_args", nargs=argparse.REMAINDER)
    ns = parser.parse_args(argv)
    runtime_args = list(ns.runtime_args)
    if runtime_args and runtime_args[0] == "--":
        runtime_args = runtime_args[1:]
    if os.environ.get("FORGE_METHOD_SKIP_UPDATE", "").strip().lower() in SKIP_VALUES:
        return 0
    if not should_run_for_args(runtime_args):
        return 0

    repo_root = find_repo_root(Path(ns.skill_dir))
    policy = effective_policy(repo_root)
    if policy == "off":
        return 0
    if not is_git_marketplace(repo_root):
        maybe_print_legacy_hint(repo_root, read_version(repo_root))
        return 0
    timeout = update_timeout()
    if policy == "notify":
        notify_available(repo_root, timeout)
        return 0
    auto_update(repo_root, timeout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

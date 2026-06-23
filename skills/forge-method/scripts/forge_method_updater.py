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
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


PLUGIN_NAME = "forge-method-core"
MARKETPLACE_REPO = "DanielCarva1/forge-method-core"
MARKETPLACE_REF = "main"
REMOTE_RELEASE_NOTES_URL = "https://raw.githubusercontent.com/DanielCarva1/forge-method-core/main/release-notes/latest.json"
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
    manifest_version = str(manifest.get("version") or "").strip()
    version_file = repo_root / "VERSION"
    file_version = ""
    if version_file.exists():
        try:
            file_version = version_file.read_text(encoding="utf-8").strip()
        except OSError:
            file_version = ""
    if file_version and (not manifest_version or is_newer_version(file_version, manifest_version)):
        return file_version
    return manifest_version or file_version


def file_hash(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return ""


def marketplace_metadata(repo_root: Path) -> dict[str, Any]:
    return read_json(repo_root / ".codex-marketplace-install.json")


def marketplace_name(repo_root: Path) -> str:
    return repo_root.name or PLUGIN_NAME


def default_marketplace_path() -> Path:
    override = os.environ.get("FORGE_METHOD_MARKETPLACE_PATH", "").strip()
    if override:
        return Path(override).expanduser()
    return Path.home() / ".agents" / "plugins" / "marketplace.json"


def marketplace_root_for_path(path: Path) -> Path:
    full_path = path.expanduser().resolve()
    plugins_dir = full_path.parent
    agents_dir = plugins_dir.parent
    if full_path.name == "marketplace.json" and plugins_dir.name == "plugins" and agents_dir.name == ".agents":
        return agents_dir.parent
    return full_path.parent


def resolve_personal_plugin_root() -> Path | None:
    marketplace_path = default_marketplace_path()
    marketplace = read_json(marketplace_path)
    root = marketplace_root_for_path(marketplace_path)
    for entry in marketplace.get("plugins") or []:
        if not isinstance(entry, dict) or entry.get("name") != PLUGIN_NAME:
            continue
        source = entry.get("source") or {}
        path_value = ""
        if isinstance(source, dict):
            path_value = str(source.get("path") or "").strip()
        elif isinstance(source, str):
            path_value = source.strip()
        if not path_value:
            continue
        candidate = Path(path_value).expanduser()
        if not candidate.is_absolute():
            candidate = root / candidate
        if (candidate / ".codex-plugin" / "plugin.json").exists():
            return candidate
    fallback = Path.home() / "plugins" / PLUGIN_NAME
    if (fallback / ".codex-plugin" / "plugin.json").exists():
        return fallback
    return None


def best_installed_repo_root(repo_root: Path) -> Path:
    if (repo_root / ".codex-plugin" / "plugin.json").exists():
        return repo_root
    return resolve_personal_plugin_root() or repo_root


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


def marketplace_add_args() -> list[str]:
    return ["plugin", "marketplace", "add", MARKETPLACE_REPO, "--ref", MARKETPLACE_REF]


def marketplace_add_command_text() -> str:
    return f"codex plugin marketplace add {MARKETPLACE_REPO} --ref {MARKETPLACE_REF}"


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


def fetch_remote_release_notes(timeout: float) -> dict[str, Any]:
    url = os.environ.get("FORGE_METHOD_RELEASE_NOTES_URL", REMOTE_RELEASE_NOTES_URL).strip()
    if not url:
        return {}
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            raw = response.read(128 * 1024)
    except (OSError, urllib.error.URLError, TimeoutError):
        return {}
    try:
        return json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return {}


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
        "Forge Method automatic updates require the Git marketplace install shape. "
        f"Run $forge-update to migrate, or run: {marketplace_add_command_text()}"
    )
    state["last_legacy_hint"] = key
    save_state(state)


def print_refresh_summary(repo_root: Path, old_version: str, notes: dict[str, Any], *, skill_changed: bool) -> None:
    new_version = str(notes.get("version") or old_version or "").strip()
    if new_version and is_newer_version(new_version, old_version):
        print_patch_notes(old_version, new_version, notes, skill_changed=skill_changed)
    else:
        eprint(f"Forge Method marketplace install refreshed at {new_version or old_version or '<unknown>'}.")
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
                "Skill instructions may have changed. This chat can continue; "
                "open a new thread later only if you want the refreshed skill text fully loaded."
            )
    state = load_state()
    if new_version:
        state["last_announced_version"] = new_version
    save_state(state)


def fallback_marketplace_add(command: list[str], repo_root: Path, old_version: str, timeout: float, *, reason: str) -> bool:
    eprint(reason)
    eprint(f"Running: {marketplace_add_command_text()}")
    try:
        result = run_with_timeout([*command, *marketplace_add_args()], timeout)
    except subprocess.TimeoutExpired:
        eprint("Forge Method marketplace migration timed out; your current install was left usable.")
        eprint(f"Run manually: {marketplace_add_command_text()}")
        return False
    except OSError:
        eprint("Forge Method marketplace migration failed to start; your current install was left usable.")
        eprint(f"Run manually: {marketplace_add_command_text()}")
        return False
    if result.returncode != 0:
        eprint("Forge Method marketplace migration failed; your current install was left usable.")
        detail = (result.stderr or result.stdout or "").strip()
        if detail:
            eprint(detail.splitlines()[0])
        eprint(f"Run manually: {marketplace_add_command_text()}")
        return False

    installed_root = resolve_personal_plugin_root() or repo_root
    new_version = read_version(installed_root) or old_version
    notes = read_release_notes(installed_root, new_version)
    print_refresh_summary(installed_root, old_version, notes, skill_changed=True)
    return True


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


def manual_update(repo_root: Path, timeout: float) -> None:
    update_root = best_installed_repo_root(repo_root)
    current_version = read_version(update_root) or read_version(repo_root)
    command = codex_command()
    if not command:
        eprint("Forge Method update unavailable: Codex CLI not found.")
        eprint(f"Run manually after the CLI is available: {marketplace_add_command_text()}")
        return

    if not is_git_marketplace(update_root):
        eprint(f"Forge Method current version: {current_version or '<unknown>'}")
        fallback_marketplace_add(
            command,
            update_root,
            current_version,
            timeout,
            reason="This install is not in Git marketplace shape; migrating it to the updateable main package.",
        )
        return

    old_version = current_version
    old_metadata = marketplace_metadata(update_root)
    old_revision = str(old_metadata.get("revision") or "")
    watched = [
        update_root / "skills" / "forge-method" / "SKILL.md",
        update_root / "skills" / "forge-update" / "SKILL.md",
        update_root / ".codex-plugin" / "plugin.json",
    ]
    old_hashes = {str(path): file_hash(path) for path in watched}

    try:
        result = run_with_timeout([*command, "plugin", "marketplace", "upgrade", marketplace_name(update_root)], timeout)
    except subprocess.TimeoutExpired:
        eprint("Forge Method update timed out; your current install was left usable.")
        return
    except OSError:
        eprint("Forge Method update failed to start; your current install was left usable.")
        return
    if result.returncode != 0:
        if fallback_marketplace_add(
            command,
            update_root,
            old_version,
            timeout,
            reason="Forge Method marketplace upgrade failed; trying the main-package refresh path instead.",
        ):
            return
        eprint("Forge Method update failed; your current install was left usable.")
        detail = (result.stderr or result.stdout or "").strip()
        if detail:
            eprint(detail.splitlines()[0])
        return

    new_version = read_version(update_root)
    new_metadata = marketplace_metadata(update_root)
    new_revision = str(new_metadata.get("revision") or "")
    updated = is_newer_version(new_version, old_version) or (bool(new_revision) and new_revision != old_revision)
    if not updated:
        eprint(f"Forge Method is already up to date: {new_version or old_version or '<unknown>'}")
        return

    state = load_state()
    skill_changed = any(file_hash(path) != old_hashes.get(str(path), "") for path in watched)
    if is_newer_version(new_version, old_version):
        print_patch_notes(old_version, new_version, read_release_notes(update_root, new_version), skill_changed=skill_changed)
        state["last_announced_version"] = new_version
    else:
        eprint(f"Forge Method package refreshed at {new_version or '<unknown>'}.")
        notes = read_release_notes(update_root, new_version)
        summary = str(notes.get("summary") or "").strip()
        if summary:
            eprint(summary)
        if skill_changed:
            eprint(
                "Skill instructions changed in this update. This chat can continue; "
                "open a new thread later only if you want the refreshed skill text fully loaded."
            )
    state["last_seen_revision"] = new_revision
    save_state(state)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Forge Method self-update helper")
    parser.add_argument("--skill-dir", required=True)
    parser.add_argument("--manual", action="store_true", help="run an explicit user-triggered update")
    parser.add_argument("runtime_args", nargs=argparse.REMAINDER)
    ns = parser.parse_args(argv)
    runtime_args = list(ns.runtime_args)
    if runtime_args and runtime_args[0] == "--":
        runtime_args = runtime_args[1:]

    repo_root = find_repo_root(Path(ns.skill_dir))
    timeout = update_timeout()
    if ns.manual:
        manual_update(repo_root, timeout)
        return 0

    if os.environ.get("FORGE_METHOD_SKIP_UPDATE", "").strip().lower() in SKIP_VALUES:
        return 0
    if not should_run_for_args(runtime_args):
        return 0

    policy = effective_policy(repo_root)
    if policy == "off":
        return 0
    if not is_git_marketplace(repo_root):
        maybe_print_legacy_hint(repo_root, read_version(repo_root))
        return 0
    if policy == "notify":
        notify_available(repo_root, timeout)
        return 0
    auto_update(repo_root, timeout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

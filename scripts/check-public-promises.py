#!/usr/bin/env python3
"""Fail-closed static audit for public identity, payload, and command promises."""

from __future__ import annotations

import html
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    raise SystemExit(f"public promise audit failed: {message}")


def command_key(command: str) -> tuple[str, ...]:
    tokens = command.strip().split()
    key: list[str] = []
    for token in tokens:
        if token == "\\" or token.startswith("-") or token.startswith("[") or token.startswith("<"):
            break
        if any(marker in token for marker in ("|", "(", ")")):
            break
        key.append(token)
    return tuple(key)


def canonical_command_keys() -> set[tuple[str, ...]]:
    generated = (ROOT / "docs/generated/command-surface.md").read_text(encoding="utf-8")
    commands = re.findall(r"<code>(forge-core .*?)</code>", generated)
    keys = {command_key(html.unescape(command)) for command in commands}
    if not keys:
        fail("generated command surface contains no forge-core usages")
    return keys


def documented_command_keys() -> list[tuple[Path, int, tuple[str, ...]]]:
    found: list[tuple[Path, int, tuple[str, ...]]] = []
    roots = [ROOT / "README.md", ROOT / "CONTRIBUTING.md", ROOT / "docs"]
    files: list[Path] = []
    for item in roots:
        files.extend(item.rglob("*.md") if item.is_dir() else [item])
    for path in files:
        if path.name == "command-surface.md" and path.parent.name == "generated":
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            stripped = line.strip().lstrip("$>").strip().strip("`")
            if stripped.startswith("forge-core "):
                key = command_key(stripped)
                if len(key) >= 2:
                    found.append((path, number, key))
    return found


def check_payload() -> None:
    manifest = ROOT / "distribution/release-payload.txt"
    entries = [
        line.strip()
        for line in manifest.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if len(entries) != len(set(entries)):
        fail("distribution/release-payload.txt contains duplicate entries")
    for entry in entries:
        path = ROOT / entry
        if not path.is_file() or path.is_symlink():
            fail(f"payload entry is not a regular in-repository file: {entry}")
    required = {
        "skill/start-forge/SKILL.md",
        "docs/getting-started.md",
        "docs/operator-guide.md",
        "docs/real-host-proof.md",
        "contracts/spec/real-host-evidence-bundle-v0.yaml",
        "contracts/spec/domain-pack-rebase-v0.yaml",
        "scripts/check-real-host-evidence.py",
    }
    missing = required - set(entries)
    if missing:
        fail(f"release payload omits promised files: {sorted(missing)}")


def main() -> int:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    version = cargo["workspace"]["package"]["version"]
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    if len(readme.splitlines()) > 180:
        fail("README exceeds the concise landing-page limit of 180 lines")
    required_readme = [
        f"`{version}`",
        "Latest prebuilt",
        "Workflow release identity",
        "Domain Pack effective epoch",
        "Forge-mediated",
    ]
    for marker in required_readme:
        if marker not in readme:
            fail(f"README omits canonical boundary marker {marker!r}")

    status = (ROOT / "docs/product-status.md").read_text(encoding="utf-8")
    audit = (ROOT / "docs/product-compliance-audit.md").read_text(encoding="utf-8")
    for marker in ("source checkpoint", "prebuilt", "workflow release", "effective epoch"):
        if marker.lower() not in status.lower():
            fail(f"product status omits identity category {marker!r}")
    for marker in ("Mediated writes", "SBOM", "host", "independence", "prebuilt"):
        if marker.lower() not in audit.lower():
            fail(f"promise matrix omits evidence category {marker!r}")

    canonical = canonical_command_keys()
    for path, line, key in documented_command_keys():
        if key not in canonical:
            relative = path.relative_to(ROOT)
            fail(f"{relative}:{line} documents unknown command path {' '.join(key)!r}")

    check_payload()
    print("Public promise audit: clean")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        fail(str(error))

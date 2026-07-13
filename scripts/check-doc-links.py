#!/usr/bin/env python3
"""Fail when a repository Markdown link points at a missing local path."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote


LINK = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
FENCE = re.compile(r"^\s*(```|~~~)")
REMOTE_SCHEMES = ("http://", "https://", "mailto:", "data:", "ftp://")


def markdown_files(root: Path) -> list[Path]:
    # Documentation authority is intentionally confined. Do not walk arbitrary
    # consumer fixtures or build outputs that may live below the checkout.
    candidates = list(root.glob("*.md"))
    for documentation_root in (root / "docs", root / "skill"):
        if documentation_root.is_dir():
            candidates.extend(documentation_root.rglob("*.md"))
    return sorted(set(candidates))


def target_path(raw: str) -> str | None:
    value = raw.strip()
    if value.startswith("<") and ">" in value:
        value = value[1 : value.index(">")]
    elif " " in value:
        # Markdown permits an optional quoted title after the destination.
        value = value.split(" ", 1)[0]
    if not value or value.startswith("#") or value.lower().startswith(REMOTE_SCHEMES):
        return None
    value = unquote(value.split("#", 1)[0].split("?", 1)[0])
    return value or None


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    failures: list[str] = []
    for document in markdown_files(root):
        fenced = False
        for line_number, line in enumerate(document.read_text(encoding="utf-8-sig").splitlines(), 1):
            if FENCE.match(line):
                fenced = not fenced
                continue
            if fenced:
                continue
            for match in LINK.finditer(line):
                destination = target_path(match.group(1))
                if destination is None:
                    continue
                candidate = (document.parent / destination).resolve()
                if not candidate.exists():
                    failures.append(
                        f"{document.relative_to(root)}:{line_number}: missing local link {destination!r}"
                    )
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("Markdown local links: clean")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

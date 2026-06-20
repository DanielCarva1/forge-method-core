#!/usr/bin/env python3
"""Validate Forge guideline and work-order markdown structure."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


GUIDELINE_SECTIONS = [
    "Purpose",
    "Source Inputs",
    "Applies To",
    "Human Promise",
    "Agent Rule",
    "Machine Contract",
    "Allowed",
    "Forbidden",
    "Examples",
    "Acceptance Evidence",
    "Checks",
    "Update Triggers",
    "Work Order Bridge",
]

WORK_ORDER_SECTIONS = [
    "Goal",
    "Allowed Files",
    "Forbidden Files",
    "Steps",
    "Checks",
    "Acceptance Evidence",
    "Rollback",
    "Human Acceptance Question",
    "Evidence Target",
    "Done When",
]

WORK_ORDER_FIELDS = [
    "work_order_id:",
    "source_guideline:",
    "source_gap:",
    "implementation_block:",
]


def has_section(text: str, section: str) -> bool:
    return bool(re.search(rf"(?m)^##\s+{re.escape(section)}\s*$", text))


def validate(path: Path, kind: str) -> list[str]:
    text = path.read_text(encoding="utf-8")
    errors: list[str] = []
    if not text.lstrip().startswith("# "):
        errors.append("missing top-level title")

    if kind == "auto":
        if any(field in text for field in WORK_ORDER_FIELDS):
            kind = "work-order"
        else:
            kind = "guideline"

    if kind == "guideline":
        for section in GUIDELINE_SECTIONS:
            if not has_section(text, section):
                errors.append(f"missing guideline section: {section}")
    elif kind == "work-order":
        for field in WORK_ORDER_FIELDS:
            if field not in text:
                errors.append(f"missing work-order field: {field}")
        for section in WORK_ORDER_SECTIONS:
            if not has_section(text, section):
                errors.append(f"missing work-order section: {section}")
    else:
        errors.append(f"unknown kind: {kind}")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", type=Path)
    parser.add_argument("--kind", choices=["auto", "guideline", "work-order"], default="auto")
    args = parser.parse_args()

    if not args.path.exists():
        print(f"missing file: {args.path}", file=sys.stderr)
        return 2

    errors = validate(args.path, args.kind)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1

    print(f"Validation passed: {args.path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

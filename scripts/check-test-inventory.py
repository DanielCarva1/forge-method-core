#!/usr/bin/env python3
"""Capture or verify an exact, normalized Rust test inventory."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

TEST_LINE = re.compile(r"^(.+): test$")
HARNESS_LINE = re.compile(r"^\s*Running (.+?)(?: \([^)]*\))?$")
DOC_LINE = re.compile(r"^\s*Doc-tests (.+)$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--label", required=True)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--update", action="store_true")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("a command is required after --")
    return args


def inventory(output: str) -> list[str]:
    harness = "unknown"
    tests: list[str] = []
    for line in output.splitlines():
        if match := HARNESS_LINE.match(line):
            harness = match.group(1)
            continue
        if match := DOC_LINE.match(line):
            harness = f"doc:{match.group(1)}"
            continue
        if match := TEST_LINE.match(line):
            tests.append(f"{harness}::{match.group(1)}")
    return sorted(tests)


def main() -> int:
    args = parse_args()
    result = subprocess.run(args.command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    sys.stdout.write(result.stdout)
    if result.returncode:
        return result.returncode
    current = inventory(result.stdout)
    if not current:
        print("test inventory is empty", file=sys.stderr)
        return 2
    document = {"schema_version": "forge-test-inventory-v1", "label": args.label, "tests": current}
    if args.update:
        args.baseline.parent.mkdir(parents=True, exist_ok=True)
        args.baseline.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
        print(f"updated {args.baseline}: {len(current)} tests")
    else:
        try:
            expected = json.loads(args.baseline.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            print(f"cannot read test inventory baseline {args.baseline}: {error}", file=sys.stderr)
            return 2
        if expected != document:
            old = set(expected.get("tests", []))
            new = set(current)
            print(
                "test inventory drift\n"
                f"removed: {sorted(old - new)}\n"
                f"added: {sorted(new - old)}",
                file=sys.stderr,
            )
            return 3
        print(f"test inventory parity: {len(current)} tests")
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

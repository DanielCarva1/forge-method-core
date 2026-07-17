#!/usr/bin/env python3
"""Fail closed when a release workflow Cargo invocation is not lock-bound."""

from __future__ import annotations

import argparse
import re
from pathlib import Path
from typing import NamedTuple


class ReleaseLockError(RuntimeError):
    """The release command topology is absent, ambiguous, or unlocked."""


class Invocation(NamedTuple):
    line: int
    tool: str
    subcommand: str
    command: str


RUN_KEY = re.compile(r"^(?P<indent> *)(?:- )?run:\s*(?P<value>.*)$")
CARGO_COMMAND = re.compile(
    r"(?<![A-Za-z0-9_./-])(?P<tool>cargo|cross)\s+"
    r"(?:--locked\s+)?(?P<subcommand>[A-Za-z][A-Za-z0-9_-]*)"
)
COMMAND_BOUNDARY = re.compile(r"&&|\|\||[;|\n]")
LOCKED_FLAG = re.compile(r"(?<!\S)--locked(?!\S)")
SBOM_RUNNER_COMMAND = "python scripts/run-release-locked-sbom.py"
SBOM_LOCKED_COMMAND = (
    'return [real_cargo, "metadata", "--locked", *arguments[1:]]'
)
SBOM_SHIM_CALL = "command = locked_metadata_command(real_cargo, sys.argv[1:])"
SBOM_SHIM_EXEC = "os.execv(real_cargo, command)"


def run_blocks(source: str) -> list[tuple[int, str]]:
    """Extract inline and block-scalar ``run`` bodies without a YAML dependency."""
    lines = source.splitlines()
    blocks: list[tuple[int, str]] = []
    index = 0
    while index < len(lines):
        match = RUN_KEY.match(lines[index])
        if match is None:
            index += 1
            continue
        line_number = index + 1
        value = match.group("value")
        if not re.fullmatch(r"[|>][+-]?", value):
            blocks.append((line_number, value))
            index += 1
            continue

        indentation = len(match.group("indent"))
        index += 1
        body: list[str] = []
        while index < len(lines):
            candidate = lines[index]
            candidate_indent = len(candidate) - len(candidate.lstrip(" "))
            if candidate.strip() and candidate_indent <= indentation:
                break
            body.append(candidate[indentation + 1 :])
            index += 1
        blocks.append((line_number, "\n".join(body)))
    return blocks


def find_invocations(source: str) -> list[Invocation]:
    invocations: list[Invocation] = []
    for run_line, body in run_blocks(source):
        logical = re.sub(r"\\\r?\n[ \t]*", " ", body)
        for match in CARGO_COMMAND.finditer(logical):
            boundary = COMMAND_BOUNDARY.search(logical, match.end())
            end = boundary.start() if boundary is not None else len(logical)
            command = " ".join(logical[match.start() : end].split())
            line = run_line + logical[: match.start()].count("\n")
            invocations.append(
                Invocation(line, match.group("tool"), match.group("subcommand"), command)
            )
    return invocations


def locked_sbom_invocation(workflow_source: str, runner: Path) -> Invocation:
    if SBOM_RUNNER_COMMAND not in workflow_source:
        raise ReleaseLockError(
            f"release workflow must invoke {SBOM_RUNNER_COMMAND}"
        )
    try:
        lines = [line.strip() for line in runner.read_text(encoding="utf-8").splitlines()]
    except OSError as error:
        raise ReleaseLockError(f"cannot read {runner}: {error}") from error
    required = (SBOM_LOCKED_COMMAND, SBOM_SHIM_CALL, SBOM_SHIM_EXEC)
    missing = [text for text in required if lines.count(text) != 1]
    if missing:
        raise ReleaseLockError(
            f"{runner}: locked cargo-cyclonedx metadata shim contract drifted: {missing}"
        )
    line = workflow_source[: workflow_source.index(SBOM_RUNNER_COMMAND)].count("\n") + 1
    return Invocation(
        line, "cargo", "metadata",
        "cargo metadata --locked (cargo-cyclonedx CARGO shim)",
    )


def check(workflow: Path, sbom_runner: Path | None = None) -> list[Invocation]:
    try:
        source = workflow.read_text(encoding="utf-8")
    except OSError as error:
        raise ReleaseLockError(f"cannot read {workflow}: {error}") from error

    invocations = find_invocations(source)
    if not invocations:
        raise ReleaseLockError(f"{workflow}: release workflow has no Cargo invocations")
    unlocked = [item for item in invocations if LOCKED_FLAG.search(item.command) is None]
    if unlocked:
        details = "\n".join(
            f"  {workflow}:{item.line}: {item.command}" for item in unlocked
        )
        raise ReleaseLockError(
            "release Cargo invocations must include a literal --locked flag:\n" + details
        )
    if sbom_runner is None:
        sbom_runner = Path(__file__).resolve().parent / "run-release-locked-sbom.py"
    invocations.append(locked_sbom_invocation(source, sbom_runner))
    return invocations


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "workflow",
        nargs="?",
        type=Path,
        default=Path(__file__).resolve().parents[1] / ".github/workflows/release.yml",
    )
    args = parser.parse_args()
    invocations = check(args.workflow)
    print(f"Release Cargo lock audit: {len(invocations)} locked invocation(s)")
    for item in invocations:
        print(f"  {args.workflow}:{item.line}: {item.command}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ReleaseLockError as error:
        raise SystemExit(f"release Cargo lock audit failed: {error}") from error

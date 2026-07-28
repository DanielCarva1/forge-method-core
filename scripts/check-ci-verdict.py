#!/usr/bin/env python3
"""Emit the required CI verdict and fail closed on non-success terminal states."""

from __future__ import annotations

import argparse
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


TERMINAL_STATES = frozenset({"success", "failure", "cancelled", "skipped"})
JOB_ID = re.compile(r"^[a-z0-9][a-z0-9_-]*$")


@dataclass(frozen=True)
class JobResult:
    """One dependency result supplied by GitHub's ``needs`` context."""

    job_id: str
    label: str
    state: str


class VerdictConfigurationError(ValueError):
    """The verdict job was invoked with an ambiguous or incomplete contract."""


def _escape_cell(value: str) -> str:
    return value.replace("\r", " ").replace("\n", " ").replace("|", "\\|")


def _display_state(state: str) -> str:
    if not state:
        return "missing"
    if state not in TERMINAL_STATES:
        return f"unknown ({_escape_cell(state)})"
    return state


def _validate_rows(
    mandatory: Sequence[JobResult], informational: Sequence[JobResult]
) -> None:
    if not mandatory:
        raise VerdictConfigurationError(
            "at least one mandatory job result is required; an empty readiness set is forbidden"
        )

    seen: set[str] = set()
    for result in (*mandatory, *informational):
        if not JOB_ID.fullmatch(result.job_id):
            raise VerdictConfigurationError(
                f"invalid CI job id {result.job_id!r}; expected lowercase slug"
            )
        if not result.label.strip():
            raise VerdictConfigurationError(
                f"CI job {result.job_id!r} must have a non-empty summary label"
            )
        if result.job_id in seen:
            raise VerdictConfigurationError(
                f"CI job {result.job_id!r} cannot be both mandatory and informational"
            )
        seen.add(result.job_id)


def render_summary(
    mandatory: Sequence[JobResult], informational: Sequence[JobResult]
) -> tuple[str, bool]:
    """Return Markdown plus the fail-closed mandatory verdict."""
    _validate_rows(mandatory, informational)
    passed = all(result.state == "success" for result in mandatory)

    lines = [
        "## Required source-only CI verdict",
        "",
        (
            "Only an actual `success` terminal state satisfies a mandatory job. "
            "Failure, cancellation, an unexpected skip, a missing state, or an "
            "unknown state fails this verdict closed."
        ),
        "",
        "| Mandatory job | Actual terminal state | Required verdict |",
        "|---|---|---|",
    ]
    for result in mandatory:
        state = _display_state(result.state)
        verdict = "PASS" if result.state == "success" else "FAIL"
        lines.append(
            f"| `{_escape_cell(result.job_id)}` - {_escape_cell(result.label)} "
            f"| `{state}` | **{verdict}** |"
        )

    lines.extend(
        [
            "",
            f"**Required source-only verdict: {'PASS' if passed else 'FAIL'}**",
        ]
    )

    if informational:
        lines.extend(
            [
                "",
                "## Prerelease-channel informational observations (excluded)",
                "",
                (
                    "The current prerelease channel does not claim native "
                    "platform-gate or P6d cumulative evidence, and neither "
                    "observation is P7F proof. These allowed-failure "
                    "observations are listed separately and cannot "
                    "satisfy this verdict or any broader readiness claim. Their "
                    "dependency result is descriptive only: job-level "
                    "`continue-on-error` can normalize an underlying failure to "
                    "`success`, so the job steps and timing artifacts remain the "
                    "authoritative observation detail."
                ),
                "",
                "| Informational job | Reported dependency result | Readiness contribution |",
                "|---|---|---|",
            ]
        )
        for result in informational:
            lines.append(
                f"| `{_escape_cell(result.job_id)}` - {_escape_cell(result.label)} "
                f"| `{_display_state(result.state)}` | **none (excluded)** |"
            )

    return "\n".join(lines) + "\n", passed


def _rows(values: Sequence[Sequence[str]] | None) -> list[JobResult]:
    return [JobResult(*value) for value in values or []]


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mandatory",
        action="append",
        nargs=3,
        metavar=("JOB_ID", "LABEL", "STATE"),
        help="mandatory job id, display label, and needs.<job>.result value",
    )
    parser.add_argument(
        "--informational",
        action="append",
        nargs=3,
        metavar=("JOB_ID", "LABEL", "STATE"),
        help="prerelease-channel observation excluded from the required verdict",
    )
    args = parser.parse_args(argv)

    mandatory = _rows(args.mandatory)
    informational = _rows(args.informational)
    try:
        summary, passed = render_summary(mandatory, informational)
    except VerdictConfigurationError as error:
        print(f"CI verdict configuration failed closed: {error}", file=sys.stderr)
        return 2

    print(summary, end="")
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        try:
            with Path(summary_path).open("a", encoding="utf-8", newline="\n") as stream:
                stream.write(summary)
        except OSError as error:
            print(f"cannot write CI job summary: {error}", file=sys.stderr)
            return 2

    if not passed:
        failed = [
            f"{result.job_id}={_display_state(result.state)}"
            for result in mandatory
            if result.state != "success"
        ]
        print(
            "mandatory CI verdict failed closed: " + ", ".join(failed),
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

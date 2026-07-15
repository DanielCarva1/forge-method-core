#!/usr/bin/env python3
"""Run one CI tier with a hard wall-clock budget and timing evidence."""

from __future__ import annotations

import argparse
import html
import json
import os
from pathlib import Path
import platform
import shlex
import signal
import subprocess
import sys
import time
from typing import Sequence


BUDGET_FAILURE_EXIT = 124
TERMINATION_GRACE_SECONDS = 2.0


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tier", required=True, help="Stable CI tier identifier")
    parser.add_argument(
        "--budget-seconds",
        required=True,
        type=float,
        help="Hard wall-clock timeout for the complete child process tree",
    )
    parser.add_argument("--report", required=True, type=Path, help="JSON report path")
    parser.add_argument(
        "--cache-context",
        default=os.environ.get("FORGE_CI_CACHE_CONTEXT", "not-provided"),
        help="Cache provider/hit context recorded with the report",
    )
    parser.add_argument("command", nargs=argparse.REMAINDER, help="Command after --")
    args = parser.parse_args(argv)
    if args.budget_seconds < 0:
        parser.error("--budget-seconds must be non-negative")
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("a command is required after --")
    return args


def normalized_exit_code(return_code: int) -> int:
    """Translate a POSIX signal return into its conventional shell exit code."""
    return 128 + abs(return_code) if return_code < 0 else return_code


def runner_context() -> dict[str, str]:
    return {
        "os": os.environ.get("RUNNER_OS", platform.system() or "unknown"),
        "arch": os.environ.get("RUNNER_ARCH", platform.machine() or "unknown"),
        "name": os.environ.get("RUNNER_NAME", "local"),
        "environment": os.environ.get("RUNNER_ENVIRONMENT", "local"),
    }


def command_display(command: Sequence[str]) -> str:
    if os.name == "nt":
        return subprocess.list2cmdline(command)
    return shlex.join(command)


def append_step_summary(report: dict[str, object]) -> None:
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary_path:
        return
    command = html.escape(str(report["command_display"]))
    cache = html.escape(str(report["cache_context"]))
    runner = report["runner_context"]
    assert isinstance(runner, dict)
    runner_label = html.escape(f"{runner['os']}/{runner['arch']} ({runner['name']})")
    lines = [
        f"### CI tier: `{report['tier']}`",
        "",
        "| Command | Runner | Cache | Elapsed | Budget | Command exit | Outcome |",
        "| --- | --- | --- | ---: | ---: | ---: | --- |",
        (
            f"| <code>{command}</code> | {runner_label} | {cache} | "
            f"{report['elapsed_seconds']:.3f}s | {report['budget_seconds']:.3f}s | "
            f"{report['command_exit_code']} | **{report['outcome']}** |"
        ),
        "",
    ]
    path = Path(summary_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8", newline="\n") as stream:
        stream.write("\n".join(lines))


def _popen(command: Sequence[str]) -> subprocess.Popen[bytes]:
    options: dict[str, object] = {}
    if os.name == "nt":
        options["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        options["start_new_session"] = True
    return subprocess.Popen(command, **options)


def _wait_briefly(process: subprocess.Popen[bytes]) -> bool:
    try:
        process.wait(timeout=TERMINATION_GRACE_SECONDS)
        return True
    except subprocess.TimeoutExpired:
        return False


def _posix_group_alive(process_group_id: int) -> bool:
    try:
        os.killpg(process_group_id, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def _wait_for_posix_group_exit(process: subprocess.Popen[bytes]) -> bool:
    deadline = time.monotonic() + TERMINATION_GRACE_SECONDS
    while time.monotonic() < deadline:
        process.poll()  # Reap the group leader so it does not keep the PGID alive.
        if not _posix_group_alive(process.pid):
            return True
        time.sleep(0.05)
    process.poll()
    return not _posix_group_alive(process.pid)


def _taskkill(process_id: int, *, force: bool) -> None:
    command = ["taskkill", "/PID", str(process_id), "/T"]
    if force:
        command.append("/F")
    try:
        subprocess.run(
            command,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=TERMINATION_GRACE_SECONDS,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        pass


def _terminate_process_tree(process: subprocess.Popen[bytes]) -> str:
    """Request graceful tree termination, then force-kill the complete tree."""
    if os.name == "nt":
        _taskkill(process.pid, force=False)
        if _wait_briefly(process):
            return "terminated"
        _taskkill(process.pid, force=True)
        return "killed" if _wait_briefly(process) else "kill_unconfirmed"

    process_group_id = process.pid
    try:
        os.killpg(process_group_id, signal.SIGTERM)
    except ProcessLookupError:
        process.wait()
        return "already_exited"
    if _wait_for_posix_group_exit(process):
        process.wait()
        return "terminated"
    try:
        os.killpg(process_group_id, signal.SIGKILL)
    except ProcessLookupError:
        pass
    group_exited = _wait_for_posix_group_exit(process)
    try:
        process.wait(timeout=TERMINATION_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        return "kill_unconfirmed"
    return "killed" if group_exited else "kill_unconfirmed"


def run(args: argparse.Namespace) -> int:
    started = time.monotonic()
    command_exit = 127
    timed_out = False
    termination = "not_required"
    launch_error: str | None = None
    try:
        process = _popen(args.command)
        try:
            process.wait(timeout=args.budget_seconds)
            command_exit = normalized_exit_code(process.returncode)
        except subprocess.TimeoutExpired:
            timed_out = True
            termination = _terminate_process_tree(process)
            command_exit = BUDGET_FAILURE_EXIT
    except (FileNotFoundError, PermissionError, OSError) as error:
        launch_error = str(error)
        print(f"CI tier command could not start: {error}", file=sys.stderr)

    elapsed = time.monotonic() - started
    budget_exceeded = timed_out or elapsed > args.budget_seconds
    if timed_out:
        wrapper_exit = BUDGET_FAILURE_EXIT
        outcome = "timed_out"
    elif command_exit != 0:
        wrapper_exit = command_exit
        outcome = "command_failed"
    elif budget_exceeded:
        # A child can exit in the scheduling interval immediately after the
        # deadline. Preserve the hard budget contract even without a signal.
        wrapper_exit = BUDGET_FAILURE_EXIT
        outcome = "budget_exceeded"
    else:
        wrapper_exit = 0
        outcome = "passed"

    report: dict[str, object] = {
        "schema_version": "2",
        "tier": args.tier,
        "command": list(args.command),
        "command_display": command_display(args.command),
        "runner_context": runner_context(),
        "cache_context": args.cache_context,
        "elapsed_seconds": round(elapsed, 3),
        "budget_seconds": float(args.budget_seconds),
        "budget_status": "exceeded" if budget_exceeded else "within_budget",
        "timed_out": timed_out,
        "termination": termination,
        "command_exit_code": command_exit,
        "command_status": "timed_out" if timed_out else ("failed" if command_exit else "passed"),
        "outcome": outcome,
        "wrapper_exit_code": wrapper_exit,
        "launch_error": launch_error,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    try:
        append_step_summary(report)
    except OSError as error:
        print(f"CI tier could not append step summary: {error}", file=sys.stderr)
    print(
        f"CI tier {args.tier}: {outcome}; elapsed={elapsed:.3f}s; "
        f"budget={args.budget_seconds:.3f}s; command_exit={command_exit}; "
        f"termination={termination}"
    )
    return wrapper_exit


def main(argv: Sequence[str] | None = None) -> int:
    return run(parse_args(argv))


if __name__ == "__main__":
    raise SystemExit(main())

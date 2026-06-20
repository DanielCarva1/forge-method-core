#!/usr/bin/env python3
"""Responsive unittest runner with debug reports and per-test timeouts."""

from __future__ import annotations

import argparse
import concurrent.futures
import datetime as dt
import json
import os
import re
import subprocess
import sys
import time
import unittest
import xml.etree.ElementTree as ET
from dataclasses import asdict, dataclass
from pathlib import Path


@dataclass
class TestResult:
    test_id: str
    status: str
    elapsed: float
    command: list[str]
    returncode: int | None = None
    output_tail: str = ""
    output: str = ""
    output_path: str = ""


def utc_stamp() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d-%H%M%SZ")


def iter_tests(suite: unittest.TestSuite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from iter_tests(item)
        else:
            yield item


def split_values(values: list[str]) -> list[str]:
    items: list[str] = []
    for value in values:
        for part in str(value).split(","):
            part = part.strip()
            if part:
                items.append(part)
    return items


def normalize_test_id(test_id: str) -> str:
    if test_id.startswith("tests."):
        return test_id
    if test_id.startswith("test_"):
        return f"tests.{test_id}"
    return test_id


def discover_tests(pattern: str) -> list[str]:
    loader = unittest.TestLoader()
    suite = loader.discover("tests", pattern=pattern)
    return [normalize_test_id(test.id()) for test in iter_tests(suite)]


def load_report(path: str) -> dict:
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def report_test_ids(path: str, mode: str, limit: int) -> list[str]:
    report = load_report(path)
    items = list(report.get("tests") or [])
    if mode == "failures":
        selected = [item for item in items if item.get("status") != "PASS"]
    elif mode == "slowest":
        selected = sorted(items, key=lambda item: float(item.get("elapsed") or 0), reverse=True)
    else:
        raise ValueError(f"unknown report mode: {mode}")
    if limit > 0:
        selected = selected[:limit]
    return [str(item.get("test_id") or item.get("id")) for item in selected if item.get("test_id") or item.get("id")]


def apply_match_filter(tests: list[str], matches: list[str]) -> list[str]:
    if not matches:
        return tests
    return [test_id for test_id in tests if any(match in test_id for match in matches)]


def run_one(test_id: str, timeout: int, *, verbose: bool, tail_chars: int) -> TestResult:
    command = [sys.executable, "-m", "unittest"]
    if verbose:
        command.append("-v")
    command.append(test_id)
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            command,
            cwd=os.getcwd(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as exc:
        elapsed = time.perf_counter() - started
        output = ((exc.stdout or "") + (exc.stderr or ""))
        return TestResult(
            test_id=test_id,
            status="TIMEOUT",
            elapsed=elapsed,
            command=command,
            output=output,
            output_tail=output[-tail_chars:],
        )

    elapsed = time.perf_counter() - started
    output = (completed.stdout or "") + (completed.stderr or "")
    status = "PASS" if completed.returncode == 0 else "FAIL"
    return TestResult(
        test_id=test_id,
        status=status,
        elapsed=elapsed,
        command=command,
        returncode=completed.returncode,
        output=output,
        output_tail=output[-tail_chars:],
    )


def safe_filename(test_id: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]+", "_", test_id).strip("._") + ".log"


def should_keep_output(result: TestResult, retain: str, slow_threshold: float) -> bool:
    if retain == "none":
        return False
    if retain == "all":
        return True
    if retain == "failures":
        return result.status != "PASS"
    if retain == "interesting":
        return result.status != "PASS" or result.elapsed >= slow_threshold
    raise ValueError(f"unknown retain mode: {retain}")


def write_output(result: TestResult, output_dir: Path) -> str:
    output_dir.mkdir(parents=True, exist_ok=True)
    path = output_dir / safe_filename(result.test_id)
    path.write_text(
        "\n".join(
            [
                f"test_id: {result.test_id}",
                f"status: {result.status}",
                f"elapsed_seconds: {result.elapsed:.3f}",
                f"returncode: {result.returncode}",
                f"command: {' '.join(result.command)}",
                "",
                result.output,
            ]
        ),
        encoding="utf-8",
    )
    return str(path)


def summarize(results: list[TestResult], total: int, elapsed: float, slow_threshold: float) -> dict:
    passed = len([result for result in results if result.status == "PASS"])
    failed = len([result for result in results if result.status == "FAIL"])
    timed_out = len([result for result in results if result.status == "TIMEOUT"])
    slow = len([result for result in results if result.status == "PASS" and result.elapsed >= slow_threshold])
    return {
        "total": total,
        "completed": len(results),
        "passed": passed,
        "failed": failed,
        "timed_out": timed_out,
        "slow": slow,
        "elapsed_seconds": round(elapsed, 3),
    }


def write_json_report(
    path: str,
    *,
    args: argparse.Namespace,
    tests: list[str],
    results: list[TestResult],
    elapsed: float,
) -> None:
    report_path = Path(path)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema_version": 1,
        "created_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "cwd": os.getcwd(),
        "python": sys.executable,
        "argv": sys.argv,
        "settings": {
            "workers": args.workers,
            "timeout": args.timeout,
            "slow_threshold": args.slow_threshold,
            "debug": bool(args.debug),
            "fail_fast": bool(args.fail_fast),
        },
        "summary": summarize(results, len(tests), elapsed, args.slow_threshold),
        "tests": [
            {
                **{key: value for key, value in asdict(result).items() if key != "output"},
                "id": result.test_id,
            }
            for result in results
        ],
    }
    report_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def write_junit(path: str, *, tests: list[str], results: list[TestResult], elapsed: float) -> None:
    result_by_id = {result.test_id: result for result in results}
    failures = len([result for result in results if result.status == "FAIL"])
    errors = len([result for result in results if result.status == "TIMEOUT"])
    suite = ET.Element(
        "testsuite",
        {
            "name": "forge-method-unit-tests",
            "tests": str(len(tests)),
            "failures": str(failures),
            "errors": str(errors),
            "time": f"{elapsed:.3f}",
        },
    )
    for test_id in tests:
        result = result_by_id.get(test_id)
        parts = test_id.split(".")
        case = ET.SubElement(
            suite,
            "testcase",
            {
                "classname": ".".join(parts[:-1]) if len(parts) > 1 else test_id,
                "name": parts[-1],
                "time": f"{(result.elapsed if result else 0):.3f}",
            },
        )
        if result is None:
            error = ET.SubElement(case, "error", {"message": "test did not complete"})
            error.text = "The runner stopped before this test completed."
            continue
        if result.status == "FAIL":
            failure = ET.SubElement(case, "failure", {"message": "unittest failure"})
            failure.text = result.output_tail
        elif result.status == "TIMEOUT":
            error = ET.SubElement(case, "error", {"message": "test timed out"})
            error.text = result.output_tail
    junit_path = Path(path)
    junit_path.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(suite).write(junit_path, encoding="utf-8", xml_declaration=True)


def print_next_steps(failed_or_timed_out: list[TestResult], slow: list[TestResult], report_path: str) -> None:
    if failed_or_timed_out:
        print("\nDebug next steps:")
        for result in failed_or_timed_out[:5]:
            print(f"- Re-run one: {sys.executable} -m unittest -v {result.test_id}")
        if report_path:
            print(f"- Re-run failures from report: {sys.executable} scripts/test-runner.py --debug --rerun-failures {report_path}")
    elif slow:
        print("\nOptimization next steps:")
        for result in slow[:5]:
            print(f"- Inspect slow test: {sys.executable} -m unittest -v {result.test_id}")
        if report_path:
            print(f"- Re-run slowest from report: {sys.executable} scripts/test-runner.py --debug --rerun-slowest {report_path} --limit 5")


def selected_tests(args: argparse.Namespace) -> list[str]:
    tests: list[str] = []
    tests.extend(normalize_test_id(item) for item in split_values(args.test))
    for path in args.rerun_failures:
        tests.extend(report_test_ids(path, "failures", args.limit))
    for path in args.rerun_slowest:
        tests.extend(report_test_ids(path, "slowest", args.limit))
    if not tests:
        tests = discover_tests(args.pattern)
    tests = apply_match_filter(tests, split_values(args.match))
    return sorted(dict.fromkeys(tests))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pattern", default="test*.py")
    parser.add_argument("--test", action="append", default=[], help="Run one test id; repeatable or comma-separated.")
    parser.add_argument("--match", action="append", default=[], help="Keep discovered/report tests whose id contains this text.")
    parser.add_argument("--workers", type=int, default=0)
    parser.add_argument("--timeout", type=int, default=90, help="Timeout per test in seconds.")
    parser.add_argument("--slow-threshold", type=float, default=10.0)
    parser.add_argument("--fail-fast", action="store_true")
    parser.add_argument("--debug", action="store_true", help="Verbose unittest output, debug directory, and repair hints.")
    parser.add_argument("--report", default="", help="Write a JSON report for agents and CI.")
    parser.add_argument("--junit", default="", help="Write a JUnit XML report.")
    parser.add_argument("--output-dir", default="", help="Directory for retained per-test logs.")
    parser.add_argument(
        "--retain-output",
        choices=["none", "failures", "interesting", "all"],
        default="interesting",
        help="Which test outputs to retain when an output directory is active.",
    )
    parser.add_argument("--output-tail-chars", type=int, default=4000)
    parser.add_argument("--show-output", choices=["none", "failures", "interesting", "all"], default="")
    parser.add_argument("--rerun-failures", action="append", default=[], metavar="REPORT")
    parser.add_argument("--rerun-slowest", action="append", default=[], metavar="REPORT")
    parser.add_argument("--limit", type=int, default=10, help="Limit report-derived selections.")
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()

    tests = selected_tests(args)
    if args.list:
        for test_id in tests:
            print(test_id)
        print(f"Total: {len(tests)}")
        return 0

    if not tests:
        print("No tests discovered.", file=sys.stderr)
        return 2

    if args.workers <= 0:
        args.workers = 1 if args.debug else max(1, min(4, (os.cpu_count() or 2)))
    workers = max(1, args.workers)
    show_output = args.show_output or ("interesting" if args.debug else "failures")

    output_dir: Path | None = Path(args.output_dir) if args.output_dir else None
    if args.debug and output_dir is None:
        output_dir = Path(".forge-method") / "test-runs" / f"debug-{utc_stamp()}"

    print(
        f"Running {len(tests)} tests with {workers} worker(s), "
        f"{args.timeout}s timeout/test, {args.slow_threshold:.1f}s slow threshold.",
        flush=True,
    )
    if args.debug:
        print("Debug mode: unittest -v, full interesting output, and repair hints enabled.", flush=True)
    if args.report:
        print(f"JSON report: {args.report}", flush=True)
    if args.junit:
        print(f"JUnit report: {args.junit}", flush=True)
    if output_dir is not None:
        print(f"Debug output directory: {output_dir}", flush=True)

    started = time.perf_counter()
    results: list[TestResult] = []
    failed_or_timed_out: list[TestResult] = []

    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as executor:
        future_map = {
            executor.submit(
                run_one,
                test_id,
                args.timeout,
                verbose=args.debug,
                tail_chars=args.output_tail_chars,
            ): test_id
            for test_id in tests
        }
        completed_count = 0
        for future in concurrent.futures.as_completed(future_map):
            result = future.result()
            if output_dir is not None and should_keep_output(result, args.retain_output, args.slow_threshold):
                result.output_path = write_output(result, output_dir)
            results.append(result)
            completed_count += 1
            marker = result.status
            is_slow = result.status == "PASS" and result.elapsed >= args.slow_threshold
            if is_slow:
                marker = "SLOW"
            print(
                f"[{completed_count:>3}/{len(tests)}] {marker:<7} {result.elapsed:>6.1f}s {result.test_id}",
                flush=True,
            )
            if (
                show_output == "all"
                or (show_output == "failures" and result.status != "PASS")
                or (show_output == "interesting" and (result.status != "PASS" or is_slow))
            ):
                print(result.output if args.debug else result.output_tail, flush=True)
            if result.status != "PASS":
                failed_or_timed_out.append(result)
                if args.fail_fast:
                    for pending in future_map:
                        pending.cancel()
                    break

    elapsed = time.perf_counter() - started
    slow = sorted(
        [result for result in results if result.status == "PASS" and result.elapsed >= args.slow_threshold],
        key=lambda item: item.elapsed,
        reverse=True,
    )

    print(f"\nCompleted {len(results)}/{len(tests)} tests in {elapsed:.1f}s.")
    if slow:
        print("\nSlow tests:")
        for result in slow[:20]:
            print(f"- {result.elapsed:.1f}s {result.test_id}")
    if failed_or_timed_out:
        print("\nFailures/timeouts:")
        for result in failed_or_timed_out:
            print(f"- {result.status} {result.elapsed:.1f}s {result.test_id}")

    if args.report:
        write_json_report(args.report, args=args, tests=tests, results=results, elapsed=elapsed)
    if args.junit:
        write_junit(args.junit, tests=tests, results=results, elapsed=elapsed)
    print_next_steps(failed_or_timed_out, slow, args.report)

    if failed_or_timed_out:
        return 1

    print("Responsive unit test run passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

# Smart test suite observability

- created_at_local: 2026-06-18T01:34:48-03:00
- created_at_utc: 2026-06-18T04:34:48+00:00
- kind: validation
- story: none
- phase: 6-evolve
- workflow: runtime-builder

## Summary

Upgraded the Forge Method Core unit suite from a responsive runner into an
agent-observable test harness. The runner now supports debug mode, JSON reports,
JUnit reports, retained per-test logs, substring filtering, and report-driven
re-runs for failed or slow tests.

## Skill Integration Status

- Local Codex skill exists at `C:\Users\Danie\.codex\skills\forge-guideline-auditor`.
- Core canonical skill exists at `skills/forge-guideline-auditor`.
- Both local and core copies include `agents/openai.yaml` with `allow_implicit_invocation: true`.
- Forge Method Core includes `guideline-audit` workflow catalog metadata, runtime routing, facilitation, template, work-order fields, and route tests.
- Forge Standalone includes the Guideline Audit Gate in `AGENTS.md` and `docs/23-guideline-audit-gate.md`.

## Suite Changes

- `scripts/test-runner.py` writes JSON reports with summary, settings, test ids, statuses, elapsed times, output tails, commands, and optional log paths.
- `scripts/test-runner.py` writes optional JUnit XML for CI surfaces.
- `--debug` runs unittest verbosely, defaults to one worker, retains interesting logs, and prints next-step commands.
- `--match` filters discovered/report tests by id substring.
- `--rerun-failures <report>` and `--rerun-slowest <report> --limit N` let agents investigate from the previous report instead of rediscovering manually.
- `verify-fast` and `verify-all` wrappers expose debug/report/JUnit options and create ignored JSON reports under `.forge-method/test-runs/` by default.
- Living validation docs now point at `scripts/test-runner.py` instead of opaque `python -m unittest discover -s tests`.

## Checks

- `python -m py_compile scripts\test-runner.py tests\test_test_runner.py` passed.
- `python scripts\test-runner.py --workers 2 --timeout 60 --test ...test_test_runner... --report .forge-method\test-runs\runner-self-test.json --junit .forge-method\test-runs\runner-self-test.xml` passed 3/3.
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -Debug -Match test_report_and_junit_for_filtered_run ...` passed.
- `bash -n scripts/verify-fast.sh; bash -n scripts/verify-all.sh` passed.
- `python scripts\test-runner.py --debug --rerun-slowest .forge-method\test-runs\runner-self-test.json --limit 1 ...` passed.
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -Match test_test_runner ...` passed 3/3 plus onboarding, workflow, and agent validation.
- `python scripts\test-runner.py --workers 4 --timeout 120 --report .forge-method\test-runs\full-smart-suite.json` passed 133/133 in 199.4s.

## Observability Evidence

- Full-suite report: `.forge-method/test-runs/full-smart-suite.json` (ignored local operational artifact).
- The full run surfaced the slowest tests and printed direct debug commands, including `--rerun-slowest .forge-method\test-runs\full-smart-suite.json --limit 5`.
- Report directories are ignored by Git through `.forge-method/test-runs/`.

## Next Action

Use the JSON report to decide whether the next runtime-builder increment should
optimize the slowest runtime tests or proceed to release readiness.

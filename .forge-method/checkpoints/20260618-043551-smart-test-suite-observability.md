# Smart test suite observability

- created_at: 2026-06-18T04:35:51+00:00
- project: forge-method-core
- phase: 6-evolve
- status: validated
- workflow: guideline-audit
- active_story: <none>

## Summary

Added debug mode, JSON/JUnit reports, retained logs, match filtering, and report-driven failure/slowest reruns to the responsive test suite.

## Decisions

- Keep full test coverage, but make the suite observable through per-test reports and debug reruns instead of opaque unittest discovery.

## Checks

- py_compile passed
- runner self-tests passed
- verify-fast debug path passed
- bash wrapper syntax passed
- full responsive suite passed 133/133 in 199.4s

## Failed Checks

- none

## Touched Files

- scripts/test-runner.py
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- scripts/verify-all.ps1
- scripts/verify-all.sh
- tests/test_test_runner.py
- AGENTS.md
- docs/07-v1-readiness-audit.md
- assets/marketplace/listing.json

## Artifacts

- .forge-method/evidence/20260618-013448-smart-test-suite-observability.md

## Next Action

decide whether to optimize the slowest runtime tests or run release readiness

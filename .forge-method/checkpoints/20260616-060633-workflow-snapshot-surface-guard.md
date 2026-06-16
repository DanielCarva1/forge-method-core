# Workflow snapshot surface guard

- created_at: 2026-06-16T06:06:33+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-snapshot-surface-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the workflow snapshot visibility gap: snapshot quality now exposes workflow validation errors that workflow validate and gate already consume.

## Decisions

- Use workflow_validation_errors as the shared workflow validation surface for snapshot quality as well as gate and workflow validate.

## Checks

- focused workflow snapshot regression: passed
- python -m unittest discover -s tests: 123 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- audit/artifact verify/workflow validate/agent validate/config validate/builder validate: passed
- parity replay: 91/91 passed
- gate --require-evals: 22/22 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-workflow-snapshot-surface-guard.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-workflow-snapshot-surface-guard.md

## Next Action

Continue the post-parity Forge audit by checking remaining validation surfaces that are still command-only or hidden from snapshots, gate, audit, or install smoke coverage.

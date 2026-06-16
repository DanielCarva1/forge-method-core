# Agent validation gate surface guard

- created_at: 2026-06-16T05:13:43+00:00
- project: forge-method-core
- phase: 6-evolve
- status: agent-validation-gate-surface-guard
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the agent validation surface gap: gate now consumes the same profile, elicitation technique, and Persona Lens validation surface as agent validate, builder validate, and snapshots.

## Decisions

- Use agent_validation_errors as the canonical agent validation surface instead of repeating profile-only checks in gate.

## Checks

- python -m unittest tests.test_runtime.RuntimeTests.test_gate_uses_full_agent_validation_surface -v: passed
- python -m unittest discover -s tests: 120 passed
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1: passed
- audit/artifact verify/workflow validate/agent validate: passed
- parity replay: 91/91 passed
- gate --require-evals: 22/22 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-agent-validation-gate-surface-guard.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-agent-validation-gate-surface-guard.md

## Next Action

Continue the post-parity Forge audit by checking remaining command-specific validation surfaces that can still diverge from gate, audit, snapshots, or installed smoke coverage.

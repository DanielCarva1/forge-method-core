# Stale Guidance Guard hardened

- created_at: 2026-06-15T12:50:22+00:00
- project: forge-method-core
- phase: 6-evolve
- status: stale-guidance-guard-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Post-parity polish audit found structurally healthy packs/refs and stale internal guidance as the main agentic risk. Added Stale Guidance Guard to artifact verification, cleaned active parity audit/plan wording, recorded a durable polish audit, and validated source plus installed runtime.

## Decisions

- Guard active parity/audit/plan/benchmark artifacts against stale closed-work markers instead of relying on future agents to notice contradictions manually.

## Checks

- artifact verify --root .: passed
- workflow validate: passed
- workflow compactness: passed
- parity replay: 89/89 passed
- python -m unittest discover -s tests: 79 tests OK
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260615-post-parity-polish-audit.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md

## Artifacts

- .forge-method/artifacts/20260615-post-parity-polish-audit.md
- .forge-method/evidence/20260615-125002-validation-stale-guidance-guard-validation.md

## Next Action

Continue post-parity Forge polish with transcript-derived improvements only; keep artifact verify clean and avoid reopening closed parity rows without a failing transcript.

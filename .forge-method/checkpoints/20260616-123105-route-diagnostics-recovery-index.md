# Route Diagnostics Recovery Index

- created_at: 2026-06-16T12:31:05+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-selected
- workflow: config-customization
- active_story: <none>

## Summary

Recovery briefs and capability index now persist Help Oracle route diagnostics, including required workflow, reason, context boundary, stale-state guard, and route surfaces.

## Decisions

- Keep route diagnostics as compact runtime surfaces in recovery artifacts and generated capability index, not as chat-only guidance.

## Checks

- python -m unittest discover -s tests: 125 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- artifact verify: passed
- audit: passed
- gate --require-evals: 22/22 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/context/capability-index.json

## Artifacts

- .forge-method/artifacts/20260616-route-diagnostics-recovery-index.md
- .forge-method/evidence/20260616-122951-validation-route-diagnostics-recovery-index.md

## Next Action

Continue post-parity Forge audit with the next Guidance Engine-selected gap; persisted route diagnostics in recovery and capability index are complete.

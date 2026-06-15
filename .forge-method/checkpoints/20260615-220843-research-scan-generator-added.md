# Research scan generator added

- created_at: 2026-06-15T22:08:43+00:00
- project: forge-method-core
- phase: 6-evolve
- status: research-scan-generator-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact research-scan so market/domain/technical research closeouts are generated, registered, and validated before downstream planning.

## Decisions

- Use first-class runtime generators for stable phase-closeout artifacts; research scans now share validator, command, workflow handoff, tests, and source/install smoke coverage.

## Checks

- focused research-scan tests passed; workflow validate passed; workflow compactness passed; parity replay 90/90 passed; smoke-runtime.ps1 passed; smoke-install.ps1 passed; python -m unittest discover -s tests passed; verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/evidence-research.md
- skills/forge-method/references/workflow-market-scan.md
- skills/forge-method/references/workflow-domain-scan.md
- skills/forge-method/references/workflow-technical-feasibility-scan.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-research-scan-generator-contract.md

## Next Action

Continue post-parity Forge polish by adding game-check generator coverage for game brief and sprint planning closeouts.

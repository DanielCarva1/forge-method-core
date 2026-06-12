# Help Oracle and facilitation gate closed

- created_at: 2026-06-12T20:31:14+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p0-help-oracle-facilitation-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed BMAD parity P0.1/P0.2: Forge now exposes a Help Oracle required_next_workflow in snapshot/resume/next/transition, keeps active 6-evolve runtime-builder work despite readiness ready, and validates that human-facing workflows have facilitation packs. This is not full BMAD parity; next P0 is PRD/UX/Quick Dev depth.

## Decisions

- Do not claim full BMAD parity; this increment makes next-workflow guidance and facilitation coverage mechanically enforced.
- Keep BMAD as internal benchmark; Forge product docs stay Codex-native and independent.

## Checks

- python -m unittest discover -s tests: passed 61 tests
- python skills\\forge-method\\scripts\\forge_method_runtime.py workflow validate: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/*.md
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/evidence/20260612-203044-validation-help-oracle-and-facilitation-coverage-validation.md

## Next Action

Implement P0.3 PRD/UX/Quick Dev parity from the BMAD parity audit.

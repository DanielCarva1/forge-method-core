# Spec kernel generator added

- created_at: 2026-06-15T21:25:43+00:00
- project: forge-method-core
- phase: 6-evolve
- status: spec-kernel-generator-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact spec-kernel so write-spec can generate, register, and validate compact spec kernel handoff artifacts instead of hand-written markdown.

## Decisions

- Use first-class generators for phase-closing artifacts when a workflow has a stable template plus validator; research-scan is the next shared generator candidate.

## Checks

- focused tests, workflow validate, workflow compactness, parity replay, smoke-runtime, smoke-install, full unittest, and verify-fast passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-phase-closeout-generator-audit.md

## Next Action

Continue post-parity Forge polish by adding research-scan generator coverage for market/domain/technical evidence closeouts.

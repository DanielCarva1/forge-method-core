# Guided experience stress audit closed

- created_at: 2026-06-17T01:50:31+00:00
- project: forge-method-core
- phase: 6-evolve
- status: correct-course-continued
- workflow: correct-course
- active_story: <none>

## Summary

Compared BMAD guided flows against Forge, patched concrete human-guidance gaps, synced the installed plugin, and validated source plus installed behavior under broad idea, rushed/simple, confused/lost, brainstorm, research, drift/frustration, and correction scenarios.

## Decisions

- Treat Guidance Engine plus facilitation packs as the canonical human-guidance surface: compact state machine for agents, rich first questions and pace/style contract for humans.

## Checks

- python -m unittest discover -s tests: passed 126 tests
- scripts\verify-fast.ps1 -SkipUnit: passed
- scripts\smoke-runtime.ps1: passed
- scripts\install-plugin-local.ps1: passed
- scripts\smoke-install.ps1: passed
- installed guidance stress matrix: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/*.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1

## Artifacts

- .forge-method/artifacts/20260617-bmad-forge-guided-experience-stress-audit.md
- .forge-method/evidence/20260617-014953-validation-guided-experience-stress-audit.md

## Next Action

Use installed Forge in real projects and collect transcript regressions for future taste/facilitation tuning; no further parity patch is currently queued.

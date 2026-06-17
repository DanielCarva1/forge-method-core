# Guidance correction and focused verification loop

- created_at: 2026-06-16T20:59:51+00:00
- project: forge-method-core
- phase: 6-evolve
- status: post-parity-audit-queued
- workflow: runtime-builder
- active_story: <none>

## Summary

Fixed a Guidance Engine precedence gap where human complaints about Forge's own facilitation could route to runtime-builder because builder keywords outweighed human-experience failure signals. Added replay/unit proof for the user's complaint shape. Added focused verify-fast modes so local development can run one or more unit labels or skip unit tests while still running lightweight validators.

## Decisions

- Human-experience failure complaints are correct-course first; runtime-builder is only the repair path after the failure is named.

## Checks

- focused guidance regression passed
- full unittest discover passed: 125 tests
- smoke-runtime.ps1 passed
- verify-fast.ps1 passed
- verify-fast.ps1 -Test passed in 13.1s
- verify-fast.ps1 -SkipUnit passed in 2.7s
- bash verify-fast.sh --skip-unit passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- README.md
- docs/00-quickstart.md

## Artifacts

- .forge-method/evidence/20260616-205937-validation-guidance-correct-course-precedence-and-focused-v.md

## Next Action

Continue post-parity experience audit with focused verification for local iterations and full suite only at runtime validation boundaries.

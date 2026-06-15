# Correct Course and Problem Solving Depth hardened

- created_at: 2026-06-15T07:28:21+00:00
- project: forge-method-core
- phase: 6-evolve
- status: correct-course-problem-solving-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the remaining human-guidance depth gap for correction and stuck/problem-solving flows: correct-course and problem-solving now have compact templates, catalog modes, richer facilitation packs, stronger Guidance Engine signals/text, and replay fixtures covering scope, human-experience, implementation contradiction, and messy constraints.

## Decisions

- Keep rich human recovery guidance in facilitation packs and guide output while compact agent contracts live in workflow refs, catalog metadata, templates, state, and replay fixtures.

## Checks

- python -m unittest discover -s tests
- python skills/forge-method/scripts/forge_method_runtime.py workflow validate
- python skills/forge-method/scripts/forge_method_runtime.py workflow compactness
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- python skills/forge-method/scripts/forge_method_runtime.py config validate --root .
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/correct-course.md
- skills/forge-method/facilitation/problem-solving.md
- skills/forge-method/templates/correct-course-artifact.md
- skills/forge-method/templates/problem-solving-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-072752-validation-correct-course-and-problem-solving-depth-validat.md

## Next Action

Continue residual parity transcript hardening: inspect game dev-story/review examples, package/distribution depth, doc utility validation, and deferred API/browser or eval-runner surfaces only if repeated projects justify them.

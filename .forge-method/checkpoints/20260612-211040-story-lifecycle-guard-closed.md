# Story lifecycle guard closed

- created_at: 2026-06-12T21:10:40+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p0-story-lifecycle-guard-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed BMAD parity P0.4: Forge now has a story-creation workflow, story-flow Guidance Engine routing, a story lifecycle facilitation pack upgrade, and an audit guard that blocks implementation-ready build stories without accepted decision-source artifacts. This does not complete full BMAD parity; next P0 is a parity replay harness.

## Decisions

- Treat stories as execution artifacts generated from accepted decisions, not as substitutes for PRD/spec/UX/architecture/test/validation decisions.
- Route story lifecycle requests through Guidance Engine story-flow to story-creation, readiness-check, create-epics, or plan-sprint before build-story.
- Keep rich human story facilitation in facilitation/story-lifecycle.md and compact agent contract in references/workflow-story-creation.md.

## Checks

- python -m unittest discover -s tests: passed 62 tests
- python skills\\forge-method\\scripts\\forge_method_runtime.py workflow validate: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py audit --root .: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py artifact verify --root .: passed with only pre-existing correct-course stale-summary warning
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1: passed
- installed forge-method guide story-flow route: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-story-creation.md
- skills/forge-method/facilitation/story-lifecycle.md
- skills/forge-method/templates/story-creation-artifact.md
- tests/test_runtime.py
- tests/fixtures/guidance_transcripts.json

## Artifacts

- .forge-method/evidence/20260612-211014-validation-story-lifecycle-guard-validation.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Implement P0.5 Parity replay harness from the BMAD parity audit.

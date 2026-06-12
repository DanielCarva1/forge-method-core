# Guided-depth execution routing closed

- created_at: 2026-06-12T01:31:37+00:00
- project: forge-method-core
- phase: 6-evolve
- status: story-done
- workflow: evolve-project
- active_story: <none>

## Summary

Guided-depth facilitation packs now include domain examples, and Guidance Engine now returns transition-workflow commands with catalog-derived phases when an existing project asks to enter narrow guided-depth workflows. This turns the recommendation into an executable next runtime step.

## Decisions

- Narrow workflows with catalog modes are executable guidance targets; broad workflows remain recommendations unless another branch requires state update.

## Checks

- workflow validate: passed
- python -m unittest discover -s tests: passed
- scripts/smoke-runtime.ps1: passed
- scripts/smoke-install.ps1: passed
- scripts/verify-fast.ps1: passed
- gate --require-evals: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/facilitation/game-lifecycle.md; skills/forge-method/facilitation/test-architecture.md; skills/forge-method/facilitation/builder-utility.md; skills/forge-method/facilitation/document-utility.md; skills/forge-method/scripts/forge_method_runtime.py; tests/fixtures/guidance_transcripts.json; tests/test_runtime.py

## Artifacts

- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Completion audit against BMAD parity objective; remaining possible work is deeper per-workflow examples/templates and post-story automation, not routing foundation.

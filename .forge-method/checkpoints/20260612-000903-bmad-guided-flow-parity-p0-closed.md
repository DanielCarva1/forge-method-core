# BMAD guided-flow parity P0 closed

- created_at: 2026-06-12T00:09:03+00:00
- project: forge-method-core
- phase: 6-evolve
- status: story-done
- workflow: evolve-project
- active_story: <none>

## Summary

Forge now has a workflow catalog, facilitation packs for core human-guided workflows, Guidance Engine routing that keeps Forge/BMAD improvement requests in runtime-builder, catalog validation, and benchmark-backed tests. Remaining BMAD parity gaps are depth gaps in game/test/builder utility flows, not routing foundation gaps.

## Decisions

- Keep BMAD as internal behavior benchmark only; keep Forge product docs independent and Codex-native.

## Checks

- python -m unittest discover -s tests: passed
- scripts/smoke-runtime.ps1: passed
- scripts/verify-fast.ps1: passed
- scripts/smoke-install.ps1: passed
- gate --require-evals: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/catalog/workflows.json; skills/forge-method/facilitation/*.md; skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; tests/fixtures/guidance_transcripts.json; .forge-method/artifacts/guidance-engine-benchmark.md

## Artifacts

- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Plan the next parity increment for deep game/test/builder utility flows: game brief depth, game story lifecycle, test architecture lifecycle, workflow/agent analysis, and spec/document utility workflows.

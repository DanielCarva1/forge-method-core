# BMAD guided-depth P1 closed

- created_at: 2026-06-12T00:41:56+00:00
- project: forge-method-core
- phase: 6-evolve
- status: story-done
- workflow: evolve-project
- active_story: <none>

## Summary

Forge now has specialized guided-depth workflow families for game lifecycle, test architecture, builder utility, and document utility. Guidance Engine routes explicit lifecycle jobs to narrow workflows instead of collapsing to broad game-brief, test-strategy, runtime-builder, or domain-scan. Remaining parity work is qualitative depth: richer per-step prompts, artifact templates for every new workflow, and end-to-end story-cycle automation.

## Decisions

- Keep the single Forge entrypoint; deepen tracks through cataloged depth workflows and separate facilitation packs rather than adding public slash commands.

## Checks

- python -m unittest discover -s tests: passed
- scripts/smoke-runtime.ps1: passed
- scripts/verify-fast.ps1: passed
- scripts/smoke-install.ps1: passed
- gate --require-evals: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/references/workflow-game-*.md; skills/forge-method/references/workflow-test-*.md; skills/forge-method/references/workflow-agent-analyze.md; skills/forge-method/references/workflow-workflow-analyze.md; skills/forge-method/references/workflow-skill-convert.md; skills/forge-method/references/workflow-doc-*.md; skills/forge-method/facilitation/*-utility.md; skills/forge-method/facilitation/game-lifecycle.md; skills/forge-method/facilitation/test-architecture.md; skills/forge-method/catalog/workflows.json; skills/forge-method/scripts/forge_method_runtime.py; tests/fixtures/guidance_transcripts.json; tests/test_runtime.py

## Artifacts

- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Plan remaining qualitative depth: artifact templates and richer per-step facilitation for the new depth workflows, then automate story-cycle transitions around those workflows.

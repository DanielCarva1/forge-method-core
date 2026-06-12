# Guided-depth templates closed

- created_at: 2026-06-12T01:11:21+00:00
- project: forge-method-core
- phase: 6-evolve
- status: story-done
- workflow: evolve-project
- active_story: <none>

## Summary

Guided-depth workflows now have family artifact templates, catalog template metadata, validation for template references, and fixture coverage proving Guidance Engine returns template metadata for game lifecycle, test architecture, builder utility, and document utility routes.

## Decisions

- Use family templates as the default artifact shape for depth workflows; add narrower per-workflow templates later only where a family template is too broad.

## Checks

- workflow validate: passed
- python -m unittest discover -s tests: passed
- scripts/verify-fast.ps1: passed
- scripts/smoke-runtime.ps1: passed
- scripts/smoke-install.ps1: passed
- gate --require-evals: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/templates/*-artifact.md; skills/forge-method/catalog/workflows.json; skills/forge-method/scripts/forge_method_runtime.py; skills/forge-method/facilitation/*.md; tests/fixtures/guidance_transcripts.json; tests/test_runtime.py

## Artifacts

- none

## Next Action

Plan remaining qualitative parity work: richer per-step domain examples and automation for story-cycle transitions around guided-depth workflows.

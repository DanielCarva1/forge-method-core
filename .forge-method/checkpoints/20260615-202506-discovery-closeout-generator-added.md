# Discovery closeout generator added

- created_at: 2026-06-15T20:25:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-generator-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact discovery-closeout, a packaged discovery-closeout-artifact template, discover-intent template metadata, and workflow handoff docs.

## Decisions

- Discovery closeout creation is now a first-class runtime command; agents should not hand-roll the required markdown fields.

## Checks

- focused tests, workflow validate, smokes, parity replay, full unittest, verify-fast passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/templates/discovery-closeout-artifact.md
- skills/forge-method/references/workflow-discover-intent.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-discovery-closeout-generator-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by improving human-facing discovery closeout guidance so artifact discovery-closeout arguments can be derived from a guided conversation cleanly.

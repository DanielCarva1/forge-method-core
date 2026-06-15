# P1.7 parity closure utilities closed

- created_at: 2026-06-15T02:34:58+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p1-parity-closure-utilities-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed P1.7 Parity Closure Utilities. Added investigation, working-backwards-challenge, sprint-status, checkpoint-preview, and adversarial-review as routeable Forge workflows with compact refs, templates, catalog/module membership, Guidance Engine routes, parity replay fixtures, refreshed Capability Index, and adversarial routing precedence.

## Decisions

- Use compact workflow refs/templates for agent handoff and keep human richness in existing facilitation packs plus guide output.
- Explicit adversarial/red-team requests outrank generic quality review when the document router detects assumption attack.

## Checks

- python -m unittest discover -s tests: 70 tests OK
- workflow validate: passed
- parity replay: 58/58 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed; installed parity replay 58/58
- artifact verify: passed
- audit: passed
- config validate: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/references/workflow-*.md
- skills/forge-method/templates/*-artifact.md
- skills/forge-method/modules/*.yaml
- tests/test_runtime.py
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/context/capability-index.json
- CHANGELOG.md

## Artifacts

- .forge-method/evidence/20260615-023334-validation-p1-7-parity-closure-utilities-validation.md
- .forge-method/artifacts/guidance-engine-benchmark.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md

## Next Action

Review the Unreleased changelog as one coherent version batch, then decide tag/publish versus real-use transcript hardening.

# Research Guidance Depth hardened

- created_at: 2026-06-15T11:10:04+00:00
- project: forge-method-core
- phase: 6-evolve
- status: research-guidance-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed market/domain/technical research strong-ish rows with narrow research routing, richer evidence-research human guidance, research-scan template, artifact research-check, replay cases, and validation proof.

## Decisions

- Research requests now route by uncertainty type: market for alternatives/adoption/demand, domain for rules/risks/review, technical feasibility for capability/data/proof path.
- Research scan artifacts must preserve decision_to_unlock, source quality, contradictions/falsifiers, uncertainty, stance, next workflow, and research-check validation.

## Checks

- targeted unittest passed
- parity replay 85/85 passed
- python -m unittest discover -s tests passed: 77 tests
- smoke-runtime.ps1 passed
- smoke-install.ps1 passed
- verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/evidence-research.md
- skills/forge-method/templates/research-scan-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-110943-validation-research-guidance-depth-validation.md

## Next Action

Continue residual game guidance hardening, especially game-brief, brainstorm-game, and game sprint planning transcript proof.

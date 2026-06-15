# Presentation Craft Fold-In hardened

- created_at: 2026-06-15T12:23:39+00:00
- project: forge-method-core
- phase: 6-evolve
- status: presentation-craft-fold-in-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Resolved the /cis-agent-presentation-master parity contradiction by folding pitch/deck narrative into storytelling with a presentation-craft Persona Lens. Added presentation routing signals, precedence over document/game routes, medium/presentation_outline/call_to_action fields, richer storytelling facilitation, replay fixture, audit/P2/benchmark/changelog updates, and capability index regeneration.

## Decisions

- Do not create a visual deck-production workflow now. Forge supports pitch/deck narrative through storytelling and keeps visual deck production deferred until it becomes explicit Forge project scope.

## Checks

- parity replay 89/89 passed
- python -m unittest discover -s tests => 78 tests OK
- smoke-runtime and smoke-install passed
- verify-fast passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/personas/overlays.json
- skills/forge-method/facilitation/storytelling.md
- skills/forge-method/references/workflow-storytelling.md
- skills/forge-method/templates/storytelling-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-122252-validation-presentation-craft-fold-in-validation.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260615-p2-scope-decisions-and-polish-plan.md

## Next Action

Begin post-parity Forge polish: audit facilitation packs for thin or generic human guidance and compact workflow refs for misleading or bloated agent instructions.

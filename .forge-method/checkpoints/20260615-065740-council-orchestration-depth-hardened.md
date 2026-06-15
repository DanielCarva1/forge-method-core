# Council Orchestration Depth hardened

- created_at: 2026-06-15T06:57:40+00:00
- project: forge-method-core
- phase: 6-evolve
- status: council-orchestration-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed party-mode and subagent-orchestration parity gaps: council-decision now has a dedicated pack/template/modes, natural Guidance Engine routing, richer live debate rounds, compact dissent/orchestration artifact contract, JSON worker/merge plan, catalog/fixture/test coverage, regenerated capability index, and validation evidence.

## Decisions

- Council is Human Experience first: show useful live specialist debate to the human, but persist only compact decision, dissent, evidence, worker-output, merge, and next-action contracts.
- Subagent/parallel mode changes orchestration style only; artifact contracts stay stable and the runtime falls back to sequential council when real subagents are unavailable or outputs are not independent.

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
- skills/forge-method/references/workflow-council-decision.md
- skills/forge-method/facilitation/council-decision.md
- skills/forge-method/templates/council-decision-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-065658-validation-council-orchestration-depth-validation.md

## Next Action

Continue real-use transcript hardening for remaining strong-ish rows; next inspect correct-course breadth, problem-solving depth, and any game/code-review examples that still feel generic before claiming full guided-flow parity.

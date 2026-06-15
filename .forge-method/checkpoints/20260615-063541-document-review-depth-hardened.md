# Document Review Depth hardened

- created_at: 2026-06-15T06:35:41+00:00
- project: forge-method-core
- phase: 6-evolve
- status: document-review-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the editorial-review and edge-case hunter guidance gap: added specialized templates, richer document-utility facilitation, catalog modes, Guidance Engine routing/precedence, replay fixtures, benchmark/audit/plan/changelog updates, regenerated capability index, and validation evidence.

## Decisions

- Specialized document review outranks generic quality review only for explicit document-review intents or non-quality edge/adversarial/editorial wording; strong ATDD/test/QA/CI intent stays in quality-flow.

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
- skills/forge-method/facilitation/document-utility.md
- skills/forge-method/references/workflow-editorial-review.md
- skills/forge-method/references/workflow-edge-case-review.md
- skills/forge-method/templates/editorial-review-artifact.md
- skills/forge-method/templates/edge-case-review-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-063437-validation-document-review-depth-validation.md

## Next Action

Continue real-use transcript hardening for remaining partial and strong-ish rows; next inspect party-mode/council and subagent orchestration gaps before claiming full guided-flow parity.

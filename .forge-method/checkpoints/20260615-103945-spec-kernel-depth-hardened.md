# Spec Kernel Depth hardened

- created_at: 2026-06-15T10:39:45+00:00
- project: forge-method-core
- phase: 6-evolve
- status: spec-kernel-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the generic spec-kernel gap by turning write-spec into the Forge-native compact WHAT contract: spec-kernel template, create/update/distill/validate modes, stable CAP ID and intent/success rules, companion/source map, decision log, preservation map, validation verdict, artifact spec-check, Guidance Engine routing, replay proof, and product-planning facilitation depth.

## Decisions

- write-spec is the lean spec-kernel workflow; product-requirements remains the richer PRD/addendum workflow.
- Spec-kernel requests outrank document-utility distillation when the human asks for create/update/validate/distill spec, SPEC.md, stable capabilities, or machine contract.
- Spec kernels must preserve load-bearing source claims in the kernel, companions, adopted sources, or open questions; silent drops are invalid.

## Checks

- Targeted unittest: 6 tests OK
- Parity replay: 83/83 passed
- Workflow validate, compactness, and config validate passed
- python -m unittest discover -s tests: 76 tests OK
- smoke-runtime.ps1, smoke-install.ps1, and verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-write-spec.md
- skills/forge-method/facilitation/product-planning.md
- skills/forge-method/templates/spec-kernel-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-103921-validation-spec-kernel-depth-validation.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Continue residual parity hardening; prioritize research and game-brief strong-ish rows where transcript evidence still shows drift.

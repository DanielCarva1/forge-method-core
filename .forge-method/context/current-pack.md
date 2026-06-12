# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: p0-prd-ux-quick-dev-done
- workflow: runtime-builder
- active_story: <none>
- next_action: Implement P0.4 Story lifecycle guard from the BMAD parity audit.

## Latest Checkpoint

# PRD UX Quick Dev parity closed

- created_at: 2026-06-12T20:47:51+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p0-prd-ux-quick-dev-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed BMAD parity P0.3: Forge now routes PRD, UX, and quick-dev requests through product-flow with executable transition commands. Product requirements and UX workflows have create/update/validate metadata and compact artifact templates. Quick-dev now exists as spec-lite workflow, facilitation pack, template, catalog entry, module workflow, and transcript fixture. This does not complete full BMAD parity; next P0 is story lifecycle guard.

## Decisions

- Translate product/UX/quick-dev behavior into Forge-native workflows, packs, templates, fixtures, and runtime routing rather than copying benchmark wording.
- Keep product-facing docs independent and describe the feature as Forge Guidance Engine/product-flow behavior.

## Checks

- python -m unittest discover -s tests: passed 61 tests
- python skills\\forge-method\\scripts\\forge_method_runtime.py workflow validate: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1: passed
- installed forge-method guide PRD/UX/quick-dev route checks: passed
- audit: passed
- artifact verify: passed with only pre-existing correct-course stale-summary warning

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-product-requirements.md
- skills/forge-method/references/workflow-
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- tests/fixtures/guidance_transcripts.json
- .forge-method/artifacts/guidance-engine-benchmark.md
- docs/adr/0008-guidance-engine.md
- install.ps1
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/*.md
- CHANGELOG.md
- skills/forge-method/references/workflow-product-requirements.md
- skills/forge-method/references/workflow-ux-plan.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-183040-validation-guidance-experience-final-validation.md
- .forge-method/evidence/20260612-183453-validation-guidance-experience-install-validation.md
- .forge-method/evidence/20260612-200602-audit-bmad-forge-systematic-parity-audit.md
- .forge-method/evidence/20260612-203044-validation-help-oracle-and-facilitation-coverage-validation.md
- .forge-method/evidence/20260612-204726-validation-prd-ux-quick-dev-parity-validation.md

## Recent Artifacts

- correct-course [active/durable]: .forge-method/artifacts/20260612-144025-correct-course-correct-course-continuation.md - Correct-course continuation - A conversa corrigiu a premissa: o problema de performance/travamento e do Codex como superficie, nao causado pelo Forge plugin. Experimentos TS/Rust/hooks devem ser encerrados como forks ativos.

Impact: Evita otimizar o Forge plugin para um problema que pertence a superficie Codex e preserva a decisao de continuar refinando o Forge como plugin enquanto a ideia de app proprio fica em pesquisa futura..

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: remover worktrees/branches experimentais e criar um artifact de referencia com pesquisa, resultados e decisao atual.
- research-reference [archived-reference/durable]: .forge-method/artifacts/20260612-independent-app-research-and-experiment-reference.md - Independent app research and experiment reference - Preserves the TS/Rust/hooks experiment findings, records that Codex instability is outside the Forge plugin boundary, and defers independent app work to a future Rust-core product track.
- correct-course [active/durable]: .forge-method/artifacts/20260612-180403-correct-course-correct-course-continuation.md - Correct-course continuation - Forge human guidance treated a critique of the method experience as generic builder work, and new project creation could seed ready stories before facilitated discovery.

Impact: New users could receive technical artifacts and stories before taste, pain, theme, UX, or route facilitation, then get procedural confirmations instead of guided or autonomous progress.

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: Keep initial projects gated by facilitation input, route method-experience criticism to correct-course first, and validate with transcript fixtures plus runtime smoke.
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Systematic first-pass parity audit comparing BMAD Method, Builder, CIS, Game Dev Studio, and TEA against Forge principles, workflows, facilitation packs, runtime contracts, scripts, state, and validation.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Updated internal benchmark to include product requirements, UX planning, and quick-dev routing expectations alongside correct-course, research, brainstorm, game, builder, document, and quality routes.

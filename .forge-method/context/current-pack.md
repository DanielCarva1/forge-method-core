# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: correct-course-continued
- workflow: correct-course
- active_story: <none>
- next_action: Use /forge-reload in a fresh project and judge the live first-run facilitation; if it still feels thin, deepen facilitation packs and transcript replay rather than creating stories early.

## Latest Checkpoint

# Guidance experience installed validation

- created_at: 2026-06-12T18:35:07+00:00
- project: forge-method-core
- phase: 6-evolve
- status: correct-course-continued
- workflow: correct-course
- active_story: <none>

## Summary

Synchronized the installed Forge skill after source changes and verified the installed runtime. The user can now test /forge-reload in another project against the corrected runtime, not the stale installed copy.

## Decisions

- The local installed skill must be refreshed after source runtime changes, otherwise live Forge usage keeps old behavior.

## Checks

- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1: passed
- Installed runtime hash matches repo runtime hash.

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- install.ps1

## Artifacts

- .forge-method/evidence/20260612-183453-validation-guidance-experience-install-validation.md

## Next Action

Use /forge-reload in a fresh project and judge the live first-run facilitation; if it still feels thin, deepen facilitation packs and transcript replay rather than creating stories early.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- .forge-method/artifacts/20260612-independent-app-research-and-experiment-reference.md
- .forge-method/artifacts/20260612-144025-correct-course-correct-course-continuation.md
- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- tests/fixtures/guidance_transcripts.json
- .forge-method/artifacts/guidance-engine-benchmark.md
- docs/adr/0008-guidance-engine.md
- install.ps1

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-044523-validation-script-audit-optimization-validation.md
- .forge-method/evidence/20260612-044806-gate-quality-gate.md
- .forge-method/evidence/20260612-181924-validation-guidance-experience-correct-course-validation.md
- .forge-method/evidence/20260612-183040-validation-guidance-experience-final-validation.md
- .forge-method/evidence/20260612-183453-validation-guidance-experience-install-validation.md

## Recent Artifacts

- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Internal behavior benchmark for route-aware human guidance, runtime audit routing, narrow guided-depth transitions, correct-course, research, brainstorm, game, builder, quality, document utility, and mechanical build routing.
- story-link [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - .forge-method/artifacts/guidance-engine-benchmark.md -> script-audit-optimization-p1 - Artifact linked to story.
- correct-course [active/durable]: .forge-method/artifacts/20260612-144025-correct-course-correct-course-continuation.md - Correct-course continuation - A conversa corrigiu a premissa: o problema de performance/travamento e do Codex como superficie, nao causado pelo Forge plugin. Experimentos TS/Rust/hooks devem ser encerrados como forks ativos.

Impact: Evita otimizar o Forge plugin para um problema que pertence a superficie Codex e preserva a decisao de continuar refinando o Forge como plugin enquanto a ideia de app proprio fica em pesquisa futura..

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: remover worktrees/branches experimentais e criar um artifact de referencia com pesquisa, resultados e decisao atual.
- research-reference [archived-reference/durable]: .forge-method/artifacts/20260612-independent-app-research-and-experiment-reference.md - Independent app research and experiment reference - Preserves the TS/Rust/hooks experiment findings, records that Codex instability is outside the Forge plugin boundary, and defers independent app work to a future Rust-core product track.
- correct-course [active/durable]: .forge-method/artifacts/20260612-180403-correct-course-correct-course-continuation.md - Correct-course continuation - Forge human guidance treated a critique of the method experience as generic builder work, and new project creation could seed ready stories before facilitated discovery.

Impact: New users could receive technical artifacts and stories before taste, pain, theme, UX, or route facilitation, then get procedural confirmations instead of guided or autonomous progress.

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: Keep initial projects gated by facilitation input, route method-experience criticism to correct-course first, and validate with transcript fixtures plus runtime smoke.

# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: parity-audit-recorded
- workflow: runtime-builder
- active_story: <none>
- next_action: Implement P0.1 Help Oracle invariant and P0.2 facilitation coverage gate from the BMAD parity audit.

## Latest Checkpoint

# BMAD Forge parity audit recorded

- created_at: 2026-06-12T20:06:14+00:00
- project: forge-method-core
- phase: 6-evolve
- status: parity-audit-recorded
- workflow: runtime-builder
- active_story: <none>

## Summary

Recorded a systematic first-pass BMAD-to-Forge parity audit. The audit covers BMAD Method core, Builder, CIS, Game Dev Studio, and TEA; maps command/token families to Forge equivalents; and identifies P0 gaps in Help Oracle, facilitation coverage, PRD/UX/Quick Dev depth, story lifecycle proof, and parity replay harness.

## Decisions

- Do not claim complete BMAD parity yet; this artifact proves the gap map, not gap closure.
- Translate BMAD behavior into Forge-native human facilitation plus compact agent runtime contracts.
- Next implementation should start with P0.1 Help Oracle invariant and P0.2 facilitation coverage gate.

## Checks

- artifact verify: passed, with only pre-existing stale warnings for older artifacts
- audit: passed
- workflow validate: passed

## Failed Checks

- none

## Touched Files

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md

## Artifacts

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/evidence/20260612-200602-audit-bmad-forge-systematic-parity-audit.md

## Next Action

Implement P0.1 Help Oracle invariant and P0.2 facilitation coverage gate from the BMAD parity audit.

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
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-044806-gate-quality-gate.md
- .forge-method/evidence/20260612-181924-validation-guidance-experience-correct-course-validation.md
- .forge-method/evidence/20260612-183040-validation-guidance-experience-final-validation.md
- .forge-method/evidence/20260612-183453-validation-guidance-experience-install-validation.md
- .forge-method/evidence/20260612-200602-audit-bmad-forge-systematic-parity-audit.md

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
- internal-parity-audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD to Forge systematic parity audit - Systematic first-pass parity audit comparing BMAD Method, Builder, CIS, Game Dev Studio, and TEA against Forge principles, workflows, facilitation packs, runtime contracts, scripts, state, and validation.

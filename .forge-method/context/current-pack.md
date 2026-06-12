# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: story-done
- workflow: evolve-project
- active_story: <none>
- next_action: Continue refining Forge Method as a Codex plugin; revisit independent Rust-core app as a future product track.

## Latest Checkpoint

# Independent app experiments archived

- created_at: 2026-06-12T14:43:22+00:00
- project: forge-method-core
- phase: 6-evolve
- status: story-done
- workflow: evolve-project
- active_story: <none>

## Summary

User clarified that Codex instability is not a Forge plugin performance defect. Archived TS/Rust/hooks experiment findings into a durable research reference, removed experiment worktrees and deleted local/remote experiment branches, and kept the current direction on refining Forge as a Codex plugin.

## Decisions

- Independent Forge app remains future research with likely Rust core and TypeScript UI; no TS/Rust experiment branch remains active now.

## Checks

- worktree list contains only the core worktree
- remote codex/experiment-* heads deleted
- artifact and ledger ndjson parse successfully

## Failed Checks

- none

## Touched Files

- .forge-method/artifacts/20260612-independent-app-research-and-experiment-reference.md
- .forge-method/artifacts/20260612-144025-correct-course-correct-course-continuation.md

## Artifacts

- .forge-method/artifacts/20260612-independent-app-research-and-experiment-reference.md
- .forge-method/artifacts/20260612-144025-correct-course-correct-course-continuation.md

## Next Action

Continue refining Forge Method as a Codex plugin; revisit independent Rust-core app as a future product track.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- .forge-method/artifacts/20260612-independent-app-research-and-experiment-reference.md
- .forge-method/artifacts/20260612-144025-correct-course-correct-course-continuation.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-024829-validation-human-facilitation-depth-validation.md
- .forge-method/evidence/20260612-035148-validation-post-release-guidance-audit.md
- .forge-method/evidence/20260612-044523-gate-quality-gate.md
- .forge-method/evidence/20260612-044523-validation-script-audit-optimization-validation.md
- .forge-method/evidence/20260612-044806-gate-quality-gate.md

## Recent Artifacts

- story-link [active/durable]: .forge-method/artifacts/script-audit-optimization.md - .forge-method/artifacts/script-audit-optimization.md -> script-audit-optimization-p1 - Artifact linked to story.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Internal behavior benchmark for route-aware human guidance, runtime audit routing, narrow guided-depth transitions, correct-course, research, brainstorm, game, builder, quality, document utility, and mechanical build routing.
- story-link [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - .forge-method/artifacts/guidance-engine-benchmark.md -> script-audit-optimization-p1 - Artifact linked to story.
- correct-course [active/durable]: .forge-method/artifacts/20260612-144025-correct-course-correct-course-continuation.md - Correct-course continuation - A conversa corrigiu a premissa: o problema de performance/travamento e do Codex como superficie, nao causado pelo Forge plugin. Experimentos TS/Rust/hooks devem ser encerrados como forks ativos.

Impact: Evita otimizar o Forge plugin para um problema que pertence a superficie Codex e preserva a decisao de continuar refinando o Forge como plugin enquanto a ideia de app proprio fica em pesquisa futura..

Policy: choose the conservative interpretation that preserves the approved spec.

Continuation: remover worktrees/branches experimentais e criar um artifact de referencia com pesquisa, resultados e decisao atual.
- research-reference [archived-reference/durable]: .forge-method/artifacts/20260612-independent-app-research-and-experiment-reference.md - Independent app research and experiment reference - Preserves the TS/Rust/hooks experiment findings, records that Codex instability is outside the Forge plugin boundary, and defers independent app work to a future Rust-core product track.

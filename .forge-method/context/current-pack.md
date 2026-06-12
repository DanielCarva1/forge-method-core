# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: story-done
- workflow: evolve-project
- active_story: <none>
- next_action: select next ready story or move to ready when build scope is complete

## Latest Checkpoint

# Human facilitation depth enforced

- created_at: 2026-06-12T02:48:59+00:00
- project: forge-method-core
- phase: 6-evolve
- status: story-done
- workflow: evolve-project
- active_story: <none>

## Summary

Responded to the correction that BMAD being stronger in facilitation is a product gap. Forge now treats rich human facilitation as required: referenced packs must include stage scripts, elicitation options, facilitator moves, quality bars, and anti-patterns. Agent workflow docs stay compact. Tests, workflow validate, smoke-runtime, smoke-install, verify-fast, and gate with evals passed.

## Decisions

- none

## Checks

- none

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

select next ready story or move to ready when build scope is complete

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/templates/*-artifact.md; skills/forge-method/catalog/workflows.json; skills/forge-method/scripts/forge_method_runtime.py; skills/forge-method/facilitation/*.md; tests/fixtures/guidance_transcripts.json; tests/test_runtime.py
- skills/forge-method/facilitation/game-lifecycle.md; skills/forge-method/facilitation/test-architecture.md; skills/forge-method/facilitation/builder-utility.md; skills/forge-method/facilitation/document-utility.md; skills/forge-method/scripts/forge_method_runtime.py; tests/fixtures/guidance_transcripts.json; tests/test_runtime.py
- skills/forge-method/references/workflow-teach-testing.md; skills/forge-method/catalog/workflows.json; skills/forge-method/modules/test-architect.yaml; skills/forge-method/facilitation/test-architecture.md; skills/forge-method/scripts/forge_method_runtime.py; tests/fixtures/guidance_transcripts.json; tests/test_runtime.py; local-comparison/bmad-forge-guided-flow-comparison.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260612-011021-validation-guided-depth-template-validation.md
- .forge-method/evidence/20260612-013040-validation-guided-depth-execution-routing-validation.md
- .forge-method/evidence/20260612-014938-validation-teach-testing-guided-workflow-validation.md
- .forge-method/evidence/20260612-020414-validation-bmad-parity-audit-cleanup-validation.md
- .forge-method/evidence/20260612-024829-validation-human-facilitation-depth-validation.md

## Recent Artifacts

- story-link [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - .forge-method/artifacts/guidance-engine-benchmark.md -> guided-depth-p1 - Artifact linked to story.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Internal behavior benchmark for route-aware human guidance, narrow guided-depth transition commands, domain examples, correct-course, research, brainstorm, game, builder, quality, document utility, and mechanical build routing.
- story-link [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - .forge-method/artifacts/guidance-engine-benchmark.md -> guided-depth-execution-p1 - Artifact linked to story.
- internal-benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine internal benchmark - Internal behavior benchmark for route-aware human guidance, narrow guided-depth transition commands, teach-testing, domain examples, correct-course, research, brainstorm, game, builder, quality, document utility, and mechanical build routing.
- story-link [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - .forge-method/artifacts/guidance-engine-benchmark.md -> teach-testing-gap-p1 - Artifact linked to story.

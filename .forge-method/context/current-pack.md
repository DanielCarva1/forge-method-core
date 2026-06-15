# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: document-utility-freshness-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue residual real-use transcript hardening; expand API/browser or eval-runner surfaces only if repeated projects justify them.

## Latest Checkpoint

# Document Utility Freshness hardened

- created_at: 2026-06-15T09:04:24+00:00
- project: forge-method-core
- phase: 6-evolve
- status: document-utility-freshness-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact doc-check and tightened doc-index/doc-shard contracts so documentation utility work records source fingerprint, source mtime, stale-check proof, original-document handling, precedence rules, and stale waivers.

## Decisions

- Index/shard parity is now represented as a Forge-native freshness validation contract rather than only facilitation prose.

## Checks

- parity replay, workflow validation, compactness, config validation/index, unittest, smoke-runtime, smoke-install, and verify-fast passed.

## Failed Checks

- none

## Touched Files

- Guidance Engine document routing, artifact doc-check runtime command, doc-index/doc-shard workflows, document-utility pack/template, catalog modes, replay fixtures, benchmark/audit/plan/changelog, and runtime tests.

## Artifacts

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md

## Next Action

Continue residual real-use transcript hardening; expand API/browser or eval-runner surfaces only if repeated projects justify them.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references/workflow-council-decision.md
- skills/forge-method/facilitation/council-decision.md
- skills/forge-method/templates/council-decision-artifact.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- skills/forge-method/facilitation/correct-course.md
- skills/forge-method/facilitation/problem-solving.md
- skills/forge-method/templates/correct-course-artifact.md
- skills/forge-method/templates/problem-solving-artifact.md
- skills/forge-method/templates/game-story-artifact.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-065658-validation-council-orchestration-depth-validation.md
- .forge-method/evidence/20260615-072752-validation-correct-course-and-problem-solving-depth-validat.md
- .forge-method/evidence/20260615-080127-validation-game-production-depth-hardening-validation.md
- .forge-method/evidence/20260615-083039-validation-module-distribution-depth-validation.md
- .forge-method/evidence/20260615-090424-validation-document-utility-freshness-validation.md

## Recent Artifacts

- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark - Benchmark updated with Game Production Depth: game-story-creation, game-sprint-status, game-test-framework, game-e2e-scaffold, and dev-story/build-story routing targets.
- audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - Module Distribution Depth audit update - Systematic parity audit now marks module builder setup/package and package distribution rows translated through Module Distribution Depth.
- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Module Distribution Depth plan update - Systematic parity plan now records Module Distribution Depth as completed and moves next focus to doc utility validation and deferred surfaces.
- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Module Distribution Depth benchmark update - Guidance Engine benchmark now includes module-distribution target behavior and fixture workflow id.
- changelog [active/durable]: CHANGELOG.md - Module Distribution Depth changelog note - Unreleased changelog records the module distribution guidance/runtime increment.

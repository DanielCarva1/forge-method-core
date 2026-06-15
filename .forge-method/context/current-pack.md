# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: enterprise-artifact-map-depth-hardened
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue residual parity hardening; prioritize bmad-spec depth and research/game brief strong-ish rows where transcript evidence still shows drift.

## Latest Checkpoint

# Enterprise Artifact Map Depth hardened

- created_at: 2026-06-15T10:16:14+00:00
- project: forge-method-core
- phase: 6-evolve
- status: enterprise-artifact-map-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the enterprise track/readiness gap by making enterprise projects carry explicit required and conditional artifact maps through track-decision, readiness-check, and release-readiness. Added artifact enterprise-check, enterprise templates, catalog metadata, workflow contracts, facilitation depth, replay fixtures, and benchmark/audit/changelog updates.

## Decisions

- Enterprise routing stays narrow: phrases like enterprise artifact map/readiness map route to lifecycle, but enterprise alone does not override quality routing.
- Enterprise baseline artifacts are risk-register, security-plan, privacy-data-plan, test-strategy, ci-quality-pipeline, nfr-evidence-audit, traceability-gate, and release-readiness; DevOps, compliance, and observability are conditional artifacts that must be named or explained.

## Checks

- Targeted unittest: 7 tests OK
- Parity replay: 82/82 passed
- Workflow validate, compactness, and config validate passed
- python -m unittest discover -s tests: 75 tests OK
- smoke-runtime.ps1, smoke-install.ps1, and verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references
- skills/forge-method/facilitation
- skills/forge-method/templates
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-101549-validation-enterprise-artifact-map-depth-validation.md
- .forge-method/arti
[checkpoint truncated]

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/templates/game-story-artifact.md
- skills/forge-method/templates/game-sprint-status-artifact.md
- skills/forge-method/templates/build-story-work-order.md
- skills/forge-method/references/workflow-build-story.md
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py
- Guidance Engine routing, workflow catalog, runtime-builder module, builder facilitation, module builder/validate workflows, distribution template, benchmark/audit docs, and runtime tests.
- Guidance Engine document routing, artifact doc-check runtime command, doc-index/doc-shard workflows, document-utility pack/template, catalog modes, replay fixtures, benchmark/audit/plan/changelog, and runtime tests.
- Guidance Engine quality routing, artifact test-check runtime command, test framework/automation/game E2E workflows and templates, game/test facilitation packs, workflow catalog, replay fixture, benchmark/audit/plan/changelog, runtime tests, capability index
- skills/forge-method/references

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260615-080127-validation-game-production-depth-hardening-validation.md
- .forge-method/evidence/20260615-083039-validation-module-distribution-depth-validation.md
- .forge-method/evidence/20260615-090424-validation-document-utility-freshness-validation.md
- .forge-method/evidence/20260615-093459-validation-e2e-test-automation-depth-validation.md
- .forge-method/evidence/20260615-101549-validation-enterprise-artifact-map-depth-validation.md

## Recent Artifacts

- benchmark [active/durable]: .forge-method/artifacts/guidance-engine-benchmark.md - Guidance Engine benchmark enterprise artifact map target - Updated benchmark target to require enterprise track decisions to produce required and conditional artifact maps, evidence status, waiver policy, and readiness/release gate consumers.
- audit [active/durable]: .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md - BMAD parity audit enterprise artifact map closure - Updated systematic parity audit to mark enterprise security/privacy/devops/compliance as translated through Enterprise Artifact Map Depth and artifact enterprise-check, while keeping full parity open for residual partial/strong-ish rows.
- plan [active/durable]: .forge-method/artifacts/20260613-systematic-parity-plan.md - Systematic parity plan enterprise artifact map update - Updated parity plan current status and next focus after Enterprise Artifact Map Depth, with remaining focus on bmad-spec depth and research/game brief transcript proof.
- capability-index [active/durable]: .forge-method/context/capability-index.json - Capability index after enterprise artifact map depth - Regenerated capability index after adding enterprise templates, workflow metadata, and artifact enterprise-check command coverage.
- changelog [active/durable]: CHANGELOG.md - Changelog enterprise artifact map depth - Recorded Unreleased changelog entry for Enterprise Artifact Map Depth, including artifact enterprise-check, artifact maps, waiver policy, templates, and replay coverage.

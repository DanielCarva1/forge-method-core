# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: durable-runtime-guidance-source-guard
- workflow: agent-analyze
- active_story: <none>
- next_action: Continue the post-parity Forge audit by checking dead code, misleading docs, and remaining runtime surfaces that lack deterministic validation.

## Latest Checkpoint

# Durable runtime guidance source guard

- created_at: 2026-06-16T04:14:37+00:00
- project: forge-method-core
- phase: 6-evolve
- status: durable-runtime-guidance-source-guard
- workflow: agent-analyze
- active_story: <none>

## Summary

Closed the durable runtime guidance source guard. Artifact index summaries, human input prompts, review findings, and story work fields are now validated before write and during audit.

## Decisions

- Durable guidance-bearing runtime records now fail fast at write boundaries and audit catches legacy contamination.

## Checks

- python -m unittest discover -s tests: 115 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed
- audit/artifact verify/workflow validate/parity replay/gate: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-durable-runtime-guidance-source-guard.md
- CHANGELOG.md

## Artifacts

- none

## Next Action

Continue the post-parity Forge audit by checking dead code, misleading docs, and remaining runtime surfaces that lack deterministic validation.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md
- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-state-guidance-write-guard.md
- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-recovery-memory-guidance-guard.md
- .forge-method/artifacts/20260616-durable-runtime-guidance-source-guard.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-023724-validation-runtime-guidance-payload-safety-guard-final-gate.md
- .forge-method/evidence/20260616-030227-validation-config-capability-index-guidance-safety-validati.md
- .forge-method/evidence/20260616-032451-validation-state-guidance-write-guard-validation.md
- .forge-method/evidence/20260616-034215-validation-recovery-memory-guidance-guard-validation.md
- .forge-method/evidence/20260616-041434-validation-durable-runtime-guidance-source-guard-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - State guidance write guard changelog - Unreleased notes record the state guidance write guard for durable next-action and route-reason fields.
- changelog [active/durable]: CHANGELOG.md - Recovery memory guidance guard changelog - Unreleased notes record the recovery memory guidance guard for checkpoints, context packs, and recovery briefs.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-recovery-memory-guidance-guard.md - Recovery memory guidance guard - Checkpoint, context pack, and recovery brief Markdown are now validated before write, and audit scans existing recovery memory files for misleading guidance.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-durable-runtime-guidance-source-guard.md - Durable runtime guidance source guard - Artifact summaries, human input prompts, review findings, and story work fields now share the guidance safety contract before they enter snapshots, context packs, or runtime JSON.
- changelog [active/durable]: CHANGELOG.md - Durable runtime guidance source guard changelog - Unreleased notes record the durable runtime guidance source guard for artifact summaries, human input prompts, review findings, and story work fields.

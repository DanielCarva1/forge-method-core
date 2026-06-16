# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: recovery-memory-guidance-guard
- workflow: agent-analyze
- active_story: <none>
- next_action: Continue the broader Forge audit by checking artifact summaries, human input prompts, review findings, and story fields that are copied into agent-facing context packs or runtime JSON.

## Latest Checkpoint

# Recovery memory guidance guard

- created_at: 2026-06-16T03:42:39+00:00
- project: forge-method-core
- phase: 6-evolve
- status: recovery-memory-guidance-guard
- workflow: agent-analyze
- active_story: <none>

## Summary

Closed an agent-analyze audit gap: checkpoints, latest-checkpoint mirrors, context packs, and recovery briefs now validate final Markdown before writing, and audit catches existing recovery memory files with misleading guidance.

## Decisions

- Treat recovery memory Markdown as agent-facing guidance because future sessions load it before broad context; validate generated text at write time and scan existing recovery memory during audit.

## Checks

- unittest 112; smoke-runtime; verify-fast; smoke-install; parity replay 91/91; workflow validate; workflow compactness; artifact verify; audit; gate 20/20

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-recovery-memory-guidance-guard.md

## Artifacts

- .forge-method/artifacts/20260616-recovery-memory-guidance-guard.md

## Next Action

Continue the broader Forge audit by checking artifact summaries, human input prompts, review findings, and story fields that are copied into agent-facing context packs or runtime JSON.

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

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-023621-validation-runtime-guidance-payload-safety-guard-validation.md
- .forge-method/evidence/20260616-023724-validation-runtime-guidance-payload-safety-guard-final-gate.md
- .forge-method/evidence/20260616-030227-validation-config-capability-index-guidance-safety-validati.md
- .forge-method/evidence/20260616-032451-validation-state-guidance-write-guard-validation.md
- .forge-method/evidence/20260616-034215-validation-recovery-memory-guidance-guard-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Config capability index guidance safety changelog - Unreleased notes record the config, agent profile, and capability index guidance safety guard.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-state-guidance-write-guard.md - State guidance write guard - State next-action and route-reason fields are now validated before write and during audit so misleading durable guidance cannot become future agent context.
- changelog [active/durable]: CHANGELOG.md - State guidance write guard changelog - Unreleased notes record the state guidance write guard for durable next-action and route-reason fields.
- changelog [active/durable]: CHANGELOG.md - Recovery memory guidance guard changelog - Unreleased notes record the recovery memory guidance guard for checkpoints, context packs, and recovery briefs.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-recovery-memory-guidance-guard.md - Recovery memory guidance guard - Checkpoint, context pack, and recovery brief Markdown are now validated before write, and audit scans existing recovery memory files for misleading guidance.

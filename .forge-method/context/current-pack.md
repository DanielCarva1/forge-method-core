# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: product-facing-docs-independence-guard
- workflow: agent-analyze
- active_story: <none>
- next_action: Continue the post-parity Forge audit by checking dead code and remaining runtime surfaces that lack deterministic validation.

## Latest Checkpoint

# Product-facing docs independence guard

- created_at: 2026-06-16T04:32:56+00:00
- project: forge-method-core
- phase: 6-evolve
- status: product-facing-docs-independence-guard
- workflow: agent-analyze
- active_story: <none>

## Summary

Closed the product-facing docs independence guard. Runtime-repo audit now blocks public Markdown from describing Forge as a clone, fork, or variant of another framework while allowing Git clone/install language.

## Decisions

- Public Forge docs now have deterministic independence validation instead of relying on reviewer memory.

## Checks

- python -m unittest discover -s tests: 118 passed
- smoke-runtime.ps1: passed
- verify-fast.ps1: passed
- smoke-install.ps1: passed
- audit/artifact verify/workflow validate/parity replay/gate: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-product-facing-docs-independence-guard.md
- CHANGELOG.md

## Artifacts

- none

## Next Action

Continue the post-parity Forge audit by checking dead code and remaining runtime surfaces that lack deterministic validation.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md
- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-state-guidance-write-guard.md
- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-recovery-memory-guidance-guard.md
- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-durable-runtime-guidance-source-guard.md
- CHANGELOG.md
- .forge-method/artifacts/20260616-product-facing-docs-independence-guard.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-030227-validation-config-capability-index-guidance-safety-validati.md
- .forge-method/evidence/20260616-032451-validation-state-guidance-write-guard-validation.md
- .forge-method/evidence/20260616-034215-validation-recovery-memory-guidance-guard-validation.md
- .forge-method/evidence/20260616-041434-validation-durable-runtime-guidance-source-guard-validation.md
- .forge-method/evidence/20260616-043253-validation-product-facing-docs-independence-guard-validatio.md

## Recent Artifacts

- runtime-contract [active/durable]: .forge-method/artifacts/20260616-durable-runtime-guidance-source-guard.md - Durable runtime guidance source guard - Artifact summaries, human input prompts, review findings, and story work fields now share the guidance safety contract before they enter snapshots, context packs, or runtime JSON.
- changelog [active/durable]: CHANGELOG.md - Durable runtime guidance source guard changelog - Unreleased notes record the durable runtime guidance source guard for artifact summaries, human input prompts, review findings, and story work fields.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-durable-runtime-guidance-source-guard.md - Durable runtime guidance source guard - Artifact summaries, human input prompts, review findings, and story work fields now share the guidance safety contract before they enter snapshots, context packs, or runtime JSON.
- runtime-contract [active/durable]: .forge-method/artifacts/20260616-product-facing-docs-independence-guard.md - Product-facing docs independence guard - Runtime-repo audit now blocks public Markdown from framing Forge as dependent on another framework while preserving normal Git install wording.
- changelog [active/durable]: CHANGELOG.md - Product-facing docs independence guard changelog - Unreleased notes record the product-facing docs independence guard for runtime-repo public Markdown.

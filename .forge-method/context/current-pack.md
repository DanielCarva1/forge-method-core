# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: next-help-oracle-surface
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue the post-parity Forge audit by checking whether guide and Help Oracle route diagnostics are consistently mirrored in persisted recovery artifacts and capability indexes.

## Latest Checkpoint

# Next Help Oracle surface

- created_at: 2026-06-16T11:28:55+00:00
- project: forge-method-core
- phase: 6-evolve
- status: next-help-oracle-surface
- workflow: runtime-builder
- active_story: <none>

## Summary

next now has a compact JSON surface and text route diagnostics, preserving Help Oracle reason, context boundary, quality, commands, state update hints, and mechanical goal handoff after resume.

## Decisions

- next remains the terse human continuation command, but next --json is now the compact agent follow-up to resume --json.
- Text next prints reason and context boundary so stale-state overrides are explainable without full snapshot parsing.

## Checks

- Focused regressions cover human input, ready stale next_action, active evolve workflow, broken workflow quality, and mechanical goal handoff.
- python -m unittest discover -s tests passed, 125 tests.
- smoke-runtime, smoke-install, verify-fast, parity replay 91/91, artifact verify, audit, and gate 22/22 passed.

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260616-next-help-oracle-surface.md

## Next Action

Continue the post-parity Forge audit by checking whether guide and Help Oracle route diagnostics are consistently mirrored in persisted recovery artifacts and capability indexes.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-snapshot-plugin-diagnostic-surface.md
- CHANGELOG.md
- .forge-method/state.yaml

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-090029-validation-bootstrap-plugin-diagnostic-surface-validation.md
- .forge-method/evidence/20260616-092053-validation-bootstrap-plugin-diagnostic-surface-final-valida.md
- .forge-method/evidence/20260616-094923-validation-bootstrap-quality-surface-validation.md
- .forge-method/evidence/20260616-103204-validation-reload-quality-surface-validation.md
- .forge-method/evidence/20260616-110005-validation-context-recovery-quality-surface-validation.md

## Recent Artifacts

- changelog [active/durable]: CHANGELOG.md - Reload quality surface changelog - Unreleased notes record compact quality summary in existing-project reload text and JSON.
- changelog [active/durable]: CHANGELOG.md - Context recovery quality surface changelog - Unreleased notes record compact quality summary in resume, context plan, and context health.
- runtime-builder [active/durable]: .forge-method/artifacts/20260616-context-recovery-quality-surface.md - Context recovery quality surface - Resume, context plan, and context health now expose compact quality so fresh-chat recovery cannot hide gate-rejected project failures.
- runtime-builder [active/durable]: .forge-method/artifacts/20260616-next-help-oracle-surface.md - Next Help Oracle surface - next --json now preserves compact Help Oracle route diagnostics, quality, commands, context boundary, and mechanical goal handoff.
- changelog [active/durable]: CHANGELOG.md - Next Help Oracle surface changelog - Unreleased notes record next --json and route diagnostics in text next.

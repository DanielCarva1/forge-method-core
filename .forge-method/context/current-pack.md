# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: context-recovery-quality-surface
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue the post-parity Forge audit by checking route diagnostics and Help Oracle surfaces where future agents may still get stale or incomplete next-step reasons.

## Latest Checkpoint

# Context recovery quality surface

- created_at: 2026-06-16T11:00:05+00:00
- project: forge-method-core
- phase: 6-evolve
- status: context-recovery-quality-surface
- workflow: runtime-builder
- active_story: <none>

## Summary

Resume, context plan, and context health now expose compact quality; context health blocks on project quality failures and points agents to audit/status instead of reporting healthy recovery context.

## Decisions

- Fresh-chat recovery payloads must carry the same compact quality truth as bootstrap/reload surfaces.
- Context health uses level blocked for failed project quality and reserves context compaction commands for budget pressure.

## Checks

- Focused regression: workflow-broken fixture covers resume/context plan/context health quality.
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

- .forge-method/artifacts/20260616-context-recovery-quality-surface.md

## Next Action

Continue the post-parity Forge audit by checking route diagnostics and Help Oracle surfaces where future agents may still get stale or incomplete next-step reasons.

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

- changelog [active/durable]: CHANGELOG.md - Bootstrap quality surface changelog - Unreleased notes record compact quality summary in start, status --brief, and existing-project preflight.
- runtime-builder [active/durable]: .forge-method/artifacts/20260616-reload-quality-surface.md - Reload quality surface - Existing-project reload now exposes compact full-quality status so stale-chat recovery cannot hide quality failures.
- changelog [active/durable]: CHANGELOG.md - Reload quality surface changelog - Unreleased notes record compact quality summary in existing-project reload text and JSON.
- changelog [active/durable]: CHANGELOG.md - Context recovery quality surface changelog - Unreleased notes record compact quality summary in resume, context plan, and context health.
- runtime-builder [active/durable]: .forge-method/artifacts/20260616-context-recovery-quality-surface.md - Context recovery quality surface - Resume, context plan, and context health now expose compact quality so fresh-chat recovery cannot hide gate-rejected project failures.

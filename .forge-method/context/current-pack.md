# Forge Method Context Pack

## State

- project: forge-method-core
- phase: 6-evolve
- status: reload-quality-surface
- workflow: runtime-builder
- active_story: <none>
- next_action: Continue the post-parity Forge audit by checking recovery outputs that still lack compact machine-readable quality, context, or route diagnostics.

## Latest Checkpoint

# Reload quality surface

- created_at: 2026-06-16T10:32:04+00:00
- project: forge-method-core
- phase: 6-evolve
- status: reload-quality-surface
- workflow: runtime-builder
- active_story: <none>

## Summary

Existing-project reload now exposes compact full-quality status in text and JSON, matching bootstrap quality surfaces and preventing stale-chat recovery from hiding workflow/config/builder/agent failures.

## Decisions

- Treat existing-project reload as a recovery bootstrap surface that must expose full compact quality, not only route/state/next.
- Keep missing-state reload route-focused until a Forge project is selected.

## Checks

- Focused regression: `tests.test_runtime.RuntimeTests.test_snapshot_uses_workflow_validation_surface`
- `python -m unittest discover -s tests` passed, 125 tests
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1` passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1` passed
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1` passed
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json` passed, 91/91 fixtures
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .` passed
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .` passed
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals` passed, 22/22 evals

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260616-reload-quality-surface.md
- .forge-method/evidence/20260616-103204-validation-reload-quality-surface-validation.md

## Next Action

Continue the post-parity Forge audit by checking recovery outputs that still lack compact machine-readable quality, context, or route diagnostics.

## Recovery Signals

### Failed Checks

- none

### Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- .forge-method/artifacts/20260616-snapshot-plugin-diagnostic-surface.md
- CHANGELOG.md

## Open Human Inputs

- none

## Open Review Findings

- none

## Recommended Agent Profiles

- facilitator (Facilitator): Clarify intent, constraints, trade-offs, and human decisions without expanding implementation scope.
- planner (Planner): Turn specs, risks, and constraints into executable stories, sequencing, and validation strategy.

## Recent Evidence

- .forge-method/evidence/20260616-043253-validation-product-facing-docs-independence-guard-validatio.md
- .forge-method/evidence/20260616-082813-validation-hot-start-plugin-diagnostic-surface-validation.md
- .forge-method/evidence/20260616-090029-validation-bootstrap-plugin-diagnostic-surface-validation.md
- .forge-method/evidence/20260616-092053-validation-bootstrap-plugin-diagnostic-surface-final-valida.md
- .forge-method/evidence/20260616-094923-validation-bootstrap-quality-surface-validation.md

## Recent Artifacts

- runtime-diagnostic [active/durable]: .forge-method/artifacts/20260616-snapshot-plugin-diagnostic-surface.md - Bootstrap Plugin Diagnostic Surface - Preflight, reload, context health, context plan, resume, and snapshot expose local plugin installation status, outdated version diagnostics, repair commands, and validation evidence without making plugin state a quality gate blocker.
- runtime-builder [active/durable]: .forge-method/artifacts/20260616-bootstrap-quality-surface.md - Bootstrap quality surface - Bootstrap surfaces now expose compact full-quality status instead of relying on audit-only health.
- changelog [active/durable]: CHANGELOG.md - Bootstrap quality surface changelog - Unreleased notes record compact quality summary in start, status --brief, and existing-project preflight.
- runtime-builder [active/durable]: .forge-method/artifacts/20260616-reload-quality-surface.md - Reload quality surface - Existing-project reload now exposes compact full-quality status so stale-chat recovery cannot hide quality failures.
- changelog [active/durable]: CHANGELOG.md - Reload quality surface changelog - Unreleased notes record compact quality summary in existing-project reload text and JSON.

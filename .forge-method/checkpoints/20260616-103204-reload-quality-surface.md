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

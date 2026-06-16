# Route Diagnostics Recovery Index validation

- kind: validation
- created_at: 2026-06-16T12:29:51+00:00
- checks: focused unittest: 4 passed | full unittest: 125 passed | smoke-runtime.ps1: passed | verify-fast.ps1: passed | config index --write: passed | artifact verify: passed | audit: passed | gate --require-evals: 22/22 passed

## Summary

Validated persisted route diagnostics across recovery briefs and capability index.

## Focused Proof

- Temporary reproduction before the patch showed no route diagnostics in full recovery, compact recovery, or capability index.
- Temporary reproduction after the patch showed Route Diagnostics, required_next_workflow, context_boundary, compact Commands preservation, and route_diagnostics surfaces for guide, resume, next, and context recover.
- Focused regressions passed for capability-index route diagnostics and recovery Route Diagnostics.

## Full Checks

- `python -m unittest discover -s tests` initially timed out at 120s, then passed with 125 tests in 462.061s.
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1` passed.
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1` initially timed out at 604s, then passed with 125 tests in 989.903s plus onboarding, workflow, and agent validation.
- `python skills\forge-method\scripts\forge_method_runtime.py config index --root . --write --json` regenerated `.forge-method/context/capability-index.json`.
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .` passed.
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .` passed.
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals` passed with evals 22/22.

## Notes

- The capability index was stale after adding `route_diagnostics`; regenerating it fixed the quality gate.
- Artifact summary timestamp warnings were fixed by refreshing artifact index entries.

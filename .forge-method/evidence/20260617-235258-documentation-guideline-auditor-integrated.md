# Forge Guideline Auditor integrated

- created_at_local: 2026-06-17T23:52:58-03:00
- created_at_utc: 2026-06-18T02:52:58+00:00
- kind: documentation
- story: none
- phase: 6-evolve
- workflow: guideline-audit

## Summary

Added Forge Guideline Auditor as a reusable Codex/Core skill and connected it
to Forge Method through a `guideline-audit` workflow, facilitation pack,
template, runtime routing, work-order fields, and regression tests.

## Artifacts

- `skills/forge-guideline-auditor/SKILL.md`
- `skills/forge-method/references/workflow-guideline-audit.md`
- `skills/forge-method/facilitation/guideline-audit.md`
- `skills/forge-method/templates/guideline-audit-artifact.md`

## Checks

- `python C:\Users\Danie\.codex\skills\.system\skill-creator\scripts\quick_validate.py C:\Users\Danie\.codex\skills\forge-guideline-auditor` passed.
- `python C:\Users\Danie\.codex\skills\.system\skill-creator\scripts\quick_validate.py C:\Users\Danie\OneDrive\Documentos\ody\skills\forge-guideline-auditor` passed.
- Local and core guideline/work-order templates passed `scripts\validate_guideline.py`.
- `skills/forge-method/catalog/workflows.json` parses as JSON.
- Core `.forge-method/artifacts/index.ndjson` and `.forge-method/ledger.ndjson` parse line-by-line.
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate` passed.
- `python skills\forge-method\scripts\forge_method_runtime.py agent validate` passed.
- `python scripts\test-runner.py --workers 4 --timeout 120 --test ...guideline_audit...` passed 2/2 tests in 3.4s.
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 -Test ...guideline_audit...,...ready_project...` passed.
- `python scripts\test-runner.py --workers 4 --timeout 120` passed 130/130 tests in 420.0s.
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1` passed before the responsive runner change.

## Responsiveness Evidence

- The previous direct `python -m unittest discover -s tests` path timed out during this work, including one run with a 904s command timeout.
- `scripts/test-runner.py` now runs each unittest in its own subprocess with a per-test timeout, progress output, focused test selection, and slow-test reporting.
- The full responsive run identified the slowest tests instead of hiding the bottleneck behind a global hang.

## Acceptance Evidence

- Codex local skill exists separately under `C:\Users\Danie\.codex\skills\forge-guideline-auditor`.
- Forge Method Core has the canonical skill and workflow contract.
- Guidance Engine has a narrow `guideline-audit` route for guideline, work-order,
  acceptance-evidence, and guarded permanent implementation requests.
- Fast/full verification scripts now use the responsive unit runner while keeping
  onboarding validation, workflow validation, agent validation, and smoke checks.

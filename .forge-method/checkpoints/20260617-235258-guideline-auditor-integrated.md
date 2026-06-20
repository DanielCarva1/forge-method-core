# Forge Guideline Auditor integrated

- created_at_local: 2026-06-17T23:52:58-03:00
- created_at_utc: 2026-06-18T02:52:58+00:00
- project: forge-method-core
- phase: 6-evolve
- status: validated
- workflow: guideline-audit
- active_story: <none>

## Summary

Added a reusable Forge Guideline Auditor skill and wired Forge Method Core to
route guideline/work-order/permanent-implementation requests into the new
`guideline-audit` workflow.

## Decisions

- Keep the skill reusable under local Codex and canonical under Forge Method Core.
- Keep the workflow compact and agent-facing; human conversation lives in the
  facilitation pack.
- Treat guideline audit as a pre-implementation gate, not as a replacement for
  `build-story`.

## Checks

- Local Codex skill quick validation passed.
- Core canonical skill quick validation passed.
- Local and core guideline/work-order template validation passed.
- Core workflow catalog JSON, artifact index NDJSON, and ledger NDJSON parsed successfully.
- `workflow validate` passed.
- `agent validate` passed.
- Targeted guideline-audit route tests passed.
- Targeted `verify-fast.ps1` run passed.
- Responsive full unit runner passed 130/130 tests in 420.0s.
- `smoke-runtime.ps1` passed before the responsive runner change.

## Failed Checks

- Legacy direct `python -m unittest discover -s tests` timed out during this work. Replaced in verification scripts with `scripts/test-runner.py`, which preserves coverage while adding progress, per-test timeouts, and slow-test reporting.

## Touched Files

- CHANGELOG.md
- scripts/test-runner.py
- scripts/verify-all.ps1
- scripts/verify-all.sh
- scripts/verify-fast.ps1
- scripts/verify-fast.sh
- skills/forge-guideline-auditor/**
- skills/forge-method/catalog/workflows.json
- skills/forge-method/facilitation/guideline-audit.md
- skills/forge-method/modules/runtime-builder.yaml
- skills/forge-method/references/workflow-guideline-audit.md
- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/templates/build-story-work-order.md
- skills/forge-method/templates/guideline-audit-artifact.md
- tests/test_runtime.py

## Next Action

Review the responsive runner evidence, then run release readiness before publishing this Forge Method Core increment.

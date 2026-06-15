# Post-Parity Polish Audit

created_at: 2026-06-15T12:32:00+00:00
workflow: runtime-builder
status: stale-guidance-guard

## Scope

Audited the current post-parity surface after Presentation Craft Fold-In:

- facilitation packs: 29 files
- compact workflow refs: 99 files
- parity replay fixtures: 89 cases across 68 workflows
- active parity/audit/plan artifacts that future agents may follow

## Findings

- Workflow refs are structurally compact and pass `workflow validate` plus `workflow compactness`.
- Facilitation packs all have the expected rich-pack sections and no missing pack references from the workflow catalog.
- The main risk is stale internal guidance in active artifacts: old mixed verdicts and next-step text can send future agents back to closed parity rows.
- Public docs did not show product-comparison language that violates the repo rule; `clone` references are Git install/distribution wording.

## Corrections

- Updated the systematic parity audit so Planning Tracks is a single current `translated` verdict.
- Replaced the old "missing packs/templates" P0.2 note with current coverage status.
- Updated the systematic parity plan's immediate next step from old partial-row hardening to current post-parity polish.
- Added a runtime stale-guidance guard to `artifact verify` for active parity/audit/plan/benchmark artifacts.

## Guard Contract

`artifact verify` now warns when active internal guidance artifacts contain markers that should not survive after closure:

- old wording that says partial and strong-ish rows remain after they were closed
- old missing-pack instructions after coverage exists
- mixed verdicts that combine translated with partial and preserve stale ambiguity

These are warnings in normal mode and become blocking when callers run strict verification.

## Validation Target

- targeted unit coverage for stale-guidance warnings
- `artifact verify --root .`
- `workflow validate`
- `workflow compactness`
- `parity replay`
- full unit suite and `verify-fast`

## Next Action

Continue post-parity polish with transcript-derived improvements only: if a facilitation pack feels thin, prove it with a replay or real transcript before adding prose.

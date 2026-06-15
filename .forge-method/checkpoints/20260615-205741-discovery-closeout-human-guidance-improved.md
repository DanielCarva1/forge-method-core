# Discovery closeout human guidance improved

- created_at: 2026-06-15T20:57:41+00:00
- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-generator-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Improved discover-intent so the first guided question and facilitation pack collect discovery-closeout fields before specification.

## Decisions

- Keep compact workflow refs as state machines; put richer field-gathering conversation in Guidance Engine human copy and facilitation pack.

## Checks

- focused tests, workflow validate, parity replay, full unittest, smoke-runtime, smoke-install, and verify-fast passed

## Failed Checks

- initial smoke-runtime run failed because it still expected the old generic first question; smoke assertions were updated and the rerun passed

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/discover-intent.md
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- tests/test_runtime.py
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-discovery-closeout-human-guidance-contract.md

## Next Action

Continue post-parity Forge polish by auditing other phase-closing workflows for first-class generators and guided field extraction.

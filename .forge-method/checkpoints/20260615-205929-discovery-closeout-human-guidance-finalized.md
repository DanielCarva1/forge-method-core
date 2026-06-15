# Discovery closeout human guidance finalized

- created_at: 2026-06-15T20:59:29+00:00
- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-human-guidance-improved
- workflow: runtime-builder
- active_story: <none>

## Summary

Finalized discover-intent human guidance after state transition so the durable checkpoint matches the improved status.

## Decisions

- The next post-parity polish should audit other phase-closing workflows for the same generator plus guided-field extraction pattern.

## Checks

- focused tests, workflow validate, parity replay, full unittest, smoke-runtime, smoke-install, verify-fast, artifact verify, audit, and gate passed

## Failed Checks

- initial smoke-runtime run failed on obsolete expected first question, then passed after smoke assertion update

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

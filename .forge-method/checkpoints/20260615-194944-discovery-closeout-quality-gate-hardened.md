# Discovery closeout quality gate hardened

- created_at: 2026-06-15T19:49:44+00:00
- project: forge-method-core
- phase: 6-evolve
- status: discovery-closeout-quality-gate-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact discovery-check and made discovery-to-spec transition require a valid closeout with Grill Gate handoff fields.

## Decisions

- A discovery closeout must be useful agent context, not just an artifact marker; weak title/summary artifacts remain blocked.

## Checks

- unit, runtime smoke, install smoke, parity replay, full unittest, verify-fast passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-discovery-closeout-quality-gate-contract.md

## Artifacts

- .forge-method/artifacts/20260615-discovery-closeout-quality-gate-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by adding a first-class discovery closeout template or generator so agents can produce the required artifact without hand-rolled markdown.

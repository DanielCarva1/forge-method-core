# v1.31.1 public install hotfix published

- created_at: 2026-06-17T20:56:19+00:00
- project: forge-method-core
- phase: 6-evolve
- status: hotfix-validated
- workflow: release-readiness
- active_story: <none>

## Summary

Published Forge Method 1.31.1 to main, tag, and GitHub Release. Validated public clone/install by tag and main, then updated the local Codex plugin to 1.31.1.

## Decisions

- Treat public install leakage of core state as a release-blocking distribution bug; maintainer core-edit mode now requires local marker/env.

## Checks

- GitHub Release v1.31.1 created
- Clone/install smoke passed for v1.31.1 and main
- Local plugin preflight no longer reports version mismatch

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

v1.31.1 is public; next work is tester feedback or a new evolve cycle.

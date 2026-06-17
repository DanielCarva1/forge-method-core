# v1.31.2 guided research drift hotfix validated

- created_at: 2026-06-17T23:22:56+00:00
- project: forge-method-core
- phase: 5-ready-operate
- status: published
- workflow: operate-support
- active_story: <none>

## Summary

Patched Guidance Engine routing so strategic standalone app stack/interface and Rust codebase-standard conversations stay in research/technical-feasibility guidance, preserve ready-project evolution phase, and do not treat performance wording as fast-path pressure.

## Decisions

- Ship as patch 1.31.2 because this changes public guided human experience and needs beta users to receive it via auto-update.

## Checks

- Full unit suite passed: 128 tests
- Parity replay passed: 101/101
- verify-fast, smoke-runtime, and smoke-install passed

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- none

## Next Action

Commit v1.31.2 hotfix, publish main/tag/release, validate clone install, and update local plugin.

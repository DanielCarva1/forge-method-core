# Forge Method 1.29.0 published

- created_at: 2026-06-15T03:06:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: v1.29.0-published
- workflow: runtime-builder
- active_story: <none>

## Summary

Published Forge Method Core v1.29.0 as the guided workflow depth release batch. The release commit is tagged v1.29.0, origin has the tag, branch codex/script-audit-optimization is pushed, and clone install smoke passed from the published tag.

## Decisions

- Treat 1.29.0 as an intermediate release batch, not final guided-flow parity completion.
- Return the runtime state to 6-evolve/runtime-builder for real-use transcript hardening and remaining partial parity rows.

## Checks

- git ls-remote --tags origin v1.29.0: found
- smoke-plugin-clone-install.ps1 -Ref v1.29.0 -ExpectedVersion 1.29.0: passed

## Failed Checks

- none

## Touched Files

- .forge-method/state.yaml

## Artifacts

- .forge-method/evidence/20260615-030535-validation-forge-method-1-29-0-published-clone-smoke.md

## Next Action

Continue real-use transcript hardening for remaining partial parity rows; do not claim full guided-flow parity until the audit rows and live transcripts prove it.

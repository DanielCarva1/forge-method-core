# Recovery memory guidance guard

- created_at: 2026-06-16T03:42:39+00:00
- project: forge-method-core
- phase: 6-evolve
- status: recovery-memory-guidance-guard
- workflow: agent-analyze
- active_story: <none>

## Summary

Closed an agent-analyze audit gap: checkpoints, latest-checkpoint mirrors, context packs, and recovery briefs now validate final Markdown before writing, and audit catches existing recovery memory files with misleading guidance.

## Decisions

- Treat recovery memory Markdown as agent-facing guidance because future sessions load it before broad context; validate generated text at write time and scan existing recovery memory during audit.

## Checks

- unittest 112; smoke-runtime; verify-fast; smoke-install; parity replay 91/91; workflow validate; workflow compactness; artifact verify; audit; gate 20/20

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-recovery-memory-guidance-guard.md

## Artifacts

- .forge-method/artifacts/20260616-recovery-memory-guidance-guard.md

## Next Action

Continue the broader Forge audit by checking artifact summaries, human input prompts, review findings, and story fields that are copied into agent-facing context packs or runtime JSON.

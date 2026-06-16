# Config capability index guidance safety guard

- created_at: 2026-06-16T03:03:17+00:00
- project: forge-method-core
- phase: 6-evolve
- status: workflow-selected
- workflow: config-customization
- active_story: <none>

## Summary

Closed a config-customization audit gap: config validation, agent profile validation, and config index now apply guidance safety to runtime-visible text before future agents consume conventions, custom capabilities, agent summaries, or generated capability indexes.

## Decisions

- Treat composed capability index output as a runtime guidance payload and validate it before print/write.

## Checks

- unittest 108; smoke-runtime; verify-fast; parity replay 91/91; workflow validate; workflow compactness; artifact verify; audit; gate 20/20; smoke-install

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py; tests/test_runtime.py; CHANGELOG.md; .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md

## Artifacts

- .forge-method/artifacts/20260616-config-capability-index-guidance-safety.md

## Next Action

Continue the broader Forge audit by finding other composed runtime-visible payloads that need final deterministic validation before emission.

# Guidance human experience polish

- created_at: 2026-06-15T01:56:41+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p2-scope-decisions-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Added contextual human lede to guide output, routed human-experience plus agent-doc polish to runtime-builder, and suppressed Reality/Evidence Gate noise for correction/runtime requests.

## Decisions

- Human richness belongs in guide output and facilitation packs; agent-facing workflow refs and JSON stay compact.

## Checks

- python -m unittest discover -s tests: 70 tests OK
- smoke-runtime: passed
- verify-fast: passed
- smoke-install: passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- docs/adr/0008-guidance-engine.md
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260615-guidance-human-experience-polish.md
- .forge-method/evidence/20260615-015628-validation-guidance-human-experience-polish-validation.md

## Next Action

Review remaining post-parity polish surface and decide the next release/version batch.

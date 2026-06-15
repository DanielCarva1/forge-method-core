# Forge Method 1.29.0 release prepared

- created_at: 2026-06-15T03:01:06+00:00
- project: forge-method-core
- phase: 6-evolve
- status: v1.29.0-release-prepared
- workflow: release-readiness
- active_story: <none>

## Summary

Prepared Forge Method Core v1.29.0 as a coherent guided workflow depth release batch. Bumped runtime/package/listing/docs metadata, moved Unreleased changelog entries into 1.29.0, added release notes, fixed launch-ops example seeding with a validation-map decision source, and validated the package with full release verification.

## Decisions

- Ship this as an intermediate release batch; do not claim full BMAD/Forge parity completion while audit rows still show partial/deferred surfaces.
- Build/verify example projects must include a decision-source artifact instead of weakening the implementation-ready story guard.

## Checks

- python -m unittest discover -s tests: 70 tests OK
- scripts/verify-onboarding-assets.py: passed
- workflow validate: passed
- parity replay: 58/58 passed
- verify-all.ps1: passed
- artifact verify: passed
- audit: passed
- config validate: passed

## Failed Checks

- none

## Touched Files

- VERSION
- .codex-plugin/plugin.json
- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- README.md
- docs/00-quickstart.md
- docs/04-distribution.md
- assets/marketplace/listing.json
- release-notes/latest.json
- release-notes/1.29.0.md

## Artifacts

- .forge-method/evidence/20260615-030025-validation-forge-method-1-29-0-release-validation.md
- release-notes/1.29.0.md
- CHANGELOG.md

## Next Action

Run clean release check after commit, tag v1.29.0 if clean, then continue real-use transcript hardening for remaining partial parity rows.

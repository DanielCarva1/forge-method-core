# 1.30.0 guided human experience versioned

- created_at: 2026-06-17T04:56:52+00:00
- project: forge-method-core
- phase: 6-evolve
- status: versioned-release-ready
- workflow: ready-release
- active_story: <none>

## Summary

Bumped Forge Method Core to 1.30.0 for the guided human experience increment, added release notes/latest metadata/marketplace listing updates, reran full source tests and install/runtime smokes, and updated state to ready-release.

## Decisions

- Treat this as a minor batch release because release check in batch mode selected the next minor version for the new guided human experience increment.

## Checks

- python -m unittest discover -s tests: passed 126 tests
- verify-fast -SkipUnit: passed
- audit: passed
- install-plugin-local: passed
- smoke-runtime: passed
- smoke-install: passed

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- release-notes/1.30.0.md
- .forge-method/evidence/20260617-045628-validation-versioned-guided-human-experience-release.md

## Next Action

Push branch, merge, tag v1.30.0, then run clone/install smoke from the published ref.

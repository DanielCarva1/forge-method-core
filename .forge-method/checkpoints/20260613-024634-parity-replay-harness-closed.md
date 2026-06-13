# Parity replay harness closed

- created_at: 2026-06-13T02:46:34+00:00
- project: forge-method-core
- phase: 6-evolve
- status: p0-parity-replay-harness-done
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed BMAD parity P0.5: Forge now ships a packaged parity replay harness inside the forge-method skill. The replay covers help, confusion, brainstorm, research, PRD, UX, architecture, quick-dev, story cycle, correct-course, builder, CIS/creative, game, and TEA-shaped guidance prompts, expecting Forge-native workflow/phase/action outputs. Install smoke now runs the installed replay fixture. Full parity goal remains active; next work is P1.1 Builder parity.

## Decisions

- Make the skill-packaged fixture the canonical transcript matrix so source tests and installed smoke exercise the same guidance routes.
- Use Forge-native expected outputs only; benchmark family labels are internal coverage metadata, not public product language.
- Keep P0 closure reflected in the internal parity audit while preserving P1/P2 as unfinished work.

## Checks

- python skills\\forge-method\\scripts\\forge_method_runtime.py parity replay: passed 20/20 cases
- python -m unittest discover -s tests: passed 64 tests
- python skills\\forge-method\\scripts\\forge_method_runtime.py workflow validate: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py audit --root .: passed
- python skills\\forge-method\\scripts\\forge_method_runtime.py artifact verify --root .: passed with only pre-existing correct-course stale-summary warning
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1: passed
- powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1: passed and ran installed parity replay 20/20

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/fixtures/guidance-parity-replay.json
- scripts/smoke-install.ps1
- scripts/smoke-install.sh
- tests/test_runtime.py
- docs/00-quickstart.md
- docs/05-v1-operating-model.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Artifacts

- .forge-method/evidence/20260613-024610-validation-parity-replay-harness-validation.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Implement P1.1 Builder parity from the BMAD parity audit: module ideation, agent builder, workflow builder, module builder, and module validation.

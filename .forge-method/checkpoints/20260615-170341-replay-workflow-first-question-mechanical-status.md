# Replay workflow first question mechanical status hardened

- created_at: 2026-06-15T17:03:41+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-workflow-first-question-mechanical-status-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added workflow-specific first questions for guided replay coverage and mechanical-build status prompts that describe autonomous build/check/evidence work instead of asking facilitation questions.

## Decisions

- Workflow-specific first questions are a runtime contract for rich human guidance; mechanical-build is status/execution handoff, not facilitation.

## Checks

- python -m unittest discover -s tests -v
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- manual replay audit: unique_first_questions 67, cross_workflow_repeats [], mechanical prompt issues []
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-replay-workflow-first-question-mechanical-status-contract.md

## Next Action

Continue post-parity Forge polish by auditing live CLI guide output shape against richer prompt contracts and non-JSON first-question visibility.

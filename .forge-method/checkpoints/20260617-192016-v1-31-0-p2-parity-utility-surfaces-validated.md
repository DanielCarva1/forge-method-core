# v1.31.0 P2 parity utility surfaces validated

- created_at: 2026-06-17T19:20:16+00:00
- project: forge-method-core
- phase: 6-evolve
- status: parity-audit-in-progress
- workflow: agent-analyze
- active_story: <none>

## Summary

Implemented and validated opt-in runtime utility parity surfaces: isolated eval runner, hook/event plan, API/browser utility, runtime routing, replay fixtures, templates, scripts, release notes, and version metadata.

## Decisions

- Ship this as the 1.31.0 parity closure/runtime utility batch, not as part of the already published 1.30.0 scoped guidance release.

## Checks

- python -m unittest discover -s tests: 126 tests passed
- workflow/config/audit validation: passed
- parity replay: 100/100 passed
- smoke-runtime and smoke-install: passed
- release metadata aligned to 1.31.0; release check only blocked by dirty worktree before commit

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/fixtures/guidance-parity-replay.json
- skills/forge-method/references/workflow-isolated-eval-runner.md
- skills/forge-method/references/workflow-hook-event-plan.md
- skills/forge-method/references/workflow-api-browser-utility.md
- skills/forge-method/facilitation/runtime-utility.md
- scripts/forge-eval-runner.ps1
- scripts/forge-hook-dispatch.ps1
- scripts/forge-eval-runner.sh
- scripts/forge-hook-dispatch.sh

## Artifacts

- .forge-method/artifacts/20260617-current-systematic-parity-completion-audit.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Commit and push the v1.31.0 parity closure batch; after the worktree is clean, rerun release check before tagging or publishing.

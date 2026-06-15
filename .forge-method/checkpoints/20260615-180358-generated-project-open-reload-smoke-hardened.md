# Generated project open reload smoke hardened

- created_at: 2026-06-15T18:03:58+00:00
- project: forge-method-core
- phase: 6-evolve
- status: generated-project-open-reload-smoke-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Hardened source and install smokes so generated projects must show first facilitation before stories, project list must expose waiting-human-input, and parent workspace preflight/reload must keep explicit project selection with stale-copy guard text.

## Decisions

- Generated project creation and parent workspace reload are part of the human guided experience; smoke tests now protect them as product behavior, not incidental console output.

## Checks

- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1

## Failed Checks

- none

## Touched Files

- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md
- .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md
- .forge-method/evidence/20260615-180331-validation-generated-project-open-reload-smoke-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-generated-project-open-reload-smoke-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by auditing the first human answer path after initial-facilitation, ensuring it routes through Guidance Engine instead of creating premature stories.

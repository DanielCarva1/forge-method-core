# Reload emergency entrypoint ready

- created_at: 2026-06-11T21:00:09+00:00
- project: forge-method-core
- phase: 5-ready-operate
- status: story-done
- workflow: ready-release
- active_story: <none>

## Summary

Added read-only runtime reload support and a minimal forge-reload skill to recover from stale Codex chat instructions.

## Decisions

- Expose reload both as a normal forge-method runtime command and as a tiny emergency skill, while keeping project workflows under the main entrypoint.

## Checks

- python -m unittest discover -s tests passed
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 passed
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-reload/SKILL.md
- install.ps1
- install.sh
- scripts/smoke-install.ps1
- scripts/smoke-install.sh

## Artifacts

- .forge-method/evidence/20260611-205934-validation-reload-emergency-entrypoint-validation.md

## Next Action

publish 1.26.4 batch to GitHub

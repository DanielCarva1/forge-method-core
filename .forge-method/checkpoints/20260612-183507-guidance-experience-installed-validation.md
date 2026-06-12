# Guidance experience installed validation

- created_at: 2026-06-12T18:35:07+00:00
- project: forge-method-core
- phase: 6-evolve
- status: correct-course-continued
- workflow: correct-course
- active_story: <none>

## Summary

Synchronized the installed Forge skill after source changes and verified the installed runtime. The user can now test /forge-reload in another project against the corrected runtime, not the stale installed copy.

## Decisions

- The local installed skill must be refreshed after source runtime changes, otherwise live Forge usage keeps old behavior.

## Checks

- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1: passed
- Installed runtime hash matches repo runtime hash.

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- install.ps1

## Artifacts

- .forge-method/evidence/20260612-183453-validation-guidance-experience-install-validation.md

## Next Action

Use /forge-reload in a fresh project and judge the live first-run facilitation; if it still feels thin, deepen facilitation packs and transcript replay rather than creating stories early.

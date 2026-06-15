# Game Brief Sprint Depth finalized

- created_at: 2026-06-15T11:51:38+00:00
- project: forge-method-core
- phase: 6-evolve
- status: game-brief-sprint-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

State finalized after Game Brief & Sprint Depth hardening. Runtime now routes game brief as a living guided workflow, uses dedicated game-sprint-planning for playable-slice planning, validates both with artifact game-check, and proves the guidance matrix with 88/88 parity replay plus full repo validation.

## Decisions

- Residual game brief, brainstorm-game, and game sprint planning rows are no longer treated as strong-ish; next parity decision is /cis-agent-presentation-master or explicit deferral.

## Checks

- python -m unittest discover -s tests => 78 tests OK
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 => passed
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 => passed
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 => passed

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- .forge-method/evidence/20260615-115018-validation-game-brief-sprint-depth-validation.md

## Next Action

Decide whether /cis-agent-presentation-master becomes a Forge workflow, folds into storytelling/presentation craft, or is explicitly deferred; keep deferred API/browser or eval-runner surfaces out unless repeated projects justify them.

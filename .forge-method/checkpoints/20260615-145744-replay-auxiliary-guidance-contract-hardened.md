# Replay auxiliary guidance contract hardened

- created_at: 2026-06-15T14:57:44+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-auxiliary-guidance-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added replay assertions for council recommendations, Codex Goal handoff, and autonomous work-order flags; narrowed council recommendation behavior; and protected runtime meta-audit prompts from council keyword routing.

## Decisions

- Route-only success is no longer enough for auxiliary guidance behavior; replay fixtures must declare behavior-changing handoff flags.

## Checks

- python -m unittest discover -s tests
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1

## Failed Checks

- none

## Touched Files

- none

## Artifacts

- .forge-method/artifacts/20260615-replay-auxiliary-guidance-contract.md

## Next Action

Continue post-parity Forge polish by auditing remaining replay surfaces for human guidance, compact artifact handoff, and automation flags that still pass on route-only evidence.

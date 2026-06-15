# Replay mutating command contract hardened

- created_at: 2026-06-15T15:25:03+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-mutating-command-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added expected_commands replay support, exact mutating command sequence validation, and replay output for mutating_commands so multi-command correct-course routes cannot pass by asserting only one state-changing command.

## Decisions

- Guidance replay must prove the full state-changing command sequence, not just one command presence.

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

- .forge-method/artifacts/20260615-replay-mutating-command-contract.md

## Next Action

Continue post-parity Forge polish by auditing remaining replay surfaces for state update contents, route reasons, and human prompt quality that still pass on indirect evidence.

# Replay state update route reason contract hardened

- created_at: 2026-06-15T16:02:36+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-state-update-route-reason-contract-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added parity replay checks that state_updates mirror classification, workflow, and route_reason, and that Persona Lens route reasons persist the selected lens marker for compact agent handoff.

## Decisions

- Guidance replay must prove compact state-update handoff coherence, not just route and phase.

## Checks

- python -m unittest discover -s tests -v
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- manual replay audit: missing_persona_route_reason_markers [] and state_update_coherence_issues []
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
- .forge-method/artifacts/20260615-replay-state-update-route-reason-contract.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-replay-state-update-route-reason-contract.md

## Next Action

Continue post-parity Forge polish by auditing human_prompt quality and route_reason specificity against the rich-human compact-agent contract.

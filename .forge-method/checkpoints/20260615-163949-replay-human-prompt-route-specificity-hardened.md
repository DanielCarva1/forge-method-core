# Replay human prompt route specificity hardened

- created_at: 2026-06-15T16:39:49+00:00
- project: forge-method-core
- phase: 6-evolve
- status: replay-human-prompt-route-specificity-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added Guidance Engine normalization so facilitated routes ask a concrete human first question, remove internal I-should phrasing, and append compact Signals/Route summaries for agent handoff.

## Decisions

- Guided human prompts are part of the runtime contract, not decorative copy; parity replay must fail when facilitated guidance reads like internal agent notes.

## Checks

- python -m unittest discover -s tests -v
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- manual replay audit: cases 90, facilitated 88, missing_first_question 0, internal_i_should 0, missing_signals_route 0
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
- .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-replay-human-prompt-route-specificity-contract.md

## Next Action

Continue post-parity Forge polish by auditing workflow-specific first-question quality and mechanical-build human/status wording.

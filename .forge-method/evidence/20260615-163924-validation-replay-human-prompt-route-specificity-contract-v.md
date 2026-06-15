# Replay Human Prompt Route Specificity Contract validation

- kind: validation
- created_at: 2026-06-15T16:39:24+00:00
- checks: python -m unittest tests.test_runtime.RuntimeTests.test_parity_replay_requires_human_facing_facilitated_prompt tests.test_runtime.RuntimeTests.test_parity_replay_requires_route_reason_specificity tests.test_runtime.RuntimeTests.test_parity_replay_command_validates_fixture_matrix | python -m unittest discover -s tests -v | python skills/forge-method/scripts/forge_method_runtime.py parity replay | manual replay audit: cases 90, facilitated 88, missing_first_question 0, internal_i_should 0, missing_signals_route 0 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .

## Summary

Validated human prompt and route specificity replay contract: targeted tests passed, unittests 91/91, parity replay 90/90, manual prompt/reason audit clean across 90 cases, smoke-runtime, verify-fast, smoke-install with installed replay 90/90, and artifact verify passed.

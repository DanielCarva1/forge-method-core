# Guide CLI first question output validation

- kind: validation
- created_at: 2026-06-15T17:32:56+00:00
- checks: python -m unittest tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract tests.test_runtime.RuntimeTests.test_mechanical_work_order_goal_and_commit_policy_contracts -v | python -m unittest discover -s tests | python skills/forge-method/scripts/forge_method_runtime.py parity replay | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .

## Summary

Validated that non-JSON guide output separates facilitated guidance from First question lines, renders mechanical-build as Status text, preserves parity replay, and keeps source plus installed runtime checks green.

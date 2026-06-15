# Replay Workflow First Question Mechanical Status Contract validation

- kind: validation
- created_at: 2026-06-15T17:03:07+00:00
- checks: python -m unittest tests.test_runtime.RuntimeTests.test_first_guidance_questions_are_workflow_specific tests.test_runtime.RuntimeTests.test_parity_replay_requires_mechanical_build_status_prompt tests.test_runtime.RuntimeTests.test_parity_replay_command_validates_fixture_matrix | python -m unittest discover -s tests -v | python skills/forge-method/scripts/forge_method_runtime.py parity replay | manual replay audit: unique_first_questions 67, cross_workflow_repeats [], mechanical prompt issues [] | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1 | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .

## Summary

Validated workflow-specific first questions and mechanical status prompt contract: targeted tests passed, unittests 93/93, parity replay 90/90, manual audit reached 67 unique first questions with no cross-workflow repeats and no mechanical prompt issues, smoke-runtime, verify-fast, smoke-install with installed replay 90/90, and artifact verify passed.

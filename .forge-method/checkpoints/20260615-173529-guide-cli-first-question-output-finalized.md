# Guide CLI first question output finalized

- created_at: 2026-06-15T17:35:29+00:00
- project: forge-method-core
- phase: 6-evolve
- status: guide-cli-first-question-output-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Finalized the non-JSON guide output contract: facilitated workflows now print Guidance and First question lines, mechanical-build prints Status text, JSON parity remains unchanged, and CHANGELOG artifact tracking is current.

## Decisions

- The live CLI text is validated as part of the human guidance surface, not just JSON replay fixtures.

## Checks

- python -m unittest tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract tests.test_runtime.RuntimeTests.test_mechanical_work_order_goal_and_commit_policy_contracts -v
- python -m unittest discover -s tests
- python skills/forge-method/scripts/forge_method_runtime.py parity replay
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1
- powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
- python skills/forge-method/scripts/forge_method_runtime.py artifact verify --root .
- python skills/forge-method/scripts/forge_method_runtime.py gate --root . --require-evals

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- tests/test_runtime.py
- CHANGELOG.md
- .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md
- .forge-method/evidence/20260615-173256-validation-guide-cli-first-question-output-validation.md
- .forge-method/artifacts/index.ndjson
- .forge-method/state.yaml

## Artifacts

- .forge-method/artifacts/20260615-guide-cli-first-question-output-contract.md
- CHANGELOG.md

## Next Action

Continue post-parity Forge polish by auditing installed reload/guide behavior in real project starts against the richer human prompt contract.

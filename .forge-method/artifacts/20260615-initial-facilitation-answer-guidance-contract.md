# Initial Facilitation Answer Guidance Contract

- kind: runtime-guidance-contract
- created_at: 2026-06-15T18:28:43Z
- phase: 6-evolve
- workflow: runtime-builder
- status: initial-facilitation-answer-guidance-hardened

## Problem

New generated projects correctly start with `initial-facilitation`, but the first human answer is the moment where an agent can accidentally jump into backlog or build work. That answer must be treated as fresh human intent and routed through Guidance Engine while the durable state remains in discovery.

## Contract

- Answering `initial-facilitation` keeps story count at zero.
- Answering `initial-facilitation` leaves the project in `1-discovery / discover-intent`.
- `resume --json` requires Grill Gate before leaving the decision phase.
- `guide --question "<first answer>"` routes through Guidance Engine, not `build-story`.
- The human lede shows the clean first question text instead of embedding the whole `human_prompt` blob.
- Source and installed smokes assert the same no-premature-story and guided-first-question behavior.

## Proof

- `python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `python -m unittest discover -s tests`
- `python skills/forge-method/scripts/forge_method_runtime.py parity replay`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Next

Continue post-parity Forge polish by auditing discovery closeout: accepted intent should produce a durable discovery artifact and only then transition toward specification.

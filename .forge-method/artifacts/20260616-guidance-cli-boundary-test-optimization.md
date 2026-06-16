# Guidance CLI Boundary Test Optimization

- created_at: 2026-06-16T01:08:00+00:00
- status: guidance-cli-boundary-optimized
- workflow: runtime-builder
- lifecycle: durable

## Problem

After the previous Guidance Engine test optimization, several tests still spawned `guide --json` for assertions that only inspect the internal payload contract. That kept test runtime higher than needed and blurred which checks were proving CLI behavior versus runtime behavior.

## Decision

- Use direct `build_guide_payload` calls for JSON-only Guidance Engine assertions.
- Use direct replay state setup where the setup behavior is not under test.
- Keep `guide` subprocess coverage where the CLI surface itself is the contract:
  - empty-workspace human routing
  - Reality/Evidence Gate text rendering
  - human lede, `Guidance:`, `First question:`, and `Status:` lines
  - broad tracks/config command integration
  - mechanical `guide`/`next` text behavior
  - generated-project guided text after first facilitation

## Result

- Remaining `guide` subprocess calls in `tests/test_runtime.py`: 6, all intentionally CLI-facing.
- Converted JSON-only payload assertions in:
  - `test_reality_evidence_gate_blocks_impossible_and_cruel_ideas`
  - `test_guidance_human_lede_and_runtime_builder_contract`
  - `test_lifecycle_closure_guidance_and_compact_contracts`
  - `test_mechanical_work_order_goal_and_commit_policy_contracts`
  - `test_project_create_seeds_real_module_project`
- Focused timings improved:
  - Reality/Evidence Gate: 2.790s -> 0.986s
  - guidance human lede/runtime-builder: 5.925s -> 2.824s
  - lifecycle closure guidance: 7.030s -> 1.699s

## Validation

Focused tests passed:

- `python -m unittest tests.test_runtime.RuntimeTests.test_reality_evidence_gate_blocks_impossible_and_cruel_ideas -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_lifecycle_closure_guidance_and_compact_contracts -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_mechanical_work_order_goal_and_commit_policy_contracts -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_project_create_seeds_real_module_project -v`

Full validation will be recorded in the evidence artifact for this increment.

## Next Gap

Continue improving Forge human guidance depth and agent compactness. Do not convert the remaining `guide` subprocess calls unless a replacement test keeps equivalent CLI proof.

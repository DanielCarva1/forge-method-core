# Forge Core M3-S1 - fixture-backed eval compare baseline

Status: implemented and verified
Date: 2026-06-29

## Why this slice exists

Imported v2 research and planning require a strong single-agent baseline before
Forge recommends graph or multi-agent workflows. The current core has M1
preview/ready/trace and M2 graph dry-run/claim preflight, but it does not yet
have a live graph executor. Therefore M3 must start with precomputed eval-run
comparison rather than pretending to run agents.

Primary sources used:

- `docs/dev-docs/forge-method-core-dev-docs-v2/01_feature_specs.md#f05---eval-compare-single-agent-baseline`
- `docs/dev-docs/forge-method-core-dev-docs-v2/02_implementation_plan.md#milestone-3---eval-baseline`
- `docs/dev-docs/forge-method-core-dev-docs-v2/05_eval_and_quality_plan.md`
- `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/ADR-0002-single-agent-baseline-before-mas.md`
- External research anchors verified during implementation:
  - arXiv 2601.12307, *Rethinking the Value of Multi-Agent Workflow: A Strong Single Agent Baseline*.
  - arXiv 2606.05670, *Do More Agents Help? Controlled and Protocol-Aligned Evaluation of LLM Agent Workflows*.

## Product rule

`forge-core eval compare` may say `try_candidate` only when the candidate is not
worse on task success and improves at least one measured efficiency or quality
proxy. If evidence is missing, mismatched, or below policy, the command blocks or
keeps the baseline.

## This slice implements

- New pure crate `forge-core-eval`.
- `EvalCompareSuite` YAML for local fixture-backed comparisons.
- `forge-core eval compare --baseline single-agent --candidate graph`.
- Deterministic JSON and concise human output.
- Metrics:
  - task success / success rate;
  - cost micros;
  - wall-clock latency;
  - tool calls;
  - turns as trajectory length;
  - token total;
  - failure clusters;
  - evidence refs and trace-like refs;
  - deltas versus baseline;
  - recommendation and measurement gaps.
- Fail-closed diagnostics for empty/mismatched run sets, missing evidence refs,
  missing trace refs, duplicate task IDs, unsupported eval-run schema versions,
  and label mismatch.
- Fixture evidence and trace refs backed by local files under
  `docs/fixtures/eval-run-v0/evidence/` and
  `docs/fixtures/eval-run-v0/traces/`.

## This slice explicitly does not implement

- live model calls;
- live graph execution;
- mutating eval runs;
- MAS orchestration;
- MCP/A2A adapters;
- sidecar writes or trace appends.

## Acceptance

- `cargo test -p forge-core-eval` passes.
- `cargo test -p forge-core-cli --test eval_cli_e2e` passes.
- `cargo check -p forge-core-cli` passes.
- `forge-core eval compare --root . --allow-bootstrap-core --baseline single-agent --candidate graph --json` returns deterministic JSON from local fixtures.
- The default command does not create or mutate `.forge-method` state.

# Forge Method Core v2 - eval, QA and quality plan

## Goal

Turn architecture decisions into evidence. If a feature does not improve quality, cost, security, explainability or reliability, it must not become default.

## Mandatory baselines

1. Single-agent anchor.
2. WorkflowGraph single-agent.
3. WorkflowGraph multi-agent with real heterogeneity.
4. Manual human-mediated flow when it makes sense.

## Metrics

- Task success.
- Gate pass rate.
- False ready rate.
- Human intervention count.
- Tool call count.
- Runtime cost proxy.
- Latency.
- Trace completeness.
- Rollback success.
- Conflict detection rate.
- Risk audit findings.
- User comprehension for explain/preview.

## Initial evals

### Eval 1 - Preview safety

Question: does the preview correctly detect mutations, side effects, commands and gates before execution?

Fixtures:

- Operation read-only.
- Operation mutable with gate pass.
- Operation mutable with gate pending.
- Operation with absent effect ref.
- Operation with invalid lane claim.

### Eval 2 - Ready truthfulness

Question: does the ready gate avoid false positives?

Fixtures:

- Real test pass.
- Real test fail.
- Missing test treated as warning or fail per policy.
- Command that fails but tries to hide the error.
- File with fail-soft pattern.

### Eval 3 - Graph vs single-agent

Question: does graph workflow improve quality or cost against the single-agent anchor?

Compare:

- Single-agent plain.
- Single-agent with OperationContract.
- WorkflowGraph with verifier.
- WorkflowGraph with replan.

### Eval 4 - Memory governance

Question: does memory help without becoming false authority?

Cases:

- Memory raw evidence correct.
- Summary contradicts raw evidence.
- Memory tries to promote a rule without approval.
- Forget request removes record and prevents future retrieval.

### Eval 5 - Protocol security

Question: can MCP/A2A not mutate state outside of scope?

Cases:

- Tool without capability.
- Tool with wrong provider.
- Delegation chain above depth.
- Prompt injection via tool output.
- A2A task without PrincipalId.

## AI Risk Audit Gate

Initial checks:

1. Exception swallowed without log or return fail.
2. Test that always passes.
3. Mock replacing a critical path without assertion.
4. Error converted into success status.
5. Security check treated as warning when policy requires fail.
6. Secret hardcoded.
7. Shell command without argv policy.
8. Network access without policy.
9. File write outside the root.
10. Destructive operation without inverse or rollback.

## Reports

Each eval must produce:

- JSON machine-readable.
- Markdown human-readable.
- Evidence refs.
- Trace refs.
- Failure taxonomy.
- Recommendation: keep, change, block or remove.

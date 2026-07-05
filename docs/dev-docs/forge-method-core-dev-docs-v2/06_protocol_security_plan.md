# Forge Method Core v2 - security plan for MCP, A2A and governance

## Position

MCP and A2A come in as interoperability interfaces. They do not come in as workflow authority. The Forge kernel remains the source of truth for contracts, gates, effects, trace and governance.

## MCP

Recommended use:

- `preview`
- `ready`
- `explain`
- `trace.query`
- `graph.validate`
- `memory.inspect`
- `effect.apply` only with a validated OperationContract

Rules:

1. No MCP tool mutates state without a contract.
2. Every MCP tool declares a capability.
3. Every invocation has a trace_id.
4. Tool output cannot change next_action.
5. Tool output does not become authority.
6. Tool server needs provider binding when possible.
7. Allowlist per repo and per principal.

## A2A

Recommended use:

- Delegate a task to an external agent.
- Receive the result as an artifact or recommendation.
- Register the external agent as an actor, not as the owner of the state.

Rules:

1. A2A is not an internal subagent protocol.
2. A2A does not replace MCP.
3. A2A task needs a PrincipalId.
4. A2A result comes in as evidence or request, not direct mutation.
5. Delegation chain must be auditable.

## Identity and capability

Entities:

- `PrincipalId`: human owner, org or bot owner.
- `AgentId`: agent instance.
- `CapabilityId`: specific permission.
- `InvocationId`: individual call.
- `DelegationChain`: signed or registered chain.

Minimum policy:

- Read-only by default.
- Mutation requires an OperationContract.
- Destructive operation requires an inverse or explicit stop.
- Publish requires a separate policy.
- Cross-principal write requires an accepted IntentContract.

## Initial threat model

Threats:

- Tool lookalike.
- Wrong provider execution.
- Prompt injection via tool output.
- Escalation via delegation chain.
- Memory poisoning.
- Conflicting intents in shared state.
- Replay of an old invocation.
- Adapter drift between runtimes.
- Silent overwrite by a concurrent agent.

Forge controls:

- Capability binding.
- Trace append-only.
- Optimistic concurrency.
- ConflictContract.
- Read snapshot.
- Effect WAL.
- Authority boundary.
- Gate before mutation.
- Human arbitration.

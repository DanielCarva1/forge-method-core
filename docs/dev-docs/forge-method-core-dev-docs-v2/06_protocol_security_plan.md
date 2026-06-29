# Forge Method Core v2 - plano de seguranca para MCP, A2A e governance

## Posicao

MCP e A2A entram como interfaces de interoperabilidade. Eles nao entram como autoridade de workflow. O kernel Forge continua sendo a fonte de verdade para contratos, gates, effects, trace e governance.

## MCP

Uso recomendado:

- `preview`
- `ready`
- `explain`
- `trace.query`
- `graph.validate`
- `memory.inspect`
- `effect.apply` somente com OperationContract validado

Regras:

1. Nenhuma tool MCP muta estado sem contrato.
2. Toda tool MCP declara capability.
3. Toda invocation tem trace_id.
4. Tool output nao pode mudar next_action.
5. Tool output nao vira authority.
6. Tool server precisa de provider binding quando possivel.
7. Allowlist por repo e por principal.

## A2A

Uso recomendado:

- Delegar task para agente externo.
- Receber resultado como artifact ou recommendation.
- Registrar external agent como actor, nao como owner do state.

Regras:

1. A2A nao e subagent protocol interno.
2. A2A nao substitui MCP.
3. A2A task precisa de PrincipalId.
4. Resultado A2A entra como evidence ou request, nao mutacao direta.
5. Delegation chain precisa ser auditavel.

## Identidade e capability

Entidades:

- `PrincipalId`: dono humano, org ou bot owner.
- `AgentId`: instancia de agente.
- `CapabilityId`: permissao especifica.
- `InvocationId`: chamada individual.
- `DelegationChain`: cadeia assinada ou registrada.

Politica minima:

- Read-only por default.
- Mutacao exige OperationContract.
- Destructive operation exige inverse ou explicit stop.
- Publish exige policy separada.
- Cross-principal write exige IntentContract aceito.

## Threat model inicial

Ameacas:

- Tool lookalike.
- Wrong provider execution.
- Prompt injection via tool output.
- Escalation por delegation chain.
- Memory poisoning.
- Conflicting intents em shared state.
- Replay de invocation antiga.
- Adapter drift entre runtimes.
- Silent overwrite por agent concorrente.

Controles Forge:

- Capability binding.
- Trace append-only.
- Optimistic concurrency.
- ConflictContract.
- Read snapshot.
- Effect WAL.
- Authority boundary.
- Gate before mutation.
- Human arbitration.

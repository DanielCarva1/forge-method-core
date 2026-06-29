# Forge Method Core v2 - plano de eval, QA e qualidade

## Objetivo

Transformar decisoes de arquitetura em evidencias. Se uma feature nao melhora qualidade, custo, seguranca, explicabilidade ou confiabilidade, ela nao deve virar default.

## Baselines obrigatorios

1. Single-agent anchor.
2. WorkflowGraph single-agent.
3. WorkflowGraph multi-agent com heterogeneidade real.
4. Manual human-mediated flow quando fizer sentido.

## Metricas

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
- User comprehension para explain/preview.

## Evals iniciais

### Eval 1 - Preview safety

Pergunta: o preview detecta corretamente mutacoes, side effects, comandos e gates antes da execucao?

Fixtures:

- Operation read-only.
- Operation mutavel com gate pass.
- Operation mutavel com gate pending.
- Operation com effect ref ausente.
- Operation com lane claim invalida.

### Eval 2 - Ready truthfulness

Pergunta: o ready gate evita falso positivo?

Fixtures:

- Test pass real.
- Test fail real.
- Test ausente tratado como warning ou fail conforme policy.
- Comando que falha mas tenta esconder erro.
- Arquivo com padrao fail-soft.

### Eval 3 - Graph vs single-agent

Pergunta: graph workflow melhora qualidade ou custo contra single-agent anchor?

Comparar:

- Single-agent plain.
- Single-agent com OperationContract.
- WorkflowGraph com verifier.
- WorkflowGraph com replan.

### Eval 4 - Memory governance

Pergunta: memoria ajuda sem virar autoridade falsa?

Casos:

- Memory raw evidence correta.
- Summary contradiz raw evidence.
- Memory tenta promover regra sem approval.
- Forget request remove record e impede future retrieval.

### Eval 5 - Protocol security

Pergunta: MCP/A2A nao conseguem mutar estado fora de scope?

Casos:

- Tool sem capability.
- Tool com wrong provider.
- Delegation chain acima de depth.
- Prompt injection via tool output.
- A2A task sem PrincipalId.

## AI Risk Audit Gate

Checks iniciais:

1. Exception swallowed sem log ou return fail.
2. Test que sempre passa.
3. Mock substituindo caminho critico sem assertion.
4. Error convertido em success status.
5. Security check tratado como warning quando policy exige fail.
6. Secret hardcoded.
7. Shell command sem argv policy.
8. Network access sem policy.
9. File write fora do root.
10. Destructive operation sem inverse ou rollback.

## Reports

Cada eval deve gerar:

- JSON machine-readable.
- Markdown humano.
- Evidence refs.
- Trace refs.
- Failure taxonomy.
- Recommendation: keep, change, block ou remove.

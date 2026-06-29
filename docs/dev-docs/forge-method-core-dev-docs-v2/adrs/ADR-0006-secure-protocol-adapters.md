# ADR-0006 - MCP e A2A como adapters seguros

Status: proposto

## Contexto

MCP e A2A sao necessarios para ecossistema, mas apresentam riscos de identity, capability, trust propagation e composicao.

## Decisao

Adapters MCP/A2A nao sao fonte de verdade e nao mutam store diretamente. Toda mutacao passa pelo kernel e OperationContract.

## Consequencias

- Interoperabilidade sem entregar autoridade.
- Menos risco de tool poisoning.
- Trace e audit ficam consistentes.

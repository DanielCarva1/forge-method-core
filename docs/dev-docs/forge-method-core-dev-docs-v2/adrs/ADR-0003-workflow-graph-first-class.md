# ADR-0003 - WorkflowGraph como entidade de primeira classe

- **Status**: Proposed

## Contexto

Prompt routing solto gera loops, routing alucinado e execucao nao reproduzivel. A literatura recente aponta para grafos executaveis.

## Decisao

Criar `WorkflowGraph` v0. `OperationContract` continua existindo, mas entra como node ou payload de node.

## Consequencias

- Melhor dry-run.
- Melhor parallelismo.
- Verifier e replan ficam estruturais.
- Trace passa a se ligar a node_id e graph_id.

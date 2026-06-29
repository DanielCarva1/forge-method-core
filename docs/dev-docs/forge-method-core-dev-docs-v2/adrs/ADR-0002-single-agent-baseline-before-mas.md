# ADR-0002 - Single-agent baseline antes de multi-agent

Status: proposto

## Contexto

Papers recentes mostram que MAS homogeneo nao vence automaticamente um single-agent bem controlado.

## Decisao

Toda arquitetura multi-agent precisa ser comparada contra single-agent anchor com mesmo loader, tools, answer contract e usage accounting.

## Consequencias

- Multi-agent deixa de ser marketing e vira decisao medida.
- `forge eval compare` vira feature central.
- MAS so e recomendado quando ha heterogeneidade, paralelismo, isolamento ou governance real.

# ADR-0004 - Trace e eval como parte do produto

Status: proposto

## Contexto

Sem trace, nao ha reproducibilidade, debug, governance ou eval confiavel.

## Decisao

Todo run relevante gera `TraceEvent`. Toda feature de arquitetura deve ter eval comparavel.

## Consequencias

- Debug melhora.
- QA fica mensuravel.
- Agents externos podem ser auditados.
- Power users ganham replay e metrics.

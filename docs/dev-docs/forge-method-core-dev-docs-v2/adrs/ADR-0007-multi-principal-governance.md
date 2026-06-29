# ADR-0007 - Multi-principal governance

Status: proposto

## Contexto

Agentes de pessoas, orgs ou vendors diferentes podem trabalhar no mesmo shared state. Lane claims nao bastam quando ha principals diferentes.

## Decisao

Adicionar PrincipalId, IntentContract, ConflictContract e GovernancePolicy.

## Consequencias

- Conflitos viram objetos estruturados.
- Overwrite silencioso fica bloqueado.
- Arbitragem humana fica auditavel.
- Forge vira camada diferenciada de shared state agentic.

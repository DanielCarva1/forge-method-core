# ADR-0001 - Rust como kernel deterministico, semantica viva declarativa

Status: proposto

## Contexto

O Forge esta sofrendo quando agentes precisam editar Rust manual para cada mudanca semantica. A codebase ja mostra valor de Rust para contracts, runtime, store, WAL e validation, mas tambem mostra boilerplate crescente.

## Decisao

Rust fica no kernel deterministico. Prompts, policies em fluxo, templates, workflows experimentais e docs ficam declarativos ate estabilizar.

## Consequencias

- Menos sofrimento para agentes de codigo.
- Mais codegen e builders.
- Menos duplicacao entre YAML, Rust, docs e tests.
- Kernel continua seguro e auditavel.

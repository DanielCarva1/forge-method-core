# ADR-0005 - Memoria como policy, nao vector DB

- **Status**: Proposed

## Contexto

Papers recentes mostram que memoria depende de admission, routing, compression level e evidence support.

## Decisao

Criar `MemoryPolicy` antes de storage rico. Summary nao cria autoridade. Promotion exige boundary.

## Consequencias

- Reduz memory poisoning.
- Permite forget e redaction.
- Mantem raw evidence.
- Separa memoria episodica, skill e regra.

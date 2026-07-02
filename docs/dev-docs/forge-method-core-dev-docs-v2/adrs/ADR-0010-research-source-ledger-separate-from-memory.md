# ADR-0010 - Source Ledger do research é um log separado do memory

Status: accepted (F14.2 implementada — crate `forge-core-research` + contract `ResearchSource`/`ResearchPolicy` + PDP `ResearchContract::can_admit_source` + PEP append-only)

## Contexto

F14 (Knowledge Orchestration mode) precisa rastrear fontes que sustentam
claims de research agents em runtime (paper, URL, doc local com `fetched_at`,
`content_hash`, `trace_ref`). Já existem no repo:

- `FieldEvidenceRegistry` + `EvidenceSource` (`forge-core-contracts/src/evidence.rs`)
  como `ContractDocument` curado/estatictico, sustentando **decisoes de design do
  proprio Forge** (validado no anchor 122).
- `validate_yaml_source_id_references` (`forge-core-validate/src/lib.rs:280`),
  que ja rejeita `source_id` desconhecido contra o registry.

A tentacao seria fundir o source ledger de runtime no `forge-core-memory`
(tratar `ResearchSource` como um novo `MemoryEvent`, reaproveitar um log, um
lock, uma projecao) para economizar boilerplate (alinhado a F15).

## Decisao

O Source Ledger do F14 vive em **log proprio** (`<state_root>/research/sources.ndjson`,
lock `locks/research.sources.lock`, projecao `ResearchProjection`), em crate
nova `forge-core-research`, **espelhando o padrao PEP do `forge-core-memory`**.
Nao e um `MemoryEvent` kind, nao compartilha log/projecao/lock com o memory.

## Rationale (o trade-off real)

Reusar o log do memory (alternativa considerada) reintroduz a classe de bug
Model B uma camada abaixo: funde **confianca** (eixos Authority/Review de F06,
"isto e ground-truth actionable") com **proveniencia de citacao** ("isto aponta
para uma fonte") num unico event-sourced log. Misturar as duas semanticas num
`MemoryProjection::apply_event` viola a ortogonalidade que ADR-0002 cravou para
o memory e reabre a superficie de memory/citation poisoning.

O custo e boilerplate: uma crate, um lock, uma projecao a mais (contra o NFR
F15 de menos sofrimento manual). Aceito porque preservar a fronteira semantica
entre confianca e citacao e o produto inteiro do F14 — perder a fronteira
para ganhar concisao troca excelencia por conveniencia.

## Consequencias

- `forge-core-research` crate com PEP append-only template-do-memory (admit
  source sob `ResearchPolicy`, projecao rebuildavel, replay deterministico).
- Citation check resolve `source_id` contra o **backing conjunto**:
  `FieldEvidenceRegistry` (curated) uniao Source Ledger (runtime); fail-closed
  se nao resolve em nenhum. Estende `validate_yaml_source_id_references`,
  nao duplica.
- `EvidenceGraph` nao e tipo first-class nem populado pelo agent: e projecao
  `SourceId -> claims citantes`, computada por walk sobre artifacts (mesmo
  padrao do `reference_index` do `forge-core-store`).
- Claim de research e polimorfica: qualquer no que carregue `source_id`. F14
  define o lado source, nao um tipo novo de claim (evita inflacao de tipos e
  respeita o deletion test).
- Fixtures em `docs/fixtures/research-v0/` + `contracts/examples/research-source.yaml`.
- Nao bloqueia F08 (MCP): F14 e semantica de citacao ortogonal ao transport.
  Exposicao via MCP vira story pos-merge de ambos.

## Anti-objetivos

- Nao reimplementar o store PEP: `forge-core-research` compoe com
  `forge-core-store` (`append_json_line_with_durability`,
  `acquire_effect_store_lock`), nao duplica.
- Nao forcar workflow-graph semantics no evidence graph (dominio diferente).
- Nao opinar sobre tier/qualidade da fonte no MVP do citation gate: o gate
  atesta so **resolucao** do `source_id`; tier-min e policy futura (eixo de
  confianca separado, analogo ao Review axis de F06).
- Nao introduzir `research run` (pipeline linear): o "modo research" e a
  `ResearchPolicy` ativa, nao um fluxo. G1 anti-script-de-novela.

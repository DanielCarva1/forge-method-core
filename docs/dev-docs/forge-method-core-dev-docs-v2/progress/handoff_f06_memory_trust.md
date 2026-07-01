# Handoff para Agente Externo — F06 Memory Policy: Trust Model (A vs B)

**Data**: 2026-07-01
**Origem**: sessão Pi que terminou antes de finalizar o grill do ADR 0002.
**Motivo**: Daniel pediu handoff auto-contido; a próxima decisão (tratar o
`MemoryContract` legado que contradiz o ADR 0002) exige outro agente "frio".
**Skill de referência**: `grill-with-docs` (o grill estava em andamento, parou na Q1).

Este documento é a fonte da verdade para **continuar o F06.1**. Leia completo
antes de mexer em qualquer arquivo. Em particular, leia o ADR 0002 (escrito
nesta sessão) e o `memory.rs` existente (a descoberta que muda tudo).

---

## 1. O que esta sessão já fez (NÃO refazer)

### 1.1 Decisão A vs B — resolvida: **Modelo A (dois eixos ortogonais)**

Decidiu-se entre dois modelos de confiança para memória de agente:
- **B** = um eixo só (`approved` já dobra como autoritativo).
- **A** = dois eixos ortogonais: **authority** (`Raw → Provisional → Authority`,
  gated by policy + raw evidence) SEPARADO de **review** (`Unreviewed → Reviewed`,
  gated por atestado de um principal).

**Veredito: Modelo A.** Justificativa completa está no ADR 0002 §Rationale
(4 frentes: threat model, novelty, cross-field theory, coerência F06↔F07).

### 1.2 Artefatos gravados (no working tree, NÃO commitados)

| Path | Status | Conteúdo |
|---|---|---|
| `docs/adr/0002-memory-trust-model.md` | **novo** (`??`) | ADR completo, status **Proposed** |
| `docs/dev-docs/.../progress/followups_v0_1_to_10.md` | modificado (`M`) | Schema delta adicionado à seção F06.2 |

> ⚠️ O `git diff` do `followups` mostra 55 insertions/23 deletions, mas **só o
> hunk do F06.2 (~15 linhas) é desta sessão**. O resto (R-LINT/R-SCM/F05 ✅,
> "F06 ⏳ EM ANDAMENTO") é trabalho pré-existente não-commitado de uma sessão
> anterior perdida. Não atribuir ao autor do handoff.

### 1.3 Protocolo Forge — seguido corretamente

Claims adquiridos/liberados para `f06-1-memory-trust-model` (escrita do ADR +
F06.2) e `f06-handoff` (este handoff). `check-write` passou em todos os paths.
Claim do ADR/F06.2 já está **released**.

---

## 2. A descoberta crítica que INVA tive (e que o ADR 0002 ainda não reflete)

Esta é a parte mais importante do handoff. O ADR 0002, como escrito, **propõe
`PrincipalId`** — mas a codebase prova que isso é incompatível.

### 2.1 JÁ EXISTE um `MemoryContract` completo

`crates/forge-core-contracts/src/memory.rs` define:
- `MemoryContractDocument` / `MemoryContract` / `MemoryEntry` (com `schemars::JsonSchema`,
  `deny_unknown_fields`, YAML round-trip, testes em `schema_bounds.rs`).
- `MemoryProvenance { source_agent: Option<StableId>, ... }` — **identidade já é
  `StableId`**, não `PrincipalId`.
- `ApprovalState { Proposed, InReview, Approved, Rejected, AutoPromoted }` — **isto
  é o Modelo B** (single-axis; `Approved` dobra como aprovado+autoritativo).

### 2.2 O bug: `AutoPromoted` viola a NFR do F06

A NFR do F06 (em 3 lugares: `01_feature_specs.md`, issue, `feature_backlog.csv`):
> "Nenhuma memória vira authority automaticamente; promote exige policy e
> evidência raw."

Mas `ApprovalState::AutoPromoted` é uma variante legítima no enum, e o exemplo
canônico `contracts/examples/memory.yaml` **usa `approval: auto_promoted`**. A
casa demonstra, no próprio exemplo, o estado que sua NFR proíbe.

### 2.3 `PrincipalId` não existe (nem deve existir)

`rg "struct PrincipalId|enum PrincipalId"` → **zero hits** no código Rust. O
padrão de identidade da casa é `StableId(pub String)` em `common.rs`, com
filosofia explícita (R8):
> "Splitting into a distinct type makes that comparison a **compile error** —
> the R8 bug class becomes unrepresentable. `#[serde(transparent)]` keeps the
> wire format identical... (**zero migration cost**)."

**O ADR 0002 §4 propõe `reviewed_by: Option<PrincipalId>` — está errado. Deve ser
`Option<StableId>`.** (Isto resolve a falsa "inversão F06↔F07" que eu levantei
antes de ler o código.)

### 2.4 Blast radius do legado

- `MemoryContract` hoje é **parse-only**: usado em `forge-core-cli/src/contract_cmd.rs`
  (parse de YAML) e `forge-core-contracts/tests/schema_bounds.rs` (schema bounds).
  **Não há validator dedicado** (`forge-contract-validator` só tem `main.rs`).
  **Não é usado em runtime/store ainda.** É o shape v0 que o F06 promove a
  subsistema real.
- `auto_promoted` aparece em YAML apenas em: `contracts/examples/memory.yaml` e
  `contracts/research/protocol-scale-with-model-v1.yaml`.

---

## 3. A decisão pendente (o grill parou aqui)

O ADR 0002 (dois eixos, sem auto-promote) **não pode coexistir** com o
`ApprovalState` + `AutoPromoted` legado sem uma decisão. Duas opções foram
colocadas:

### Opção A — Additive + deprecation via risk-audit (recomendada pela casa)
1. **Adicionar** campos opcionais a `MemoryEntry` (non-breaking, `deny_unknown_fields`
   permite `Option` com `#[serde(default)]`):
   ```rust
   authority_level: Option<AuthorityLevel>,   // None = legacy → Raw
   review_state: Option<ReviewState>,          // None = legacy → Unreviewed
   reviewed_by: Option<StableId>,              // REUSA o newtype da casa
   reviewed_at: Option<String>,
   ```
2. **`AuthorityLevel` como enum distinto de `ApprovalState`** — aplicação direta
   do caso R8 (conceitos distintos = tipos distintos).
3. **`AutoPromoted` não removido (breaking) — marcado como anti-pattern via
   `risk-audit-v0`** (o mecanismo nativo do forge, definido em CONTEXT.md:
   "detect AI induced anti-patterns... rules are parametric YAML contracts").
   Detector `deny_auto_promoted`. Corrigir o exemplo canônico para
   `approved` + `authority_level: provisional`.
4. **`approval` legado vira ponte**: `approval: approved` → `authority_level: Provisional`
   (não Authority — respeita NFR) + `review_state: Reviewed`. Até bump
   `schema_version: 0.2`.

**Pró**: respeita "zero migration cost" (princípio documentado em `common.rs:22`).
**Contra**: período de coexistência `approval` ↔ novos eixos.

### Opção B — Breaking, schema_version bump 0.2
Modelo limpo de dois eixos, adapter legado, rewrite de fixtures.

**Pró**: sem período de coexistência. **Contra**: quebra o exemplo canônico,
exige reescrever `schema_bounds.rs` + `contract_cmd.rs` + fixtures, contradiz o
princípio "zero migration".

### Evidência externa — LIMITAÇÃO desta sessão
⚠️ **As tools de pesquisa web falharam nesta sessão** (sem API key OpenRouter →
perplexity/sonar indisponível; `web_explore` sem retorno; `intelli_search`/`intelli_research`
idem). **Não foi possível trazer papers/cases externos de Rust.** A evidência de
"melhores práticas" usada foi o **case interno R8** (`common.rs`), que é
autoritativo para esta codebase mas não substitui pesquisa externa. **Daniel pediu
expressamente** fundamentação em cases de sucesso/papers externos — isto ainda
está em aberto e deve ser feito na próxima sessão (ver §5).

---

## 4. Pesquisa que JÁ ESTÁ consolidada (pode reusar)

Para a justificativa do Modelo A (no ADR 0002 §Rationale). IDs confirmados:

**Threat model (vetor de envenenamento de memória/RAG):**
- Greshake et al., arXiv:2302.12173 — indirect prompt injection via retrieval.
- PoisonedRAG — Zou, Geng, Wang, Jia, arXiv:2402.07867.
- AgentPoison — Chen, Xiang, Xiao, Song, Li, arXiv:2407.12784 (NeurIPS 2024).
- MINJA — Dong et al., arXiv:2503.03704 (query-only injection).
- MEXTRA — Wang et al., arXiv:2502.13172 (ACL 2025, extração de privado).

**Novelty (ninguém tem a escada raw→provisional→authority com review separado):**
- Qwen-Agent: `class Memory(Agent)`, `source/content` = provenance fraca de doc.
- MetaGPT: `class Memory(BaseModel)`, `storage: list[Message]` + `metadata`; "verify" é SOP, não atributo de memória.
- AgentBench (THUDM): só tool-use/multi-turn; estado = `history`.
- Memory OS of AI Agent — arXiv:2506.06326 (Tencent/BUPT, EMNLP 2025): tiers short/mid/long mas **temporais**, não de confiança. Prior art mais próximo, eerra o eixo.
- "Memory in the Age of AI Agents" — arXiv:2512.13564: cita "trustworthiness" mas não propõe escada.
- DeepSeek: stateless. Kimi/Moonshot: tem memória persistente, sem provenance/trust público.

**Cross-field (ortogonalidade é o primitivo correto):**
- Berenson SIGMOD'95 (isolation levels); Bell-LaPadula MITRE'73 (security lattice);
  Sandhu RBAC96 IEEE'96; Buneman ICDT'01 (provenance).

---

## 5. Próximos passos (ordem)

1. **Decidir A vs B para o legado** (§3). Antes disso, fazer a **pesquisa web que
   faltou** (cases de sucesso Rust / papers) — Daniel pediu fundamentação externa.
   Buscar: newtype pattern para IDs (Alexis King "parse don't validate"), evolução
   de schema serde non-breaking (splitting enum em eixos), validator design com
   invariants estruturais vs de autorização (layered validation).
2. **Atualizar o ADR 0002** com a decisão de §3:
   - Corrigir `PrincipalId` → `StableId` em todo lugar (§schema delta, §invariants).
   - Adicionar seção "Coexistência com `ApprovalState` legado" (ponte ou breaking).
   - Registrar o detector `deny_auto_promoted` (se A) ou a migração 0.2 (se B).
3. **Marcar ADR 0002 como Accepted** após sobreviver ao grill (`grill-with-docs`).
4. **F06.2 (schemas)** — refletir a decisão no `memory.rs` real (não só no followups doc).
5. **F06.3** — criar crate `forge-core-memory`. **F06.4–F06.8** — admission/retention/promote/CLI/tests.

### Claims Forge
- Claim `f06-1-memory-trust-model`: **released** (escreveu ADR + F06.2 doc).
- Claim `f06-handoff`: **ativo** ao final desta sessão (este handoff). **Liberar** com:
  ```
  forge-core claim release --root '<repo-root>' --allow-bootstrap-core \
    --id 'claim.story.f06-handoff.f06-handoff' --agent 'codex-main'
  ```
- Para continuar editando o ADR/F06.2, re-adquirir com `Start-ForgeRepo.ps1`
  `-ScopeId f06-1-memory-trust-model -ClaimPath <paths>`.

---

## 6. Arquivos-chave para o próximo agente ler (em ordem)

1. `docs/adr/0002-memory-trust-model.md` — a decisão, como está hoje (com o bug `PrincipalId`).
2. `crates/forge-core-contracts/src/memory.rs` — o contrato legado que contradiz o ADR.
3. `crates/forge-core-contracts/src/common.rs` — o padrão `StableId`/R8 (a "melhor prática" interna).
4. `contracts/examples/memory.yaml` — exemplo canônico que usa `auto_promoted`.
5. `docs/dev-docs/forge-method-core-dev-docs-v2/01_feature_specs.md` (linhas 173+) — spec F06 + NFR.
6. `docs/dev-docs/forge-method-core-dev-docs-v2/progress/followups_v0_1_to_10.md` (linhas 154+) — epic F06.1–F06.8.

## 7. Mapa do terreno (crates)

```
forge-core-contracts/   ← memory.rs VIVE AQUI (newtypes, ApprovalState)
forge-contract-validator/ ← só main.rs hoje; F06 precisa de validator aqui
forge-core-cli/         ← contract_cmd.rs parseia MemoryContractDocument
forge-core-runtime/     ← F06 vai precisar tocar (promote é operação mutável)
forge-core-store/       ← F06 persiste memória aqui
(não existe) forge-core-memory/ ← a ser criado em F06.3
```

## 8. Glossário rápido (termos do F06)

- **Authority axis** (eixo 1): pode o agente tratar como ground truth? `Raw→Provisional→Authority`.
  Gated por policy + raw evidence. Nunca auto-promovido (NFR).
- **Review axis** (eixo 2): um principal atestou a curadoria? `Unreviewed→Reviewed`.
  Ortogonal ao authority. Modelado como atestado com `StableId` (não `PrincipalId`).
- **Promote**: sobe no eixo de authority (F06.6). **Review** é comando distinto
  (eixo 2). Conflitar os dois no CLI = reintroduzir o Modelo B pela porta dos fundos.

---

**Resumo uma linha**: ADR 0002 propõe Modelo A mas usa `PrincipalId` (incompatível
com `StableId` da casa) e ignora o `ApprovalState`/`AutoPromoted` legado —
decidir A-additive vs B-breaking (§3) com pesquisa web externa (§5.1), corrigir
o ADR, marcar Accepted, seguir F06.2→F06.8.

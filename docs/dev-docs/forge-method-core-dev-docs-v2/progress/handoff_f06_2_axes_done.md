# Handoff — F06.2 Two Trust Axes Implemented + MD→YAML Diretriz

**Data**: 2026-07-01
**Origem**: sessão que continuou o F06 após o handoff `handoff_f06_memory_trust.md`
(que parou na decisão A vs B). Esta sessão **resolveu A vs B, implementou o
schema delta, e recebeu uma diretriz estratégica nova sobre MD→YAML**.
**Skill de referência**: `improve-codebase-architecture` (deletion-test pass
feito; 4 candidatos identificados, 2 executados, 2 pendentes).

Este documento é a fonte da verdade para continuar. Leia completo.

---

## 1. O que esta sessão fez (NÃO refazer)

### 1.1 Decisão A vs B — RESOLVIDA: Opção A (additive)

Pesquisa web externa (que faltou na sessão anterior) confirmou Opção A por
unanimidade em 3 frentes:

1. **Newtype para IDs** — Alexis King "Parse, don't validate" + consenso
   comunidade (Palmieri, Kondi, worthe-it). **Padrão casa `StableId` está
   correto.** Inventar `PrincipalId` = contra-padrão.
2. **serde non-breaking** — `Option<T>` + `#[serde(default)]` é não-breaking
   mesmo com `deny_unknown_fields` (serde-rs/serde#2634, docs oficiais).
3. **Enum variant deprecation** — `#[deprecated]` em variante é suportado
   (RFC 1270, Rust Reference), mas tem caveat: derives geram warning espúrio
   (rust-lang/rust#92313). **Decisão: não usar `#[deprecated]`** no enum
   (tem 6 derives, quebraria CI `-D pedantic`); execução via detector
   `deny_auto_promoted` (risk-audit, fail-closed no YAML) — mais forte que
   warning de compile.

### 1.2 Artefatos gravados (commits pendentes — ver §5)

| Path | Status | Conteúdo |
|---|---|---|
| `crates/forge-core-contracts/src/memory.rs` | modificado | +2 enums (`AuthorityLevel`, `ReviewState`), +4 campos `Option` em `MemoryEntry` (`authority_level`, `review_state`, `reviewed_by`, `reviewed_at` — todos `#[serde(default)]`), +2 métodos ponte (`authority_level_effective`, `review_state_effective`), +7 testes da ponte |
| `docs/adr/0002-memory-trust-model.md` | **novo** (`??`) | ADR completo, status **Accepted** (era Proposed). `PrincipalId`→`StableId` corrigido em todo lugar (5 refs explicativas restantes, nenhuma como tipo real). Seção "Coexistence with legacy" com tabela de mapeamento |
| `CONTEXT.md` | modificado | +9 termos glossário F06 (Memory Document, Memory Kind, Authority Axis, Review Axis, Admission, Retention, Promote, Review Attestation, AutoPromoted Anti-pattern, Principal Attestation). "Fact" explicitamente **não é** `MemoryKind` |
| `contracts/examples/memory.yaml` | modificado | `auto_promoted`→`approved` + eixos explícitos (`authority_level`, `review_state`, `reviewed_by`, `reviewed_at`) — demonstra modelo novo |
| `contracts/risk-audits/deny-auto-promoted.yaml` | **novo** (`??`) | Detector regex fail-closed no token `approval: auto_promoted`. Sem Rust change |
| `AGENTS.md` | modificado | +seção "Context hygiene" (auto-monitor ~150-200k → handoff + /clear) |
| `docs/.../progress/excellence_roadmap.md` | modificado | tracking: Segurança 8→10, F05 fechado, R-LINT/R-SCM done, Features 9.7→9.8 |
| `docs/.../progress/followups_v0_1_to_10.md` | modificado | R-LINT/R-SCM/F05 marcados ✅, DoD checkboxes, ordem exec atualizada |

**Validação**: `cargo check` limpo. `cargo test -p forge-core-contracts` 14/14
verdes (7 novos da ponte). `cargo build -p forge-core-cli` verde. Anchor 122
preservado (validate não tocado).

### 1.3 `improve-codebase-architecture` — 4 candidatos, 2 feitos

Deletion-test pass identificação 4 deepening opportunities. **2 executados**:

- ✅ **Candidato 4** — CONTEXT.md glossary (pré-requisito de linguagem)
- ✅ **Candidato 3** — ponte legado ApprovalState→2 eixos + ADR Accepted

**2 pendentes** (ver §3):

- ⏳ **Candidato 1** — deepening `MemoryContract` (gates de confiança como métodos)
- ⏳ **Candidato 2** — criar crate `forge-core-memory`

---

## 2. DIRETRIZ ESTRATÉGICA NOVA (Daniel, esta sessão)

**Esta é a mudança mais importante da sessão.** Daniel decretou:

> "Em teoria não deveria existir nada em MD neste projeto com exceção do
> mínimo básico pra humanos: um guide (o quê/pra quê/instalar) + patch notes.
> Todos os outros documentos são feitos pra agentes, de agentes pra agentes.
> Documentos em YAML tipado como contratos. Escala com LLMs; prosa não."

### 2.1 Política já existente (confirmada)

O projeto **já tem** `contracts/migration/markdown-debt-inventory.yaml` com a
**allowlist final explícita**: `README`, `installation_instructions`,
`release_notes`. A diretriz de Daniel **confirma e acelera** política oficial.

### 2.2 Estado real da migração (inventory completo feito)

| Lado | Estado |
|---|---|
| Tipado (`contracts/`) | ✅ ~100% YAML (357 YAML / 1 MD) |
| Humano (`README.md`) | ✅ na allowlist |
| MD narrativo agente→agente | ❌ dívida concentrada |

**3 clusters de dívida** (priorizados):

**Cluster A — Glossário & specs (estrutural, alto valor)**
1. `CONTEXT.md` (185 linhas) — glossário. **Já marcado no debt-inventory** p/ virar `contracts/glossary.yaml`. *Acabei de adicionar 9 termos F06 — mais razão pra tipar.*
2. `docs/dev-docs/.../01_feature_specs.md` (501) — specs com campos repetidos. Naturalmente tipável.
3. `contracts/research/supply_chain.md` — único MD numa pasta de 15 YAML. Anomalia.

**Cluster B — Roadmaps & tracking (transiente)**
4. `excellence_roadmap.md` (481) + `followups_v0_1_to_10.md` (415) — os que editei hoje. Estruturados, tipáveis como `progress_v0`.
5. `progress/*.md` (25 arquivos) — handoffs/inventários. Inventários (`r1`, `r12`) são tabelas puras.

**Cluster C — Dev-docs master (pesado, decisório)**
6. `09_system_design_roadmap.md` (508), `08_priority_recommendations_plan.md` (449). Sobreposição com `data/release_plan.yaml`.

### 2.3 Gap no roadmap de excelência

A frente "Docs/rastreabilidade" está em 10/10, mas mede rastreabilidade de
ADR/papers, **não tipagem de docs**. Falta um critério "zero narrativa
agente→agente" alinhado ao debt-inventory.

**Novo épico candidato**: **R-DOC** — markdown-debt → typed contracts. DoD =
allowlist final satisfeita (só README + installation + release_notes como MD).

---

## 3. Próximos passos (ordem)

### Caminho A — Continuar F06 (candidatos 1+2 do improve-codebase)

1. **Candidato 1** — deepening `MemoryContract`: adicionar gates de confiança
   como métodos (`can_admit(policy, evidence)`, `can_promote(policy, evidence)`).
   Deletion test: passou — complexidade se concentra, não espalha.
2. **Candidato 2** — criar crate `forge-core-memory` (`admission.rs`,
   `retention.rs`, `promote.rs`). 2 adapters garantidos (store + CLI) = seam real.
3. F06.3-F06.8 — admission gate, TTL sweep, forget log, promote evidence gate,
   CLI `forge-core memory ingest/list/forget/promote/review` (command_registry
   pattern: 2 edit points, ver `command_registry.rs:68`), fixtures + E2E.

### Caminho B — Migração MD→YAML (diretriz nova do Daniel)

1. **R-DOC** — adicionar épico ao `followups_v0_1_to_10.md` com stories + DoD.
2. Cluster A item 1: `CONTEXT.md` → `contracts/glossary.yaml` (já marcado no
   debt-inventory; acabou de receber termos F06).
3. Resto Cluster A/B/C por prioridade.

### Caminho C — Diagnóstico produto 10/10

Daniel pediu: "usa teu contexto de 1M pra ler o projeto todo e decidir o que
falta". O inventory de docs (§2.2) já é metade desse diagnóstico. Falta cruzar
com `feature_backlog.csv` + `release_plan.yaml` pra verfeatures P1/P2/P3 vs dívida.

---

## 4. Arquivos-chave para o próximo agente ler (em ordem)

1. `docs/adr/0002-memory-trust-model.md` — **Accepted**, modelo A, Opção A.
2. `crates/forge-core-contracts/src/memory.rs` — implementação real (campos + ponte).
3. `CONTEXT.md` — glossário com termos F06 (9 entradas novas; **migrar p/ YAML**).
4. `contracts/risk-audits/deny-auto-promoted.yaml` — detector do anti-pattern.
5. `contracts/migration/markdown-debt-inventory.yaml` — **política oficial MD→YAML**.
6. `docs/dev-docs/.../progress/followups_v0_1_to_10.md` — roadmap (F06 pendentes).
7. Este handoff.

## 5. Claims Forge

Nenhum claim ativo ao fim desta sessão (só leitura + escrita em arquivos não
claim-governados nesta sessão). Para continuar editando `memory.rs` ou criar
`forge-core-memory`, adquirir claim `f06-2-trust-axes` ou similar.

## 6. Notas de contexto

- **rust-analyzer.toml** tem diff pré-existente (não desta sessão, não deste
  commit). Mudou prefix keys p/ `rust-analyzer.*`. Deixado untouched.
- **CRLF warnings** nos docs `progress/*.md` são informativos (Git normaliza).
- **`#[deprecated]` evitado** no enum ApprovalState — ver §1.1 ponto 3.

---

**Resumo uma linha**: F06.2 implementado (2 eixos, ponte, ADR Accepted, 14
testes verdes); Daniel decretou migração MD→YAML total (política já existe no
debt-inventory); próximos passos = candidatos 1+2 do improve-codebase OU
épico R-DOC MD→YAML, a definir em sessão limpa.

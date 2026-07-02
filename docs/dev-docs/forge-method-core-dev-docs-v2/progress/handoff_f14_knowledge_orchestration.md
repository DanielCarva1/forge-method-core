# Handoff — F14 Knowledge Orchestration mode (P3)

**Data**: 2026-07-02 (grill F14.1 fechado nesta sessão)
**Branch a criar**: `f14-knowledge-orchestration` (a partir de `f06.2-trust-axes`, NÃO de `master` — ver §14)
**Prioridade**: P3 (mais baixa; research/orchestration; greenfield)
**Esfôrço**: alto   **Risco**: médio   **Impacto**: destrava research agents
**Especificação**: `docs/dev-docs/forge-method-core-dev-docs-v2/01_feature_specs.md:437-468`
**Stories detalhadas**: `progress/followups_v0_1_to_10.md` (Epic F14 — **F14.1 ✅ FECHADO**, F14.2–F14.8 definidas)
**Depende de**: memory crate (F06 ✅), eval harness (F05 ✅), trace — todos prontos.
**ADR**: ADR-0010 (`adrs/ADR-0010-research-source-ledger-separate-from-memory.md`, status `proposto` → `accepted` ao fechar F14.2).
**Glossário**: `CONTEXT.md` § Knowledge Orchestration (F14).

> ✅ **F14.1 (grill) FECHADO.** As 7 decisões abaixo estão cravadas em ADR-0010
> + CONTEXT.md + followups. O próximo agente **pode codar a partir de F14.2**
> (não precisa re-grillar). Este handoff foi atualizado pós-grill; a seção
> "Estado atual — GREENFIELD" (§2) foi corrigida: F14 **não** é greenfield
> total — `SourceId`, `FieldEvidenceRegistry`, e `validate_yaml_source_id_references`
> já existem e são reusados/estendidos (não reimplementados).

Este documento é autocontido. Leia completo.

---

## 0. Decisões cravadas no grill F14.1 (NÃO re-debater)

1. **Ledger distinto, reusa `SourceId`** — F14 cria `forge-core-research`
   (PEP append-only, template `forge-core-memory`) com log próprio
   `research/sources.ndjson`. **Não funde** runtime no `FieldEvidenceRegistry`
   (reintroduz Model B). **Não forja** tipos novos de id (recria bug R8). Reuso
   é só `SourceId` (`forge-core-contracts/src/common.rs:57`).
2. **Claim polimórfica** — **sem tipo `ResearchClaim`**. F14 define o lado
   source + uma restrição sobre claims ("tem `source_id`? é citável"). Evidence
   graph é **projeção**, não struct first-class (deletion test aprova).
3. **Não bloqueia F08** — F14 é semântica de citação ortogonal ao transport.
   Exposição via MCP vira story pós-merge de ambos (F14.8, opcional).
4. **Store próprio + ADR-0010** — `research/sources.ndjson` + lock próprio +
   projeção própria, espelhando `forge-core-memory`. ADR-0010 obrigatório
   (passa nos 3 testes: hard-to-reverse, surprising, real trade-off).
5. **Fail-closed = resolução** — citation gate atesta só que `source_id`
   resolve (curated ∪ runtime), **não tier/qualidade**. Validator offline **+**
   gate runtime no path mutável (padrão risk-audit). Tier-min é policy futura.
6. **Verbs atômicos + `ResearchPolicy`** — **sem `research run`** (G1
   anti-script-de-novela). O "modo research" é a policy ativa.
7. **Glossário** — termos F14 em `CONTEXT.md`; ambiguidade "evidence" (3 eixos:
   `evidence_ref` F06 / `EvidenceTier` / resolução `source_id` F14) flaggeada.

---

## 1. O que é o F14

**Knowledge Orchestration mode** = um modo de operação do Forge para **research
agents** (pesquisa, produto, analistas) que precisam de **fontes, claims e
evidências** — não só resumo.

Especificação verbatim (`01_feature_specs.md:437-468`):
- **Usuários**: research/product/analysts.
- **Evidence papers**: P13, P14, P15.
- **Crates envolvidas**: `memory`, `trace`, `eval`.
- **Demanda**: "Research agents precisam de fontes, claims e evidências, não só
  resumo."
- **Produto**: "Modo research com **evidence graph**, **source ledger** e
  **citation checks**."
- **Critério de aceitação**: cada claim importante aponta para um `source_id` +
  evidência local/web.

Em uma frase: **research agents produzem claims sempre rastreados a fontes
auditáveis** (evidence graph + source ledger + citation check), em vez de
síntese opaca.

---

## 2. Estado atual — GREENFIELD (confirmado) — PARCIALMENTE CORRIGIDO pelo grill

> ⚠️ **Correção pós-grill (F14.1):** a afirmação original "zero código /
> nenhum hit" estava **errada**. Os conceitos de source/citation **já existem
> parcialmente** e são reusados/estendidos (decisão §0.1 e §0.5). O que é
> greenfield é só: a crate `forge-core-research` (PEP), o contract
> `ResearchSource`/`ResearchPolicy`, a projeção `evidence_graph`, o CLI
> `research`, e o ADR-0010.

- **Já existe (reusar/estender, NÃO reimplementar):**
  - `SourceId` (newtype) — `forge-core-contracts/src/common.rs:57`.
  - `FieldEvidenceRegistry` + `EvidenceSource { id, tier, title, url,
    confirmed_origin, observed_claims, forge_implications }` —
    `forge-core-contracts/src/evidence.rs`. Backing **curated** de citation.
  - `validate_evidence_registry` + `validate_yaml_source_id_references`
    (emite `UnknownEvidenceSourceRef` p/ `source_id` desconhecido) —
    `forge-core-validate/src/lib.rs:255,280`. F14.3 **estende** este validator
    p/ aceitar 2 backings, não duplica.
  - `contracts/research/field-evidence-20260625.yaml` (registry real) e
    `contracts/research/*.yaml` com campo `citation:` (fixtures vivas).
- **Greenfield (F14 cria):**
  - Crate `forge-core-research` (PEP append-only).
  - Contract `ResearchSource` + `ResearchPolicy` (em
    `forge-core-contracts/src/research.rs`).
  - Log `research/sources.ndjson` + lock + `ResearchProjection`.
  - Projeção `evidence_graph` (claim→source, walk sobre artifacts).
  - CLI `forge-core research source|cite|check|graph`.
  - ADR-0010 (criado nesta sessão, status `proposto`).
- **"orchestration" / "knowledge orchestration"** aparecem **só como citações de
  paper**, nunca como features construídas:
  - `00_master_development_doc.md:151` P05 GraphBit "deterministic graph orchestration".
  - `00_master_development_doc.md:155` P09 MemFlow "memory orchestration".
  - `00_master_development_doc.md:161` P15 Agents-K1 "knowledge orchestration".
- `00_master_development_doc.md:86-88`: F14 = P3 "Knowledge Orchestration mode"
  (P13, P14, P15).

### Primitivas existentes que F14 pode reusar (NÃO reimplementar)

- **`forge-core-memory` (F06 ✅)**: trust model com eixos Authority (1) e Review
  (2), `evidence_refs`, `promote` exigindo raw evidence. `MemoryEntry` com
  `evidence_ref`. **F14 provavelmente estende o conceito de evidence para
  "source"**.
- **`forge-core-trace`**: rastreabilidade de run (TraceEvent). F14 precisa
  correlacionar claims a runs/traces.
- **`forge-core-eval` / `forge-core-eval-harness` (F05 ✅)**: arms single-agent /
  graph / mas / manual. Comparison harness que canonicaliza + grada. **F14 pode
  adicionar um novo `EvalArm`** (research/orchestration) e medir contra baseline
  single-agent — o harness já existe.

→ F14 não inventa do zero; compõe memory + trace + eval sob um novo "modo
research".

---

## 3. Os 3 primitivos do produto F14 (a sharpenar no grill)

Da spec: "evidence graph + source ledger + citation checks". Definições
iniciais (a validar com papers P13/P14/P15):

1. **Source Ledger** — registro append-only de **fontes** (papers, URLs, docs
   locais) com provenance. Provavelmente um novo contract + PEP no padrão do
   `forge-core-memory` (append-only JSONL, fs4 lock, `append_json_line_*`).
   Análogo a `MemoryEntry` mas para sources.
2. **Evidence Graph** — grafo connecting claims → sources (via `source_id`).
   Reusar `forge-core-graph`? Ou é um grafo diferente (provenance, não workflow)?
   Decisão do grill.
3. **Citation Check** — validador que **rejeita** claims sem `source_id` +
   evidência (fail-closed). Padrão `forge-core-validate`: diagnostics tipados
   acumulando em `ValidationReport`, sem short-circuit.

**Princípio-chave (provável, a confirmar)**: research claims **sem fonte** são
**rejeitados** (fail-closed), como memory `promote` sem raw evidence é rejeitado
(F06). Espelhar a filosofia.

---

## 4. Perguntas em aberto (resolver no grill F14.1)

1. **F14 é crate nova ou modo?** Provável: nova crate
   `forge-core-research` (PEP source ledger + citation validator) + verb
   `forge-core research source/cite/check`. Confirmar com deletion test.
2. **F14 vs MCP (F08)**: research agents consomem Forge via CLI ou via MCP tools?
   Se F08 estiver pronto, F14 pode ser só "expor research tools via MCP". **Se F08
   não estiver pronto, F14 ainda faz sentido como crate + CLI standalone.**
   Decidir se F14 bloqueia em F08 ou não (recomendação: não bloquear).
3. **Evidence Graph**: reusar `forge-core-graph` (workflow graph) ou é domínio
   diferente (provenance graph)? Provável: **diferente** — não forçar workflow
   semantics em provenance. Mas verificar reuse de primitives.
4. **Source Ledger vs MemoryEntry**: source é um kind de memory, ou tipo
   distinto? Se kind de memory, F14 = policy + validator sobre memory existente.
   Se distinto, crate nova. Tendência: **distinto** (source tem metadata próprio
   — URL, DOI, hash, fetched_at — que não cabe em `MemoryEntry`).
5. **Citation Check como validator ou gate runtime?** Provável: ambos —
   validator em `forge-core-validate` (diagnostics) + gate opcional no
   execute-operation path.
6. **Anti-script-de-novela (G1)**: o "modo research" não deve virar roteiro
   linear de pesquisa. Deve ser paramétrico.
7. **Novos termos `CONTEXT.md`**: "SourceLedger", "EvidenceGraph", "Citation",
  "ResearchClaim", "SourceId". Sharpen no grill.

---

## 5. Ordem de execução (estimada — F14.1 define o resto)

### F14.1 — [grill + improve + research] Design  ⚠️ OBRIGATÓRIO PRIMEIRO
- Ler papers P13/P14/P15 (citar em `contracts/research/`).
- Grill-with-docs nas perguntas §4.
- Deletion test: source ledger / evidence graph / citation check — cada um é deep?
- ADR novo (provável ADR-0010) se houver trade-off hard-to-reverse (ex.: "source é
  tipo distinto de memory").
- Atualizar `CONTEXT.md` com termos.
- Breakdown de F14.2-F14.x no followups.

### F14.2-F14.x — definir após grill
Prováveis (a confirmar):
- **F14.2** — `SourceLedger` contract + crate `forge-core-research` + PEP
  append-only (template: `forge-core-memory`).
- **F14.3** — `EvidenceGraph` (claim → source via `source_id`).
- **F14.4** — Citation check validator em `forge-core-validate` (diagnostics
  tipados, fail-closed).
- **F14.5** — CLI `forge-core research source add|list` / `cite` / `check`.
- **F14.6** — Fixtures + E2E (template: `memory_cli_e2e.rs`).
- **F14.7** — (opcional) novo `EvalArm` "research" no eval harness para medir
  research agents contra baseline.

---

## 6. Anti-objetivos (NÃO fazer)

- ❌ Reimplementar memory PEP. Source ledger **compõe** com `forge-core-store`
  (fs4 lock, `append_json_line_*`), não duplica.
- ❌ Forçar workflow-graph semantics em evidence graph. Domínio diferente.
- ❌ Síntese opaca. Todo claim research aponta para `source_id` + evidência.
- ❌ `Result<_, String>` / `anyhow` / `thiserror`. Convenção AGENTS.md.
- ❌ Codar antes de F14.1 (grill) — F14 é greenfield e pouco especificada.

---

## 7. Edits em arquivos "shared" (conflito de merge previsível)

Mesmo padrão — 3 arquivos append-only (adicionar no fim do array):

1. **`Cargo.toml` (root) `members`** — append `"crates/forge-core-research"`.
2. **`crates/forge-core-cli/src/lib.rs`** — append `pub mod research_cmd;`.
3. **`crates/forge-core-cli/src/command_registry.rs`** — append `CommandSpec`:
   ```rust
   CommandSpec {
       name: "research",
       usage_lines: &[
           "       forge-core research source add    --source-file <path> [--root <path>] [--allow-bootstrap-core] [--research-dir <path>] [--no-json]",
           "       forge-core research source list    [--root <path>] [--allow-bootstrap-core] [--research-dir <path>] [--no-json]",
           "       forge-core research cite     --claim-id <id> --source-id <id> [--evidence <ref>...] [--no-json]",
           "       forge-core research check    [--root <path>] [--allow-bootstrap-core] [--no-json]",
       ],
       handler: crate::research_cmd::run_research_command,
   },
   ```
   `CommandSpec` em `command_registry.rs:40-49`.

Possível 4º overlap se adicionar validator:
4. **`crates/forge-core-validate/src/lib.rs`** — append `validate_citation_*`
   functions. **Cuidado**: F07 também adiciona validators lá. Resolução mecânica
   se ambos appendarem no fim.

Ver `handoff_master_parallelization.md`.

---

## 8. Template de Cargo.toml da nova crate

Espelhar `crates/forge-core-memory/Cargo.toml`:

```toml
[package]
name = "forge-core-research"
version.workspace = true
edition.workspace = true

[dependencies]
forge-core-contracts = { path = "../forge-core-contracts" }
forge-core-store = { path = "../forge-core-store" }   # reusa append-only + lock
serde.workspace = true
serde_json.workspace = true
schemars.workspace = true
tracing.workspace = true

[dev-dependencies]
proptest.workspace = true
```

---

## 9. CLI command module — padrão a espelhar

Criar `crates/forge-core-cli/src/research_cmd.rs`. Template: `memory_cmd.rs` (F06).
- `pub fn run_research_command(args: &[String]) -> Result<(), ExitError>`.
- `CliEnvelope` (`crates/forge-core-contracts/src/envelope.rs:77`): `ok/err/reject`.
- Dual-output via `emit()` local (ver `memory_cmd.rs:869-895`).

---

## 10. Validação obrigatória por story

Hook `pi-green-loop`: `check + clippy -W pedantic + test + fmt` por turno.
CI usa `-D pedantic`.
**Anchor 122** preservada: `validate --json` → 122× `"diagnostics": 0`.

---

## 11. Forge governance — protocolo claim/check-write

1. `forge-core project resolve` → `claims_dir`.
2. `forge-core claim acquire --scope <kind> --id <scope-id> --agent <id> --path <repo-path>... --claims-dir <path>`.
3. `forge-core claim check-write --agent <id> --target <path> --claims-dir <path>`
   — antes de editar. Falhou → claim/scope/report.
4. Heartbeat; `claim release` ao terminar.
5. `expired_requires_handoff` → `forge-core claim handoff` (não delete/retry),
   depois status + acquire fresh.

---

## 12. Fixtures — convenção

- `contracts/examples/` — flat, `<contract>.yaml` ou `<contract>-<variant>.yaml`.
- `docs/fixtures/<thing>-v0/` — subdir versionado + `README.md`.

F14 provável: `docs/fixtures/research-v0/` (source-ledger.yaml,
citation-valid.yaml, citation-missing-source.yaml) + `contracts/examples/source-ledger.yaml`.

---

## 13. Arquivos-chave para ler (em ordem)

1. `docs/dev-docs/forge-method-core-dev-docs-v2/01_feature_specs.md:437-468` — spec F14.
2. `docs/dev-docs/forge-method-core-dev-docs-v2/00_master_development_doc.md:151,155,161,86-88`
   — papers P05/P09/P13/P14/P15 + classificação F14.
3. `crates/forge-core-memory/src/lib.rs` — **template PEP** (append-only,
  projection, lock) para source ledger.
4. `crates/forge-core-memory/src/admission.rs` / `promote.rs` — fail-closed
  pattern (evidence gate).
5. `crates/forge-core-validate/src/lib.rs` — padrão validator (diagnostics
  tipados, `ValidationReport`, sem short-circuit).
6. `crates/forge-core-graph/` — avaliar reuse para evidence graph (provável: não
  reusar, domínio diferente).
7. `crates/forge-core-eval/src/lib.rs` — arms (single-agent/graph/mas/manual);
  F14 pode adicionar arm "research".
8. `CONTEXT.md` — glossário (termos trust model: Authority/Review axes,
  evidence_refs).
9. `crates/forge-core-cli/src/memory_cmd.rs` — template CLI.
10. `progress/followups_v0_1_to_10.md` (Epic F14) — placeholder; preencher após grill.
11. Este handoff.

---

## 14. Notas operacionais

- **Branch**: `f14-knowledge-orchestration` a partir de **`f06.2-trust-axes`**
  (NÃO de `master`). `master` está 10 commits atrás, sem F06/F07.
  `f06.2-trust-axes` tem F06+F07 committed. `git checkout -b f14-knowledge-orchestration`
  a partir do HEAD atual. Não trabalhar em `master` nem em `f06.2-trust-axes`.
- **Greenfield**: sem ADR, sem contract, sem código. Grill é ~50% do trabalho.
- **Prioridade P3**: pode esperar F08 (MCP) se houver sinergia research-via-MCP,
  mas **não bloqueia** — F14 faz sentido como crate + CLI standalone.
- **Overlap com F07**: possível em `forge-core-validate/src/lib.rs` (ambos
  adicionam validators). Resolução mecânica.
- **Contexto**: per-story, commit + `/clear`. Handoff incremental se pesar.

---

**Resumo uma linha**: F14 = modo research (crate `forge-core-research`) com
**source ledger** append-only + **evidence graph** (claim→source via `source_id`)
+ **citation check** validator fail-closed, compondo com memory/trace/eval
existentes; **greenfield e pouco especificada — F14.1 (grill + papers P13/P14/P15)
é obrigatório primeiro**; spec em `01_feature_specs.md:437-468`.

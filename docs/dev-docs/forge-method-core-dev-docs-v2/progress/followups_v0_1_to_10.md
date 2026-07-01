# Follow-ups Roadmap — v0.1.0 → 10/10

**Data**: 2026-07-01
**Status**: plano ativo (atualizado 2026-07-01 — R-LINT ✅, R-SCM ✅, F05 ✅ fechados; F06 em andamento por outro agente)
**Dono**: Daniel (codebase owner) + agente executor
**Norte estratégico**: rápido, robusto, performativo, protocolo-guia que escala
com a capacidade dos agentes, nunca script de novela, sempre Rust ou compatível,
sempre lastreado em melhores práticas e papers científicos (orientais e
ocidentais).

## Contexto

O v0.1.0 foi lançado publicamente (Apache-2.0, 5 binários cross-compilados,
CI verde, release em 2 remotes). Progresso desde então:

- ✅ **Rust best practices** — formalizado com CI `-D warnings` (R-LINT: 41 pedantic → 0)
- ✅ **Segurança supply chain 8→10** — SBOM (CycloneDX) + sigstore keyless no `release.yml` (R-SCM)
- ✅ **F05 eval harness** — fechado (design, schema, executor, grader, CLI, trace, E2E)

Resta chegar a 10/10 em **2 frentes**:

1. **Rápido 9→10** — otimizações pontuais de hot paths (Epic R-FAST, último na fila)
2. **Features comunidade 9.8→10** — F06/F07/F08/F12/F14

As outras 9 frentes já estão em 10/10.

## Metodologia (aplicada a cada epic)

Cada feature P1 (F05-F08) segue este fluxo antes de codar:

1. **Entender** — ler specs, código existente, fixtures
2. **Pesquisar** — Context7/papers para padrões e melhores práticas
3. **`improve-codebase-architecture`** — aplicar deletion test, identificar
   deepening opportunities, decidir onde ficam as seams
4. **`grill-with-docs`** — stress-test do design, sharpen terminology,
   atualizar `CONTEXT.md` (glossary) e ADRs inline conforme decisões cristalizam
5. **Planejar** — breakdown em stories pequenos e bem definidos
6. **Documentar** — epics/stories neste arquivo
7. **Desenvolver** — implementar story por story, validando a cada passo
8. **Medir** — atualizar scores no `excellence_roadmap.md`

## Épicos e Stories

### Epic R-LINT — Lint cleanup (41 pedantic → 0, CI `-D warnings`) ✅ FECHADO

**Frente**: Rust best practices (formalizar 10/10 com CI deny)
**Esforço**: médio **Risco**: baixo **Impacto**: formaliza 10/10
**Status**: COMPLETO (2026-07-01). 41 pedantic lints zerados em `--all-targets`;
CI flipado de `-W` para `-D clippy::pedantic` (R-LINT.6, commit `e1439c6`).
Auditoria por categoria em `progress/r_lint_audit.md`. R-LINT.1 (auditoria),
R-LINT.2 (fixes mecânicos + refatoro), R-LINT.5 (testes/benches), R-LINT.6 (CI flip)
executados; R-LINT.3/R-LINT.4 absorvidos nos fixes de R-LINT.2 (funções longas e
`too_many_arguments` resolvidos junto aos lints mecânicos).

#### R-LINT.1 — Auditar e categorizar os 41 lints
- Listar todos os 41 warnings com arquivo + linha + categoria
- Classificar: mecânico (~10) / precisa-refatorar (~15) / código-de-teste (~10) / cosmético (~6)
- Output: tabela no `progress/r_lint_audit.md`

#### R-LINT.2 — Fix lints mecânicos
- `format!` appended to String → `push_str`
- `let_and_return` → return direto
- `needless_pass_by_value` (onde seguro) → `&`
- `naive_byte_count` → `.len() as u64`
- `incompatible_msrv` → proteger com `#[cfg]` ou alternative API
- Um commit por grupo de lint relacionado

#### R-LINT.3 — Refatorar funções longas
- `too_many_lines` (288/100, 161/100) → extrair helpers nomeados
- Aplicar `improve-codebase-architecture`: cada helper vira módulo deep
  (deletion test: complexidade se concentra, não se espalha)

#### R-LINT.4 — Address `too_many_arguments`
- Introduzir parameter structs (ex: `PreviewRequest`, `ClaimRequest`)
- Reduz argc de 9-10 para ≤7

#### R-LINT.5 — Limpar warnings de teste/bench
- `doc_overindented_list_items`, `panic!` com Debug, `format!` em iterator
- Aplicar `#[allow]` documentado onde é idiomático de teste

#### R-LINT.6 — Flip CI para `-D warnings` + atualizar DoD
- `.github/workflows/ci.yml`: `-W clippy::pedantic` → `-D clippy::pedantic`
- Atualizar `excellence_roadmap.md` DoD: `<100 warnings` → `0 warnings`
- Validar anchor 122 preservada

---

### Epic R-SCM — Supply chain hardening (SBOM + sigstore) ✅ FECHADO

**Frente**: Segurança supply chain 8→10
**Esforço**: médio **Risco**: baixo **Impacto**: sobe de 8 para 10 ✅ (agora 10)
**Papers**: SLSA, sigstore (CT log transparency)
**Status**: COMPLETO (2026-07-01, commit `060a5a9`). `release.yml` agora:
cosign keyless signing via GitHub OIDC (cada archive assinado, bundle `.sigstore`
com signature + cert + Rekor entry); CycloneDX SBOM gerada do `Cargo.lock` por
target. R-SCM.1-R-SCM.5 entregues num único commit consolidado.

#### R-SCM.1 — Adicionar cargo-cyclonedx + gerar SBOM em CI
- `[workspace.dev-dependencies]`: `cyclonedx-bom` (Rust-native) ou
  `cargo-cyclonedx`
- Step no `release.yml`: `cargo cyclonedx -f json --output-pattern package`
  antes do packaging
- Output: `bom.json` por target

#### R-SCM.2 — Anexar SBOM à GitHub Release
- `release.yml`: upload do `bom.json` como asset da release
- Naming: `forge-core-<target>-sbom.cdx.json`

#### R-SCM.3 — Sigstore signing (cosign sign-blob)
- CI: `cosign sign-blob --yes <binary>` (keyless via OIDC)
- Output: `<binary>.sig` + `<binary>.pem` (certificate)
- Requer `id-token: write` permission no job

#### R-SCM.4 — Documentar verificação no README + release notes
- Seção "Verify supply chain integrity" no README
- Comandos: `cosign verify-blob`, comparação de SBOM

#### R-SCM.5 — Atualizar scores
- `excellence_roadmap.md`: Segurança supply chain 8→10
- Citar papers SLSA/sigstore em `contracts/research/`

---

### Epic F05 — Eval Compare single-agent baseline (harness) ✅ FECHADO

**Frente**: Features comunidade (fecha P1)
**Esforço**: médio **Risco**: médio **Impacto**: completa feature P1 ✅
**Pré-existente**: `forge-core-eval` (934 linhas, lib de comparação madura)
**Papers**: SWE-agent, OpenDev, CoAgent (harness engineering)
**Status**: COMPLETO (2026-07-01, commits `2d56f33a`→`e42b1609`). Nova crate
`forge-core-eval-harness` adiciona o executor (subprocess por arm), grader,
corpus loader e canonicalização sobre a lib existente. CLI `forge-core
eval-harness --config <yaml>`. Trace integration (3 novos TraceEventKind).
Fixtures + E2E. Ver `progress/f05_eval_harness_design.md`.

#### F05.1 — [grill + improve] Design do harness
- Pergunta central: o que EXECUTA os eval arms? Subprocess? In-process?
- Decisão recomendada: subprocess por arm (isolamento, mesma CLI que produção)
- `grill-with-docs`: sharpen "EvalArm", "EvalHarness", "EvalRunner"
- Atualizar `CONTEXT.md` se novos termos surgirem
- ADR se houver trade-off hard-to-reverse

#### F05.2 — Definir `EvalHarnessConfig` YAML schema
- Campos: `arms: [EvalArmSpec]`, `loader`, `tools`, `output_contract`,
  `usage_accounting`, `policy`
- Validator com diagnostics tipados (seguir padrão `forge-core-validate`)
- Fixture válida + inválida

#### F05.3 — Implementar arm executor
- Spawn subprocess por arm com args padronizados
- Coletar: accuracy, cost, latency, trajectory length, failures
- Mesmo loader/tools/output contract entre arms (controle)

#### F05.4 — Implementar report generator
- Reusar `compare_eval_runs` da lib existente
- Output JSON (agent-facing) + humano (CLI)

#### F05.5 — CLI `forge-core eval-compare --config <yaml>`
- Registro em `command_registry::COMMANDS`
- 2 edit points (padrão F15)

#### F05.6 — Trace integration
- `TraceEventKind::EvalCompareStarted/Passed/Failed`
- `trace_id` quando participa de run mutável

#### F05.7 — Fixtures + E2E tests
- Fixture válida (passa), inválida (falha fechado)
- E2E: run completo, report JSON estável, anchor preservada

---

### Epic F06 — Memory Policy (nova crate `forge-core-memory`)

**Frente**: Features comunidade (fecha P1)
**Esforço**: alto **Risco**: médio **Impacto**: completa feature P1
**Princípio chave**: nenhuma memória vira authority automaticamente;
promote exige policy + evidência raw.
**Papers**: RAC (retrieval-augmented), tau-bench (memory evaluation)

#### F06.1 — [grill + improve] Design do subsistema de memória
- Sharpen: "Memory" vs "Fact" vs "Preference" vs "Authority"
- Deletion test: Memory module é deep? (sim — admission/retention/promote
  são comportamentos não-triviais atrás de uma interface pequena)
- Authority boundary explícita: promote NUNCA automático
- ADR sobre admission policy (hard-to-reverse)

#### F06.2 — Definir schemas (`MemoryDocument`, `MemoryPolicy`)
- **Trust model**: ADR `docs/adr/0002-memory-trust-model.md` (status: **Accepted**,
  com addendum Candidato 1). Decisão: **dois eixos ortogonais** — authority (eixo 1,
  já existente) e review (eixo 2, novo, modelado como principal-attestation
  de F07). Promote de authority NÃO implica review; review NÃO implica
  promote de authority. Seis células de estado, todas expressáveis.
- `MemoryDocument` (implementado em `memory.rs` como `MemoryEntry`): id, kind,
  content, provenance (evidence_ref), freshness (ttl), confidence, approval
  (legacy), + 4 campos F06.2 aditivos com `#[serde(default)]`:
  `authority_level`, `review_state`, `reviewed_by` (`Option<StableId>`, **não**
  `PrincipalId` — corrigido no ADR), `reviewed_at`.
- `MemoryPolicy` (✅ **Candidato 1 feito**, ver addendum ADR 0002): struct
  tipada `{ permitted_kinds, required_evidence_fields, min_evidence_refs_for_authority }`.
  Gates `can_admit`/`can_promote` são **predicados puros** (PDP/PEP: Cedar, OPA,
  K8s validating webhooks, XACML) — decidem, não mutam. A mutação TOCTOU-safe é
  F06.3 (Candidato 2, crate `forge-core-memory`). `MemoryPolicy` **não tem
  `Default`** (uma default permissiva seria o bug AutoPromoted de outro nome).
- Validator com diagnostics tipados — deve rejeitar combinações ilegais
  (`Reviewed` sem `reviewed_by`/`reviewed_at`; `Authority` sem
  `evidence_refs` ou promote policy satisfeita; `reviewed_by` não
  autorizado pela GovernancePolicy) — **pendente** (F06.4/validator).

#### F06.3 — Criar crate `forge-core-memory` ✅ **DONE**
- ✅ `src/lib.rs`: tipos (`MemoryEvent`, `MemoryProjection`, `MemoryProjectionDiagnostic`),
  `replay`, `project`/`project_locked`, `next_sequence`, `now_unix`.
- ✅ `src/admission.rs`: PEP `admit` (lock → `can_admit` → append `Admitted` → projection).
- ✅ `src/retention.rs`: `list_now` (lazy TTL sweep on read) + `forget` (before-image).
- ✅ `src/promote.rs`: PEP `promote` (lock → find → `can_promote` → append `Promoted`).
- ✅ `src/error.rs`: per-op enums (`AdmitError`, `PromoteError`, `ForgetError`,
  `MemoryProjectionError`) mirroring `ClaimWal*Error`.
- ✅ ADR `docs/adr/0003-memory-pep-store.md` (Accepted) — composition over
  invention; reusa `forge-core-store` (`acquire_effect_store_lock`,
  `append_json_line_with_durability`, `WalDurability`); mirror `claim_wal.rs`
  (event-sourcing projection, torn-write recovery, per-op errors). CWE-367
  atomicidade no write site. 29 testes (27 unit + 2 proptest replay-determinism).
- **Decisão de scope**: o crate é o PEP puro; CLI (F06.7) e fixtures/E2E (F06.8)
  são stories separadas (a API pública retorna result-structs shaped para o
  `CliEnvelope`).

#### F06.4 — Admission gate ✅ **efetivamente DONE** (via Candidato 1 + F06.3)
- ✅ Policy check on ingest (`can_admit` PDP + `admit` PEP).
- ✅ Falha fechado se faltar evidence ou policy (`DeniedByGate`, nada appended).

#### F06.5 — Retention + forget ✅ **parcialmente DONE** (PEP existe; CLI pendente)
- ✅ TTL expiry (lazy sweep on read via `list_now` → `mark_stale`).
- ✅ Explicit forget (PEP `forget` com before-image append-only, auditable).
- ⏳ CLI `forget` verb (F06.7).

#### F06.6 — Promote (com evidence gate) ✅ **efetivamente DONE** (via Candidato 1 + F06.3)
- ✅ PEP `promote` com evidence gate (`can_promote` exige raw evidence).
- ✅ Requer evidence raw (não inferência) — `InsufficientEvidenceForAuthority`.
- ✅ Never auto-promotes (zero threshold still needs ≥1 ref — NFR).
- ⏳ CLI `promote` verb (F06.7).

#### F06.7 — CLI `forge-core memory ingest/list/forget/promote`
- Registro em `command_registry::COMMANDS`
- JSON output + humano

#### F06.8 — Fixtures + E2E tests ✅ **DONE** (fecha o epic F06)
- ✅ Fixtures em `contracts/examples/`: `memory-policy.yaml` + 4 entradas
  (`memory-entry-admitted.yaml`, `memory-entry-expired.yaml`,
  `memory-entry-promoted.yaml`, `memory-entry-rejected.yaml`) cobrindo os 4
  estados do spec (admitida, expirada, promoted, rejected sem evidence).
- ✅ E2E CLI (`crates/forge-core-cli/tests/memory_cli_e2e.rs`, 6 testes): o
  ciclo completo `ingest → list → promote → list → forget → list` via
  `assert_cmd` contra o binário real + fixtures permanentes. Afirma que
  autoridade muda (raw→authority), review fica em `unreviewed` (NFR
  ortogonalidade), denied-by-gate appenda nada, lazy-TTL sweep, review
  deferred, text mode, unknown-subcommand usage error.
- ✅ E2E PEP (`crates/forge-core-memory/tests/lifecycle.rs`, 8 testes):
  denied-appends-nothing, denial-carries-typed-reason, idempotent-forget,
  before-image-hash (tamper-evident), promote-leaves-review-untouched,
  promote-without-evidence-denied, sweep-idempotent-across-reads,
  projection-replays-deterministically (Fowler replay guarantee).
- ✅ Anchor 122 preservada (fix: fixtures com `evidence_ref` apontando para
  paths que existem no índice de known-repo-refs — `docs/adr/*` não é indexado).
- 🎉 **Epic F06 FECHADO.** F06.1-F06.8 todos ✅. Próximo: F07 (governance),
  que destrava o verb `memory review` (atualmente deferred).

---

### Epic F07 — Multi-principal governance

**Frente**: Features comunidade (fecha P1)
**Esforço**: alto **Risco**: médio **Impacto**: completa feature P1
**Princípio chave**: conflito entre principals vira objeto estruturado,
não merge silencioso.
**Papers**: communication-centric MAS (China-origin), multi-agent failure modes

#### F07.1 — [grill + improve] Design do governance ✅ **DONE**
- ✅ Sharpen via 3 frentes de pesquisa (RBAC/ReBAC/Cedar/Zanzibar; seam no
  codebase; R8 PrincipalId-vs-StableId). Modelo de **3 camadas** (GaaS-shaped):
  autorizacao (ReBAC/Cedar) + coordenacao (intent-locks de Gray) + conflito
  (objeto first-class, Git/Apel/Berenson lineage — NUNCA merge silencioso).
- ✅ Seam mapeado: deteccao no `claim_engine.rs:317` (acquire), NAO no WAL.
- ✅ ADR `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/ADR-0007-multi-principal-governance.md`
  expandido de stub 18-linhas → Accepted completo, supersede da previsao do
  ADR 0002 sobre PrincipalId.

#### F07.2 — Adicionar `PrincipalId` tipado aos contracts ✅ **DONE**
- ✅ `PrincipalId(pub String)` newtype em `common.rs`, `#[serde(transparent)]`,
  mesmos derives que `ScopeId`/`ClaimId` (R8: principal↔resource swap vira
  compile error). Custo de migracao zero (serde-transparent).
- ✅ `reviewed_by: Option<StableId>` → `Option<PrincipalId>` (one-concept-
  one-type; legacy YAML ainda parseia). Supersede formal da previsao do ADR 0002.
- ⏳ Migracao dos outros ~17 campos principal + 6 CLI `String` fields: tracked
  follow-up mecanico (fora desta story).

#### F07.3 — Definir `IntentContract` + `ConflictContract` schemas ✅ **DONE**
- ✅ `governance.rs`: `GovernancePolicy`, `IntentContract` (com `expires_at`
  load-bearing), `ConflictContract` (first-class, `ConflictResolutionState`,
  `ResolutionDecision`, `ConflictDetectionReason`), `IntentScope`/`IntentScopeKind`,
  `ConflictPolicy` (default `EmitContract`; `SilentLastWriterWins` = anti-pattern).
- ✅ Validator em `forge-core-validate`: `validate_intent_contract`,
  `validate_conflict_contract`, `validate_governance_policy` com diagnostics
  tipados (5 codigos F07). 6 testes.
- ✅ Fixtures em `contracts/examples/`: `governance-policy.yaml`,
  `intent-contract.yaml`, `conflict-contract.yaml` (round-trip testado).

#### F07.4 — Conflict detection no runtime
- Antes do WAL write: checar se ref é disputado entre principals
- Se conflito: NÃO faz merge silencioso, emite `ConflictContract`

#### F07.5 — Arbitration ledger (append-only)
- Log de todos os conflitos detectados + resoluções
- Queryable: `forge-core governance conflicts --status open`

#### F07.6 — CLI surface
- `forge-core governance intent/conflicts/arbitrate`
- JSON output + humano

#### F07.7 — Fixtures + E2E tests
- 2 principals disputando mesmo ref → ConflictContract emitido
- Resolução manual → arbitration ledger atualizado
- Anchor preservada

---

### Epic F08 — Secure MCP adapter (nova crate `forge-core-protocol-mcp`)

**Frente**: Features comunidade (fecha P1)
**Esforço**: alto **Risco**: médio-alto **Impacto**: completa feature P1
**Princípio chave**: nenhuma tool MCP muta estado sem OperationContract +
authority validada.
**Papers**: MCP spec (Anthropic), tau-bench (tool-call evaluation)

#### F08.1 — [grill + improve] Design do MCP server
- Sharpen: "MCPTool", "Allowlist", "Attestation", "MutateGate"
- Seam: MCP server é adapter sobre command_registry existente (deep)
- Deletion test: se remover, callers perdem acesso programático (ganha sua vida)
- ADR sobre attestation model (signed tool calls)

#### F08.2 — Criar crate `forge-core-protocol-mcp`
- Depende de `rmcp` (Rust MCP SDK) ou implementação manual JSON-RPC
- `src/server.rs`: MCP server (tools/list, tools/call)
- `src/allowlist.rs`: allowlist enforcement
- `src/attestation.rs`: signed tool calls

#### F08.3 — Implementar MCP server sobre command_registry
- Expor: preview, ready, graph, trace, memory, effect como MCP tools
- Cada tool = adapter sobre comando CLI existente

#### F08.4 — Allowlist enforcement
- Config: `mcp-allowlist.yaml` (quais tools expostas)
- Mutate tools (execute-operation, claim) requerem OperationContract

#### F08.5 — Attestation (signed tool calls)
- Cada `tools/call` deve carregar assinatura do caller
- Verify com chave autorizada

#### F08.6 — CLI `forge-core mcp serve [--allowlist <yaml>]`
- Registro em `command_registry::COMMANDS`
-stdio JSON-RPC (compatível com Claude Desktop, etc.)

#### F08.7 — Fixtures + E2E tests
- Allowlist deny (tool não listada) → rejeitado
- Mutate sem OperationContract → rejeitado
- Read-only tool sem attestation → permitido (política)
- Anchor preservada

---

### Epic F12 — Guided Start e Product UX (P2)

**Frente**: Features comunidade (P2)
**Esforço**: médio **Risco**: baixo **Impacto**: melhora UX onboarding
**Depende**: F05-F08 podem fornecer contexto

#### F12.1 — [grill] Design do fluxo guiado
- Sem YAML manual: wizard interativo ou scaffolding
- Pergunta: TUI? prompts no CLI? generated project?

#### F12.2-F12.5 — (definir após grill)

---

### Epic F14 — Knowledge Orchestration mode (P3)

**Frente**: Features comunidade (P3)
**Esforço**: alto **Risco**: médio **Impacto**: research agents
**Depende**: F08 (MCP), F06 (memory)

#### F14.1 — [grill] Design do evidence graph
- (deferido até F06/F08 completos)

---

### Epic R-FAST — Rápido 9→10

**Frente**: Rápido
**Esforço**: médio **Risco**: baixo **Impacto**: sobe de 9 para 10
**Pré-requisito**: R6 benchmarks já medidos (baseline existe)

#### R-FAST.1 — Profile hot paths
- Usar criterion baselines existentes (R6.1, R6.2, R6.3)
- Identificar top 3 otimização targets
- `cargo flamegraph` nos hot paths

#### R-FAST.2 — Implementar otimizações
- (definir após profile)
- Possíveis: pre-allocation, avoid clone, batch I/O

#### R-FAST.3 — Medir + atualizar baseline
- `docs/perf/baseline.md` atualizado
- Regression gate (R6.4) protege contra regressão

---

## Ordem de execução (dependências + valor + risco)

```
R-LINT ─────────────────────────────► [PRIMEIRO: low risk, formaliza 10/10] ✅ FECHADO
   │
   ▼
R-SCM ───────────────────────────────► [sobe Segurança 8→10] ✅ FECHADO
   │
   ▼
F05 (eval harness) ─────────────────► [lib existe, só harness] ✅ FECHADO
   │
   ▼
F06 (memory) ────────────────────────► [novo subsistema, foundation] ⏳ EM ANDAMENTO
   │
   ▼
F07 (governance) ────────────────────► [toca runtime/store]
   │
   ▼
F08 (MCP) ───────────────────────────► [usa F06/F07 + command_registry]
   │
   ▼
F12 (guided start) ─────────────────► [P2, UX]
   │
   ▼
F14 (knowledge orch) ────────────────► [P3, research]
   │
   ▼
R-FAST ──────────────────────────────► [último: profile + otimizar]
```

## Definition of Done — projeto 10/10

- [ ] Todas as 11 frentes com nota 10 (audit re-executado)
- [x] `cargo clippy --workspace --all-targets -- -D clippy::pedantic` verde (R-LINT completo, CI flipado)
- [x] SBOM anexada a cada release + sigstore signing (R-SCM completo, commit `060a5a9`)
- [x] **F05 ✅** operacional com fixtures + E2E; F06 em andamento (outro agente); F07/F08 pendentes
- [ ] Anchor `validate --json` preservada: 122 diagnostics 0
- [ ] Papers citados em `contracts/research/` (orientais + ocidentais)
- [ ] Sem script de novela (todas as policies paramétricas)
- [ ] Zero `anyhow`/`thiserror`/`Result<_, String>` novo

## Tracking

Cada story completado recebe commit com prefixo (`R-LINT.2`, `F05.3`, etc.)
e este arquivo + `excellence_roadmap.md` são atualizados.

# Follow-ups Roadmap — v0.1.0 → 10/10

**Data**: 2026-07-01
**Status**: plano ativo (criado após release v0.1.0 pública)
**Dono**: Daniel (codebase owner) + agente executor
**Norte estratégico**: rápido, robusto, performativo, protocolo-guia que escala
com a capacidade dos agentes, nunca script de novela, sempre Rust ou compatível,
sempre lastreado em melhores práticas e papers científicos (orientais e
ocidentais).

## Contexto

O v0.1.0 foi lançado publicamente (Apache-2.0, 5 binários cross-compilados,
CI verde, release em 2 remotes). Este documento cobre o trabalho restante
para chegar a 10/10 nas 4 frentes que ainda têm lacuna:

1. **Rápido 9→10** — otimizações pontuais de hot paths
2. **Features comunidade 9.7→10** — F05/F06/F07/F08/F12/F14
3. **Rust best practices** — formalizar CI com `-D warnings` (41 lints pendentes)
4. **Segurança supply chain 8→10** — SBOM + sigstore

As outras 7 frentes já estão em 10/10.

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

### Epic R-LINT — Lint cleanup (41 pedantic → 0, CI `-D warnings`)

**Frente**: Rust best practices (formalizar 10/10 com CI deny)
**Esforço**: médio **Risco**: baixo **Impacto**: formaliza 10/10

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

### Epic R-SCM — Supply chain hardening (SBOM + sigstore)

**Frente**: Segurança supply chain 8→10
**Esforço**: médio **Risco**: baixo **Impacto**: sobe de 8 para 10
**Papers**: SLSA, sigstore (CT log transparency)

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

### Epic F05 — Eval Compare single-agent baseline (harness)

**Frente**: Features comunidade (fecha P1)
**Esforço**: médio **Risco**: médio **Impacto**: completa feature P1
**Pré-existente**: `forge-core-eval` (934 linhas, lib de comparação madura)
**Papers**: SWE-agent, OpenDev, CoAgent (harness engineering)

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
- `MemoryDocument`: id, principal, kind, content, evidence_refs,
  admitted_at, ttl, authority_level (sempre `Raw` até promote explícito)
- `MemoryPolicy`: admission rules, retention rules, promote rules
- Validator com diagnostics tipados

#### F06.3 — Criar crate `forge-core-memory`
- `src/lib.rs`: tipos + validator
- `src/admission.rs`: admission gate
- `src/retention.rs`: TTL + explicit forget
- `src/promote.rs`: promote com evidence gate
- Hand-rolled error enums (`MemoryAdmissionError`, etc.)

#### F06.4 — Admission gate
- Policy check on ingest: tipo permitido? evidência presente?
- Falha fechado se faltar evidence ou policy

#### F06.5 — Retention + forget
- TTL expiry (lazy sweep on read)
- Explicit `forget --memory-id <id>`
- Append-only log de forgets (auditable)

#### F06.6 — Promote (com evidence gate)
- `promote --memory-id <id> --evidence <ref>`
- Requer evidence raw (não inferência)
- Authority level: `Raw` → `Provisional` (nunca `Authority` automático)

#### F06.7 — CLI `forge-core memory ingest/list/forget/promote`
- Registro em `command_registry::COMMANDS`
- JSON output + humano

#### F06.8 — Fixtures + E2E tests
- Fixture: memória admitida, expirada, promoted, rejected (sem evidence)
- E2E: ingest → list → promote → list (autoridade muda)
- Anchor preservada

---

### Epic F07 — Multi-principal governance

**Frente**: Features comunidade (fecha P1)
**Esforço**: alto **Risco**: médio **Impacto**: completa feature P1
**Princípio chave**: conflito entre principals vira objeto estruturado,
não merge silencioso.
**Papers**: communication-centric MAS (China-origin), multi-agent failure modes

#### F07.1 — [grill + improve] Design do governance
- Sharpen: "Principal" (humano OU agente), "IntentContract",
  "ConflictContract", "GovernancePolicy", "ArbitrationLedger"
- Seam: onde conflito é detectado? (runtime, antes do WAL write)
- ADR sobre arbitration model (manual vs auto-resolve)

#### F07.2 — Adicionar `PrincipalId` tipado aos contracts
- Hoje: strings implícitas. Depois: `PrincipalId` (typed, validated)
- Migration: additive (vazio = legacy single-principal)

#### F07.3 — Definir `IntentContract` + `ConflictContract` schemas
- `IntentContract`: principal, goal, authority_scope, expires_at
- `ConflictContract`: principals em conflito, ref disputado, detection_reason
- Validator com diagnostics tipados

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
R-LINT ─────────────────────────────► [PRIMEIRO: low risk, formaliza 10/10]
   │
   ▼
R-SCM ───────────────────────────────► [sobe Segurança 8→10]
   │
   ▼
F05 (eval harness) ─────────────────► [lib existe, só harness]
   │
   ▼
F06 (memory) ────────────────────────► [novo subsistema, foundation]
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
- [ ] `cargo clippy --workspace --all-targets -- -D clippy::pedantic` verde
- [ ] SBOM anexada a cada release + sigstore signing
- [ ] F05/F06/F07/F08 operacionais com fixtures + E2E tests
- [ ] Anchor `validate --json` preservada: 122 diagnostics 0
- [ ] Papers citados em `contracts/research/` (orientais + ocidentais)
- [ ] Sem script de novela (todas as policies paramétricas)
- [ ] Zero `anyhow`/`thiserror`/`Result<_, String>` novo

## Tracking

Cada story completado recebe commit com prefixo (`R-LINT.2`, `F05.3`, etc.)
e este arquivo + `excellence_roadmap.md` são atualizados.

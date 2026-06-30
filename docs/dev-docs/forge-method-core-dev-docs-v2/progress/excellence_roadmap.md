# Excellence Roadmap — Forge Method Core até 10/10

**Data**: 2026-06-30
**Status**: plano ativo (última atualização: 2026-06-30 — E1 completo; auditoria F04/F01
feita, breakdown em sub-tasks adicionado)
**Dono**: Daniel (codebase owner) + agente executor
**Norte estratégico**: rápido, robusto, performativo, protocolo-guia que escala com a
capacidade dos agentes, nunca script de novela, sempre Rust ou compatível, sempre
lastreado em melhores práticas e papers científicos (orientais e ocidentais).

## Score atual por frente (audit 2026-06-30)

| Frente | Hoje | Meta | Lacuna principal |
|---|---|---|---|
| Rápido | 5 | 10 | Sem benchmarks mensuráveis |
| Robusto | 9 | 10 | Tracing completo; zero Result<_,String>; sem zeroize ainda |
| Performativo | 4 | 10 | Sem criterion, sem profile, sem hot-path baselines |
| Protocolo guia | 9 | 10 | F04 validate+run --dry-run completos; F01 bugs críticos fechados |
| Workflows | 7 | 10 | WAL/claim ok, mas F11/F13 não existem |
| Agente guia humano | 9 | 10 | F01 bugs de integridade fechados; rollback_available real |
| Não-script-de-novela | 9 | 10 | Já é framework paramétrico; faltam fixtures que provem |
| Features comunidade | 8 | 10 | F03/F04 operacionais; F01 plumbing+semântica completos; falta F02/F15 |
| Rust best practices | 9 | 10 | clippy pedantic em 0 warnings (comecou ~245); E1 fechado |
| Segurança supply chain | 6 | 10 | serde_yaml deprecated (R7), sem zeroize (R5), sem fuzz (R4) |
| Docs/rastreabilidade | 6 | 10 | Bootstrap Exception pendente; papers sem status doc |

## Princípios (não negociáveis)

1. **Sem script de novela**: todo contrato/policy/workflow deve ser paramétrico (matriz
   de decisão), nunca sequência prescritiva. Spans de auditoria com `#[instrument]`
   comprovam framework.
2. **Sem `anyhow`/`thiserror`**: enums hand-rolled, derive `Debug, Clone, PartialEq, Eq`.
3. **Sem `Result<_, String>` novo**: migrar os ~10 sites residuais em store/crypto.
4. **Workspace deps compartilhadas**: nada de pin por crate divergente.
5. **Commits pequenos e freqüentes**: 1 preocupação por commit, cada commit verde.
6. **Anchor preservado**: `validate --root . --json` deve emitir 122 `"diagnostics": 0`.
7. **Papers**: cada feature grande cita paper(s) em `contracts/research/` com
   `relevance:` no backlog. Orientais (China/Korea/Japan) e ocidentais representados.
8. **Rust-first**: tudo em Rust ou compatível. Sem Python no runtime. Sem JS no runtime.

## TODO list — ordenado por dependência e impacto

### Trilha A — Sistema técnico (R-tracks do 09_system_design_roadmap.md)

- [x] **R1 estendido + R12** — Decomposição god-file + testes desacoplados
- [x] **R10** — Criar `forge-core-crypto`, mover cripto da CLI
- [x] **R11** — Decompor `main.rs` (125 linhas agora)
- [x] **R8** — Remover 146 `process::exit` do lib code
- [x] **R2** — Migrar `Result<_, String>` residuais em store/crypto (~10 sites)
      - **COMPLETO** (commit `0846c79`): zero `Result<_, String>` em `crates/*/src/`
      - Migrated: `cli_util::resolve_stateful_command_roots`,
        `catalog::load_one`/`parse_workflow_yaml`,
        `eval::load_eval_corpus`, `isolation::shell_metachar_check`
      - Typed enums: `StatefulRootsError`, `CatalogLoadError`,
        `EvalCorpusLoadError`; `shell_metachar_check` vira
        `Option<&'static str>` (callers só passam reason pra IsolationError)
- [x] **R3** — Tracing estruturado (COMPLETO)
      - [x] R3.1 deps + subscriber init + flag `FORGE_LOG_FORMAT`
      - [x] R3.2 spans em validate/store/runtime/crypto/cli
      - [x] R3.3 correlação multi-agente via `agent_id`
      - [x] R3.4 analisado: todos `eprintln!` em `_cmd.rs` são
            user-facing contract output, não logging. No-op legítimo.
- [ ] **R4** — Fuzz harness (`cargo-fuzz`)
      - Alvos: `parse_rekor_log_entry`, `parse_signed_checkpoint`,
        `claim_wal_decode`, `ocsp_response_decode`
      - DoD: `cargo fuzz run <target> -- -max_total_time=60` sem panic
- [x] **R6.1** — Benchmarks (`criterion`) store hot paths ✅
      - `claim_wal.rs`: append 1/100/1000 entries (32ms / 37ms / 41ms)
      - `claim_wal.rs`: replay 1/100/1000 (157µs / 719µs / 7.2ms)
      - `reference_index.rs`: workspace (~1.5ms) + minimal (~205µs)
      - Achado: fsync Windows é durability-bound (25–50ms), não bug
      - Ver `progress/r6_benchmarks.md`
- [ ] **R6.2** — Benchmarks crypto hot paths
      - `verify_rekor_checkpoint`, `verify_merkle_inclusion`, parse+verify combinados
      - Em `crates/forge-core-crypto/benches/`
- [ ] **R6.3** — Benchmarks `serde_yaml::from_str` vs `serde_yml::from_str` (pós-R7)
- [ ] **R6.4** — CI: bench em PR com label `perf` compara com main
- [ ] **R5** — `zeroize` em material cripto
      - Inventariar `VerifyingKey`, `ed25519_dalek::VerifyingKey`, sig brutas, nonces
        OCSP, payload pré-hash
      - Wrap em `Zeroizing<Vec<u8>>` ou `Zeroizing<Box<[u8]>>`
      - Constant-time compare via `subtle::ConstantTimeEq`
- [x] **R7** — `serde_yaml` → `yaml_serde` ✅
      - Descoberta: `serde_yml` também está deprecated (shim)
      - Migrado para `yaml_serde 0.10.4` (The YAML Organization, API 1:1)
      - 124 refs em 42 arquivos, anchor 122 preservado
      - Ver `progress/r7_yaml_serde.md`
- [ ] **R13** — Alinhar docs com realidade
      - `04_rust_refactor_guide.md`: remover `thiserror`/`clap` menções
      - Auditar todos dev-docs por conflito com `AGENTS.md`
- [ ] **R14** — Criar `paper_implementation_status.md`
      - Cada paper em `contracts/research/` mapeado pra implementação
- [ ] **R9** — Remover Bootstrap Exception
      - Quaisquer docs/contratos que assumem "humano lê" precisam migrar pra
        "agente lê e explica"

### Trilha B — Features P0 da comunidade

- [x] **F03** — TraceEvent canonico + `forge explain` ✅
      - Commit `7c96a37` (2026-06-30)
      - `forge explain --last-run | --run-id <id>` narra cronologicamente
      - Narrativa: header (run/trace/agent) + eventos ordenados por `recorded_at`
        + totais (events, outputs, model_calls, tool_calls, tokens) + peak risk
      - 10 testes cobrem: ordem cronológica, agregação, peak risk, parser,
        mutuamente exclusivo, empty events, non-matched
      - Helpers quebrados (`narrate_header`, `narrate_event`, `narrate_summary`,
        `narrate_non_matched`) — clippy pedantic limpo no trabalho novo
- [ ] **F04** — WorkflowGraph v0 (PARCIAL — plumbing completo, gaps semânticos)
      - **Achado da auditoria 2026-06-30:** ambos subcomandos já executam end-to-end.
        `forge graph validate` (4 passes: identity, nodes, edges, cycles via Kahn).
        `forge graph run --dry-run` (topological order + per-node preview +
        claim preflight + blocked_by upstream verifiers). `forge graph run`
        sem `--dry-run` é rejeitado por design (ainda não há executor real).
      - DoD original: `forge graph validate` + `forge graph run --dry-run`
        funcionam — **substantivamente atendido**
      - Gaps remanescentes (commits pequenos):
        - [x] **F04.1** Per-node `touched_refs` no dry-run output
              (commit `1c9a7dd`)
        - [x] **F04.2** Validar referências secundárias: `verifies`,
              `GraphBudget.node_id` (commit `e9eb579`)
        - [x] **F04.3** Edge-kind semantics documentadas + warning para
              `blocks_until_passed` de non-Verifier (commit `58ef7d8`)
        - [ ] **F04.4** Tests E2E: `validate` (Passed + Blocked + cycle),
              `run --dry-run` (Planned + Blocked + Invalid) via CLI
              (`run_validate`, `run_dry_run`). Lib-level tests já cobrem o
              behavior (10/10 em `crates/forge-core-graph/tests/`).
      - Depende: F03 (tracing) pra narrar execução do grafo
- [ ] **F01** — `forge preview` (plumbing completo, gaps semânticos)
      - **Achado da auditoria 2026-06-30:** CLI plumbing é completo. JSON
        output já carrega todos os 6 campos DoD (`status`, `touched_refs`,
        `risk_level`, `required_gate_refs`/`gate_contract_refs`,
        `rollback_available`, `next_human_action`). Bugs reais no runtime:
      - **Bug crítico de integridade:** `preview_status` IGNORA `blockers` →
        um plano Ready com blockers reporta `status: Ready` enquanto
        `risk_level: Blocked`. Os dois sinais discordam. `next_human_action`
        retorna `None` pra Ready, então o humano não é orientado.
      - Gaps remanescentes (commits pequenos):
        - [x] **F01.1** Fix bug: `preview_status` consulta `blockers` e desqualifica
              Ready→Blocked quando non-empty (commit `4330d31`)
        - [x] **F01.2** Fix bug: `ready_gate_blockers` consulta `_required_gates`:
              Pass + required gates não-verificados → `RequiredGateStatusUnknown`
              (commit `f4158d9`)
        - [x] **F01.3** Implementar `rollback_available` de verdade:
              `compute_rollback_available` helper + `preview_operation_with_effect_documents`
              (commit `64ac22d`)
        - [x] **F01.4** Union `touched_refs`: `collect_effect_touched_refs` helper
              faz union de `CoordinationScope.target.paths` + write-sets dos
              `ToolEffectContractDocument`. Path plan-only mantém `Vec::new()`
              (commit `ef163d3`)
        - [x] **F01.5** Garantir `next_human_action` sempre `Some` quando bloqueado
              (coberto por F01.1: Ready+blockers retorna "resolve blockers")
        - [x] **F01.6** Tests: `ReviewRequired`, `ReadOnly`, `Publish` (High
              risk) — todos cobertos agora (commit `a3cfce6`)
        - [ ] **F01.7** (opcional, deferido) Separar `PreviewJsonPayload` do envelope
              CLI (hoje mistura com `project_root`, `state_root`, `trace_id`)
      - É o coração do "agente guia humano"
      - Depende: F03 (tracing explica decisão), F04 (graph preview)
- [ ] **F02** — `forge ready`
      - Gate unificado: tests + lint + typecheck + evals + security
      - DoD: run só passa se todos gates obrigatórios passam; failures tipadas
- [ ] **F15** — Rust ergonomics + codegen track (PARCIAL)
      - Reduzir sofrimento do agente escrevendo Rust repetitivo
      - Snapshot tests, fixtures, module split, codegen de contratos
      - Critério: novo comando/contrato não exige editar >2 pontos fora de tests/docs

### Trilha C — Features P1 da comunidade

- [ ] **F05** — Eval Compare single-agent baseline (PARCIAL)
      - `forge-core-eval` existe (934 linhas); falta harness comparativo
      - DoD: relatório com accuracy, cost, latency, trajectory, failures, delta
- [ ] **F06** — Memory Policy
      - Admission, retention, forget, promote, raw evidence, authority boundary
      - Nenhuma memória vira authority automaticamente
- [ ] **F07** — Multi-principal governance (PARCIAL)
      - `PrincipalId`, `IntentContract`, `ConflictContract`, `GovernancePolicy`
      - Conflito entre principals vira objeto estruturado, não merge silencioso
- [ ] **F08** — Secure MCP adapter
      - MCP server para preview/ready/graph/trace/memory/effect
      - Allowlist + attestation; nenhuma tool muta sem OperationContract
- [ ] **F11** — Risk Audit Gate
      - Checks determinísticos + extensão SAST/linters
      - Falha fechado em padrões proibidos (fail-soft, exception swallowing)

### Trilha D — Features P2/P3 da comunidade

- [ ] **F09** — Secure A2A adapter (agent-to-agent cross-vendor)
- [ ] **F10** — Control Plane local (TUI ou HTML estático lendo `.forge-method`)
- [ ] **F12** — Guided Start + Product UX (fluxo guiado sem YAML manual)
- [ ] **F13** — Budget and Cost Accounting (per run/graph/agent/principal/tool)
- [ ] **F14** — Knowledge Orchestration mode (research agents com evidence graph)

### Trilha E — Rust best practices

- [x] **E1** — Zerar warnings clippy pedantic (baseline ~245, **agora 0**)
- Estrategia hibrida: fechar lints mecanicos + escrever `# Errors`/`# Panics`
  em todo workspace; `#![allow]` documentado so pro cosmético que sobrar
- [x] E1.1 large_enum_variant em `ProjectInitError` (commit `a05188d`)
- [x] E1.1b `pub(crate) ClaimReconcileLoopConfig` p/ `private_interfaces`
      (commit `669ea28`)
- [x] E1.2 assigning_clones em crypto via `clone_from` (commit `2a56d16`,
      21 warnings)
- [x] E1.3 docs `# Errors`/`# Panics` em todo workspace (132 warnings → 0)
- [x] E1.4 lints mecanicos (~20 warnings): manual_let_else, match_same_arms,
      redundant_continue, redundant_guard, doc_markdown backticks
- [x] E1.5 needless_pass_by_value: allow documentado onde breaking-change,
      corrigido onde seguro
- [x] E1.6 struct_excessive_bools: allow documentado nos 6 policy checklists
- [x] E1.7 `#![allow]` documentado em crate roots para too_many_lines,
      redundant_closure_for_method_calls
- Resultado final: 0 warnings em `cargo clippy --workspace --lib
  -- -W clippy::pedantic` (comecou com ~245)
- [x] **E2** — Profile release documentado em `Cargo.toml` (commit `984618b`)
      - LTO thin, codegen-units 1, panic abort, opt-level 3, strip symbols
      - DoD: `cargo build --release` produz binário otimizado
- [x] **E3** — CI: gates automáticos em PR (commit `d3af5c4`)
      - `cargo check --workspace`, `cargo clippy --pedantic -D warnings`,
        `cargo test`, `cargo fmt --check`, anchor `validate --json | grep -c 122`

### Trilha F — Papers e evidência científica

- [ ] **F-sci** — Para cada feature P0/P1, citar paper em `contracts/research/`
      com `relevance:` (orientais e ocidentais)
      - SLSA, sigstore, merkle (rekor), OCSP, RFC3161, saga pattern, autonomy
      - Papers chineses: _CoAgent_, _OpenDev_, _Code-as-Agent Harness_
      - Papers ocidentais: _SWE-agent_, _RAC_, _Microservices Saga_
      - DoD: `docs/.../paper_implementation_status.md` lista todos

### Trilha G — System design (não-R-track)

- [ ] **G1** — Auditar todos os `contracts/policies/*.yaml` por "script de novela"
      - Critério: cada policy deve ser matriz paramétrica (modes + thresholds),
        não sequência prescritiva
      - DoD: report diz "X/57 policies são framework, Y são script"
- [ ] **G2** — Fixtures que provam framework (não script)
      - Para cada policy, fixture testando múltiplos inputs no mesmo policy
      - DoD: `cargo test policies_framework` passa
- [ ] **G3** — Runtimeização do autonomy router
      - Hoje é estático (ler YAML, decidir). Capacidade de mudar thresholds em
        runtime sem re-deploy
      - DoD: `forge autonomy set-threshold <class> <value>` funciona

## Ordem de execução (dependências + valor)

```
Fase 3 (R3) ─────────────────────► [EM ANDAMENTO]
   │
   ├── R3.3 agent_id            (próximo)
   ├── R3.4 eprintln migration
   │
   ▼
Fase 4 (R6.1 ✅ + R4 + R6.2) ─────► [R6.1 DONE; R4 próximo]
   │  benchmarks + fuzz
   │  (valida "rápido" e "performativo")
   ▼
Fase 5 (R7 + R5) ────────────────► supply chain + segurança
   │
   ▼
Fase 2 conclusão (R2) ───────────► disciplina de erro completa
   │
   ▼
F03 forge explain ───────────────► fecha feature P0 parcial
   │
   ▼
F04 forge graph run ─────────────► fecha feature P0 parcial
   │
   ▼
F01 forge preview ───────────────► coração do "agente guia humano"
   │
   ▼
F02 forge ready ─────────────────► workflow de bom trabalho
   │
   ▼
F15 + F11 ───────────────────────► ergonomics + risk gate
   │
   ▼
F05 + F06 + F07 ─────────────────► eval + memory + governance
   │
   ▼
F08 + F09 + F10 + F12 + F13 + F14 ► ecossistema
   │
   ▼
Fase 6 (R13 + R14 + R9) ─────────► docs + rastreabilidade finais
```

## Definition of Done — projeto 10/10

- [ ] Todas as 11 frentes com nota 9 ou 10 (audit re-executado)
- [ ] `cargo bench` roda sem erro, hot paths medidos
- [ ] `cargo fuzz run` em cada target por ≥1 min sem panic
- [ ] `cargo clippy --workspace --all-targets -- -W clippy::pedantic` com
      <100 warnings (baseline ~436)
- [ ] Zero `process::exit` em lib code (mantém R8)
- [ ] Zero `Result<_, String>` novo (R2)
- [ ] Zero `serde_yaml` (R7)
- [ ] Zero material cripto sem `Zeroizing<>` (R5)
- [ ] Anchor `validate --json` preservado: 122 `"diagnostics": 0`
- [ ] F01/F02/F03/F04/F15 (P0) todos operacionais com fixtures
- [ ] `forge preview` mostra plano+gates+rollback antes de mutar
- [ ] `forge ready` unifica todos gates
- [ ] `forge explain <run_id>` narra cronologicamente
- [ ] Multi-agent: `agent_id` aparece em todo span quando setado
- [ ] Sem script de novela: G1 + G2 fechados
- [ ] Papers: F-sci completo, orientais + ocidentais representados

## Tracking

Cada item completado recebe commit com prefixo `R3.3`, `F03.1`, etc. e este
arquivo é atualizado com checkbox marcado.

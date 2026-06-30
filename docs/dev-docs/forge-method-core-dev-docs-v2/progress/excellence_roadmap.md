# Excellence Roadmap — Forge Method Core até 10/10

**Data**: 2026-06-30
**Status**: plano ativo
**Dono**: Daniel (codebase owner) + agente executor
**Norte estratégico**: rápido, robusto, performativo, protocolo-guia que escala com a
capacidade dos agentes, nunca script de novela, sempre Rust ou compatível, sempre
lastreado em melhores práticas e papers científicos (orientais e ocidentais).

## Score atual por frente (audit 2026-06-30)

| Frente | Hoje | Meta | Lacuna principal |
|---|---|---|---|
| Rápido | 5 | 10 | Sem benchmarks mensuráveis |
| Robusto | 8 | 10 | Tracing parcial, `Result<_,String>` residuais, sem zeroize |
| Performativo | 4 | 10 | Sem criterion, sem profile, sem hot-path baselines |
| Protocolo guia | 8 | 10 | Catálogo real, mas preview/ready não operacional |
| Workflows | 7 | 10 | WAL/claim ok, mas F11/F13 não existem |
| Agente guia humano | 8 | 10 | Política tipada, mas UX de "preview→human" ausente |
| Não-script-de-novela | 9 | 10 | Já é framework paramétrico; faltam fixtures que provem |
| Features comunidade | 6 | 10 | F03/F04/F05 parciais; F01/F02/F15 P0 não iniciados |
| Rust best practices | 7 | 10 | clippy pedantic com ~436 warnings; sem thiserror/clap (bom) |
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
- [ ] **R2** — Migrar `Result<_, String>` residuais em store/crypto (~10 sites)
      - `parse_rekor_log_entry`, `required_string`, etc. em forge-core-crypto/rekor.rs
      - `EffectStoreLockError` variants com String → enum tipado
      - Status: ~50% feito; empacotar por arquivo
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
- [ ] **R7** — `serde_yaml` → `serde_yml` (deprecated)
      - Inventariar usos, trocar dep, fuzz + bench validam equivalência
- [ ] **R13** — Alinhar docs com realidade
      - `04_rust_refactor_guide.md`: remover `thiserror`/`clap` menções
      - Auditar todos dev-docs por conflito com `AGENTS.md`
- [ ] **R14** — Criar `paper_implementation_status.md`
      - Cada paper em `contracts/research/` mapeado pra implementação
- [ ] **R9** — Remover Bootstrap Exception
      - Quaisquer docs/contratos que assumem "humano lê" precisam migrar pra
        "agente lê e explica"

### Trilha B — Features P0 da comunidade

- [ ] **F03** — TraceEvent canonico + `forge explain` (PARCIAL)
      - `forge-core-trace` existe (287 linhas), mas `forge explain` não é comando
      - DoD: `forge explain <run_id>` lê NDJSON e narra cronologicamente
      - Depende: R3.3 (`agent_id`) pra filtrar por agente
- [ ] **F04** — WorkflowGraph v0 (PARCIAL)
      - `forge-core-graph` existe (1014 linhas), mas `forge graph run` não executa
      - DoD: `forge graph validate` + `forge graph run --dry-run` funcionam
      - Depende: F03 (tracing) pra narrar execução do grafo
- [ ] **F01** — `forge preview`
      - Operação mutável → preview JSON determinístico com `status`, `touched_refs`,
        `risk`, `gates`, `rollback_available`, `next_human_action`
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

- [ ] **E1** — Zerar warnings clippy pedantic (~436 hoje)
      - Não corrigir warnings pré-existentes que não sejam do trabalho atual
      - Critério: cada commit novo não adiciona warnings novos
      - Meta: baseline cai pra <100 ao fim de R2/R3/R5/R7
- [ ] **E2** — Profile release documentado em `Cargo.toml`
      - LTO thin, codegen-units 1, panic abort, opt-level 3
      - DoD: `cargo build --release` produz binário otimizado
- [ ] **E3** — CI: gates automáticos em PR
      - `cargo check --workspace`, `cargo clippy --pedantic`, `cargo test`,
        `cargo fmt --check`, anchor `validate --json | grep -c 122`

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

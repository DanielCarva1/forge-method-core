# Excellence Roadmap — Forge Method Core até 10/10

**Data**: 2026-06-30
**Status**: plano ativo (última atualização: 2026-06-30 — R5+R4 inventariados e
quebrados em sub-tasks; E1/E2/E3/R2 completos; F04/F01/F02 completos)
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
| Features comunidade | 9 | 10 | F03/F04/F01 operacionais; F02 preflight implementado; falta F15/F05-F14 |
| Rust best practices | 9 | 10 | clippy pedantic em 0 warnings (comecou ~245); E1 fechado |
| Segurança supply chain | 6 | 10 | serde_yaml já migrado (R7); sem zeroize ainda (R5), sem fuzz (R4) |
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
- [ ] **R4** — Fuzz harness (`cargo-fuzz`) — inventariado 2026-06-30
      - Inventário completo em `progress/r4_fuzz_inventory.md`
      - Alvos reais confirmados (nomes do roadmap eram aproximados):
        `parse_rekor_log_entry` (rekor.rs:114), `parse_signed_checkpoint`
        (rekor.rs:263), `decode_ocsp_response` (ocsp.rs:30, era
        `ocsp_response_decode`), `decode_prefix` (claim_wal.rs:1818, era
        `claim_wal_decode`)
      - Todos os 4 alvos são `pub(crate)` ou privado — exige feature `fuzz`
        expondo via `#[cfg(feature = "fuzz")] pub use`
      - Seed corpus: gerado a partir de `crates/forge-core-cli/tests/validate.rs`
        e `crates/forge-core-store/tests/claim_wal.rs` (não há fixtures estáticos)
      - Toolchain OK (rustc 1.94 stable); `cargo-fuzz` e `fuzz/` ainda ausentes
      - Sub-tasks:
      - [ ] R4.1 Setup: `cargo install cargo-fuzz` + `cargo fuzz init` + feature
            `fuzz` em `forge-core-crypto`/`forge-core-store` expondo os 4 alvos
      - [ ] R4.2 Harness `parse_signed_checkpoint` (mais isolado, valida infra)
      - [ ] R4.3 Harness `parse_rekor_log_entry` (JSON+base64 duplo)
      - [ ] R4.4 Harness `decode_ocsp_response` (DER/ASN.1 via rasn)
      - [ ] R4.5 Harness `decode_prefix` (WAL binário com CRC32C)
      - [ ] R4.6 DoD: `cargo fuzz run <target> -- -max_total_time=60` sem panic
            em cada um dos 4 alvos
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
- [ ] **R5** — `zeroize` em material cripto — inventariado 2026-06-30
      - Inventário completo em `progress/r5_crypto_inventory.md`
      - Estado deps: `zeroize`/`subtle` ausentes do workspace (mas já transitivos
        via curve25519-dalek/elliptic-curve). `ed25519-dalek` e `p256` precisam
        da feature `zeroize` habilitada explicitamente.
      - Prioridades (alta → baixa):
        1. Nonces OCSP `expected_nonce_hex`/`observed_nonce_hex` (único segredo
           de cliente do fluxo, fields de `HostAdapterCertificateOcspStatusVerification`)
        2. `signature_bytes`, `bundle.signature`, `sct_bytes`, `ocsp_der`
        3. `public_key_bytes`, `rekor_key: P256VerifyingKey`, DERs de cert
        4. Prehash payloads (baixa — conteúdo público)
      - Comparações em tempo constant (R5.5): `rekor.rs:358` (Merkle root),
        `ocsp.rs:327` (nonce OCSP), `ocsp.rs:185` (serial — bug de corretude
        também, não só timing)
      - Tipos third-party sem `Zeroize`: `rasn_ocsp::{OcspResponse,
        BasicOcspResponse}`, `asn1_rs::BitString` — mitigação: descartar cedo
        e copiar campos sensíveis para `Zeroizing<>` no caller
      - Sub-tasks em 3 fases:
      - FASE A (não-breaking):
      - [ ] R5.1 Workspace deps: adicionar `zeroize = { version = "1.9",
            features = ["derive"] }`, `subtle = "2.6"`; habilitar feature
            `zeroize` em `ed25519-dalek` e `p256`
      - [ ] R5.2 Wrap locals em `rekor.rs` (prehash/signature opcional,
            preparar `ParsedCheckpoint.signatures` field type sem quebrar API)
      - [ ] R5.3 Wrap locals em `ocsp.rs` (`signature_der`, `sha1_digest`,
            `ocsp_digest_for_algorithm` retornos → `Zeroizing<Vec<u8>>`)
      - [ ] R5.4 Wrap locals em `host_adapter_verification.rs`
            (`signature_bytes`, `public_key_bytes`, `bundle_bytes`,
            `sct_bytes`, `ocsp_der`)
      - [ ] R5.5 Constant-time compares em `rekor.rs:358`, `ocsp.rs:327`,
            `ocsp.rs:185` (decodificar hex/decimal → bytes → `ConstantTimeEq`)
      - FASE B (`pub(crate)` breaking, sem bump externo):
      - [ ] R5.6 `ParsedCheckpoint.signatures` → `Vec<Zeroizing<Vec<u8>>>`;
            `read_certificate_der` em sigstore.rs → `Option<Zeroizing<Vec<u8>>>`;
            `CertificateTransparencyLogMaterial` deriva `Zeroize, ZeroizeOnDrop`
      - FASE C (API pública breaking, requer bump minor — pre-1.0 OK):
      - [ ] R5.7 `file_io::read_signature_file`/`read_public_key_file`/
            `read_required_file` → `Option<Zeroizing<Vec<u8>>>` (re-exportidos
            em lib.rs:71, callers: forge-core-cli, tests)
      - [ ] R5.8 `HostAdapterCertificateOcspStatusVerification` fields
            `expected_nonce_hex`/`observed_nonce_hex` → `Option<Zeroizing<String>>`
            (exige impl Serialize manual ou wrapper com serde passthrough)
      - [ ] R5.9 Sanity test: zeroize drop chamado (verificar ZeroizeOnDrop
            acionado via drop semantics)
      - FOLLOW-UP (inventariar antes de expandir):
      - [ ] R5.10 Inventariar `sigstore.rs` (`ParsedBundle.signature`,
            `verify_ed25519_signature` internals, `parse_certificate`)
      - [ ] R5.11 Inventariar `file_io.rs`/`hashing.rs`/`slsa_transparency.rs`/
            `tuf.rs`
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
- [x] **F02** — `forge preflight` (commit `986536d`)
      - Gate unificado: cargo check / fmt / clippy pedantic / test / validate /
        regression anchor — todos com status tipado + duração + log tail
      - JSON output estável, accumulating (não pula gates), fail-soft
        (optional gates falhando vira `Degraded`, exit 0)
      - `--gate <name>...` permite rodar subset; `--expected-anchor` configura
        o count esperado (default 122)
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

# Excellence Roadmap — Forge Method Core até 10/10

**Data**: 2026-07-01
**Status**: plano ativo (última atualização: 2026-07-01 — **R-LINT ✅** completo (41 pedantic → 0, CI flipado para `-D clippy::pedantic`, commits `247107a`/`f95cd54`/`e1439c6`); **R-SCM ✅** completo (sigstore keyless + CycloneDX SBOM no `release.yml`, commit `060a5a9`); **F05 ✅** fechado (eval harness F05.1-F05.7: design, schema, executor, grader, CLI, trace, E2E, commits `2d56f33a`→`e42b1609`); R9/G1/G2 fechados;
**F15 fechado** (commits `7d0934b`→`7474139`); R4 completo via CI Linux;
E1/E2/E3/R2 completos; F04/F01/F02/F03 completos; R5.1-R5.9 completos; R6.3 benches;
**R6.4 ✅** regression gate ativo no CI (cache criterion baseline + awk parser, fail-on-alert >15%, commit `84c730e`);
**F11 ✅** completo (1-4) e **F13 ✅** (`forge-core cost`) — commits `d4b338b`, `2185277`, `f82f792`)
**Próximo**: F06 — Memory Policy (crate `forge-core-memory`); F06.1 (grill + improve-codebase) em andamento por outro agente
**Dono**: Daniel (codebase owner) + agente executor
**Norte estratégico**: rápido, robusto, performativo, protocolo-guia que escala com a
capacidade dos agentes, nunca script de novela, sempre Rust ou compatível, sempre
lastreado em melhores práticas e papers científicos (orientais e ocidentais).

## Score atual por frente (audit 2026-06-30)

| Frente | Hoje | Meta | Lacuna principal |
|---|---|---|---|
| Rápido | 9 | 10 | Benchmarks crypto R6.2 (~420µs verify, ~6µs parse) + store R6.1 + serde R6.3 medidos e consolidados em `docs/perf/baseline.md` (acessível sem rodar bench); `--no-sync` cobre claim + execute-operation + rebuild-effect-index (F15.7b). Restam: otimizações pontuais de hot paths (Epic R-FAST, último na fila) |
| Robusto | 10 | 10 | Tracing completo; zero Result<_,String>; R5 zeroize completo (R5.1-R5.11) |
| Performativo | 10 | 10 | `--no-sync` cobre claim + execute-operation + rebuild-effect-index (F15.7b-extend); crypto + store + serde benchmarks medidos (R6.1, R6.2, **R6.3 ✅**); **R6.4 ✅** regression gate no CI (cache criterion baseline + awk parser, fail-on-alert >15%); baseline consolidado em `docs/perf/baseline.md` (acessível sem rodar bench) |
| Protocolo guia | 10 | 10 | F04 ✅ fechado (validate + dry-run + 34 E2E tests); F01 bugs críticos fechados |
| Workflows | 10 | 10 | WAL/claim ok; **F11.1 ✅** CLI standalone + **F11.2 ✅** 4 policies + **F11.3 ✅** enforcement no `execute-operation` + **F11.4 ✅** TraceEvent (started/passed/failed emitidos no gate e standalone); **guia unificado consolidado** via `forge-core guide describe` (110 workflows em 7 phases, JSON estruturado, embedded no binário — funciona em greenfield sem `--catalog-dir`) + `guide decide` (valida decisão) + `guide status` (orienta agente em phase atual) |
| Agente guia humano | 10 | 10 | F01 bugs de integridade fechados (F01.1-F01.6); `rollback_available` real (F01.3); `next_human_action: Option<String>` populado em todos os estados não-Ready do `RuntimePreviewReport` (Blocked → "inspect blockers", AwaitingHuman → prompt ou "provide required human input", GateRequired → "provide required gate evidence", ReviewRequired → "review and approve the operation boundary", ReadOnlyStatus → "show read-only status", ReadyToCallOperation+blockers → "resolve blockers"); `forge explain` narra cronologicamente (F03); `forge guide describe/status/decide` fornece routing surface de 110 workflows |
| Não-script-de-novela | 10 | 10 | **G1 ✅** fechado: 62/62 policies em `contracts/policies/` são framework paramétrico (0/62 script). Auditoria em `progress/g1_policies_script_novela_audit.md`. Bússola `human-agent-interface.yaml` honrada |
| Features comunidade | 9.8 | 10 | F03/F04/F01/F02/F15 operacionais; **F11.1+F11.2+F11.3+F11.4 ✅**; **F13 ✅** `forge-core cost` (agregação por run/graph/agent/principal); **F05 ✅** fechado (eval harness completo: design, schema, executor, grader, CLI, trace, E2E); falta F06-F08, F12, F14 |
| Rust best practices | 10 | 10 | E1 fechado (0 warnings lib); **F15 fechado** (2 edit points); **R-LINT ✅** completo: 41 pedantic → 0 em `--all-targets`, CI flipado para `-D clippy::pedantic` (R-LINT.6) |
| Segurança supply chain | 10 | 10 | serde_yaml migrado; zeroize feito; fuzz (R4) completo via ADR-0008; **R-SCM ✅** completo: sigstore keyless signing (cosign via GitHub OIDC) + CycloneDX SBOM no `release.yml` |
| Docs/rastreabilidade | 10 | 10 | R13 alinhado; R14 paper status criado; ADR-0008; **R9 ✅** fechado: Bootstrap Core Exception explícita, opt-in (`--allow-bootstrap-core`), 22 tests E2E comprovam consumer repo limpo opera clean sem ela |

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
- [x] **R4** — Fuzz harness (`cargo-fuzz`) ✅ COMPLETO via ADR-0008
      - Inventário completo em `progress/r4_fuzz_inventory.md`
      - Alvos reais: `parse_rekor_log_entry`, `parse_signed_checkpoint`,
        `decode_ocsp_response`, `decode_prefix`
      - Decisão de design: 4 parsers `pub` direto nos módulos-fonte
        (alternativa B do deletion test; wrapper `pub mod fuzz` shallow removido)
      - 28 seeds sintéticos commitados (6+7+8+7) cobrindo branches principais
      - Runs locais em Windows-MSVC bloqueados por limitações de cargo-fuzz
        (ASAN DLL faltante, `__stop___sancov_pcs` undefined em `-s none`)
      - Solução madura: CI Linux (`fuzz.yml` com cron diário + workflow_dispatch
        + label `fuzz` em PRs), 4 targets × 5 min cada
      - Ver `progress/r4_fuzz_plan.md` e `adrs/ADR-0008-fuzz-runs-on-linux-ci-not-windows-local.md`
      - Sub-tasks:
      - [x] R4.1 Setup: `cargo install cargo-fuzz` + `cargo fuzz init` +
            parsers `pub` direto + `[workspace]` vazio em `fuzz/Cargo.toml`
            (commit `9b31150`)
      - [x] R4.2 Harness `parse_signed_checkpoint` + 6 seeds (commit `0d00008`)
      - [x] R4.3 Harness `parse_rekor_log_entry` + 7 seeds (commit `8f7d43d`)
      - [x] R4.4 Harness `decode_ocsp_response` + 8 seeds (R4.6 batch)
      - [x] R4.5 Harness `decode_prefix` + 7 seeds (R4.6 batch)
      - [x] R4.6 DoD: CI workflow + ADR-0008 + docs (este commit)
- [x] **R6.1** — Benchmarks (`criterion`) store hot paths ✅
      - `claim_wal.rs`: append 1/100/1000 entries (32ms / 37ms / 41ms)
      - `claim_wal.rs`: replay 1/100/1000 (157µs / 719µs / 7.2ms)
      - `reference_index.rs`: workspace (~1.5ms) + minimal (~205µs)
      - Achado: fsync Windows é durability-bound (25–50ms), não bug
      - Ver `progress/r6_benchmarks.md`
- [x] **R6.2** — Benchmarks crypto hot paths ✅
      - `crates/forge-core-crypto/benches/rekor.rs` com 5 cenários:
        `parse_signed_checkpoint`, `parse_rekor_log_entry`,
        `verify_rekor_full_path/aux_{0,10,100}`
      - Decisão de design (skill `improve-codebase-architecture`): helpers
        internos `verify_*` mantidos `pub(crate)`, medidos via entrypoint
        público `run_host_adapter_rekor_verification`. Deletion test aprova.
      - Baselines (dev, Windows 11 / WSL): parse 2-7µs, verify full path
        420-655µs (p256 verify domina; Merkle walk scales O(log n))
      - Ver `progress/r6_benchmarks.md`
- [x] **R6.3** — Benchmarks `serde_yaml::from_str` vs `serde_yml::from_str` vs `yaml_serde::from_str` ✅
      - `crates/forge-core-validate/benches/yaml_deserialize.rs` mede
        `from_str::<OperationContractDocument>` no fixture de produção
        `docs/fixtures/operation-contract-v0/facilitate-first-product-idea.yaml`
        (3.025 bytes, structs aninhadas, `deny_unknown_fields`, optionals,
        enums, arrays).
      - **Resultado**: `yaml_serde` 99.7µs (~21.7 MiB/s) vs `serde_yaml`
        92.9µs (~23.3 MiB/s) vs `serde_yml` 93.4µs (~23.2 MiB/s).
        `yaml_serde` é ~7% mais lento mas **R7 não é revertida**: não é hot
        path, diferença é ~7µs/contrato (abaixo de I/O + crypto dominantes),
        e ganhos de governança/manutenção superam o custo. Baseline salva
        para acionar reavaliação se regredir >30% ou workload mudar.
      - **Decisão de design** (`improve-codebase-architecture`): bench vive
        em `forge-core-validate` (já tem `yaml_serde` em deps de produção e
        faz parsing de contratos — locality + leverage). `serde_yaml` e
        `serde_yml` ficam como dev-deps apenas deste bench; não há caminho
        de produção com eles.
      - Ver `progress/r6_benchmarks.md` (R6.3) e `progress/r7_yaml_serde.md`
- [x] **R6.4** — CI: regression gate em PR com label `perf` ✅ (commit `84c730e`)
      - `.github/workflows/perf.yml`: cron diário + `workflow_dispatch` +
        label `perf` em PRs (opt-in)
      - Roda `cargo bench -p forge-core-store` e `-p forge-core-crypto --bench rekor`
        sem `--save-baseline` (criterion compara automaticamente contra o baseline
        restaurado do cache)
      - **Cache de baseline**: `target/criterion` keyed por `criterion-${OS}-${branch}`
        (restore-keys fallback pra `main`), persistido entre runs via `actions/cache@v4`
      - **Regression gate (>15%)**: step `Detect performance regressions (>15%)` parseia
        linhas `change: [a% b% c%]` do output do criterion via `awk`, falha o PR se o
        `b` (mediana) exceder 15%. Primeira run (sem baseline cached) trivialmente
        passa e estabelece o baseline pra próxima run.
      - Output `.txt` upado como artifact (30 dias retention) pra inspeção manual
      - Threshold de 15% escolhido como meio-termo: baixo o suficiente pra capturar
        regressões reais, alto o suficiente pra não flaggear ruído de CI runner.
- [x] **R5** — `zeroize` em material cripto ✅ COMPLETO
      - Inventário completo em `progress/r5_crypto_inventory.md`
      - FASE A, B, C (R5.1-R5.9) completas em commits `9c3c5f3`, `5171cee`,
        `39a7dc3`, `c63765b`, `087d843`, `710ec74`, `41edd21`, `d0bc68f`,
        `888a27c`, `e44ea39`.
      - FOLLOW-UP (auditado nesta sessão, sem ação necessária):
      - [x] R5.10 Inventariar `sigstore.rs`: `ParsedSigstoreMessageSignatureBundle`
            e `ParsedSigstoreDsseBundle` têm `signature/certificate_der/
            message_digest/payload: Vec<u8>`. **Nenhum é secret material** —
            signatures ECDSA são provas públicas (não expõem chave privada);
            certificates, digests, payloads são públicos por construção.
            Decisão: não zeroizar (overhead sem ganho).
      - [x] R5.11 Inventariar `file_io.rs`/`hashing.rs`/`slsa_transparency.rs`/`tuf.rs`:
            - `file_io.rs` 100% zeroizado (R5.7 confirmado)
            - `hashing.rs` é puramente funcional (sem locals secret)
            - `slsa_transparency.rs` recebe `&[u8]` slices; usa
              `Ed25519VerifyingKey`/`Ed25519Signature` que já zeroizam
              internamente (feature `zeroize` em R5.1)
            - `tuf.rs` lê via `read_required_file` (R5.7 retorna `Zeroizing<>`)
            Decisão: sem ação necessária.
- [x] **R7** — `serde_yaml` → `yaml_serde` ✅
      - Descoberta: `serde_yml` também está deprecated (shim)
      - Migrado para `yaml_serde 0.10.4` (The YAML Organization, API 1:1)
      - 124 refs em 42 arquivos, anchor 122 preservado
      - Ver `progress/r7_yaml_serde.md`
- [x] **R13** — Alinhar docs com realidade ✅
      - R13.1 (commit pre-sessão): `04_rust_refactor_guide.md` alinhado
      - R13.2 (esta sessão): `00_master_development_doc.md`,
        `02_implementation_plan.md`, `01_feature_specs.md`,
        `08_priority_recommendations_plan.md` alinhados com AGENTS.md
        (argv manual sem clap; error enums sem thiserror/anyhow). Todas as
        menções agora são em negação ou descrição de decisão.
- [x] **R14** — Criar `paper_implementation_status.md` ✅
      - 15 papers mapeados (6 ✅ Implemented, 9 🟡 Partial, 0 ❌ Pending)
      - Cobertura regional auditada: 8 Western, 0 Oriental-led, 5 Mixed (orientais
        vivem dentro destes como sub-findings), 2 unspecified
      - 3 convergências cross-cutting documentadas: "hard gates + freedom",
        "R8 id-coupling bug class unifies 4 papers", "pending findings → F05-F14"
      - Cobertura oriental marcada como gap pra R15/R16 (política geográfica
        existe em field-evidence-20260625 mas não há paper puramente oriental)
      - Arquivo: `paper_implementation_status.md`
- [x] **R9** — Fechar Bootstrap Core Exception ✅
      - Exceção temporária que permite `D: Forge-method-core` manter `.forge-method/` local
        enquanto Forge desenvolve a si mesmo. Definida formalmente em `CONTEXT.md`
        (linhas 21-23) e referenciada por `09_system_design_roadmap.md:70` e
        `08_priority_recommendations_plan.md:51` (R9 ↔ F12 Guided Start).
      - Gate opt-in `--allow-bootstrap-core` em `resolve_project()` (project_cmd.rs:880):
        só retorna `BootstrapCoreLocal` se `allow_bootstrap_core` AND
        `is_bootstrap_core_root()` forem ambos verdadeiros.
      - 22 tests E2E distribuídos em 4 arquivos (`project_init_e2e.rs`,
        `project_resolve_e2e.rs`, `project_link_hardening_e2e.rs`,
        `operation_sidecar_e2e.rs`) comprovam: (a) exceção é isolada e opt-in,
        (b) consumer repo fresh-init opera clean end-to-end sem o flag,
        (c) state-bearing writes (execute-operation) + reads (claim status,
        query-effect-index) + rebuild-effect-index todos via sidecar,
        (d) fail-closed paths preservados.
      - Sharpen (grill-with-docs): descrição anterior do item era ambígua
        (título "Remover Bootstrap Exception" mas descrição sobre "docs humanos
        → agentes", que é coberto por G1). Alinhado com `CONTEXT.md`.
      - Ver `progress/r9_bootstrap_exception.md`

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
- [x] **F04** — WorkflowGraph v0 ✅ FECHADO (2026-06-30)
      - **Auditoria 2026-06-30:** ambos subcomandos executam end-to-end.
        `forge graph validate` (4 passes: identity, nodes, edges, cycles via Kahn).
        `forge graph run --dry-run` (topological order + per-node preview +
        claim preflight + blocked_by upstream verifiers). `forge graph run`
        sem `--dry-run` é rejeitado por design (ainda não há executor real).
      - Sub-passes:
      - [x] **F04.1** Per-node `touched_refs` no dry-run output (`1c9a7dd`)
      - [x] **F04.2** Validar referências secundárias: `verifies`,
            `GraphBudget.node_id` (`e9eb579`)
      - [x] **F04.3** Edge-kind semantics documentadas + warning para
            `blocks_until_passed` de non-Verifier (`58ef7d8`)
      - [x] **F04.4** Tests E2E CLI: `validate` (Passed + Blocked + cycle),
            `run --dry-run` (Planned + Blocked + Invalid). **34 testes
            passando** em `crates/forge-core-cli/tests/graph_cli_e2e.rs`
            (2.03s), cobrindo ainda: security (symlink escape, parent escape,
            absolute refs), claim preflight, file/glob/state-key effect
            targets, expired/peer claims, bootstrap exception. Lib-level
            tests em `crates/forge-core-graph/tests/graph_contract.rs`
            também verdes.
      - Depende: F03 (tracing) pra narrar execução do grafo ✅
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
- [x] **F15** — Rust ergonomics + codegen track ✅ FECHADO (2026-06-30)
      - Critério atendido: novo comando/contrato exige **2 edit points** (criar módulo +
        registrar entrada em `command_registry::COMMANDS`); era 6+ antes.
      - Sub-passos:
      - [x] F15.0 — plano (`7d0934b`)
      - [x] F15.1 — `ArgvCursor` em `cli_util.rs` + piloto telemetry (`0d4431d`)
      - [x] F15.2 — migrate eval_cmd + graph_cmd (`47a51bd`)
      - [x] F15.3 — delete dupes (`next_<cmd>_value_or_err` absorvidos em `ArgvCursor`)
      - [x] F15.4 — `command_registry::COMMANDS` table, dispatch vira lookup (`c4cd6f4`)
      - [~] F15.5 — co-localize usage (opcional, deferido; não bloqueia critério)
      - [x] F15.6 — validar critério F15 (2 edit points) (`6ebba3e`)
      - [x] F15.7a — `WalDurability` enum + APIs `_with_durability` + ADR-0009 (`c2df8c4`)
      - [x] F15.7b — CLI threading `--no-sync` no claim surface (`7474139`)
      - [x] F15.7b-extend — Replicar `--no-sync` em `execute-operation` +
            `rebuild-effect-index` (commits `c2d2571`, `ba049f3`, `a21666d`).
            `query-effect-index` excluído (read-only). ADR-0009 amended.

### Trilha C — Features P1 da comunidade

- [x] **F05** — Eval Compare single-agent baseline (harness) ✅ FECHADO (2026-07-01)
      - `forge-core-eval` (lib de comparação) + nova crate `forge-core-eval-harness`
        (executor subprocess, grader, corpus loader, canonicalização)
      - Sub-passes (commits `2d56f33a`→`e42b1609`):
        - [x] **F05.1** — design do harness (`progress/f05_eval_harness_design.md`):
              subprocess por arm (isolamento, mesma CLI que produção)
        - [x] **F05.2** — `EvalHarnessConfig` YAML schema (arms, loader, tools,
              output_contract, usage_accounting, policy) + validator cumulativo
        - [x] **F05.3** — grader + corpus loader + canonicalização (pure half,
              commit `1b04da9`) e executor subprocess impuro (F05.3b, commit
              `d78085f`): `execute_run` por (arm, task), timeout via try_wait poll,
              stdout/stderr → null (child chatter não polui harness JSON), sempre
              retorna um contract por run (Error-verdict em falha, nada droppado)
        - [x] **F05.4** — report generator (`build_compare_suite` →
              `generate_comparison_report` reusando `compare_eval_runs`)
        - [x] **F05.5** — CLI `forge-core eval-harness --config <yaml>` (argv
              manual sem clap, registrado em `command_registry::COMMANDS`)
        - [x] **F05.6** — trace: 3 novos `TraceEventKind`
              (`EvalCompareStarted/Passed/Failed`) + `telemetry_cmd` cobertura
        - [x] **F05.7** — fixtures + E2E (run completo, report JSON estável)
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
- [ ] **F11** — Risk Audit Gate (parcial: **F11.1 ✅** standalone CLI, **F11.2 ✅** canonical policies)
      - Checks determinísticos + extensão SAST/linters
      - Falha fechado em padrões proibidos (fail-soft, exception swallowing)
      - [x] **F11.1** — Standalone CLI `forge-core risk-audit --rules <yaml>`;
            rule engine em `forge-core-validate::risk_audit` com 4 detector
            kinds (`regex`, `path_glob`, `file_glob_must_exist`,
            `external_linter`); fail-closed via `ExitReason::RejectedByGate`;
            5 E2E tests + 11 unit tests; rule set canônico em
            `tests/fixtures/risk-audit/valid-rust-antipatterns.yaml`
      - [x] **F11.2** — 4 policies canônicas em `contracts/risk-audits/`
            (fail-soft, exception-swallowing, security-slop, false-test),
            cada uma com fixture `valid/` (passa) e `invalid/` (falha
            fechado); 8 E2E tests em `risk_audit_policies_e2e.rs`
      - [x] **F11.3** — Enforcement real no `execute-operation`
            (gate antes do WAL; flag `--require-risk-audit`)
      - [x] **F11.4** — Integração com `TraceEvent` (rastreabilidade F03)

### Trilha D — Features P2/P3 da comunidade

- [ ] **F09** — Secure A2A adapter (agent-to-agent cross-vendor)
- [ ] **F10** — Control Plane local (TUI ou HTML estático lendo `.forge-method`)
- [ ] **F12** — Guided Start + Product UX (fluxo guiado sem YAML manual)
- [x] **F13** — Budget and Cost Accounting (per run/graph/agent/principal) ✅
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
- [x] **E4** — R-LINT: zero pedantic warnings em `--all-targets` (2026-07-01)
      - Categoria A (lib, 7) corrigida ou refatorada (RiskAuditTraceContext)
      - Categoria B (testes, 28) com `#[allow]` documentado onde idiomático
      - Categoria C (benches, 6) com fixes mecânicos
      - CI flipado para `-D clippy::pedantic` em R-LINT.6

### Trilha F — Papers e evidência científica

- [ ] **F-sci** — Para cada feature P0/P1, citar paper em `contracts/research/`
      com `relevance:` (orientais e ocidentais)
      - SLSA, sigstore, merkle (rekor), OCSP, RFC3161, saga pattern, autonomy
      - Papers chineses: _CoAgent_, _OpenDev_, _Code-as-Agent Harness_
      - Papers ocidentais: _SWE-agent_, _RAC_, _Microservices Saga_
      - DoD: `docs/.../paper_implementation_status.md` lista todos

### Trilha G — System design (não-R-track)

- [x] **G1** — Auditar todos os `contracts/policies/*.yaml` por "script de novela" ✅
      - Critério: cada policy deve ser matriz paramétrica (modes + thresholds),
        não sequência prescritiva
      - **DoD atingido**: 62/62 policies são framework paramétrico, 0/62 são
        script. Quatro heurísticas em camadas (lexicais / semânticas /
        prosa-longa / framework-positivo) + leitura manual de amostras
        representativas (`human-agent-interface.yaml`,
        `rust-workspace-architecture.yaml`, `rust-validation-authority.yaml`).
      - Ver `progress/g1_policies_script_novela_audit.md`
- [x] **G2** — Fixtures que provam framework (não script) ✅
      - Para cada policy, fixture testando múltiplos inputs no mesmo policy
      - **DoD atingido**: `cargo test -p forge-core-engine --test policies_framework`
        passa (8 tests, 0 failures). 4 fixtures de `AutonomyPolicyContract` em
        `docs/fixtures/autonomy-policy-v0/` (manual-default / yolo-default /
        mixed-with-manual-secret / yolo-disabled-escalation) × 3 fixtures de
        `VerificationGoalContract` em `docs/fixtures/verification-goal-v0/`
        (satisfied / pending / failed) exercitam todos os 6 branches do
        `route_lane_for_tool_classes`. Cada fixture cobre um eixo paramétrico
        distinto (`default_mode`, per-tool override, escalation flag,
        goal status); remover qualquer um quebra pelo menos um teste — prova
        que o router é framework (N inputs → N outputs coerentes via mesmo
        código), não script (1 input hardcoded → 1 output esperado).
      - Schema-drift guard: último teste carrega todos os 7 fixtures de uma
        vez pra quebrar cedo e com nome claro em vez de N panics cascateados.
      - Ver `crates/forge-core-engine/tests/policies_framework.rs`
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
- [x] **cargo fuzz run** em cada target por ≥1 min sem panic (via CI Linux;
      ver ADR-0008). Não roda em Windows-MSVC local por limitações do toolchain.
- [x] `cargo clippy --workspace --all-targets -- -D clippy::pedantic`
      com **0 warnings** (R-LINT completo, CI flipado para `-D` em R-LINT.6)
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

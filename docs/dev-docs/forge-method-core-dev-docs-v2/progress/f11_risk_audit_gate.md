# F11 — Risk Audit Gate

**Status**: em andamento (**F11.1 ✅ fechado**, **F11.2 ✅ fechado**)
**Data de abertura**: 2026-06-30
**Data de fechamento F11.1**: 2026-06-30
**Data de fechamento F11.2**: 2026-06-30
**Branch**: `codex/forge-frust-052-ocsp-boundary`
**Frente do roadmap**: Workflows (7 → 8.5), Features comunidade (9.5 → ?)
**Spec**: `docs/dev-docs/forge-method-core-dev-docs-v2/01_feature_specs.md` (F11)
**Papers de apoio**: P22 (AIRA), P23 (Governance), P30 (Pulse on GenAI)

## Objetivo

Implementar um gate de inspeção **fail-closed** que detecta anti-padrões
induzidos por IA no código fonte do consumidor (fail-soft, exception
swallowing, security slop, false tests). As regras são contratos YAML
paramétricos (`risk-audit-v0`) — adicionar uma regra nunca exige uma
mudança em Rust.

O gate responde a duas perguntas que o protocolo não conseguia responder
antes:

1. *Este código que o agente está prestes a escrever contém um padrão
   proibido?* (detecção)
2. *Quais regras cobrem este repo?* (declaração out-of-band, sem acoplamento
   ao runtime)

## Decisões de design (deepening opportunities)

Aplicação direta do skill `improve-codebase-architecture`:

- **Deepening do `forge-core-validate`**: o validador existente já acumula
  `Diagnostic`s em `ValidationReport`. O Risk Audit é um novo módulo que
  produz o mesmo tipo de `Diagnostic` (com `DiagnosticCode::RiskAudit*`),
  então a UI do CLI (`CliEnvelope` + `ValidationReport`) é reaproveitada
  sem nova superficie.
- **Seam = detector kind**: a enum `RiskAuditDetector` expõe um pequeno
  conjunto de kinds (`regex`, `path_glob`, `file_glob_must_exist`,
  `external_linter`). Cada kind é um adapter que satisfaz a interface
  "dado um `RiskAuditTarget`, produza zero ou mais `Diagnostic`s". Adicionar
  um novo detector é adicionar um variant.
- **Leverage**: a interface pública do módulo é duas funções
  (`validate_risk_audit_rule_set`, `evaluate_risk_audit`). Toda a lógica de
  glob matching, regex compilation, e detector dispatch fica atrás delas.
- **Locality**: regras YAML e código Rust são desacoplados. Mudar o que é
  proibido é editar YAML; mudar como regras são avaliadas é editar Rust.
  Não há mistura.

## F11.1 — Standalone CLI (FECHADO)

### Entregue

- `crates/forge-core-validate/src/risk_audit.rs` (novo, ~400 linhas)
  - `RiskAuditSeverity`, `RiskAuditDetector`, `RiskAuditRule`,
    `RiskAuditRuleSet`, `RiskAuditTarget`
  - `validate_risk_audit_rule_set(&ruleset) -> ValidationReport`
  - `evaluate_risk_audit(&ruleset, &[target]) -> ValidationReport`
  - Glob matcher (`*` single-segment + `**` recursivo)
  - 11 unit tests
- `crates/forge-core-cli/src/risk_audit_cmd.rs` (novo)
  - `run_risk_audit_command(args) -> Result<(), ExitError>`
  - `run_risk_audit(root, rules_path) -> CliEnvelope<RiskAuditSummary>`
  - Walker `collect_targets()` (skip `.git`, `target`, `node_modules`,
    `dist`, `build`, `.forge-method`)
  - `print_human()` para CLI humano-legível
- `crates/forge-core-cli/src/command_registry.rs`
  - Registro do command `risk-audit`
- `crates/forge-core-validate/src/lib.rs`
  - `pub mod risk_audit;`
  - `Diagnostic::warning(...)` construtor simétrico ao `error(...)`
  - 14 variants `RiskAudit*` em `DiagnosticCode`
- `Cargo.toml` (workspace) — adicionado `regex = "1.11"`
- `crates/forge-core-validate/Cargo.toml` — `regex.workspace = true`
- `crates/forge-core-cli/tests/fixtures/risk-audit/valid-rust-antipatterns.yaml`
  - Rule set canônico: `no-unwrap`, `no-expect`, `no-empty-catch`,
    `must-have-readme`
- `crates/forge-core-cli/tests/risk_audit_cli_e2e.rs` (novo, 5 tests):
  - `risk_audit_missing_rules_flag_fails_clearly`
  - `risk_audit_invalid_rules_yaml_fails_clearly`
  - `risk_audit_empty_ruleset_fails_closed`
  - `risk_audit_passes_when_no_anti_pattern_matches`
  - `risk_audit_fails_closed_when_anti_pattern_matched`
- `README.md` — seção "Risk audit gate" em Features
- `CONTEXT.md` — termos "Risk Audit" e "Anti-pattern (AI Code)" sharpened

### Comportamento do envelope

| Cenário | `ok` | `exit_reason` | exit code | `data` presente? |
|---|---|---|---|---|
| Sem `--rules` | `false` | `env_config` | 5 | não |
| YAML malformado | `false` | `invalid_decision_shape` | 3 | não |
| Rule set vazio | `false` | `invalid_decision_shape` | 3 | não |
| Rules válidas, alvo limpo | `true` | `ok` | 0 | sim (summary) |
| Rules válidas, anti-pattern | `false` | `rejected_by_gate` | 2 | **sim** (summary com todos findings) |

A opção `CliEnvelope::reject(...)` foi escolhida propositalmente para o
caso "anti-pattern encontrado": o shell vê non-zero, mas agentes podem ler
o summary completo com todos os findings sem re-rodar o gate.

### Validação rodada

- `cargo check -p forge-core-cli` ✅
- `cargo clippy -p forge-core-validate -p forge-core-cli --all-targets -- -W clippy::pedantic` ✅ (0 warnings no trabalho novo; warnings preexistentes em `conflict_detection.rs`, `validate.rs`, `current_contracts.rs`, `benches/yaml_deserialize.rs` não tocados)
- `cargo test -p forge-core-validate --lib` ✅ (11 risk_audit tests + 143 pré-existentes)
- `cargo test -p forge-core-cli --test risk_audit_cli_e2e` ✅ (5/5)
- `cargo fmt -p forge-core-validate -p forge-core-cli -- --check` ✅ (meus arquivos limpos; drift preexistente em `claim.rs`, `cli_util.rs`, `preflight_cmd.rs`, `claims.rs` não tocado)

## F11.2 — Canonical policies (FECHADO)

### Entregue

- `contracts/risk-audits/fail-soft.yaml` (6 regras: `rust-no-unwrap`,
  `rust-no-todo-macro`, `rust-no-panic-in-product`,
  `python-no-assert-for-control-flow`, `python-no-bare-except-pass`,
  `ts-no-empty-catch`)
- `contracts/risk-audits/exception-swallowing.yaml` (6 regras cobrindo
  Rust `let _ = result` + `.ok()`, Python bare except + suppress(Exception),
  Go `_ = fn()`, TS `.catch(() => {})`)
- `contracts/risk-audits/security-slop.yaml` (5 regras: hardcoded password/key,
  AWS key shape, disabled TLS verify, empty `expect("...")`, TODO em
  security-sensitive paths)
- `contracts/risk-audits/false-test.yaml` (5 regras: `assert!(true)`,
  tautological `assert_eq!`, empty test body, `#[ignore]` sem reason,
  Python `pass`-only test)
- Fixtures pareadas para cada policy:
  - `contracts/risk-audits/fixtures/<policy>/valid/lib.rs` — passa limpo
  - `contracts/risk-audits/fixtures/<policy>/invalid/lib.rs` — falha fechado
- `crates/forge-core-cli/tests/risk_audit_policies_e2e.rs` (8 tests, 2 por
  policy): `assert_valid_fixture_passes` + `assert_invalid_fixture_fails_closed`

### Lições de design

1. **Comentários que descrevem anti-padrões disparam os próprios
   anti-padrões**: o detector regex é textual e não entende semântica da
   linguagem. Uma fixture não pode mencionar `let _ = result` no comentário
   sem disparar a regra `rust-no-let-underscore-result`. A solução é não
   documentar o padrão literal na fixture; o policy yaml já documenta.
2. **O crate `regex` do Rust é RE2-based e não suporta backreferences**
   (`\1`). Isso limita a regra `any-no-tautological-assert`: em vez de
   casar `assert_eq!(X, X)` arbitrário, listamos os literais comuns
   (`0`, `1`, `2`, `true`, `false`, `None`, `Some(N)`, strings curtas).
   Isso é feature, não bug: o regex crate é linear-time e não tem catastrophic
   backtracking, o que é importante porque o Risk Audit processa código
   não-confiável de consumidores.
3. **YAML quoting**: `fix_hint: Replace with `X: Y`.` quebra o parser YAML
   porque o `: ` é interpretado como mapping. Strings com `:` precisam
   de aspas duplas.

### Validação rodada

- 4 policies × 2 fixtures = 8 combinações testadas manualmente, todas
  consistentes (válidas passam; inválidas falham fechado com contagens
  esperadas)
- `cargo test -p forge-core-cli --test risk_audit_policies_e2e` ✅ 8/8
- `cargo clippy -p forge-core-cli` ✅ 0 warnings no trabalho novo
- `cargo fmt -p forge-core-cli` ✅ limpo

## Próximos passos

### F11.3 — Enforcement real

- Campo `risk_audit_required: bool` em `RuntimeOperationExecutionContext`
- Quando mutável + required: chama `evaluate_risk_audit` antes do WAL
- Se report tem errors: `ExecuteOperationError::RiskAuditFailed` rejeita
- CLI flag `--require-risk-audit <policy>` em `execute-operation`
- E2E em `crates/forge-core-cli/tests/operation_sidecar_e2e.rs`

### F11.4 — TraceEvent integration

- Variants: `RiskAuditStarted`, `RiskAuditFindingRecorded`,
  `RiskAuditPassed`, `RiskAuditFailed`
- `forge explain` já narra — só adicionar variantes

## Lições aprendidas

1. **Wire format usa snake_case**, não kebab-case. `ExitReason::as_str()`
   retorna `"env_config"`, `"invalid_decision_shape"`, `"rejected_by_gate"`.
   Testes E2E que comparam strings contra o JSON envelope precisam usar o
   snake_case, senão falham silenciosamente.
2. **`CliEnvelope::reject(command, exit, message, data)`** é a ferramenta
   certa para fail-closed com payload: shell vê non-zero, agentes leem data.
3. **Skills aplicados inline, não baixados**: as skills `improve-codebase-architecture`
   e `grill-with-docs` vivem no contexto, não em arquivos. Aplicar durante
   design, não como passo separado.
4. **Iteração barata**: `cargo check -p <crate>` (~5s após warm cache) é
   suficiente pra iterar em erros de tipo; reservar `cargo test --workspace`
   pro fim da fase.

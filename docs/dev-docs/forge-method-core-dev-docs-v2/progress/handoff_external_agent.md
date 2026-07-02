# Handoff para Agente Externo — Forge Method Core

**Data**: 2026-06-30
**Branch**: `codex/forge-frust-052-ocsp-boundary`
**Motivo**: agente Zed estava crashando (suspeita: commit-charge Windows com
subagentes + compilações paralelas). Daniel pediu plano consolidado, auto-contido,
para execução por agente "frio" (sem contexto desta sessão).

Este documento é a fonte da verdade. Leia completo antes de mexer em qualquer
arquivo.

---

## 1. O produto (norte estratégico)

Forge Method Core é um **protocolo em Rust** que orienta agentes a desenvolverem
produtos com qualidade/segurança/excelência. Feito **por agentes, para agentes**.
O único doc para humanos é o `README.md` (instalação, patch notes). Todo o
restante é para agentes.

Norte final:
- **Rápido, robusto, performativo**
- **Protocolo-guia** que escala com a capacidade dos agentes (sem script de novela)
- **Agente guia humano** — o agente orienta o humano nas decisões
- **Sempre Rust** ou compatível
- **Lastreado em melhores práticas e papers científicos** (orientais + ocidentais)

---

## 2. Estado atual (snapshot 2026-06-30, working tree LIMPA)

### Branch e commits

```
codex/forge-frust-052-ocsp-boundary
fd4249f (HEAD) F11.2: add canonical risk-audit policies with paired fixtures
7e91b05 F11.1: add risk-audit standalone CLI gate
98fd9e2 Reactivate pi-green-loop after R9 phase
5c6df64 R9: close Bootstrap Core Exception trilha
```

### Scores por frente (audit `2026-06-30`)

| Frente | Hoje | Meta | Lacuna |
|---|---|---|---|
| Rápido | 7.5 | 10 | crypto/store/serde benchmarks já medidos; falta perf regression suite (R6.4) |
| Robusto | **10** | 10 | ✅ |
| Performativo | 8 | 10 | Faltam: R6.4 (perf regression suite em CI), F13 (budget/cost) |
| Protocolo guia | **10** | 10 | ✅ |
| Workflows | 8 | 10 | Falta: **F11.3** (enforcement), F11.4 (TraceEvent) |
| Agente guia humano | 9 | 10 | Praticamente fechado; `next_human_action` sempre `Some` quando bloqueado |
| Não-script-de-novela | **10** | 10 | ✅ G1 + G2 fechados |
| Features comunidade | 9.5 | 10 | Faltam F05–F14 (F11 parcial) |
| Rust best practices | **10** | 10 | ✅ E1–E3 + F15 fechados |
| Segurança supply chain | 8 | 10 | serde_yaml migrado; zeroize completo; fuzz R4 fechado via ADR-0008 |
| Docs/rastreabilidade | **10** | 10 | ✅ R9 fechado |

**7 frentes em 10/10.** Próximo close de maior impacto: **F11.3** (sobe Workflows 8→9).

### Working tree

Após commit `fd4249f`: working tree **limpa**. `pi-green-loop` ativo.

### Anchor preservado (DEVE manter)

```bash
cargo run --quiet -p forge-core-cli -- validate --root . --json 2>/dev/null \
  | grep -c '"diagnostics": 0'
# → 122
```

---

## 3. Princípios não-negociáveis (AGENTS.md + trilhas fechadas)

1. **Sem `anyhow`/`thiserror`** — enums hand-rolled, `derive(Debug, Clone, PartialEq, Eq)`.
   Converter via `.map_err(NamedError::from)` ou `From` impl.
2. **Sem `clap`/derive macros** — argv parsing manual:
   ```rust
   let mut index = 1usize;
   while index < args.len() {
       match args[index].as_str() { ... }
       index += 1;
   }
   ```
3. **Sem `Result<_, String>` novo** — legacy em parsers (`parse_rekor_log_entry`,
   `required_string`) não propagar.
4. **Workspace deps compartilhadas** — usar `serde.workspace = true`, nunca pin por crate.
5. **Validação acumulativa** — `ValidationReport` (não short-circuit no primeiro erro).
   `Diagnostic` campos fixos: `severity`, `code`, `path`, `message`. Use
   `Diagnostic::error(...)` / `Diagnostic::warning(...)`.
6. **Sem script de novela** — todo contrato/policy/workflow é matriz paramétrica.
7. **Commits pequenos** — 1 preocupação por commit, cada commit verde.
8. **Anchor 122 preservado** — `validate --root . --json | grep -c 122`.
9. **Rust-first** — nada de Python/JS no runtime.
10. **Língua com Daniel**: PT-BR. Código/comentários em inglês.

---

## 4. Skills inline (NÃO baixar — estão no contexto do Daniel)

### 4.1 `improve-codebase-architecture` — vocabulário e princípios

Aplicar em toda decisão de arquitetura. Vocabulário fixo (não variar):

- **Module** — qualquer coisa com interface + implementação.
- **Interface** — tudo que o caller precisa saber (types, invariants, error modes,
  ordering, config).
- **Implementation** — código dentro.
- **Depth** — leverage na interface: muito comportamento atrás de interface pequena.
  **Deep** = alta leverage. **Shallow** = interface tão complexa quanto implementação.
- **Seam** — onde interface vive; ponto de alteração sem editar in-place. (Usar
  "seam", não "boundary".)
- **Adapter** — coisa concreta que satisfaz interface num seam.
- **Leverage** — o que callers ganham do depth.
- **Locality** — o que maintainers ganham: bugs/mudanças concentrados num lugar.

**Deletion test**: imagine deletar o módulo. Se complexidade some, era pass-through.
Se complexidade reaparece em N callers, ganhava sua vida.

**Um adapter = seam hipotético. Dois adapters = seam real.**

ADRs em `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/` (9 ADRs, último ADR-0009).
`CONTEXT.md` em `<repo-root>\CONTEXT.md`.

### 4.2 `grill-with-docs` — terminology + ADR gate

- `CONTEXT.md` é **glossário puro** — sem detalhes de implementação.
- Quando termo emerge, sharpenar inline em `CONTEXT.md`.
- **ADR gate** — só criar ADR se as 3 coisas:
  1. Hard to reverse (custo de mudar ideia é significativo)
  2. Surprising sem contexto (futuro leitor pergunta "por quê?")
  3. Real trade-off (alternativas genuínas, escolha por reasons específicas)
- Se qualquer um falta, **não criar ADR**.

### 4.3 CONTEXT.md — termos já sharpened

- **Risk Audit** (seção): "fail-closed inspection pass over source code that
  detects AI induced anti-patterns..."
- **Anti-pattern (AI Code)** (seção): "named, parametrizable pattern in source
  code or test artifacts..."

---

## 5. Pitfalls (aprendidos em sessões anteriores — NÃO repetir)

| # | Pitfall | Mitigação |
|---|---|---|
| 1 | OOM com subagentes em Zed (Windows commit charge) | Não usar subagentes. Iteração direta. |
| 2 | Dois agentes/IDEs compilando Rust → OOM code 3221226505 | Apenas um agente ativo por vez. |
| 3 | `pub use` de items `pub(crate)` → E0364 | Mudar item pra `pub`. |
| 4 | `clippy --fix` em crypto QUEBRA `ocsp.rs`/`host_adapter_verification.rs` | Mudanças manuais. |
| 5 | `cargo test --workspace` transient filesystem lock no Windows (tempdirs) | Rodar workspace test 1x no fim, não iterar nele. |
| 6 | 5 warnings preexistentes em `crates/forge-core-decisions/src/conflict_detection.rs` (linhas 681, 776) | Não são de trilha ativa. NÃO corrigir. |
| 7 | fmt drift preexistente em `claim.rs`, `cli_util.rs`, `preflight_cmd.rs`, `claims.rs`, `graph_contract.rs` | Não corrigir se não for da trilha ativa. |
| 8 | `ExitReason` variants: só `Ok`, `RejectedByGate`, `InvalidDecisionShape`, `Conflict`, `EnvConfig` | Não inventar `Usage`/`InvalidValue`/`Failed`. |
| 9 | Wire format JSON usa **snake_case**: `"env_config"`, `"invalid_decision_shape"`, `"rejected_by_gate"` | Nunca kebab-case. |
| 10 | `StableId(pub String)` é `#[serde(transparent)]` — serializa como string direta | Não como objeto `{stable_id: ...}`. |
| 11 | `CliEnvelope::reject(command, exit, message, data)` — fail-closed + payload | Perfeito pra risk audit: shell vê non-zero, agentes leem summary completo. |
| 12 | `next_<cmd>_value_or_err` duplicados foram deletados em F15.1/F15.2 | Não recriar. Parsing manual inline. |
| 13 | `panic = "abort"` em `profile.release` | `fuzz/Cargo.toml` já tem override. Não remover. |
| 14 | Skills não estão em arquivos do repo | Aplicar inline, não baixar. |
| 15 | CRLF warnings ao stagear no Windows | Inofensivos. Não bloquear commit. |
| 16 | `Blocking waiting for file lock on build directory` | Outro cargo roda. Esperar e prosseguir. |
| 17 | R9, F15, F04, F01, G1, G2 já fechados e commitados | Não re-litigar. |
| 18 | `regex = "1.11"` já está no workspace | Não adicionar de novo. |
| 19 | Comentários que descrevem anti-padrões disparam os próprios anti-padrões | Fixtures não mencionam o padrão literal; o YAML da regra já documenta. |
| 20 | Crate `regex` é RE2-based, **sem backreferences** (`\1` não funciona) | Listar literais comuns explicitamente. Linear-time, sem catastrophic backtracking. |
| 21 | YAML quoting: `fix_hint: Replace with X: Y` quebra parser (`: ` é mapping) | Strings com `:` precisam aspas duplas. |
| 22 | `evaluate_risk_audit` trabalha sobre `&[RiskAuditTarget]`, NÃO lê filesystem | Caller (CLI walker) popula `content`. |
| 23 | Green-loop: desativar durante edição grande (`mv pi-green-loop.json pi-green-loop.json.disabled`), reativar e commitar no fim da fase | Evita loops de fix que incham logs e atrapalham revisão. |

---

## 6. Épicos pendentes — ordem de execução

```
Epic 1: F11.3 (Risk Audit Gate enforcement)     ★★★ próximo, maior impacto
   │   abre Workflows 8→9
   ▼
Epic 2: F11.4 (TraceEvent integration)            ★★ rastreabilidade F03
   │   abre Docs/rastreabilidade (já 10, consolida) + Workflows?
   ▼
Epic 3: R6.4 (Perf regression suite em CI)        ★★ Performativo 8→9
   │
   ▼
Epic 4: F13 (Budget/Cost Accounting)              ★ Features comunidade 9.5→10
   │
   ▼
Pendências menores: F01.7, F05, F06, F07, F08
```

---

## 7. Epic 1 — F11.3: Risk Audit Gate enforcement no `execute-operation`

### Por quê

Hoje o `risk-audit` (F11.1) é um comando standalone: roda sob demanda e falha
fechado se achar anti-padrão. Mas o **`execute-operation`** (o caminho que
efetivamente muta o repo via WAL) **não consulta o risk audit**. Um agente pode
ignorar a saída do `risk-audit` e executar mutações sobre código com anti-padrões.

**F11.3 conecta os dois**: um caller pode exigir `--require-risk-audit <policy.yaml>`
ao executar uma operação mutável. O gate roda **antes do WAL write** e bloqueia a
operação se `report.has_errors()`.

### API surface já existente (reusar, NÃO recriar)

Em `crates/forge-core-validate::risk_audit`:
```rust
pub struct RiskAuditRuleSet { /* regras em YAML */ }
pub struct RiskAuditTarget { pub path: String, pub content: String }

pub fn validate_risk_audit_rule_set(&ruleset) -> ValidationReport;
pub fn evaluate_risk_audit(&ruleset, &[target]) -> ValidationReport;

impl ValidationReport {
    pub fn has_errors(&self) -> bool;
    pub fn diagnostics(&self) -> &[Diagnostic];
}
```

Em `crates/forge-core-cli::risk_audit_cmd`:
```rust
fn collect_targets(root: &Path) -> Result<Vec<RiskAuditTarget>, CollectTargetsError>;
// privado — precisa virar pub(crate) ou ser movido pra módulo compartilhado
```

Em `forge_core_contracts::CliEnvelope`:
```rust
pub fn reject(command, exit, message, data);  // fail-closed com payload
```

### Stories

#### Story F11.3.1 — Expor `collect_targets` para reuso cross-cmd

**Arquivo**: `crates/forge-core-cli/src/risk_audit_cmd.rs`

**Mudança**:
- Tornar `collect_targets` em `pub(crate) fn`.
- Tornar `CollectTargetsError` em `pub(crate)` (já é enum, só mudar visibilidade).
- Tornar `walk_dir`, `repo_relative`, `SKIP_DIRS` em `pub(crate)` se `collect_targets`
  depender deles — mas idealmente manter `walk_dir` privado e expor só `collect_targets`.

**Deletion test**: o walker é o único lugar que conhece SKIP_DIRS e a regra de
"ler conteúdo se < `RISK_AUDIT_MAX_FILE_BYTES`, senão target vazio". Se duplicar,
complexidade se espalha. **Concentra**: expor `collect_targets` como `pub(crate)`.

**Comando de verificação**:
```bash
cargo check -p forge-core-cli
```

#### Story F11.3.2 — Adicionar campo `risk_audit_rules` em `ExecuteOperationInput`

**Arquivo**: `crates/forge-core-cli/src/execute_operation.rs`

**Mudança** (por volta da linha 48–62):

```rust
#[derive(Debug, Clone)]
pub struct ExecuteOperationInput {
    pub root: PathBuf,
    pub effect_store_root: Option<PathBuf>,
    pub operation_path: PathBuf,
    pub command_paths: Vec<PathBuf>,
    pub effect_paths: Vec<PathBuf>,
    pub payloads: Vec<PayloadFileSpec>,
    pub payload_policy: PayloadLoadPolicy,
    pub recorded_at: String,
    pub tx_id_prefix: String,
    pub durability: WalDurability,
    /// F11.3: se presente, roda o risk-audit gate antes do WAL write.
    /// Falha fechado (`ExecuteOperationError::RiskAuditFailed`) se o report
    /// tiver errors. O `PathBuf` aponta para um `risk-audit-v0` YAML.
    pub risk_audit_rules: Option<PathBuf>,
}
```

**Comando de verificação**:
```bash
cargo check -p forge-core-cli
```

#### Story F11.3.3 — Adicionar variant `RiskAuditFailed` em `ExecuteOperationError`

**Arquivo**: `crates/forge-core-cli/src/execute_operation.rs`

**Mudança** (por volta da linha 109–139):

```rust
pub enum ExecuteOperationError {
    ReferenceIndexBuild(String),
    ReadFile { path: PathBuf, source: io::Error },
    ParseYaml { path: PathBuf, source: yaml_serde::Error },
    InvalidEffectPath { root: PathBuf, path: PathBuf },
    ContractPathOutsideRoot { /* ... */ },
    PayloadPathOutsideRoot { /* ... */ },
    PayloadTooLarge { /* ... */ },
    /// F11.3: o risk-audit gate falhou fechado. `error_count` é o total de
    /// findings com severity Error; `first_error` é o path+message do primeiro
    /// para contexto rápido. O caller (CLI) é responsável por imprimir o
    /// relatório completo se quiser (esta variant carrega só o resumo).
    RiskAuditFailed {
        error_count: usize,
        first_error: String,
    },
}
```

E no `impl fmt::Display` (por volta da linha 141–183), adicionar o braço:
```rust
ExecuteOperationError::RiskAuditFailed { error_count, first_error } => write!(
    formatter,
    "risk-audit gate failed with {error_count} error(s); first: {first_error}"
),
```

**Comando de verificação**:
```bash
cargo check -p forge-core-cli
```

#### Story F11.3.4 — Implementar gate em `run_execute_operation`

**Arquivo**: `crates/forge-core-cli/src/execute_operation.rs`

**Ponto de injeção**: entre a linha 260 (`payloads` resolvidos) e a linha 261
(construção de `RuntimeOperationExecutionContext`).

**Lógica**:
```rust
// F11.3: Risk Audit Gate. Se o caller passou `risk_audit_rules`, roda o gate
// ANTES de construir o contexto e chamar `execute_operation`. Falha fechado
// significa que NADA é escrito no WAL — o repo fica intocado.
if let Some(rules_path) = &input.risk_audit_rules {
    let rules_yaml = fs::read_to_string(rules_path).map_err(|source| {
        ExecuteOperationError::ReadFile {
            path: rules_path.clone(),
            source,
        }
    })?;
    let ruleset: RiskAuditRuleSet = yaml_serde::from_str(&rules_yaml)
        .map_err(|source| ExecuteOperationError::ParseYaml {
            path: rules_path.clone(),
            source,
        })?;
    let structure_report = validate_risk_audit_rule_set(&ruleset);
    if structure_report.has_errors() {
        let first_error = structure_report
            .diagnostics()
            .iter()
            .find(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
            .map_or_else(
                || "unknown structural error".to_string(),
                |d| format!("{}: {}", d.path, d.message),
            );
        return Err(ExecuteOperationError::RiskAuditFailed {
            error_count: structure_report.diagnostics().len(),
            first_error,
        });
    }
    let targets = crate::risk_audit_cmd::collect_targets(&root)
        .map_err(|source| ExecuteOperationError::ReferenceIndexBuild(
            format!("risk-audit collect_targets: {source}")
        ))?;
    let findings = evaluate_risk_audit(&ruleset, &targets);
    if findings.has_errors() {
        let error_count = findings
            .diagnostics()
            .iter()
            .filter(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
            .count();
        let first_error = findings
            .diagnostics()
            .iter()
            .find(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
            .map_or_else(
                || "unknown error".to_string(),
                |d| format!("{}: {}", d.path, d.message),
            );
        return Err(ExecuteOperationError::RiskAuditFailed {
            error_count,
            first_error,
        });
    }
}
```

**Imports necessários** no topo do `execute_operation.rs`:
```rust
use forge_core_validate::risk_audit::{
    evaluate_risk_audit, validate_risk_audit_rule_set, RiskAuditRuleSet,
};
use forge_core_validate::DiagnosticSeverity;
```

**Deletion test do gate**: se remover o gate, callers perdem a capacidade de
exigir "não mutar se código tem anti-padrão". Complexidade volta pros callers
(teriam que orquestrar `risk-audit` + `execute-operation` manualmente com race
condition entre os dois). Gate ganha sua vida.

**Comando de verificação**:
```bash
cargo check -p forge-core-cli
cargo clippy -p forge-core-cli --all-targets -- -W clippy::pedantic
```

#### Story F11.3.5 — Adicionar flag `--require-risk-audit` no argv parser

**Arquivo**: `crates/forge-core-cli/src/execute_operation.rs`

**Mudança em `run_execute_operation_command`** (por volta da linha 414–480):

Antes do loop `while index < args.len()`, adicionar:
```rust
let mut risk_audit_rules: Option<PathBuf> = None;
```

Dentro do `match args[index].as_str()`, adicionar o braço (próximo de `--no-sync`):
```rust
"--require-risk-audit" => {
    index += 1;
    let Some(value) = args.get(index) else {
        return Err(ExitError::usage(usage()));
    };
    risk_audit_rules = Some(PathBuf::from(value));
}
```

E na construção do `ExecuteOperationInput` (linha ~494), adicionar o campo:
```rust
let input = ExecuteOperationInput {
    root: roots.project_root,
    effect_store_root: Some(roots.effect_store_root),
    operation_path,
    command_paths,
    effect_paths,
    payloads,
    payload_policy,
    recorded_at,
    tx_id_prefix,
    durability,
    risk_audit_rules,  // <-- novo
};
```

**Comando de verificação**:
```bash
cargo check -p forge-core-cli
cargo test -p forge-core-cli --test operation_sidecar_e2e
```

#### Story F11.3.6 — Testes E2E

**Arquivo novo** (preferível) ou extensão: `crates/forge-core-cli/tests/execute_operation_risk_audit_e2e.rs`

**Casos**:

1. **`execute_operation_blocked_by_risk_audit`**:
   - Setup: tempdir com operation/command/effect contracts válidos + source
     file com anti-pattern (`unwrap()` em `src/lib.rs`).
   - Args: `execute-operation --root <tmp> --operation <op.yaml> --require-risk-audit contracts/risk-audits/fail-soft.yaml`
   - Assertion: `ExitError::failed` (non-zero), mensagem contém "risk-audit gate failed",
     WAL **NÃO foi criado** em `<tmp>/.forge-method/wal/effects.ndjson`.

2. **`execute_operation_passes_when_risk_audit_clean`**:
   - Setup: tempdir idêntico mas sem anti-pattern.
   - Args: idem.
   - Assertion: sucesso, WAL criado.

3. **`execute_operation_without_flag_skips_audit`**:
   - Setup: tempdir com anti-pattern.
   - Args: `execute-operation --root <tmp> --operation <op.yaml>` (sem flag).
   - Assertion: sucesso (gate não roda), WAL criado.

4. **`execute_operation_risk_audit_invalid_rules_yaml_fails_clearly`**:
   - Setup: tempdir + arquivo `bad.yaml` com YAML inválido.
   - Args: `--require-risk-audit bad.yaml`.
   - Assertion: `ExecuteOperationError::ParseYaml` propagado.

**Fixtures disponíveis** (já existem de F11.2):
- `contracts/risk-audits/fail-soft.yaml`
- `contracts/risk-audits/exception-swallowing.yaml`
- `contracts/risk-audits/security-slop.yaml`
- `contracts/risk-audits/false-test.yaml`

**Pattern de teste E2E já estabelecido** — ver `tests/risk_audit_policies_e2e.rs`
para o scaffold de tempdir + walker.

**Comando de verificação**:
```bash
cargo test -p forge-core-cli --test execute_operation_risk_audit_e2e
```

#### Story F11.3.7 — Documentação

**Arquivos a atualizar**:

1. **`docs/dev-docs/forge-method-core-dev-docs-v2/progress/f11_risk_audit_gate.md`**:
   - Mover "Próximos passos → F11.3" para nova seção "## F11.3 — Enforcement (FECHADO)"
   - Listar entregues (campo, variant, gate, flag, E2E)
   - Comando de validação rodado

2. **`docs/dev-docs/forge-method-core-dev-docs-v2/progress/excellence_roadmap.md`**:
   - Marcar `[x] F11.3`
   - Atualizar score: Workflows 8→9
   - Atualizar scorecard do topo

3. **`README.md`**:
   - Na seção Risk Audit Gate (Features), mencionar `--require-risk-audit` flag
     no `execute-operation`

4. **`CONTEXT.md`** (somente se termo novo surgir):
   - Provavelmente não precisa — Risk Audit já definido.
   - Se "Risk Audit Gate enforcement" for sharpenado como termo canônico, adicionar.

#### Story F11.3.8 — Commit + reativar green-loop + anchor check

**Workflow**:

```bash
# 1. Desativar green-loop durante edição grande (Story F11.3.1-F11.3.6)
mv pi-green-loop.json pi-green-loop.json.disabled

# 2. Trabalhar nas Stories F11.3.1-F11.3.7

# 3. Iteração barata durante trabalho:
cargo check -p forge-core-cli                              # ~5s
cargo clippy -p forge-core-cli --all-targets -- -W clippy::pedantic
cargo test -p forge-core-cli --test execute_operation_risk_audit_e2e
cargo test -p forge-core-cli --test operation_sidecar_e2e
cargo test -p forge-core-cli --test risk_audit_policies_e2e  # garantir que não quebrou F11.2

# 4. Workspace completo no FIM (1x só):
cargo test --workspace                                       # 3-5min
cargo run --quiet -p forge-core-cli -- validate --root . --json 2>/dev/null \
  | grep -c '"diagnostics": 0'                               # → 122

# 5. Commit
git add -A
GIT_EDITOR=true git commit -m "F11.3: enforce risk-audit gate in execute-operation

- ExecuteOperationInput.risk_audit_rules: Option<PathBuf>
- ExecuteOperationError::RiskAuditFailed { error_count, first_error }
- run_execute_operation: gate ANTES do WAL write; fail-closed se has_errors
- run_execute_operation_command: flag --require-risk-audit <path>
- risk_audit_cmd::collect_targets: pub(crate) para reuso cross-cmd
- E2E: tests/execute_operation_risk_audit_e2e.rs (4 casos)
- Workflows 8 -> 9"

# 6. Reativar green-loop
mv pi-green-loop.json.disabled pi-green-loop.json
git add pi-green-loop.json
GIT_EDITOR=true git commit -m "Reactivate pi-green-loop after F11.3 phase"

# 7. Anchor check final
cargo run --quiet -p forge-core-cli -- validate --root . --json 2>/dev/null \
  | grep -c '"diagnostics": 0'  # → 122
```

### Critérios de aceitação F11.3

- [ ] `ExecuteOperationInput` tem campo `risk_audit_rules: Option<PathBuf>`.
- [ ] `ExecuteOperationError` tem variant `RiskAuditFailed { error_count, first_error }`.
- [ ] `run_execute_operation` roda gate ANTES do WAL write se campo presente.
- [ ] Gate falha fechado se `report.has_errors()`; nada é escrito no WAL.
- [ ] `collect_targets` é `pub(crate)` (não duplicado).
- [ ] `--require-risk-audit <path>` parseado no argv parser.
- [ ] E2E cobre: bloqueado / passa / sem-flag-skips / invalid-yaml.
- [ ] `f11_risk_audit_gate.md` atualizado com F11.3 fechado.
- [ ] `excellence_roadmap.md` marcado `[x] F11.3`, Workflows 8→9.
- [ ] `README.md` menciona `--require-risk-audit`.
- [ ] Anchor 122 preservado.
- [ ] `cargo clippy -p forge-core-cli --all-targets -- -W clippy::pedantic` com 0 warnings no trabalho novo (warnings preexistentes listados na Pitfall #6/#7 não tocados).
- [ ] Commit `F11.3: enforce risk-audit gate in execute-operation` verde.

---

## 8. Epic 2 — F11.4: TraceEvent integration

### Por quê

Hoje o risk audit (F11.1 standalone) e o futuro gate F11.3 **não emitem
TraceEvent**. Um humano ou agente rodando `forge explain --run-id <id>` não vê
que o risk audit rodou. Falta rastreabilidade.

**F11.4 adiciona variants de TraceEvent e emite no caminho do gate (e idealmente
no `risk-audit` standalone também).**

### API surface já existente

Em `crates/forge-core-trace/src/lib.rs`:

```rust
pub struct TraceEvent {
    pub schema_version: String,
    pub kind: String,
    pub project_id: Option<String>,
    pub trace_id: String,
    pub event_id: String,
    pub run_id: String,
    pub graph_id: Option<String>,
    pub node_id: Option<String>,
    pub event_kind: TraceEventKind,
    pub recorded_at: String,
    pub actor: TraceActor,
    pub authority: TraceAuthority,
    pub inputs: Vec<TraceRef>,
    pub outputs: Vec<TraceRef>,
    pub risk: TraceRisk,
    pub cost: TraceCost,
    pub message: String,
}

#[serde(rename_all = "snake_case")]
pub enum TraceEventKind {
    RunStarted,
    OperationPlanned,
    PreviewCompleted,
    ReadyCompleted,
    GatePassed,      // ← reaproveitar ou estender
    GateBlocked,     // ← reaproveitar ou estender
    EffectStaged,
    EffectApplied,
    RunCompleted,
    RunFailed,
}
```

### Stories (rascunho — precisa investigação complementar)

#### Story F11.4.1 — Investigar surface do `m1_cmd.rs` para emissão de TraceEvent

Antes de editar, o agente deve:
- Ler `crates/forge-core-cli/src/m1_cmd.rs` linhas 380–500 (onde `TraceEventKind::RunStarted`,
  `PreviewCompleted`, `GatePassed`, `GateBlocked` são emitidos).
- Verificar como `trace_id`/`run_id`/`event_id` são gerados (provável UUID ou timestamp).
- Verificar onde o `TraceEvent` é persisted (provavelmente `.forge-method/trace/*.ndjson`).

#### Story F11.4.2 — Adicionar variants `RiskAudit*` em `TraceEventKind`

**Arquivo**: `crates/forge-core-trace/src/lib.rs`

**Decisão de design (grill-with-docs)**:

**Opção A** — Reusar `GatePassed`/`GateBlocked` com `message: "risk-audit: ..."`.
- Pró: zero mudança no enum.
- Contra: `forge explain` não consegue distinguir "preview gate" de "risk audit gate".

**Opção B** (recomendada) — Adicionar variants específicas:
```rust
pub enum TraceEventKind {
    // ... existentes ...
    RiskAuditStarted,
    RiskAuditPassed,
    RiskAuditFailed,
}
```
- Pró: `forge explain` pode ter branch específico ("risk audit found 3 errors").
- Contra: enum cresce.

**Recomendação**: Opção B. O `RiskAuditFindingRecorded` individual seria ruído —
`RiskAuditStarted` + `RiskAuditPassed`/`RiskAuditFailed` com `message` carregando
o summary é suficiente.

#### Story F11.4.3 — Emitir TraceEvent no `run_execute_operation` (F11.3 gate)

No bloco do gate (Story F11.3.4), após determinar `passed` ou `failed`, emitir
`TraceEvent`. Reusar o `trace_id`/`run_id` que já existe no contexto do
`execute-operation` (provavelmente derivado de `tx_id_prefix`).

#### Story F11.4.4 — Emitir TraceEvent no `run_risk_audit_command` (standalone)

Adicionar emissão no `risk_audit_cmd.rs` para que o standalone CLI também deixe
rastro (mesmo sem `run_id` formal, usar `run_id = "standalone"` ou gerar um UUID).

#### Story F11.4.5 — Atualizar `forge explain` para narrar variants novas

Em `m1_cmd.rs` (função `narrate_event` ou equivalente), adicionar braços para
`RiskAuditStarted`/`RiskAuditPassed`/`RiskAuditFailed`.

#### Story F11.4.6 — E2E: verificar que TraceEvent aparece no `forge explain`

Após rodar `execute-operation --require-risk-audit`, rodar `forge explain --last-run`
e verificar que o evento `risk_audit_*` aparece na narrativa.

### Critérios de aceitação F11.4

- [ ] `TraceEventKind` tem variants `RiskAudit*`.
- [ ] Gate F11.3 emite TraceEvent (started + passed/failed).
- [ ] `risk-audit` standalone emite TraceEvent.
- [ ] `forge explain` narra os novos eventos.
- [ ] E2E cobre: `execute-operation` com gate → `forge explain` mostra evento.
- [ ] `CONTEXT.md`: se "Risk Audit Gate enforcement" vira termo canônico, sharpenar.
- [ ] Docs atualizados.

---

## 9. Epic 3 — R6.4: Perf regression suite em CI

### Por quê

R6.1, R6.2, R6.3 mediram baselines (store, crypto, serde). Mas não há
**regression suite automatizada** — se alguém introduzir uma regressão de
performance, ninguém percebe até reclamação.

**R6.4 cria um workflow de CI que roda `cargo bench` e compara com baseline.**

### Lacuna de Performativo 8→9

Hoje: benchmarks existem e são medidos. Falta: regression detection automático.

### Stories (rascunho)

#### Story R6.4.1 — Investigar ferramenta de regression

Opções:
- **`criterion`** já usado — suporta `--save-baseline` e comparação.
- **`iai`** (instruction-count) — bom pra CI sem ruído de hardware, mas adicional.
- **GitHub Action `benchmark-action/github-action-benchmark`** — popular, mantém
  histórico em JSON, posta comentários em PRs.

**Recomendação**: usar `criterion` existente + `benchmark-action/github-action-benchmark`.
Sem adicionar nova dependency Rust.

#### Story R6.4.2 — Criar baseline canônico

Rodar `cargo bench` em ambiente estável (CI Linux), salvar baseline em
`.github/benchmarks/baseline.json` ou repositório separado.

#### Story R6.4.3 — Workflow `bench.yml`

```yaml
# .github/workflows/bench.yml
name: bench
on:
  pull_request:
    paths: ['crates/**', 'Cargo.toml', 'Cargo.lock']
  schedule:
    - cron: '0 4 * * *'  # diário 04:00 UTC
jobs:
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo bench --workspace | tee bench-output.txt
      - uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'cargo'
          output-file-path: bench-output.txt
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: false
          comment-on-alert: true
          fail-on-alert: true  # regressão > 10% falha o job
          alert-threshold: '110%'
```

#### Story R6.4.4 — Documentar baseline no `progress/r6_benchmarks.md`

Adicionar seção "Regression suite" com instruções de como regenerar baseline
quando intencional.

### Critérios de aceitação R6.4

- [ ] Workflow `bench.yml` existe e roda em PRs.
- [ ] `fail-on-alert: true` bloqueia regressões > 10%.
- [ ] Baseline canônico commitado ou persistido pela action.
- [ ] `progress/r6_benchmarks.md` documenta.
- [ ] `excellence_roadmap.md`: Performativo 8→9.

---

## 10. Epic 4 — F13: Budget/Cost Accounting

### Por quê

Hoje o `TraceEvent` já tem campo `cost: TraceCost`, mas não há comando que
agregue custos por run/graph/agent/principal/tool. Comunidade pede (P2).

### Stories (rascunho — precisa investigação maior)

#### Story F13.1 — Investigar `TraceCost` schema

Ler `crates/forge-core-trace/src/lib.rs` em torno de `TraceCost` — quais campos?
Provável: `tokens_in`, `tokens_out`, `model_calls`, `tool_calls`, `wall_clock_ms`.

#### Story F13.2 — Novo comando `forge-core cost`

```bash
forge-core cost --run-id <id>            # custo de uma run
forge-core cost --graph-id <id>          # custo agregado por graph
forge-core cost --principal <id>         # por principal
forge-core cost --last-run               # atalho
forge-core cost --json                   # envelope
```

Output: breakdown por `tool_class` / `model` / `agent_id`.

#### Story F13.3 — Aggregator em `forge-core-trace`

Nova função `aggregate_costs(events: &[TraceEvent]) -> CostReport`. Pura, testável.

#### Story F13.4 — CLI cmd + registry

Novo módulo `cost_cmd.rs`, registrar em `command_registry::COMMANDS`.

#### Story F13.5 — E2E

Construir trace sintético com N eventos, rodar `forge-core cost --run-id`, validar totais.

### Critérios de aceitação F13

- [ ] `forge-core cost` existe.
- [ ] Aggregation por run/graph/agent/principal/tool.
- [ ] JSON envelope com `CliEnvelope`.
- [ ] E2E cobre agregação.
- [ ] Papers: citar Microservices Saga / SLSA provenance para cost attribution.

---

## 11. Pendências menores

### F01.7 — Separar `PreviewJsonPayload` do envelope CLI (deferido)

Hoje `PreviewJsonPayload` mistura com `project_root`, `state_root`, `trace_id`.
Separação é opcional, não bloqueia nada. Deferido.

### F05 — Eval Compare single-agent baseline (P1)

`forge-core-eval` existe (934 linhas), falta harness comparativo com accuracy,
cost, latency, trajectory, failures, delta.

### F06 — Memory Policy (P1)

Admission, retention, forget, promote, raw evidence, authority boundary.

### F07 — Multi-principal governance (P1, parcial)

`PrincipalId`, `IntentContract`, `ConflictContract`, `GovernancePolicy`.

### F08 — Secure MCP adapter (P1)

MCP server para preview/ready/graph/trace/memory/effect. Allowlist + attestation.

---

## 12. Comandos de verificação (referência rápida)

### Iteração barata durante trabalho
```bash
cargo check -p forge-core-cli                              # ~5s
cargo clippy -p forge-core-cli --all-targets -- -W clippy::pedantic
cargo fmt -p forge-core-cli -- --check
```

### Pré-commit
```bash
cargo test -p forge-core-cli                               # 30s-1min
```

### Fim de fase (1x só)
```bash
cargo test --workspace                                      # 3-5min
cargo clippy --workspace --all-targets -- -W clippy::pedantic
cargo fmt --all -- --check
cargo run --quiet -p forge-core-cli -- validate --root . --json 2>/dev/null \
  | grep -c '"diagnostics": 0'                              # → 122
```

### Anchor preservation (sempre verde)
```bash
cargo run --quiet -p forge-core-cli -- validate --root . --json 2>/dev/null \
  | grep -c '"diagnostics": 0'
# → 122
```

---

## 13. Ordem de execução recomendada

```
1. F11.3 (Epic 1)           ★★★ próximo, Workflows 8→9, maior impacto
2. F11.4 (Epic 2)           ★★  consolida rastreabilidade
3. R6.4 (Epic 3)            ★★  Performativo 8→9, CI workflow
4. F13  (Epic 4)            ★   Features comunidade 9.5→10
5. F05/F06/F07/F08          ★   fecha P1 da comunidade
```

---

## 14. Workflow maduro por épico

Para cada épico:

1. **Desativar green-loop** no início (`mv pi-green-loop.json pi-green-loop.json.disabled`).
2. **Investigar primeiro** (ler arquivos relevantes, 80 linhas por `read_file`).
3. **Aplicar `improve-codebase-architecture`** no design (deletion test em cada
   módulo novo/modificado).
4. **Aplicar `grill-with-docs`** se termo novo surgir (sharpenar `CONTEXT.md`
   inline; criar ADR só se hard-to-reverse + surprising + real-tradeoff).
5. **Implementar story por story**, com `cargo check -p <crate>` entre cada.
6. **E2E testes** cobrem cada behavior novo.
7. **Documentação** (progress/, roadmap, README, CONTEXT.md se termo novo).
8. **Workspace test 1x** no fim + anchor check.
9. **Commit** com mensagem clara (formato `<TRACK>: <verbo> <objeto>`).
10. **Reativar green-loop** + commit da reativação.

---

## 15. Definition of Done — projeto 10/10

- [ ] Todas as 11 frentes com nota 9 ou 10.
- [ ] `cargo bench` roda sem erro, hot paths medidos.
- [ ] `cargo fuzz` em CI Linux (ADR-0008) sem panic.
- [ ] `cargo clippy --workspace --all-targets -- -W clippy::pedantic` < 100 warnings
      (baseline ~436, atualmente perto de 0 no lib code; restante em testes/benches).
- [ ] Zero `process::exit` em lib code (R8).
- [ ] Zero `Result<_, String>` novo (R2).
- [ ] Zero `serde_yaml` (R7).
- [ ] Zero material cripto sem `Zeroizing<>` (R5).
- [ ] Anchor `validate --json | grep -c 122` preservado.
- [ ] F01/F02/F03/F04/F11/F15 (P0) todos operacionais com fixtures.
- [ ] `forge preview` mostra plano+gates+rollback antes de mutar.
- [ ] `forge ready` unifica todos gates.
- [ ] `forge explain <run_id>` narra cronologicamente.
- [ ] Multi-agent: `agent_id` aparece em todo span quando setado.
- [ ] Sem script de novela: G1 + G2 fechados.
- [ ] Papers: F-sci completo, orientais + ocidentais representados.

---

## 16. Contatos e referências

- **Dono da codebase**: Daniel (PT-BR).
- **Repo**: `<repo-root>` (Rust workspace, 10 crates, edition 2021).
- **Toolchain**: WSL via Windows. Shims expõem `cargo` e `cargo.exe` — preferir `cargo`.
- **Docs críticos**:
  - `AGENTS.md` — convenções do projeto.
  - `CONTEXT.md` — glossário (só glossário).
  - `docs/dev-docs/forge-method-core-dev-docs-v2/progress/excellence_roadmap.md` — TODO master.
  - `docs/dev-docs/forge-method-core-dev-docs-v2/progress/f11_risk_audit_gate.md` — detail F11.
  - `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/` — 9 ADRs (último ADR-0009).
- **Skills**: `improve-codebase-architecture`, `grill-with-docs` — aplicar inline.
- **Warning inofensivo**: `bash: warning: setlocale: LC_ALL: cannot change locale ("en_US.UTF-8"): No such file or directory`
  aparece sempre. **Ignorar.**

---

**Fim do handoff. Próximo agente: leia tudo, depois comece pela Epic 1 (F11.3).**

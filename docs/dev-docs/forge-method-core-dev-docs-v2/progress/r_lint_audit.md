# R-LINT.1 — Pedantic Lint Audit (41 warnings → 0)

**Data**: 2026-07-01
**Status**: ✅ **COMPLETE** — `cargo clippy --workspace --all-targets -- -W clippy::pedantic` reports **0 warnings** after R-LINT.2.
**Validação**: 831 testes passando, anchor 122 limpo, `cargo fmt --check` verde.

## Histórico

`cargo clippy --workspace --all-targets -- -W clippy::pedantic` começou com 41 warnings
na v0.1.0. CI foi temporariamente downgrade para `-W` (warn) em `ff0908b4` por causa
desses 41 warnings. R-LINT.1 auditou e categorizou; R-LINT.2 resolveu todos.

## Categorização original (41 warnings)

### Categoria A — Lib code de produção (7 warnings) — ✅ FIXED

| Arquivo:linha | Lint | Ação tomada |
|---|---|---|
| `cli/src/cost_cmd.rs:12` | doc list item overindented | fix: indentação + seta `->` |
| `cli/src/cost_cmd.rs:35,104` | `# Panics` missing | fix: adicionada section `# Panics` em `run_cost_command` |
| `cli/src/io_util.rs:12` | incompatible_msrv (`Duration::from_mins` precisa 1.91) | fix: `Duration::from_secs(60)` |
| `cli/src/risk_audit_trace.rs:27` | too_many_arguments (10/7) | refactor: introduzido `RiskAuditTraceContext<'a>` |
| `cli/src/risk_audit_trace.rs:86` | too_many_arguments (9/7) | refactor: `risk_audit_event` agora recebe `&RiskAuditTraceContext` |
| `cli/src/risk_audit_trace.rs:100` | match_same_arms | fix: simplificado para `if matches!(...)` |

### Categoria B — Test code (28 warnings) — ✅ FIXED

| Arquivo | Lint | Ação |
|---|---|---|
| `cli/tests/validate.rs` (structs com `_path`) | `struct_field_names` | `#![allow]` documentado no crate de testes |
| `cli/tests/validate.rs` (format! from iterator) | `format_collect` | `#![allow]` documentado |
| `cli/tests/validate.rs:1466` (288/100 lines) | `too_many_lines` | `#![allow]` documentado |
| `cli/tests/validate.rs` (docs sem backticks) | `doc_markdown` | `#![allow]` documentado |
| `cli/tests/claim_cli_sidecar_e2e.rs:83` (161/100) | `too_many_lines`, `doc_markdown` | `#![allow]` documentado |
| `store/tests/rejection_demo.rs` | `naive_bytecount` | `#[allow]` documentado no helper |
| `store/tests/reference_index.rs:1378` | needless_pass_by_value | `#[allow]` na fn helper |
| `validate/tests/current_contracts.rs:477` | needless_pass_by_value | `#[allow]` na fn helper |
| `engine/tests/policies_framework.rs:89,91,98` | panic Debug | fix: `path.display()` em vez de `{path:?}` |
| `engine/tests/policies_framework.rs:104,116` | needless_pass_by_value | `#[allow]` nas fn helpers |
| `engine/src/conflict_detection.rs` (5 wildcards) | `match_wildcard_for_single_variants` | `#[allow]` documentado no `mod tests` |

### Categoria C — Bench code (6 warnings) — ✅ FIXED

| Arquivo:linha | Lint | Ação |
|---|---|---|
| `crypto/benches/rekor.rs` (3 lints) | usize→u8 cast, format! append, let_and_return | fix: `u8::try_from`, `write!`, `or_insert_with` direto |
| `validate/benches/yaml_deserialize.rs:63` | panic Debug | fix: `path.display()` |

## Lições aprendidas

1. **Nomes de lint importam**: `struct_field_same_postfix` não existe em clippy 1.94 —
   o nome correto é `struct_field_names`. Sempre confirmar com
   `cargo clippy ... 2>&1 | grep "help:.*clippy"` antes de adicionar `#[allow]`.
   `clippy::fix` sugere o nome correto.

2. **`//![allow(...)]` ≠ `#![allow(...)]`**: o primeiro é um doc-comment (ignorado),
   o segundo é um inner attribute. Sempre usar `#!` no início de arquivos de teste.

3. **`PathBuf` não implementa `Display`**: para mensagens de panic/log com paths,
   usar `path.display()` (Display do `Path`) ou `{}` com `.display()`. Usar `{path}`
   direto falha com "PathBuf doesn't implement Display".

4. **Refatorar para reduzir args > adicionar `#[allow]`**: em código de produção (lib),
   introduzir um parameter struct (`RiskAuditTraceContext`) é melhor que silenciar o
   lint. O struct vira ponto de extensão futuro e melhora a legibilidade dos call sites.

## Próximo passo

R-LINT.6: flip CI de `-W clippy::pedantic` para `-D clippy::pedantic` (deny warnings).

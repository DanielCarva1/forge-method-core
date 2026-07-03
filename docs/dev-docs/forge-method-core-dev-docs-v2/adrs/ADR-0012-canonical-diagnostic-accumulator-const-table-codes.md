# ADR-0012 - Acumulador de diagnósticos canônico + const-table de códigos

- **Status**: Accepted (V1.B foundation implementada — `DiagnosticCodeDef`, `declare_diagnostic_code!`, `DiagnosticRegistry`; V2.B migrou 4 famílias: graph/eval/eval-harness/CLI para o `ValidationReport`/`Diagnostic` canônico)
- **Date**: 2026-07-02
- **Track**: V1.B / V2.B — canonical diagnostic accumulator
- **Supersedes**: none
- **Superseded by**: none

## Contexto

Haviam **cinco famílias de tipos de diagnóstico clonadas** no workspace:
`forge-core-validate` (`ValidationReport`/`Diagnostic`/`DiagnosticCode`),
`forge-core-graph` (`GraphValidationReport` com seu próprio `error_count`),
`forge-core-eval`, `forge-core-eval-harness`, e a CLI. Cada uma declarava sua própria
estrutura de severidade/código/mensagem e suas próprias contagens.

Pior: o `DiagnosticCode` canônico era um enum de ~90 variantes, e na fronteira do CLI ele era
**degradado para `format!("{:?}")`** — um `Debug` string. O resultado eram wire-strings feias
(`YamlReadFailed`) em vez de identificadores estáveis (`yaml_read_failed`), e a informação de
tipo se perdia ao cruzar a fronteira JSON/MCP. Todo site de consumo (MCP, agent) tinha que
re-parsear strings ou perder o código.

A pesquisa nos maiores linters Rust e no rust-analyzer mostrou convergência clara:

- **rustc** (`DiagCtxt`): accumulate-then-`abort_if_errors`, com phase gates. Os metadados
  de lint são uma **tabela `static`** (struct `Lint`), não um enum variant carregando sua
  própria descrição/severidade.
- **`rust-analyzer`'s own source** *recomenda contra* um enum strongly-typed para
  diagnósticos: acopla o conjunto de diagnósticos conhecidos a uma unidade de compilação
  única e torna diagnósticos externos/config-driven impossíveis.
- **`clippy` / `deno_lint` / `dprint`**: nenhum usa enum para `code`. Todos declaram codes
  como uma tabela `const`/`static` de identificadores `&'static str`, com severidade/categoria
  como **dados** ao lado do id, emitidos por uma macro de declaração
  (`declare_clippy_lint!`, `deno_lint::declare_deno_lint!`).

## Decisao

O `forge-core-validate`'s `Diagnostic` / `ValidationReport` tornam-se o **canon**. As outras
quatro famílias migram para ele (V2.B).

### 1. `DiagnosticCodeDef` const-table + `declare_diagnostic_code!`

A entrada da const-table é uma struct de `&'static str` + severidade default, modelada no
`Lint` do rustc e na abordagem `clippy`/`deno_lint`:

```rust
pub struct DiagnosticCodeDef {
    pub code: &'static str,           // "memory_authority_floor" — wire format estável
    pub description: &'static str,
    pub category: &'static str,       // "memory", "graph", "risk-audit"
    pub default_severity: DiagnosticSeverity,
}
```

A macro `declare_diagnostic_code!` emite um `pub static <NAME>: DiagnosticCodeDef` por linha
— espelha `declare_tool_lint!` do rustc e `declare_clippy_lint!`. O `$sev` é um nome de
variante não-qualificado (`Error`/`Warning`) para o call site ler como a decl de lint-level
do rustc. **Não** é proc-macro: `macro_rules!`, zero build cost.

O wire-format `code` é o `&'static str` snake_case estável desde o início — nunca o `Debug`
de um variant. Isto mantém a informação de tipo através da fronteira JSON/MCP.

### 2. `DiagnosticRegistry` — o lookup seam

Um registry construído a partir de um slice `&'static [&'static DiagnosticCodeDef]`,
const-constructable, read-only. Consumers query por code string → metadata. Isto substitui
o `match`-on-enum gigante que forçava o `format!("{:?}")` na fronteira do CLI.

### 3. `ValidationReport::abort_if_errors` — o phase-gate

O `ValidationReport` ganha `abort_if_errors(self) -> Result<(), Self>` modelado no
`DiagCtxt::abort_if_errors` do rustc: roda a fase inteira, acumula *todos* os diagnósticos,
depois decide parar. Ao contrário de short-circuit no primeiro erro, deixa a fase completa
rodar e coletar todo problema antes de parar. Consome `self` para o success path poder drop
o report vazio/warnings-only.

### 4. Migração sem flag-day

As 4 clones migram via **type aliases + impls de `From`**, não com uma flag-day. O
`GraphValidationReport` (com seu `error_count` próprio) vira um alias para o
`ValidationReport` canônico; `error_count`/`warning_count` foram adicionados ao canônico
para preservar os contadores. A regra acumuladora de `AGENTS.md` ("acumular todos os
diagnósticos antes de parar") passa a ser enforced em um único seam.

## Consequencias

**Positivas:**

- `DiagnosticCode` para de ser stringified nas fronteiras. Um consumidor programático (MCP,
  agent) recebe um `code` estável como `yaml_read_failed`, não `format!("{:?}")`.
- A regra acumuladora de `AGENTS.md` é enforced em um seam: `ValidationReport::extend` +
  `abort_if_errors`. Quatro clones de `has_errors` viram uma.
- Future config-driven severity overrides (o modelo lint-level de ESLint/rustc: um code
  declara seu nível *default*, e um consumer config pode promote/demote) têm um lookup seam
  pronto — o `DiagnosticRegistry`. Uma config entry `codes: { memory_authority_floor: warn }`
  resolve pelo registry.
- O enum de ~90 variantes e a const-table coexistem (a const-table é foundation aditiva);
  V2.B migra callers e os variants do enum encolhem à medida que cada um migra para a
  const-table correspondente.

**Negativas:**

- Dois sources de verdade (enum de ~90 variants + const-table seed) coexistem até a migração
  completa. Mitigação: o seed é aditivo, o enum é untouched até V2.B; V2.B deleta variants
  conforme migra cada caller para a const entry correspondente.
- `DiagnosticCodeDef` precisa que novos codes sejam declarados **e** adicionados ao slice do
  registry (`SEED_ENTRIES`). Esquecer o último deixa o code não-lookupable. Mitigação: um
  teste itera o registry e verifica que todo `code` declarado resolvível aparece.

## Anti-objetivos

- **Não** substitui o enum existente na V1.B (foundation aditiva); a migração é V2.B,
  caller-a-caller.
- **Não** introduz config-driven severity nesta ADR — só provê o seam de lookup
  (`DiagnosticRegistry`) que a tornará possível depois.
- **Não** é proc-macro: `declare_diagnostic_code!` é `macro_rules!`.

## Referencias

- rustc `DiagCtxt` (accumulate-then-`abort_if_errors`, phase gates):
  https://doc.rust-lang.org/nightly/nightly-rustc/rustc_errors/diagnostic.html
- rust-analyzer `AnyDiagnostic` self-critique (recomendação contra enum strongly-typed para
  diagnostics).
- `clippy` `declare_clippy_lint!` (const-table): https://github.com/rust-lang/rust-clippy
- `deno_lint` const-table: https://docs.rs/deno_lint
- `garde::Report` (acumulador canônico de validação): https://docs.rs/garde
- `jsonschema` `OutputFormat::Basic` (report canônico): https://docs.rs/jsonschema
- In-repo: ADR-0001 (kernel determinístico, validação é decisão pura),
  `crates/forge-core-validate/src/{lib.rs (ValidationReport/abort_if_errors), codes.rs}`.

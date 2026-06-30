# F15 — Rust ergonomics + codegen track

## Acceptance criterion

> "novo comando/contrato não exige editar >2 pontos manuais fora de tests/docs"

Hoje um novo comando exige **6+ edit points manuais**:

1. `main.rs`: `use` import
2. `main.rs`: match arm em `dispatch()`
3. `lib.rs`: `pub mod X_cmd;`
4. `cli_util.rs`: linha em `usage()` concat
5. `cli_util.rs`: `<cmd>_usage()` fn nova
6. `X_cmd.rs`: `next_<cmd>_value_or_err` duplicado
7. `X_cmd.rs`: `while index < args.len() { match... }` skeleton

Meta: reduzir a **2 edit points** — (1) criar o módulo do comando, (2) registrar na tabela.

## Skills aplicadas (inline)

- `improve-codebase-architecture`: deletion test em cada helper, glossário deep/shallow/seam.
- `grill-with-docs`: terminology gate — "ArgvCursor" é detalhe de impl (NÃO vai pro CONTEXT.md); "CommandSpec" idem.

## Deepening opportunities identificadas

### C1: `ArgvCursor` (deep module)

**Files**: `crates/forge-core-cli/src/cli_util.rs` + todos `*_cmd.rs`.

**Problem**: Cada command repete o skeleton `while index < args.len() { match args[index].as_str() { "--flag" => { index += 1; x = next_X_value_or_err(args, index, "flag")?; } } index += 1; }`. O helper `next_X_value_or_err` é copiado por command só pra embeddar o nome do command na mensagem de erro. Deletion test: deletar `next_telemetry_value_or_err` forçaria 7 inlinings em `parse_telemetry_export_args` — complexity reaparece em N callers → **shallow per-command, mas o padrão é o problema**.

**Solution**: Tipo `ArgvCursor<'a>` com interface pequena (`new`, `peek_flag`, `expect_value`, `consume_flag`, `exhausted`) e implementação que encapsula bounds-check, dash-rejection, formatação de erro com nome do command. Deep module: muita lógica atrás de 4 métodos.

**Benefits**:
- Locality: todo argv-walking numa única implementação. Bug em dash-rejection consertado 1x.
- Leverage: novo command = match nos flags, sem boilerplate de index/bounds.
- Test surface: `ArgvCursor` vira a unit-test surface; commands ficam trivialmente testáveis.

### C2: Command registry (deep module)

**Files**: `crates/forge-core-cli/src/main.rs` + `crates/forge-core-cli/src/lib.rs` + `crates/forge-core-cli/src/cli_util.rs`.

**Problem**: `dispatch()` em `main.rs` é um match de 90 linhas, um arm por command. `usage()` é um `concat!` gigante hand-editado. Adicionar command = tocar 4 pontos (use, match arm, mod decl, usage line).

**Solution**: `const COMMANDS: &[CommandSpec]` array onde cada entry declara `name`, `usage_line: &'static str`, `handler: fn(&[String]) -> Result<(), ExitError>`. `dispatch()` vira lookup linear. `usage()` vira join dos `usage_line`s. Sem macros, sem derive — só const array + fn pointer (compatível com AGENTS.md "SEM clap/derive").

**Benefits**:
- Locality: registro do command num único lugar.
- Leverage: novo command = 1 linha no array.
- Reduz edit points de 4 (dispatch+mod+usage+use) para 1 (entrada no array).

### C3/C4: sub-pieces de C1/C2

- C3 (co-localizar usage) é absorvido por C2: cada command module expõe `pub const USAGE: &str`.
- C4 (deletar `next_X_value_or_err`) é absorvido por C1: foldado no `ArgvCursor`.

## Plano de execução

| Step | O quê | Critério de aceite |
|---|---|---|
| F15.1 | Criar `ArgvCursor` em `cli_util.rs` + migrar 1 command piloto (`telemetry_cmd`) | `cargo test -p forge-core-cli` verde; anchor 122; telemetry tests passam sem alteração de semântica |
| F15.2 | Migrar commands restantes para `ArgvCursor` | workspace test verde; anchor 122; clippy verde |
| F15.3 | Deletar `next_<cmd>_value_or_err` obsoletos | Sem refs restantes; workspace test verde |
| F15.4 | Criar `CommandSpec` registry + refatorar `dispatch()` e `usage()` | dispatch() vira lookup; usage() vira join; anchor 122 |
| F15.5 | Mover `USAGE` para cada command module | cli_util.rs perde `<cmd>_usage()` fns; anchor 122 |
| F15.6 | Validar critério F15: simular add command novo, contar edit points | ≤2 edit points documentados |
| F15.7 | Features P0 paralelas: `--no-sync` flag pra WAL (requer ADR) | Ganho 25-50ms/append; opt-in |

## Estado

- [ ] F15.1 — ArgvCursor + piloto
- [ ] F15.2 — migrate rest
- [ ] F15.3 — delete dupes
- [ ] F15.4 — registry
- [ ] F15.5 — co-localize usage
- [ ] F15.6 — validate criterion
- [ ] F15.7 — --no-sync WAL flag

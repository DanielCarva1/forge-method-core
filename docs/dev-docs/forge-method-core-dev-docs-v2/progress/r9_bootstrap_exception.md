# R9 — Bootstrap Core Exception — **COMPLETE**

Branch: `codex/forge-frust-052-ocsp-boundary`
Started: 2026-06-30
Completed: 2026-06-30
Status: **complete** (exception é explícita, opt-in, com cobertura E2E fechada e consumer repo limpo opera clean sem ela)

## Goal

Garantir que a **Bootstrap Core Exception** — exceção temporária que permite
`D:\Forge-method-core` manter `.forge-method/` local enquanto Forge desenvolve a
si mesmo — seja explícita, opt-in, jamais propagada para Consumer Project Repos,
e que um consumer repo limpo opere end-to-end (init → state-bearing commands)
**sem** precisar da exceção.

A exceção é formalmente definida em `CONTEXT.md` → "Bootstrap Core Exception"
e referenciada em `09_system_design_roadmap.md` (linha 70) e
`08_priority_recommendations_plan.md` (linha 51) como a trilha R9 ↔ F12
(Guided Start).

## Result

- A exceção vive atrás de um gate **opt-in** (`--allow-bootstrap-core`).
- Consumer Project Repos operam clean via `forge-core project init` + sidecar
  state sem nunca tocar a exceção.
- Censo E2E confirma cobertura completa: init, resolve, hardening, state-bearing
  writes (execute-operation), state-bearing reads (claim status, query-effect-index),
  rebuild-effect-index, fail-closed paths.
- Interpretação ambígua em `excellence_roadmap.md` foi sharpen alinhando com
  `CONTEXT.md` (fonte de verdade).

## Approach

R9 é uma trilha predominantemente de **documentação/verificação**, não de
código novo. O mecanismo já estava implementado em `crates/forge-core-cli/src/project_cmd.rs`
e coberto por tests E2E. A execução de R9 formaliza o estado e corrige a
interpretação ambígua que havia entre o título do item do roadmap e sua
descrição.

### Passos executados

1. **Sharpen interpretação (grill-with-docs)** — `excellence_roadmap.md`
   trazia título "Remover Bootstrap Exception" mas descrição sobre "docs
   humanos → agentes", que são coisas diferentes. A descrição correta é a
   alinhada com `CONTEXT.md` (Bootstrap Core Exception temporária, opt-in,
   consumer não copia).
2. **Censo de implementação** — confirmado que o gate está correto:
   - `is_bootstrap_core_root()` retorna true só quando o root tem `Cargo.toml`
     + `crates/forge-core-cli/` + `.forge-method/`.
   - `resolve_project()` só retorna `BootstrapCoreLocal` se `allow_bootstrap_core`
     AND `is_bootstrap_core_root` forem ambos verdadeiros.
3. **Censo de cobertura E2E** — inventariados todos os tests relevantes
   (ver seção Inventory abaixo). Confirma que consumer repo fresh-init opera
   clean end-to-end sem `--allow-bootstrap-core`.
4. **Deletion test (improve-codebase-architecture)** — `is_bootstrap_core_root()`
   tem 1 caller (`resolve_project`). Se deletada, a regra "esse repo é o Forge
   core?" vaza para `resolve_project` como 3 file-existence checks inline.
   **Mantém**: concentra a regra, deep module.
5. **Atualização de roadmap** — R9 marcado `[x]`, score Docs/rastreabilidade
   8 → 10, lacuna "falta R9 Bootstrap" removida.

## Inventory — Cobertura E2E

22 tests distribuídos em 4 arquivos comprovam que a exceção é isolada e que
consumer repos operam clean:

### `tests/project_init_e2e.rs` (8 tests)

| Test | Garante |
|---|---|
| `project_init_creates_project_link_and_sibling_sidecar_only` | init cria link + sidecar, **não** cria local state |
| `claim_status_after_project_init_uses_sidecar_claim_bus` | state-bearing read opera via sidecar após init |
| `project_init_is_idempotent_for_same_root` | init idempotente |
| `project_init_rejects_preexisting_consumer_local_state_without_creating_link` | init rejeita local state pré-existente |
| `project_init_rejects_conflicting_existing_project_link_without_overwrite` | init rejeita link conflitante |
| `project_init_accepts_custom_external_sidecar_and_state_roots` | init aceita custom roots |
| `project_init_rejects_custom_state_root_without_dot_forge_method_leaf` | init rejeita state_root sem leaf `.forge-method` |
| `project_init_rejects_consumer_local_state_root_without_creating_local_state` | init rejeita consumer-local state_root |
| `project_init_missing_root_fails_clearly` | init rejeita missing root |

### `tests/project_resolve_e2e.rs` (4 tests)

| Test | Garante |
|---|---|
| `project_resolve_finds_sidecar_via_project_link` | resolve normal via link |
| `project_resolve_accepts_utf8_bom_project_link` | resolve aceita BOM |
| `project_resolve_without_link_fails_closed_for_consumer_repo` | consumer sem link → exit 5, fail closed |
| `project_resolve_allows_core_bootstrap_exception_explicitly` | bootstrap exception funciona **só** com flag; asserts `layout=bootstrap_core_local`, `bootstrap_core_exception=true` |

### `tests/project_link_hardening_e2e.rs` (relevantes)

| Test | Garante |
|---|---|
| `resolve_rejects_consumer_local_state_root_without_bootstrap_exception` | resolve rejeita consumer-local state_root sem bootstrap |
| `claim_status_rejects_missing_resolved_state_root_and_does_not_create_local_state` | claim status não cria local state mesmo se sidecar falta |

### `tests/operation_sidecar_e2e.rs` (state-bearing writes/reads via sidecar)

| Test | Garante |
|---|---|
| `execute_operation_rejects_outside_root_operation_path_before_read` | execute-operation rejeita path fora root |
| `execute_operation_rejects_outside_root_command_path_before_read` | execute-operation rejeita command path fora root |
| `execute_operation_rejects_outside_root_effect_path_before_read` | execute-operation rejeita effect path fora root |
| `execute_operation_writes_command_evidence_to_sidecar_not_consumer_repo` | state-bearing WRITE vai pro sidecar, não consumer repo |
| `execute_operation_writes_forge_state_to_sidecar_not_consumer_repo` | state-bearing WRITE (forge state) vai pro sidecar |
| `rebuild_and_query_effect_index_default_to_resolved_sidecar_state` | rebuild + query effect index default sidecar |
| `state_bearing_commands_fail_closed_without_project_link` | state-bearing commands fail closed sem link |

## ADR gate

Avaliação conforme grill-with-docs:

- **Hard to reverse?** Não. A correção é um sharpen de descrição ambígua em doc.
- **Surprising without context?** Parcial — alguém lendo `excellence_roadmap.md`
  pode notar a mudança, mas é um sharpen alinhado com `CONTEXT.md`.
- **Real trade-off?** Não. A interpretação B já era a operacional; só a
  descrição do roadmap estava divergente.

2 dos 3 critérios ausentes → **não cria ADR**. A decisão fica registrada
neste doc de progresso e no histórico do commit.

## Pontos de atenção futuros

- A exceção é **temporária**: o objetivo de longo prazo é o Forge operar a si
  mesmo também via sidecar, eliminando totalmente a necessidade do flag.
  Isso depende de F12 (Guided Start) amadurecer o suficiente para que o
  Forge core repo possa ser tratado como consumer.
- `is_bootstrap_core_root()` é uma heurística de 3 file-existence checks
  (`Cargo.toml` + `crates/forge-core-cli/` + `.forge-method/`). Se a estrutura
  do repo mudar (ex: rename de `forge-core-cli`), o detector quebra silenciosamente.
  Não é um problema hoje, mas vale monitorar quando F12 evoluir.

## Referências

- `CONTEXT.md` linhas 21-23 (definição formal) e 36-39 (Remaining Bootstrap Gaps)
- `crates/forge-core-cli/src/project_cmd.rs:36` (`bootstrap_core_exception: bool`)
- `crates/forge-core-cli/src/project_cmd.rs:23` (`ProjectLayoutKind::BootstrapCoreLocal`)
- `crates/forge-core-cli/src/project_cmd.rs:861-896` (`resolve_project` com gate)
- `crates/forge-core-cli/src/project_cmd.rs:1016-1020` (`is_bootstrap_core_root`)
- `crates/forge-core-cli/src/project_cmd.rs:1121-1147` (parsing `--allow-bootstrap-core`)
- `README.md:191-303` (promessa pública sobre init/resolve/bootstrap)
- `docs/dev-docs/forge-method-core-dev-docs-v2/09_system_design_roadmap.md:70`
- `docs/dev-docs/forge-method-core-dev-docs-v2/08_priority_recommendations_plan.md:51`

# Excellence Roadmap — forge-method-core

**Estado:** v0.1.0 quick-wins landed (integrity, CLI/UX, docs consistency).
Este documento mapeia o trabalho restante para "100% e excelente" que **não
cabe numa sessão**. Cada item tem contexto, arquivos, estimativa e gate de
aceite. Trabalhe um item por sessão; commit; próxima.

> **Autoria:** Este roadmap foi produzido após uma auditoria técnica brutal
> (dois agents exploraram dívida técnica + UX de primeiro-contato). Os itens
> abaixo são os genuinamente pendentes, priorizados por impacto.

---

## Prioridades em resumo

| # | Item | Impacto | Esforço | Risco |
|---|------|---------|---------|-------|
| 1 | `derive_state` layer (v0.2) | Alto — fecha o gap real de roadmap | Alto (2-3 sessões) | Médio |
| 2 | Migrar 5 `Result<_, String>` | Médio — consistência + safety | Baixo (1 sessão) | Baixo |
| 3 | Cobertura de testes (4 crates) | Alto — governance/MCP sem testes | Alto (3-4 sessões) | Baixo |
| 4 | First-use skill wiring | Médio — onboarding automático | Baixo (1 sessão) | Baixo |
| 5 | Consolidação física dos ADRs | Baixo — cosmético | Baixo (1 sessão) | Baixo |
| 6 | Parsers granulares (B5) | Médio — erros acionáveis | Médio (1 sessão) | Baixo |

---

## 1. `derive_state` layer (o gap real de v0.2) — ✅ LANDED

**Status:** concluído em 3 commits (`f94eac45`, `d8a36c1d`, `d8a36c1d`+tests).

**O que landed:**
- `crates/forge-core-store/src/derive_state.rs` — o único construtor de
  autoridade para claim state. Enrola a projeção já existente
  (`replay_claim_wal`) e incorpora a dança de auto-repair de torn-tail que
  vivia inline em `claim.rs`.
- `load_claims()` em `claim.rs` agora roteia por `derive_state` internamente
  (zero churn nos 7 call sites: acquire/heartbeat/release/handoff/status/
  reconcile/check-write + graph_cmd.rs migraram transparentemente).
- `forge-core claim status --from-cache` adicionado (debug/diagnóstico, lê o
  YAML legado; spec AC5).
- 5 testes novos provam os ACs: tamper-fail-closed (ac1/ac4),
  cache-mutation-inert (ac7), from-cache-flag (ac5).
- Toda a rede de regressão verde: 66 store + 204 CLI lib + 22 claim E2E.

**O que NÃO landed (follow-up opcional):**
- Snapshot/rotation como cache de leitura (P3.3, "later perf layer" no spec).
- Tipo opaco `ClaimState` com seal compile-time (defense-in-depth, opção b).

---

## 1-OLD (arquivo histórico — substituído por ✅ acima)

**Contexto.** Hoje o estado de coordenação é reconstruído lendo os YAMLs de
claim a cada invocação (`load_claims()` em `claim.rs:823`). O WAL
(`.forge-method/wal/claims.fmw1`) já é a autoridade para mutação, mas a
leitura ainda faz replay completo em cada chamada. A spec
`contracts/spec/claims-integrity-spine-spec.yaml:56` manda existir um
`crates/forge-core-store/src/derive_state.rs` como **único construtor de
estado** — ele **não existe**.

---

## 2. Migrar 5 `Result<_, String>` (AGENTS.md manda)

**Contexto.** AGENTS.md:24 proíbe novos `Result<_, String>` e manda migrar os
existentes quando tocados. Há 5 offenders vivos (o resto dos hits são
doc-comments documentando a migração — bons).

**Os 5 sites:**
1. `crates/forge-core-store/src/lib.rs:1574` —
   `parse_effect_wal_records_for_recovery() -> Result<(Vec<_>, Vec<String>), String>`
   com `return Err(format!(...))` na linha 1592. **O pior:** String error num
   caminho estrutural de recovery.
2. `crates/forge-core-cli/src/mcp_cmd.rs:77` — `parse_serve_args() -> Result<ServeArgs, String>`
3. `crates/forge-core-cli/src/research_cmd.rs:799` — `load_evidence() -> Result<FieldEvidenceRegistry, String>`
4. `crates/forge-core-protocol-mcp/src/attestation.rs:238` — `hex_decode() -> Result<Vec<u8>, String>` (security-adjacent: ed25519)
5. `crates/forge-core-protocol-mcp/src/server.rs:379` — `Option<Result<AttestationInput, String>>`

**Padrão a seguir.** Definir um enum error nomeado por operação (Derive
`Debug, Clone, PartialEq, Eq`), ao lado da função. Espelhar
`AppendJsonLineError` / `ReferenceIndexBuildError` em `forge-core-store`.
Converter com `.map_err(NamedError::from)` ou `From` impl no boundary.

**Gate de aceite.** Zero `Result<_, String>` em `crates/*/src/` (grep
confirma); `cargo clippy --workspace --all-targets -- -W clippy::pedantic`
verde; `cargo test --workspace` verde.

**Estimativa.** 1 sessão. Cada site é isolado.

---

## 3. Cobertura de testes — 4 crates sem testes

**Contexto.** O spine é bem testado (store, validate, decisions, kernel,
cli têm suites E2E + unit). Mas 4 crates têm lógica real e zero testes:

| Crate | LOC | Risco | Por que dói |
|-------|-----|-------|-------------|
| `forge-core-governance` | 1447 | Alto | Lógica de arbitrate/escalate/record — governança sem prova |
| `forge-core-eval-harness` | 1371 | Alto | Referenciado por ADR-0023; decide baseline vs candidate |
| `forge-core-research` | 1025 | Médio | Admission/graph; tem `proptest` dev-dep mas 0 testes |
| `forge-core-protocol-mcp` | 2016 | Alto | Security-sensitive: ed25519 attestation, allowlist; só 3 smoke tests |

**Abordagem.** Um crate por sessão. Comece pelo `forge-core-protocol-mcp`
(security-sensitive tem prioridade). Para cada:
1. Testes unitários nos módulos puros (`allowlist.rs`, `attestation.rs`).
2. Testes E2E em `tests/` usando `assert_cmd` no binário `forge-core mcp serve`.
3. Para governance/eval-harness/research: property tests com o `proptest` já
   no dev-deps.

**Gate de aceite.** Cada crate tem ≥1 teste E2E + cobertura unitária nos
caminhos críticos; `cargo test -p <crate>` verde.

**Estimativa.** 3-4 sessões (1 por crate, MCP pode precisar de 2).

---

## 4. First-use skill wiring

**Contexto.** `skill/forge-method/SKILL.md` documenta `project resolve` para
repos já linkados mas **nunca chama `forge-core project init`** para um repo
sem link. O `start` command agora emite o `next_step` correto (Fase B B4
landed), mas o skill não o consome. Um repo novo fica sem bootstrap
automatizado.

**Arquivo.** `skill/forge-method/SKILL.md` (Step 0).

**Abordagem.** Em "Step 0", quando o skill detecta que não há
`.forge-method.yaml`, rodar `forge-core start --root .` e seguir o
`next_step.command` retornado (que é `project init`). Isto fecha o loop:
start já dá a resposta certa, o skill só precisa executá-la.

**Gate de aceite.** Um repo virgem + skill instalado → skill roda `start` →
segue `next_step` → `project init` → estado linkado, sem intervenção humana.

**Estimativa.** 1 sessão.

---

## 5. Consolidação física dos ADRs

**Contexto.** A colisão de numeração foi resolvida (Fase 3 landed: Registry A
em `docs/adr/` 0022-0024, Registry B em `docs/dev-docs/.../adrs/` 0001-0014,
documentado em `docs/adr/README.md`). Mas os 14 ADRs de Registry B estão
fisicamente separados dos 3 de Registry A. Mover todos para `docs/adr/`
unifica o registro fisicamente.

**Arquivo.** `git mv docs/dev-docs/forge-method-core-dev-docs-v2/adrs/*.md docs/adr/`

**Risco.** Baixo. Os ADRs são citados por número (não por path) em código.
Mas verificar: grep por `dev-docs/.../adrs/` em `crates/` e `contracts/` —
se houver path refs, atualizar.

**Gate de aceite.** Todos os ADRs em `docs/adr/`; `docs/adr/README.md`
atualizado para refletir uma só localização; nenhum path ref quebrado.

**Estimativa.** 1 sessão (cosmético).

---

## 6. Parsers granulares (B5 do plano CLI/UX)

**Contexto.** Os 10+ helpers `parse_*_or_err` em
`crates/forge-core-cli/src/cli_util.rs` (linhas 196-405) devolvem o usage
dump de 10KB para um valor de enum inválido. Ex: `--target-kind foo` devolve
toda a usage em vez de `"unknown --target-kind 'foo'; expected
file_path|glob|state_key|..."`.

**Arquivo.** `crates/forge-core-cli/src/cli_util.rs`.

**Abordagem.** Para cada parser de enum, listar as variantes válidas na
mensagem de erro. O helper genérico `parse_strict_or_err` (linha 405) já faz
isso parcialmente — espelhar o padrão.

**Gate de aceite.** Cada enum inválido mostra as opções válidas; nenhum
parser devolve o usage dump global para um erro de valor único.

**Estimativa.** 1 sessão.

---

## Não-ítens (decisões tomadas, não reconsiderar)

- **110 workflows** ficam. Cada produto usa um punhado; o catálogo largo é
  intencional (atende gama maior de produtos).
- **Repo URLs** (Stable-Studio/forge-method-rust é o canonical no README e
  SKILL; DanielCarva1/Forge-method-core é o fork pessoal open-source). Ambos
  existem.
- **WAL de claims** é append-only por design (audit log). Não truncar.

## Histórico desta sessão

Ver `git log` entre `1ebcdc06` (Fase A) e `d9dbe1d9` (Fase C). As 4 fases
landed:
- **A:** 89 claims órfãos limpos + AGENTS.md handoff removido + 12 ponteiros
  reparados.
- **B:** CLI/UX — `--version`, no-args→help, `--help` framing, `start`
  no_link guidance, unknown-command diagnosis, `--no-sync` stderr em JSON.
- **C:** README MCP corrigido, VERSION alinhado, inventory reescrito,
  `--json` consistency, SKILL URL.
- **D:** este documento.

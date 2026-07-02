# Handoff Consolidado — forge-method-core (2026-07-02)

**Leia este documento primeiro. Ele substitui e supersede os handoffs por-epic
(`handoff_f08_*.md`, `handoff_f12_*.md`, `handoff_f14_*.md`,
`handoff_master_parallelization.md`) e o `merge-plan.yaml` como fonte da
verdade.** Aqueles foram úteis durante a paralelização multi-agente; agora que
voltamos a um único agente, este é o ponto de partida canônico.

> **ATUALIZADO 2026-07-02 (pós-merge F08)**: F08 foi mergeado em master via
> rebase + ff. As seções §2 e §6 refletem o estado real reconciliado. **Nota de
> reconciliação**: a versão original deste handoff dizia que F08.6/F08.7
> faltavam — isso estava incorreto; F08 estava 100% pronto e foi mergeado
> integralmente (commits `ebb39cfd`→`0711d3de`). O `start_cmd.rs` fmt fix
> (`0711d3de`) é correção incidental de uma regressão pré-existente do F12.2.

---

## 1. O que é este projeto

forge-method-core: kernel Rust agente-nativo (protocolo/governança) que
deixa múltiplas pessoas e agentes IA construírem o mesmo produto juntas.
`v0.1.0` já lançado publicamente (Apache-2.0, 5 binários, CI verde).
Workspace de 15 crates (14 no `members` + `forge-core-governance`), 9 ADRs,
CI com `-D clippy::pedantic`.

**Norte estratégico** (do `excellence_roadmap.md`): rápido, robusto,
performativo, protocolo-guia que escala com a capacidade dos agentes, nunca
script de novela, sempre Rust ou compatível, sempre lastreado em melhores
práticas e papers científicos.

---

## 2. Estado atual — 2026-07-02

### Topologia git
```
D:/Forge-method-core  0711d3de [master]              ← worktree principal (F08 mergeado)
D:/forge-f08          0711d3de [f08-mcp-adapter]     ← worktree F08 (= master pós-ff, descartável)
D:/forge-f14          2e3c86ba [f14-knowledge-orchestration] ← worktree F14 (NÃO mergeado)
```
- **Master** está saudável: `cargo check` ✅, `clippy -D pedantic` ✅ (zero
  warnings, gate do CI), `cargo fmt --check` ✅, anchor 122 ✅, `cargo test
  --workspace` ✅ (stress test `claim_wal_stress` é flaky sob carga — passa
  isolado; pré-existente, não relacionado a F08).
- **14 commits à frente de `origin/master`** (NÃO pushed — decisão do usuário:
  esperar).
- **Stashes**: vazio.

### O que já está no master (Features comunidade → ~9.9/10)
- ✅ **F06** (memory policy completo — gates + PEP + CLI + E2E)
- ✅ **F07** (multi-principal governance — PrincipalId + ConflictContract + arbitration ledger + CLI + E2E)
- ✅ **F12** (Guided Start — `forge-core start` read-only + 5 estados de bootstrap + fixtures + E2E)
- ✅ **R-LINT.6** (limpeza de lints pedantic que o gate expôs)
- ✅ **F08** (Secure MCP adapter — crate `forge-core-protocol-mcp` + ADR-0006
  Accepted + rmcp 2.0 stdio server + Allowlist/MutateGate/Attestation gates +
  CLI `mcp serve` + 58 unit + 8 E2E tests. **Mergeado nesta sessão via
  rebase+ff**.)

### O que está em andamento (worktree isolado, NÃO mergeado)

#### F14 — Knowledge Orchestration (worktree `D:/forge-f14`, branch `f14-knowledge-orchestration`)
**Estado: 2 stories commitadas (F14.1 + F14.2); working tree limpo.**

Commits:
- `6b8b0378` F14.1 — grill + ADR-0010 (proposto) + story breakdown
- `c3aa2473` F14.1 — glossário research (ResearchSource/Source Ledger/SourceId/Citation/etc.)
- `4b89e454` F14.1 — handoff F14 + merge-plan (auto-organização)
- `72a2cf19` merge-plan: REGRA ZERO
- `2e3c86ba` F14.2 — crate `forge-core-research` + `ResearchSource` contract + PEP (ADR-0010 accepted)

**Base do F14**: `3e9f9abb` (F07 fechado) — **está atrás do master atual
(`0711d3de`)**. Precisa rebase sobre master antes do merge. Overlap mecânico
esperado: `Cargo.toml` `members` (F14 adiciona `forge-core-research` na mesma
linha onde F08 adicionou `forge-core-protocol-mcp` — segundo a chegar conflita;
resolução: manter ambos os member entries). F14 **não** toca
`command_registry.rs`, `lib.rs`, nem `validate/lib.rs`, então o único conflito
de merge será `Cargo.toml` members.

**Falta fazer em F14** (ver `followups_v0_1_to_10.md` F14.3-F14.7 — numeração
autoritativa pós-grill; o handoff F14 por-epic tem numeração mais velha):
- **F14.3** — Citation Check validator em `forge-core-validate`: estender
  `validate_yaml_source_id_references` (lib.rs:280) p/ resolver `source_id`
  contra `FieldEvidenceRegistry ∪ ResearchProjection`. Novo
  `DiagnosticCode::UnresolvedSourceId` (NÃO existe ainda). Adicionar dep
  `forge-core-research` ao `forge-core-validate/Cargo.toml`. Preservar anchor 122.
- **F14.4** — Evidence Graph como projeção (não struct first-class):
  `evidence_graph(projection, artifacts) -> BTreeMap<SourceId, Vec<ClaimRef>>`.
  Definir `ClaimRef`. Padrão `reference_index` do `forge-core-store`.
- **F14.5** — CLI `forge-core research source add|list` / `cite` / `check` /
  `graph` em `research_cmd.rs` (template `memory_cmd.rs`). Append
  `CommandSpec` no fim de `COMMANDS`. Adicionar dep `forge-core-research` ao
  CLI `Cargo.toml`.
- **F14.6** — Runtime gate em path mutável (padrão risk-audit):
  `execute-operation` consulta citation validator; `has_errors()` →
  `DeniedByGate`. Flag `--require-citation` (opt-in primeiro).
- **F14.7** — fixtures (`docs/fixtures/research-v0/`) + E2E (template
  `memory_cli_e2e.rs`).

---

## 3. Lições aprendidas da paralelização (importante ler)

A tentativa de rodar 3 chats em paralelo **funcionou tecnicamente** (worktrees
isolados evitaram a mistura que causei no início), mas **o custo de coordenação
foi alto** e a decisão de voltar a um único agente é acertada. O que aconteceu:

1. **Mistura inicial de working tree** (antes dos worktrees): 3 chats editando
   o mesmo diretório sobrepuseram trabalho. Corrigi isolando via `git worktree`.
2. **Branch base errada**: handoffs diziam "a partir de master" mas master não
   tinha F06/F07. Base correta era `f06.2-trust-axes`. Detectado tarde.
3. **Desincronização de worktree**: quando rebasei `f12-guided-start` num
   worktree paralelo, o worktree principal ficou com índice/working tree velho.
   Precisei `git reset --hard HEAD` para re-sincronizar.
4. **Gate `-D pedantic` pegou dívida técnica**: o F07 foi fechado antes do CI
   virar `-D pedantic`; o FF merge trouxe lints que ninguém limpou. O gate
   funcionou como projetado (pegou regressão), mas gerou trabalho de limpeza.

**Conclusão**: para um único agente, tudo isso some. Trabalhe sequencialmente,
commit frequentemente, mantenha master verde.

---

## 4. Convenções do projeto (NÃO violar)

De `AGENTS.md` (sempre carregado):
- **Sem `anyhow`/`thiserror`**. Roll enums de erro à mão (`Debug, Clone, PartialEq, Eq`).
- **Sem `Result<_, String>` novo**. Os existentes são legacy; migrar ao tocar.
- **Validation = accumulating diagnostics** (não short-circuit). Colete em
  `ValidationReport`; construtores `Diagnostic::error()` / `Diagnostic::warning()`.
- **Workspace deps**: `serde.workspace = true`, etc. Sem version pins por-crate divergentes.
- **CI**: `cargo clippy --workspace --all-targets -- -D clippy::pedantic`
  (deny, não warn). `cargo fmt --all -- --check`. Hook `pi-green-loop` roda
  após cada turno.
- **Anchor 122**: `forge-core validate --json` emite 122× `"diagnostics": 0`.
  É a regression anchor do projeto. **Verificar após cada merge.**

### Editor stability (WSL + Windows + rust-analyzer)
- `target/debug` acumula ~130k files; rust-analyzer OOMs se não excluir.
- `rust-analyzer.toml` no repo root configura `files.exclude`. **Não regredir**
  (ver comentário SCHEMA HISTORY no arquivo).
- Periodicamente purgar test tempdirs sob `target/` (padrão `*-[0-9]+$`).
- **Nunca rodar dois cargo em paralelo** (editor r-a + terminal).

---

## 5. Protocolo Forge (claim/check-write)

Para editar arquivos governados (skill `forge-method`):
1. `forge-core project resolve` → resolve `claims_dir`.
2. `forge-core claim acquire --scope <kind> --id <scope-id> --agent <id> --path <repo-path>... --claims-dir <path>`.
3. `forge-core claim check-write --agent <id> --target <path> --claims-dir <path>` **antes de editar**.
4. Heartbeat em trabalho longo; `claim release` ao terminar.
5. Em `expired_requires_handoff`: **não** delete/retry. Use `forge-core claim handoff`, depois status + acquire fresh.

Read-only/resolve não precisam de claim. Bootstrap core (`D:\Forge-method-core`)
mantém `.forge-method/` sob `--allow-bootstrap-core`.

---

## 6. Próximos passos recomendados (em ordem)

### Feito nesta sessão
- ✅ **F08 mergeado em master** (rebase sobre master + ff). Master agora contém
  F06+F07+F12+R-LINT.6+F08. HEAD `0711d3de`. Verde (check/clippy -D/fmt/anchor
  122/test). 14 commits ahead de origin, **não pushed** (decisão do usuário).

### Imediato — fechar F14 (última feature P1 pendente)
1. **F14.3-F14.7** (ver §2 acima para detalhe; numeração autoritativa é a do
   `followups_v0_1_to_10.md`, não a do handoff F14 por-epic).
   - Trabalhar na worktree `D:/forge-f14`, branch `f14-knowledge-orchestration`.
   - **Rebase sobre master antes de começar** (F14 está em `3e9f9abb`, atrás do
     master `0711d3de`). O rebase traz F08/F12/R-LINT.6.
2. **Merge F14 → master**: após rebase, ff-merge. Conflito esperado = só
   `Cargo.toml` members (F14 adiciona `forge-core-research` na mesma linha que
   F08 adicionou `forge-core-protocol-mcp`; manter ambos).

### Final
3. **Push master → origin** (decisão pendente do usuário — recomendado deixar
   local até F14 pronto, então um push único). São 14+N commits não pushed.
4. Atualizar `excellence_roadmap.md` (score "Features comunidade" → 10/10
   quando F14 mergeado).
5. **Consolidar worktrees**: depois do merge F14, `git worktree remove
   /d/forge-f08 /d/forge-f14` (f08 já é descartável pois = master pós-ff).

---

## 7. Arquivos-chave para o próximo agente (em ordem de leitura)

1. **Este handoff** (`progress/handoff_consolidado.md`).
2. `progress/excellence_roadmap.md` — scores por frente, norte estratégico.
3. `progress/followups_v0_1_to_10.md` — stories detalhadas F08/F14 (epics).
4. `AGENTS.md` — convenções (sempre carregado, mas reler pra fixar).
5. `CONTEXT.md` — glossário (memory/trust/governance/guided-start).
6. `crates/forge-core-cli/src/command_registry.rs:68` — array `COMMANDS` (ponto de registro de verbos).
7. `crates/forge-core-cli/src/memory_cmd.rs` — template CLI (multi-subcommand + `emit()` dual-output).
8. `crates/forge-core-contracts/src/envelope.rs:77` — `CliEnvelope`.
9. ADRs: `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/` (ADR-0006 MCP, ADR-0007 governance, ADR-0010 research).
10. `crates/forge-core-memory/` — template PEP append-only (se F14.3 precisar de padrão similar).

---

## 8. Observações operacionais

- **`darkest-roguelite/`** (untracked no repo): **não é deste projeto**. Nunca
  foi staged/committed. O usuário está ciente. **Não mexer.**
- **`rust-analyzer.toml`**: tem diff local às vezes (config do editor).
  Branch-agnostic; deixar quieto.
- **Commit author**: `Codex <codex@example.local>`.
- **Push**: não foi feito. 5 commits no master à frente de origin. Aguardar
  decisão do usuário antes de pushar.
- **Worktrees F08/F14**: ainda ativos. Se for consolidar tudo num único
  diretório, pode `git worktree remove /d/forge-f08 /d/forge-f14` depois que
  F08/F14 forem mergeados (ou manter se for alternar entre eles).
- **Lixos untracked no principal** (meus handoffs por-epic): podem ser
  deletados agora que este consolidado os substitui. São:
  `handoff_f08_mcp_adapter.md`, `handoff_f12_guided_start.md`,
  `handoff_f14_knowledge_orchestration.md`, `handoff_master_parallelization.md`,
  `merge-plan.yaml`.

---

## 9. Context hygiene (importante para sessões longas)

Modelo degrada past ~150k tokens. **Trabalhar per-story, não per-epic**:
work → commit → se contexto pesar, escrever handoff incremental e parar.
Sessões são baratas; degradação de contexto não.

Quando estimar ~150-200k tokens acumulados, pausar e avisar:
*"Contexto pesado (~Nk estimado). Hora de handoff + /clear."*

---

**Resumo uma linha**: Master saudável (F06+F07+F12+R-LINT.6+**F08 mergeado**,
HEAD `0711d3de`, verde, 14 commits não pushed — decisão do usuário); F08
**fechado e mergeado** via rebase+ff nesta sessão; falta apenas F14
(F14.3-F14.7 na worktree `D:/forge-f14`, precisa rebase sobre master antes de
continuar). Leia este handoff, depois `excellence_roadmap.md` +
`followups_v0_1_to_10.md`, e continue por F14.

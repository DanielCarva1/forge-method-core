# Handoff — F07 completo (multi-principal governance)

**Data**: 2026-07-02
**Branch**: `f06.2-trust-axes` (todos os commits pushados para
`personal` = `https://github.com/DanielCarva1/forge-method-core`)
**Origem**: sessão que fechou o **epic F07 inteiro** (Multi-principal governance):
F07.5 (arbitration ledger) + F07.6 (CLI) + F07.7 (E2E). Juntamente com o F07.1-F07.4
da sessão anterior e o F06 fechado, esta é a fonte da verdade para continuar.

Este documento é a fonte da verdade para continuar. Leia completo.

---

## 1. O que esta sessão fez (NÃO refazer)

### F07 — FECHADO 🎉 (F07.1-F07.7 todos ✅)

| Commit | Story | Entrega |
|---|---|---|
| `317cafd6` | F07.1-F07.3 (sessão anterior) | ADR-0007 Accepted; modelo 3-camadas; `PrincipalId` newtype; `governance.rs` (GovernancePolicy, IntentContract, ConflictContract + enums); validator; fixtures. |
| `bd9ddd7b` | F07.4 (sessão anterior) | Wire do `ConflictContract` no `claim_engine.rs` acquire. |
| `01451033` | F07.5 | Novo crate `forge-core-governance` (PEP) — ledger append-only `governance/conflicts.ndjson`. Event log + projection + `record`/`arbitrate`/`escalate`/`list` PEPs. Gate `GovernancePolicy::can_arbitrate`. 22 testes. |
| `<esta sessão>` | F07.6 + F07.7 | CLI `forge-core governance record\|conflicts\|arbitrate\|escalate` (4 verbos, `CliEnvelope` dual-output). E2E suite `governance_cli_e2e.rs` (5 testes, assert_cmd no binário real). Fecha F07. |

---

## 2. Decisões-chave (research-justified, para não re-debater)

- **Crate `forge-core-governance` (novo, 1:1 com forge-core-memory)**: o claim engine é puro (DD16, sem fs); persistir o ConflictContract é um PEP separado. Espelha o split F06: contracts=PDP puro, crate-PEP=mutação sob lock. Colocar no engine quebraria PDP/PEP (ADRs 0002/0003/0007); colocar no memory fundiria dois epics/ADRs distintos.
- **Gate `can_arbitrate` reusa `authorized_reviewers`** (zero schema churn, zero ripple): o papel revisor e o papel árbitro coincidem hoje (um reviewer é confiável para adjudicar conflitos sobre o que ele já atesta). O split reviewer↔arbiter (`authorized_arbiters` distinto) é refinamento futuro documentado (YAGNI; nenhum fixture separa-os hoje).
- **Idempotência do `record` por `conflict_id`**: o id é determinístico/ordering-independent (`build_conflict` ordena os principals), então dois acquires do mesmo overlap geram o mesmo id; o segundo `record` é `AlreadyRecorded` (sem append, sem consumir sequência).
- **`--status` é filtro de CATEGORIA, não valor-exato**: `--status resolved` casa TODOS os resolvidos independentemente de arbiter/decision. A comparação por variante usa `resolution_tag(&str)`, não `== ConflictResolutionState` (que seria false para `Resolved{awarded_to(alice)}` vs `Resolved{both_released}`).
- **F07.6 cobre o que existe**: o handoff anterior previa `intent` verb, mas não persisti `IntentContract` (a camada de coordenação/intent-locks ficou como story própria). O conflito é detectado no acquire (claim_engine), não precisa de intent-persistência para funcionar. `intent`-persistence é follow-up, não bloqueia o epic.
- **CLI dispatch é registry-driven** (NÃO match-arm): adiciona um `CommandSpec` ao array `COMMANDS` em `command_registry.rs`, não um match arm.

---

## 3. Validação no fim da sessão

- `cargo check --workspace` verde.
- `cargo clippy --workspace --all-targets -- -W clippy::pedantic` exit 0 (zero warnings no código novo; pré-existentes em memory_cli_e2e/autonomy + 2 must_use_candidate em contracts intocados).
- `cargo test --workspace` verde. **78 suites ok, 0 falhas**.
  - governance lib 22; governance E2E 5 (novo); CLI lib **177** (+10 parser); memory E2E 6; contracts 93; engine 157; memory 29; validate 17; claims 16; claim_e2e 5.
- `cargo fmt --all -- --check` clean.
- **Anchor 122 preservado** (`validate --json` emite 122× `"diagnostics": 0`).

---

## 4. Próximos passos (pós-F07 — rumo ao 10/10)

O epic F07 está **completo**. Os epics pendentes (ver `followups_v0_1_to_10.md`):

1. **F08** — Secure MCP adapter (nova crate `forge-core-protocol-mcp`).
2. **F12**, **F14** — ver followups.
3. **Follow-ups de F07 (não bloqueiam o 10/10, mas são refinamentos naturais)**:
   - `intent`-persistence (declarar/persistir IntentContract) — camada de coordenação do ADR-0007.
   - Split `authorized_arbiters` distinto de `authorized_reviewers` (quando uma política precisar).
   - Wiring do `record` no call-site do acquire: quem observa a rejeição `AlreadyClaimedByOther{conflict: Some(..)}` chama `forge_core_governance::record`. Hoje o engine emite o contract mas ninguém o persiste automaticamente (o CLI `record` é o caminho explícito).

---

## 5. Arquivos-chave para o próximo agente ler (em ordem)

1. `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/ADR-0007-multi-principal-governance.md` — Accepted, modelo 3-camadas.
2. `crates/forge-core-governance/src/lib.rs` — o PEP + projection + event log (F07.5).
3. `crates/forge-core-governance/src/{record,arbitrate,escalate}.rs` — as 3 operações.
4. `crates/forge-core-contracts/src/governance.rs` — schemas + `GovernancePolicy::can_arbitrate`.
5. `crates/forge-core-cli/src/governance_cmd.rs` — o CLI (F07.6, template: memory_cmd.rs).
6. `crates/forge-core-cli/tests/governance_cli_e2e.rs` — E2E (F07.7, template: memory_cli_e2e.rs).
7. `crates/forge-core-decisions/src/claim_engine.rs` — o seam de detecção (acquire; helper `build_conflict`).
8. `crates/forge-core-memory/src/lib.rs` — o template arquitetural espelhado.
9. Este handoff + o anterior (`handoff_f06_f07_session.md`).

---

## 6. Notas operacionais

- **`darkest-roguelite/`** é untracked desde antes (não deste projeto). **`rust-analyzer.toml`** tem diff pré-existente. Não mexer em nenhum.
- **Branch**: todo o trabalho está em `f06.2-trust-axes`, pushado para `personal` (DanielCarva1/forge-method-core). O `origin` (Stable-Studio) não foi tocado.
- **Commit author**: `Codex <codex@example.local>`.
- **Convenção de teste E2E**: helpers `bin()`/`repo_root()`/`example()`/`fresh_<thing>_dir()` vivem no topo de cada `*_e2e.rs` (hand-rolled, sem `tempfile`); fresh dirs sob `target/` com `AtomicUsize` seq para paralelismo.

---

## 7. Claims Forge

Nenhum claim ativo ao fim desta sessão (escrita em contracts/governance/CLI/docs, nenhum claim-governado).

---

**Resumo uma linha**: F07 fechado (governance completo: ADR-0007 + PrincipalId + schemas + claim-engine conflict wire + forge-core-governance PEP ledger + CLI 4 verbos + E2E); F06+F07 fechados; próximos passos = F08 (Secure MCP adapter) + F12 + F14 rumo ao 10/10.

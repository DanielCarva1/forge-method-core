# Handoff — F06 completo + F07.1-F07.4 (governance foundation)

**Data**: 2026-07-02
**Branch**: `f06.2-trust-axes` (todos os commits pushados para
`personal` = `https://github.com/DanielCarva1/forge-method-core`)
**Origem**: sessão que fechou o **epic F06 inteiro** (Memory Policy) e entregou
a **fundação do F07** (Multi-principal governance) até o conflict-emission wire.

Este documento é a fonte da verdade para continuar. Leia completo.

---

## 1. O que esta sessão fez (NÃO refazer)

### Epic F06 — FECHADO 🎉 (F06.1-F06.8 todos ✅)

| Commit | Story | Entrega |
|---|---|---|
| `9c1de6ff` | F06.2 Candidato 1 | Gates de decisão puros (`can_admit`/`can_promote`) em `MemoryContract` + `MemoryPolicy` tipada + ADR 0002 addendum (PDP/PEP, Cedar/OPA/Zanzibar citations) |
| `18798543` | F06.3 (Candidato 2) | Novo crate `forge-core-memory` — o PEP (lock → PDP → append → projection). Reusa `forge-core-store` (fs4 lock, append_json_line, event-sourcing projection). ADR 0003. 29 testes. |
| `09d21ba8` | F06.7 | CLI `forge-core memory ingest\|list\|forget\|promote\|review` (5 verbos, `CliEnvelope` dual-output). `review` deferido (precisa F07). |
| `dff073cc` | F06.8 | Fixtures (`contracts/examples/memory-*.yaml`) + E2E suite (CLI via assert_cmd + PEP integration). Fecha F06. |

### F07 — Fundação entregue (F07.1-F07.4 ✅, F07.5-F07.7 ⏳)

| Commit | Story | Entrega |
|---|---|---|
| `317cafd6` | F07.1-F07.3 | **ADR-0007** (Accepted, expandido de stub): modelo 3-camadas (autorização ReBAC/Cedar + coordenação intent-locks Gray + conflito first-class Git/Apel/Berenson). **`PrincipalId`** newtype (supersede previsão do ADR 0002; R8). `governance.rs` (`GovernancePolicy`, `IntentContract`, `ConflictContract` + enums). Validator. Fixtures. `reviewed_by` migrou `StableId`→`PrincipalId`. |
| `bd9ddd7b` | F07.4 | Wire do `ConflictContract` no `claim_engine.rs` acquire: campo `conflict: Option<ConflictContract>` nos variantes `AlreadyClaimedByOther`/`PathAlreadyClaimed` (additive, backward-compat). Helper `build_conflict`. 3 testes. |

---

## 2. Decisões-chave (research-justified, para não re-debater)

- **F06 PDP/PEP split**: gates são predicados puros (Cedar/OPA/K8s/XACML consensus); mutação é PEP separado sob lock (CWE-367 atomicidade no write site). Vec-not-Set é divergência intencional do Cedar (crate deriva só Eq).
- **F07 modelo 3-camadas**: RBAC/ReBAC/ABAC/Cedar/Zanzibar respondem single-principal (sem contention semantics). F07 = coordenação (intent-locks Gray) + conflito first-class (Git/Apel/Berenson, NUNCA merge silencioso).
- **`PrincipalId` tipado**: supersede ADR 0002 (que previu "não introduzir"). R8 wins: authz structures passam o "comparison would be a bug" test (principal↔resource swap = security bug silencioso). Type alias rejeitado (transparente = zero proteção).
- **Seam do conflito**: `claim_engine.rs` acquire (NÃO no WAL). Os 2 variantes de rejeição por sobreposição **já carregam** os dados de atribuição; F07.4 apenas popula o `ConflictContract` alongside.
- **F07.4 é additive**: campo `conflict: Option<ConflictContract>` ao lado dos campos flat existentes. Consumers usam `{ .. }` → não-breaking.

---

## 3. Validação no fim da sessão

- `cargo check --workspace` verde.
- `cargo test -p forge-core-contracts` 93/93; `-p forge-core-memory` 29/29 + lifecycle 8/8; `-p forge-core-decisions` 157/157; `-p forge-core-validate` 17/17; `-p forge-core-cli` lib 167/167 + claims 16/16 + claim_e2e 5/5 + memory_cli_e2e 6/6.
- clippy `-W pedantic`: zero warnings no código novo (2 pre-existing must_use no Candidato-1 bridge methods — unchanged).
- `cargo fmt` clean.
- **Anchor 122 preservado** (`validate --json` emite 122× `"diagnostics": 0`).

---

## 4. Próximos passos (em ordem)

### Caminho direto — continuar F07 (fechar o epic)

1. **F07.5 — Arbitration ledger (append-only)**: persistir o `ConflictContract` que F07.4 agora retorna na `ClaimLifecycleDecision`. Modelo: append-only JSONL (mesmo padrão do `forge-core-memory` event log — reusar `append_json_line_with_durability` + fs4 lock). Queryable (`forge-core governance conflicts --status open` = F07.6). Resolution lifecycle `Pending → Resolved/Escalated` já está no schema (`ConflictResolutionState`).
2. **F07.6 — CLI `forge-core governance intent/conflicts/arbitrate`**: 3 verbos, `CliEnvelope` dual-output (template: `memory_cmd.rs` / `autonomy_cmd.rs`). O `arbitrate` move `ConflictResolutionState` Pending→Resolved.
3. **F07.7 — Fixtures + E2E**: 2 principals disputando mesmo ref → ConflictContract emitido (via assert_cmd no binário real, template: `memory_cli_e2e.rs`). Resolução manual → ledger atualizado. Anchor preservado.

### Após F07 — outros epics pendentes para 10/10
- **F08** (Secure MCP adapter, nova crate `forge-core-protocol-mcp`)
- **F12**, **F14** (ver `followups_v0_1_to_10.md`)

---

## 5. Arquivos-chave para o próximo agente ler (em ordem)

1. `docs/dev-docs/forge-method-core-dev-docs-v2/adrs/ADR-0007-multi-principal-governance.md` — Accepted, modelo 3-camadas, PrincipalId decision.
2. `crates/forge-core-decisions/src/claim_engine.rs` — o seam do F07.4 (acquire ~329, ~369; helper `build_conflict` ~780).
3. `crates/forge-core-contracts/src/governance.rs` — os schemas (GovernancePolicy, IntentContract, ConflictContract + enums).
4. `crates/forge-core-contracts/src/common.rs` — `PrincipalId` newtype (R8, serde-transparent).
5. `crates/forge-core-memory/src/lib.rs` — o PEP + projection + event log (template para F07.5 ledger).
6. `crates/forge-core-cli/src/memory_cmd.rs` — template CLI para F07.6.
7. `docs/adr/0002-memory-trust-model.md` + `docs/adr/0003-memory-pep-store.md` — contexto F06 (supersedido em parte pelo ADR-0007 re: PrincipalId).
8. Este handoff.

---

## 6. Notas operacionais

- **`darkest-roguelite/`** é um diretório untracked que existe no working tree desde antes desta sessão. **Nunca foi staged/committed** (`git log --all -- darkest-roguelite/` é vazio). Não é deste projeto. O usuário está ciente. Não mexer.
- **`rust-analyzer.toml`** tem diff pré-existente (prefix keys `rust-analyzer.*`) que também não é desta sessão. Deixado untouched.
- **Branch**: todo o trabalho está em `f06.2-trust-axes`, pushado para `personal` (DanielCarva1/forge-method-core). O `origin` (Stable-Studio) não foi tocado.
- **Commit author**: `Codex <codex@example.local>`.

---

## 7. Claims Forge

Nenhum claim ativo ao fim desta sessão (escrita em contracts/engine/memory/CLI/docs, nenhum claim-governado). Para continuar editando engine/contracts, adquirir claim conforme o fluxo normal.

---

**Resumo uma linha**: F06 fechado (memory policy completo: gates + PEP + CLI + E2E); F07 fundação entregue (ADR-0007 Accepted, PrincipalId, governance schemas, validator, conflict-emission wire no acquire); próximos passos = F07.5 (arbitration ledger append-only) → F07.6 (CLI) → F07.7 (E2E), depois F08/F12/F14 para 10/10.

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

## Handoff para o próximo agente (Commit 2.2 — rekor parse + verify)

> **Leia isto primeiro.** É a tarefa corrente, auto-contida, com tudo o que
> um agente fresco precisa para continuar sem redescobrir o que esta sessão
> já mapeou.

### Próxima tarefa: Commit 2.2 — rekor log entry parse + inclusion proof verify

**Arquivo-alvo:** `crates/forge-core-crypto/src/rekor.rs` (397 LOC).

**Status atual do crate:** o crate quebrou a cobertura zero no Commit 2.1
(agora tem 14 testes lib + 7 integration). A Fase 2 está ~25% completa.
O `rekor.rs` ainda tem **zero testes unitários** — só é exercido
indiretamente pelo CLI E2E em
`crates/forge-core-cli/tests/validate.rs` (`rekor_entry_fixture` linha 337,
`rekor_entry_fixture_for_bundle` linha 1174).

**Funções a cobrir (4 entrypoints, ordem por criticidade):**

1. `pub fn parse_rekor_log_entry(text: &str) -> Result<ParsedRekorEntry, RekorParseError>`
   (linha 122) — **pública** (fuzz-exposed). Já tem `RekorParseError` tipado
   com 16 variantes (`LogEntryJsonInvalid`, `MissingField`, `BodyBase64Invalid`,
   `VerificationMissing`, `InclusionProofMissing`, `InclusionHashesMissing`,
   `InclusionHashInvalid`, etc.). Testar cada variante de erro + happy path.
2. `pub fn parse_signed_checkpoint(checkpoint: &str) -> Result<ParsedCheckpoint, RekorParseError>`
   (linha 278) — **pública**. Variações: `CheckpointFormatInvalid` (sem
   `\n\n`), `CheckpointNoteInvalid` (<4 linhas), `CheckpointOriginMissing`,
   `CheckpointTreeSizeInvalid` (não-numérico), happy path.
3. `pub(crate) fn verify_rekor_checkpoint(proof, rekor_key) -> Result<(), RekorParseError>`
   (linha 243) — **`pub(crate)`, teste inline.** Verifica assinatura p256
   do checkpoint. `CheckpointTreeSizeMismatch`, `CheckpointRootHashMismatch`,
   `CheckpointSignatureMissing`, `CheckpointSignatureInvalid` (key errada).
4. `pub(crate) fn verify_merkle_inclusion(leaf_hash, hashes, log_index, tree_size, root_hash) -> bool`
   (linha 341) — **`pub(crate)`, teste inline.** Matemática Merkle RFC 6962.
   Casos: tree_size=1 (trivial), log_index>=tree_size (reject), path válido,
   path adulterado, hashes com tamanho errado.

**Padrões a reusar (NÃO reinventar):**

- **Fixture de rekor entry real:** `validate.rs:337-435` (`rekor_entry_fixture`)
  gera uma log entry JSON completa com checkpoint assinado por
  `P256SigningKey::from_slice(&[8u8;32])`. **Espelhar este helper** para os
  testes — ele já resolve toda a dança de leaf hash, root hash e assinatura
  de checkpoint. Copie o helper para um `#[cfg(test)] mod tests` no
  `rekor.rs` (ou um `tests/rekor.rs` para as funções `pub`).
- **Ponte p256 signing key ↔ verifying key:** estabelecida no Commit 2.1.
  Para o `verify_rekor_checkpoint`, assine com
  `P256SigningKey::from_slice(&[8u8;32])` (seed fixa, como `validate.rs:341`)
  e construa o `P256VerifyingKey` via
  `signing_key.verifying_key()`. **Não** use `KeyPair::generate()` para o
  checkpoint (precisa de seed determinística para KAT).
- **KAT determinístico:** se for fixar uma log entry canônica + seus hashes,
  espelhar o padrão do Commit 2.1 (`ed25519_deterministic_kat_*` em
  `slsa_transparency.rs:374`) — pinar leaf_hash/root_hash/signature em hex.

**Decisões de design já tomadas (NÃO reconsiderar):**

- **Visibilidade:** `parse_rekor_log_entry` e `parse_signed_checkpoint` são
  `pub` por estratégia de fuzz (documentada em `lib.rs:74-80`). Os testes
  dessas duas podem ir em `tests/rekor_parse.rs` (integration test) OU
  inline `#[cfg(test)]`. `verify_rekor_checkpoint` e `verify_merkle_inclusion`
  são `pub(crate)` → **obrigatoriamente inline**.
- **Tratamento de erro:** `RekorParseError` já existe e deriva
  `Debug, Clone, PartialEq, Eq` (cumpre AGENTS.md). Tem `.display()`
  `pub(crate)` que renderiza a string de diagnóstico legacy. Testes podem
  comparar `assert_eq!(err, RekorParseError::MissingField { field: "logID" })`.
- **Não migrar nada para `Result<_, String>`** — o caminho oposto já foi
  feito (commit `a2ff9ac9` migrou 4 sites; rekor já estava tipado).

**Gate de aceite (Commit 2.2):**

- `cargo test -p forge-core-crypto` verde.
- Clippy pedantic limpo (roda automático via `pi-green-loop` hook, OU
  manualmente: `cargo clippy -p forge-core-crypto --all-targets --
  -W clippy::pedantic`).
- `cargo fmt -p forge-core-crypto -- --check` limpo.
- Cada uma das 4 funções com: ≥1 happy path + ≥1 caso de erro por variante
  de `RekorParseError` relevante + (para `verify_merkle_inclusion`) casos
  de borda de tree_size/log_index.
- Zero churn em código de produção (só `#[cfg(test)]` ou `tests/`).

**Verificação automática:** o hook `pi-green-loop` roda após cada turno de
edição e reporta `cargo check --workspace`, `cargo clippy --workspace
--all-targets -- -W clippy::pedantic`, `cargo test --workspace`, e
`cargo fmt --all -- --check`. Normalmente não é necessário rodar manualmente.
`/green` roda agora; `/green on|off` toggla o auto-fix loop.

**Convenções do repo a respeitar** (em `AGENTS.md`, sempre carregadas):

- **Sem `anyhow`/`thiserror`.** Roll error enums à mão. `RekorParseError`
  já existe — não criar novo.
- **Validação é acumuladora** — mas `rekor.rs` usa `Result` (bail-out),
  não `ValidationReport`. Manter o padrão existente do módulo.
- **Editor stability (WSL+Windows+r-a):** nunca rodar dois cargos em
  paralelo. Um cargo de cada vez. `target/debug` acumula ~130k arquivos;
  se o editor morrer com OOM, ver `AGENTS.md → Editor stability`.
- **Context hygiene:** uma story por sessão. Este handoff é Commit 2.2
  (uma sessão). Ao terminar: commit, marcar Commit 2.2 ✅ LANDED neste
  doc, `/clear`, próximo agente pega Commit 2.3.
- **Commits:** o usuário commita explicitamente quando pede. Esta sessão
  fez 1 commit (`21f0840d`).

**Depois do 2.2 (roadmap restante da Fase 2):**

- Commit 2.3 — OCSP/CRL/CT-SCT (`ocsp.rs` 408 LOC).
- Commit 2.4 — TUF trusted-root freshness (`tuf.rs` 207 LOC).
- Fase 3 — governance, eval-harness, research, eventlog, eval, trace
  (1 crate por sessão).

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

## 2. Migrar `Result<_, String>` (AGENTS.md manda) — ✅ LANDED (5/5)

Todos os 5 sites migrados para enums tipados (Debug, Clone, PartialEq, Eq):

1. ~~`store/lib.rs` `parse_effect_wal_records_for_recovery`~~ → `EffectWalRecoveryParseError`
2. ~~`cli/mcp_cmd.rs` `parse_serve_args`~~ → `ServeArgsError`
3. ~~`cli/research_cmd.rs` `load_evidence`~~ → `EvidenceLoadError`
4. ~~`protocol-mcp/attestation.rs` `hex_decode`~~ → `HexDecodeError`
5. ~~`protocol-mcp/server.rs` `extract_attestation`~~ → `AttestationExtractError`

**Zero `Result<_, String>` em `crates/*/src/`** (grep confirma). Gate de
aceite cumprido: clippy pedantic verde, testes verde.

---

## 3. Cobertura de testes — 4 crates sem testes

**Contexto.** O spine é bem testado (store, validate, decisions, kernel,
cli têm suites E2E + unit). O audit inicial dizia que 4 crates tinham zero
testes, mas isso estava **errado para o MCP** — ele já tinha ~33 testes
inline. O gap real do MCP era vetores de ataque específicos não cobertos.

| Crate | LOC | Risco | Estado |
|-------|-----|-------|--------|
| `forge-core-crypto` | 5812 | **P0 — security-critical** | Pendente — só 101 LOC de smoke tests. ed25519/p256/rekor/OCSP/TUF. **Prioridade máxima.** |
| `forge-core-protocol-mcp` | 2016 | Alto | ✅ **Attestation gaps fechados** (44 testes) |
| `forge-core-governance` | 1447 | Alto | Pendente — arbitrate/escalate/record sem prova |
| `forge-core-eval-harness` | 1371 | Alto | Pendente — decide baseline vs candidate (ADR-0023) |
| `forge-core-research` | 1025 | Médio | Pendente — admission/graph; `proptest` dev-dep mas 0 testes |
| `forge-core-eventlog` | 1132 | Médio | Pendente — EventSourced trait mechanics |
| `forge-core-eval` | 890 | Baixo | Pendente — contract types |
| `forge-core-trace` | 479 | Baixo | Pendente — trivial |

### Fase 2 — `forge-core-crypto` (P0, prioridade máxima) — Commit 2.1 ✅ LANDED

O crate de maior risco: 5812 LOC de verificação criptográfica com cobertura
essencialmente zero. Um bug aqui é silencioso e catastrófico. Cobertura
ampla por commit:

- **Commit 2.1 — ed25519/p256 signature verification.** ✅ LANDED (14
  testes). Round-trip sign→verify (Ok), tampered sig→verify (Invalid),
  wrong key→verify (Invalid). KAT determinístico com seed fixa pinando
  verifying key + assinatura ed25519 (espelha o padrão do MCP
  `attestation.rs:568`). p256 bundle + DSSE verify testados ponta-a-ponta
  com a signing key extraída do certificado de teste (ponte
  rcgen `KeyPair::serialize_der()` → `p256::ecdsa::SigningKey::from_pkcs8_der`).
  Cobertura dos 3 sites: `verify_ed25519_signature`,
  `verify_bundle_signature_with_certificate`,
  `verify_dsse_signature_with_certificate`. Proptest sign/verify+tamper
  em ambos os algoritmos. `cargo test -p forge-core-crypto` verde (14 lib
  + 7 zeroize_smoke), clippy pedantic limpo.
- **Commit 2.2 — rekor log entry parse + verify.** PRÓXIMO. Parse de uma
  transparency log entry real, inclusão proof, reject de entry forjada.
- **Commit 2.3 — OCSP/CRL/CT-SCT status.** Decode OCSP (good/revoked/
  unknown), CRL revoked detection, CT/SCT timestamp validation.
- **Commit 2.4 — TUF trusted-root freshness.** Version monotonicity,
  timestamp/snapshot/targets version checks, expiry.

Cada commit: `cargo test -p forge-core-crypto` a passar.

### Fase 3 — crates sem testes (SESSÕES seguintes, ordem por risco)

1. `forge-core-governance` (arbitrate/escalate/record)
2. `forge-core-eval-harness` (decide baseline vs candidate)
3. `forge-core-research` (admission/graph; `proptest` dev-dep disponível)
4. `forge-core-eventlog` (EventSourced trait mechanics)
5. `forge-core-eval` / `forge-core-trace` (baixo risco)

### `forge-core-protocol-mcp` — ✅ LANDED (parcial)

Os gaps de attestation/authorization foram fechados (3 commits, sessão
seguinte ao derive_state):
- 7 testes novo: RequireAll gate, present-but-invalid no read-only
  (defense-in-depth), malformed `_meta.attestation`, unauthorized-key
  pin do contrato documentado, proptest sign/verify+tamper.
- KAT determinístico (seed fixa) que pin canonical bytes + assinatura
  ed25519 — apanha regressões de canonicalização que eram flaky em OsRng.
- `hex_decode` migrado de `Result<_, String>` para `HexDecodeError` tipado
  (também fecha item #2 parcialmente para o crate MCP).

**O que NÃO landed:** allowlist tem 11 testes (cobertura boa); server.rs
tem 17 testes (gate coberto). O `run_stdio` live loop fica implícito.

**Gate de aceite.** Cada crate tem ≥1 teste E2E + cobertura unitária nos
caminhos críticos; `cargo test -p <crate>` verde.

**Estimativa.** Fase 2: 1-2 sessões. Fase 3: 4-5 sessões (1 por crate).

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

## Histórico

### Sessão Fase 2 / Commit 2.1 (ed25519/p256 signature tests) — `21f0840d`

Quebrou a cobertura zero do `forge-core-crypto` nos 3 sites de verificação
de assinatura. 14 testes novos, zero churn em produção:

- **`slsa_transparency.rs`** (ed25519, 7 testes): round-trip Ok, tampered
  signature, tampered message, wrong key, malformed lengths, KAT
  determinístico (seed `[7u8;32]`, pin verifying key + assinatura em hex),
  proptest sign/verify+tamper.
- **`sigstore.rs`** (p256 ECDSA, 7 testes): bundle + DSSE verify
  ponta-a-ponta com signing key extraída do cert de teste via ponte
  rcgen `KeyPair::serialize_der()` (PKCS#8) →
  `p256::ecdsa::SigningKey::from_pkcs8_der`. Round-trip Ok, tampered DER,
  wrong-message, single-byte digest mutation, DSSE tampered payload,
  proptest.

**Descoberta técnica:** o `validate.rs` (CLI E2E) assinava com
`P256SigningKey::from_slice(&[8u8;32])` *não relacionada* à chave pública
do certificado — os testes unitários agora cobrem o caminho real onde as
chaves correspondem.

**Descoberta de contrato:** `verify_ed25519_signature` só promete
fail-closed em erros *estruturais* (tamanho de key/sig). Keys degeneradas
(all-zero) codificam um ponto válido em ed25519 e NÃO são rejeitadas —
testado e documentado honestamente no teste `ed25519_malformed_*`.

Gate: `cargo test -p forge-core-crypto` verde (14 lib + 7 zeroize_smoke),
clippy pedantic limpo, fmt limpo.

### Sessão original (Fases A–D)

Ver `git log` entre `1ebcdc06` (Fase A) e `d9dbe1d9` (Fase C). As 4 fases
landed:
- **A:** 89 claims órfãos limpos + AGENTS.md handoff removido + 12 ponteiros
  reparados.
- **B:** CLI/UX — `--version`, no-args→help, `--help` framing, `start`
  no_link guidance, unknown-command diagnosis, `--no-sync` stderr em JSON.
- **C:** README MCP corrigido, VERSION alinhado, inventory reescrito,
  `--json` consistency, SKILL URL.
- **D:** este documento.

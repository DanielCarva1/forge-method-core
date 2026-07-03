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

## Handoff para o próximo agente (Commit 2.4 — TUF trusted-root freshness)

> **Leia isto primeiro.** É a tarefa corrente, auto-contida, com tudo o que
> um agente fresco precisa para continuar sem redescobrir o que esta sessão
> já mapeou.

### Histórico da sessão: Commit 2.3 ✅ LANDED

Commit 2.3 cobriu os 11 helpers `pub(crate)` de `ocsp.rs` com 34 testes
inline. Detalhes na seção Fase 2 abaixo. Descobertas técnicas para não serem
re-litigadas:

1. **`ocsp.rs` já tinha cobertura E2E completa** (17 integration tests em
   `validate.rs` cobrindo o entrypoint público via DER assinado rcgen). O
   Commit 2.3 focou em cobertura unitária direta dos helpers, construindo
   structs `rasn-ocsp` em Rust puro — fail-close ao ponto de falha.
2. **`rasn::types::Name` (X.500 Distinguished Name) não é publicamente
   construído** em rasn 0.28. Testes usam `ResponderId::ByKey` (que só
   precisa de `OctetString`) em vez de `ResponderId::ByName(Name)`.
3. **`ObjectIdentifier::new()` retorna `Option`**; use
   `ObjectIdentifier::new_unchecked(arcs.into())` em fixtures de teste.
4. **`SequenceOf<T>` é alias para `Vec<T>`** — `.into()` é inútil (clippy
   pega); atribua direto.
5. **Dev-deps adicionadas:** `chrono` + `rasn-pkix` (workspace deps).
6. **Serial do rcgen CA default é grande demais para u64** — sempre setar
   `params.serial_number = Some(rcgen::SerialNumber::from(<u64>))` em
   fixtures de teste.

### Próxima tarefa: Commit 2.4 — TUF trusted-root freshness

**Arquivo-alvo:** `crates/forge-core-crypto/src/tuf.rs` (207 LOC).

**Status atual do crate:** Fase 2 ~75% completa (Commits 2.1-2.3 landed:
ed25519/p256/rekor/OCSP cobertos, 78 testes lib + 7 zeroize_smoke). `tuf.rs`
tem **zero testes inline** — mas já tem 6 integration tests E2E em
`validate.rs` (linhas 4576-4742) cobrindo o entrypoint público
`run_host_adapter_tuf_trusted_root_freshness_verification`.

**Funções a cobrir (1 `pub(crate)` + 5 privadas, todas inline):**

| # | Função | Visib. | Linha | Retorno |
|---|--------|--------|-------|---------|
| 1 | `verify_tuf_metadata_freshness_role(...)` | `pub(crate)` | 39 | `()` |
| 2 | `parse_tuf_datetime_utc_to_unix(expires, reasons)` | priv | 132 | `Option<i64>` |
| 3 | `parse_fixed_i32(slice, start, end)` | priv | 175 | `Option<i32>` |
| 4 | `days_in_month(year, month)` | priv | 181 | `i32` |
| 5 | `is_leap_year(year)` | priv | 192 | `bool` |
| 6 | `days_from_civil(y, m, d)` | priv | 199 | `i64` |

**Estilo de erro:** mesmo padrão `Vec<String>` accumulation do OCSP —
`verify_tuf_metadata_freshness_role` recebe `&mut Vec<HostAdapterTufMetadataFreshnessRole>`
+ `&mut Vec<String>` (evidence) + `&mut Vec<String>` (reasons). **Sem
enum de erro.** Testes inspecionam os vetores + o `Option<i64>` do parser.

**Vantagem: muito mais simples que OCSP.** `tuf.rs` depende só de
`serde_json::Value` + `std::path::Path` — **sem chrono, sem rcgen, sem
rasn-ocsp.** Datetime parsing é hand-rolled (RFC 3339 → unix secs via
`days_from_civil`). Fixtures são JSON simples.

**Padrões a reusar (NÃO reinventar):**

- **E2E já cobre os 6 cenários** (fresh metadata, expired root, rollback
  version, invalid expiry format, non-TUF policy, binary JSON output).
  Commit 2.4 = cobertura unitária direta dos helpers, focando em edge cases
  de datetime parsing (`parse_tuf_datetime_utc_to_unix`,
  `days_from_civil`, `is_leap_year`, `days_in_month`) que o E2E não isola.
- **Tipos de entrada:** `verify_tuf_metadata_freshness_role` recebe um
  `serde_json::Value` (o metadata doc) + strings de role type/version floor.
  Construa JSON fixtures inline.

**Gate de aceite (Commit 2.4):**

- `cargo test -p forge-core-crypto` verde.
- Clippy pedantic + fmt limpos (auto via `pi-green-loop` hook).
- Cobertura mínima: `verify_tuf_metadata_freshness_role` (fresh + expired +
  rollback), `parse_tuf_datetime_utc_to_unix` (RFC 3339 válido, formatos
  inválidos), `is_leap_year`/`days_in_month`/`days_from_civil` (edge cases
  de calendário: ano bissexto, fevereiro, datas limite).
- Zero churn em produção (só `#[cfg(test)]`).

**Decisões de design já tomadas (NÃO reconsiderar):**

- **Visibilidade:** `verify_tuf_metadata_freshness_role` é `pub(crate)` →
  **teste inline obrigatório**. As 5 helpers privadas só são acessíveis
  inline dentro do próprio módulo.
- **Não criar enum de erro** — manter o `Vec<String>` accumulation.
- **Entrypoint público não está em tuf.rs** — é
  `crate::run_host_adapter_tuf_trusted_root_freshness_verification` em
  `host_adapter_verification.rs:1521` (chama
  `verify_tuf_metadata_freshness_role` 4× — uma por role root/timestamp/
  snapshot/targets).

**Convenções do repo a respeitar** (em `AGENTS.md`, sempre carregadas):

- **Sem `anyhow`/`thiserror`.**
- **Editor stability (WSL+Windows+r-a):** nunca dois cargos em paralelo.
- **Context hygiene:** uma story por sessão. Commit 2.4 = uma sessão.
  Ao terminar: commit, marcar Commit 2.4 ✅ LANDED, Fase 2 completa.
  Próximo agente pega Fase 3 (governance, eval-harness, etc.).
- **Commits:** o usuário commita explicitamente quando pede.

**Depois do 2.4 (Fase 3):**

- Fase 3 — 1 crate por sessão, ordem por risco:
  governance → eval-harness → research → eventlog → eval/trace.

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

### Fase 2 — `forge-core-crypto` (P0, prioridade máxima) — Commits 2.1-2.2 ✅ LANDED

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
- **Commit 2.2 — rekor log entry parse + inclusion proof verify.** ✅
  LANDED (30 testes lib + 1 KAT regenerator `#[ignore]`d). Cobertura direta
  dos 4 entrypoints de `rekor.rs` (397 LOC), todos inline `#[cfg(test)]`
  (os 2 `pub(crate)` exigem):
  - `parse_rekor_log_entry` — happy path + cada variante de
    `RekorParseError` (8 variantes: JSON inválido, body base64 inválido,
    body não-JSON, `verification`/`inclusionProof`/`hashes` ausentes,
    hash não-string, e cada `MissingField` via remoção de campo por path).
  - `parse_signed_checkpoint` — happy-path KAT (pina `tree_size` + root
    hash), extensão de note lines, e 6 variantes de erro de formato
    (`CheckpointFormatInvalid`, `NoteInvalid`, `OriginMissing`,
    `TreeSizeInvalid`, `RootHashBase64Invalid`).
  - `verify_rekor_checkpoint` — Ok + 4 variantes (`TreeSizeMismatch`,
    `RootHashMismatch`, `SignatureMissing`, `SignatureInvalid` via key
    errada). KAT p256 pina o verifying key sec1-hex derivado da seed
    `[8u8;32]` (regenerador `#[ignore]`d).
  - `verify_merkle_inclusion` — tree_size=1 trivial match/mismatch,
    tree_size=0 / log_index≥tree_size reject, árvore 2-leaf (ambos
    índices), árvore 4-leaf (todos índices + tamper + hash malformado),
    proptest sobre árvores 4-leaf aleatórias (fail-closed para impostor
    leaf e root errado).
  - Plus: regression guard do `RekorParseError::display()` (legacy strings).
  Zero churn de produção (+752 LOC, só `#[cfg(test)]`). `cargo test -p
  forge-core-crypto` verde (44 lib + 7 zeroize_smoke + 1 ignored KAT),
  clippy pedantic limpo, fmt limpo. Workspace: 1 falha pré-existente
  (`operation_sidecar_e2e::execute_operation_rejects_outside_root_operation_path_before_read`)
  já falha em `b46d0bf2` — não regressão deste commit.
- **Commit 2.3 — OCSP helpers: cobertura unitária direta dos `pub(crate)`.**
  ✅ LANDED (34 testes inline). O crate já tinha cobertura E2E completa do
  entrypoint público OCSP (17 integration tests em `validate.rs` cobrindo
  good/revoked/unknown/expired/future/nonce/sig/responder-mismatch via DER
  assinado rcgen). O gap era cobertura unitária direta dos 11 helpers
  `pub(crate)` de `ocsp.rs` — só exercitados indiretamente. Cobertura por
  construção de structs `rasn-ocsp` em Rust puro (sem DER assinado):
  - `decode_ocsp_response`/`decode_basic_ocsp_response` — round-trip
    (encode→decode) + DER inválido → `None` + reason.
  - `verify_ocsp_single_response_freshness` — janela válida, this_update no
    futuro, next_update expirado, next_update ausente.
  - `apply_ocsp_cert_status` — Good/Revoked (revoked_at + reason Debug)/Unknown.
  - `extract_ocsp_response_nonce_hex` — nonce presente (double-wrapped
    OCTET STRING), extensões ausentes, OID não-nonce.
  - `verify_ocsp_nonce` — match/mismatch/missing/present-without-expectation/
    neither-supplied (todos os 5 ramos).
  - `normalize_expected_ocsp_nonce_hex` — lowercase, separadores (`:`/`-`/
    espaço), odd-length, caractere inválido, vazio.
  - `rasn_oid_matches` — match, prefix-only, arcos diferentes.
  - `ocsp_responder_id_matches_issuer` (ByKey) + `find_matching_ocsp_single_response`
    — match, serial mismatch, hash algorithm unsupported (com issuer cert
    rcgen real).
  - `verify_basic_ocsp_signature_with_issuer` — caminho negativo (sig sintética;
    happy-path já coberto no E2E).
  Adicionadas dev-deps `chrono` + `rasn-pkix` (workspace). Zero churn de
  produção. `cargo test -p forge-core-crypto` verde (78 lib + 7 zeroize_smoke
  + 1 ignored KAT), clippy pedantic limpo, fmt limpo.
- **Commit 2.4 — TUF trusted-root freshness.** PRÓXIMO. Version monotonicity,
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

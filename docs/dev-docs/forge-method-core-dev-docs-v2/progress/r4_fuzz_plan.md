# R4 — Fuzz Track Plan (skills-informed)

**Date**: 2026-06-30
**Tracks**: R4.1 → R4.6
**Skills ativas**: `improve-codebase-architecture` (aplicada em cada decisão de módulo), `grill-with-docs` (aplicada em cada terminology/ADR gate)

## Status snapshot

| Item | Estado |
|---|---|
| `cargo-fuzz` instalado localmente (0.13.2) | ✅ |
| `fuzz/` workspace criado | ✅ |
| Parsers `pub` direto nos módulos-fonte (alt B do deletion test) | ✅ |
| `recover_claim_wal_from_bytes` em store (deep) | ✅ |
| 4 harnesses `.rs` | ✅ |
| Corpus seed commitado (6+7+8+7 = 28 seeds) | ✅ |
| R4.6 DoD via CI Linux (ADR-0008) | ✅ |
| Runs locais em Windows-MSVC | ❌ bloqueado (ver ADR-0008) |

## Decisão de plataforma (ADR-0008)

`cargo-fuzz` no Windows-MSVC tem duas limitações conhecidas:
1. ASAN default: `STATUS_DLL_NOT_FOUND` (DLL dinâmica não embarcada no toolchain).
2. `-s none` (coverage-only): link error `__stop___sancov_pcs` undefined.

Decisão: fuzz roda em CI GitHub Actions Linux (cron diário + `workflow_dispatch`
+ label `fuzz` em PRs). Harnesses + seeds + workflow commitados; ADR-0008
registra o trade-off. Ver `adrs/ADR-0008-fuzz-runs-on-linux-ci-not-windows-local.md`.

## Como rodar localmente (Linux / WSL com Rust nightly)

```bash
cd fuzz
cargo +nightly fuzz run <target> -- -max_total_time=60
# targets: parse_signed_checkpoint, parse_rekor_log_entry,
#          decode_ocsp_response, decode_prefix
```

Crashes são salvos em `fuzz/artifacts/<target>/`.

## TODO — R4 com skills como etapas explícitas

### R4.1 — Infra ✅

- [x] Instalar `cargo-fuzz 0.13.2`
- [x] `cargo fuzz init` + renomear package pra `forge-method-core-fuzz`
- [x] `fuzz/Cargo.toml` com 4 `[[bin]]`, deps path, profiles `panic = "unwind"`
- [x] `[workspace]` vazio em `fuzz/Cargo.toml` (não vazar pro workspace principal)
- [x] **[`improve-codebase-architecture`]** Aplicada alternativa B: tornar os
      4 parsers `pub` direto nos módulos-fonte (`rekor::parse_*`,
      `ocsp::decode_ocsp_response`). Wrapper `pub mod fuzz` removido por
      falhar no deletion test (shallow pass-through).
- [x] `feature = "fuzz"` em `forge-core-store` mantida (deep: expõe
      `recover_claim_wal_from_bytes` que reusa `decode_prefix` em buffer
      in-memory, sem equivalente na API de filesystem).
- [x] **[`grill-with-docs`] Terminology gate**: "fuzz exposure" NÃO entra em
      `CONTEXT.md` (detalhe de implementação, não conceito de domínio).
- [x] `cargo check -p forge-core-crypto` verde
- [x] `cd fuzz && cargo check` verde
- [x] Commit `9b31150`

### R4.2 — `parse_signed_checkpoint` ✅

- [x] Harness chama `forge_core_crypto::rekor::parse_signed_checkpoint`
- [x] 6 seeds sintéticos cobrindo: valid shape, no-separator, bad treesize,
      truncated note, minimal valid, alternative `--` signature prefix
- [x] **[`grill-with-docs`] ADR gate**: pulado (reversível, óbvio)
- [x] **[`improve-codebase-architecture`] Deletion test no harness**: mantém
      (precisa de `#![no_main]` + `fuzz_target!` macro)
- [x] Commit `0d00008`

### R4.3 — `parse_rekor_log_entry` ✅

- [x] 7 seeds sintéticos cobrindo: full valid shape, invalid JSON, bad base64,
      missing verification, missing inclusionProof, missing hashes array,
      empty object
- [x] **[`grill-with-docs`] ADR gate**: pulado (igual R4.2)
- [x] Commit `8f7d43d`

### R4.4 — `decode_ocsp_response` ✅

- [x] 8 DER/ASN.1 seeds: empty, minimal SEQUENCE, truncated length, garbage,
      malformed long length, SHA1 OID, responseStatus success, long outer
- [x] **[`grill-with-docs`] ADR-0008 candidate**: SIM — escolha de plataforma
      (CI Linux vs Windows local) é hard-to-reverse, surpreendente sem
      contexto, e tem real trade-off. Criado ADR-0008.
- [x] Commit (R4.6 batch)

### R4.5 — `decode_prefix` (WAL binário) ✅

- [x] 7 binary seeds: empty, too short, wrong magic, wrong version, invalid
      flags, truncated payload, wrong CRC (IEEE vs Castagnoli)
- [x] Nota: seed com CRC Castagnoli válido fica deferido — exigiria um
      helper `#[ignore]` em `forge-core-store` que escreve o seed via
      `encode_record`. O path de rejeição já é coberto pelos seeds atuais.
- [x] Commit (R4.6 batch)

### R4.6 — DoD ✅

- [x] 4 targets com harnesses + seeds commitados
- [x] CI workflow `.github/workflows/fuzz.yml`: cron diário + manual + label `fuzz`
- [x] **[`grill-with-docs`] ADR-0008** criado
- [x] Commit final (este)

## Anti-padrões evitados (lições da skill)

1. **Shallow wrappers** que só delegam — aplicado alternativa B (pub direto).
2. **Múltiplos wrappers pra mesma fn** — não criados.
3. **Feature `fuzz` que vaza tipos públicos** — isolada em store, removida de crypto.
4. **Corpus dinâmico** gerado em runtime por testes — commitado estático.
5. **Workarounds Windows frágeis** — escolhida plataforma Linux/CI em vez de
   perseguir DLL ASAN ausente ou símbolos de coverage undefined.

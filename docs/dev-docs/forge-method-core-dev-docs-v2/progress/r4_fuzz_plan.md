# R4 — Fuzz Track Plan (skills-informed)

**Date**: 2026-06-30
**Tracks**: R4.1 → R4.6
**Skills ativas**: `improve-codebase-architecture` (aplicada em cada decisão de módulo), `grill-with-docs` (aplicada em cada terminology/ADR gate)

## Status snapshot

| Item | Estado |
|---|---|
| `cargo-fuzz` instalado (0.13.2) | ✅ |
| `fuzz/` workspace criado | ✅ |
| `feature = "fuzz"` em crypto + store | ✅ |
| `pub mod fuzz` em crypto (3 wrappers `()`) | ⚠️ SHALLOW — refazer |
| `recover_claim_wal_from_bytes` em store | ✅ DEEP (mantém) |
| 4 harnesses `.rs` criados | ✅ estrutura |
| Corpus seed | ❌ pendente |
| R4.6 DoD (60s/target sem panic) | ❌ pendente |

## TODO — R4 com skills como etapas explícitas

### R4.1 — Infra (parcialmente feito, refazer parte)

- [x] Instalar `cargo-fuzz 0.13.2`
- [x] `cargo fuzz init` + renomear package pra `forge-method-core-fuzz`
- [x] `fuzz/Cargo.toml` com 4 `[[bin]]`, deps path com `features = ["fuzz"]`, profiles `panic = "unwind"`
- [x] `feature = "fuzz"` em `forge-core-crypto/Cargo.toml` e `forge-core-store/Cargo.toml`
- [ ] **[`improve-codebase-architecture`] REFAZER `pub mod fuzz` em crypto**: expor tipos públicos sob a feature em vez de wrappers `()`. Passa no deletion test.
  - Alternativa A (preferida): re-export `pub use crate::rekor::{parse_*, ParsedRekorEntry, ParsedCheckpoint, RekorParseError}` dentro de `#[cfg(feature="fuzz")] pub mod fuzz`
  - Alternativa B: tornar `pub(crate)` → `pub` sob `#[cfg(feature="fuzz")]` nos próprios módulos
  - Decisão: **A**, porque isola a exposição no namespace `fuzz::` e mantém o módulo-fonte limpo
- [x] Manter `recover_claim_wal_from_bytes` como está (deep, retorna tipo)
- [ ] **[`grill-with-docs`] Terminology gate**: "fuzz exposure" é um termo que merece entrar em `CONTEXT.md`? Verificar.
  - Resposta provável: **não** — é detalhe de implementação, não conceito de domínio. Pular.
- [ ] Validar `cargo check -p forge-core-crypto --features fuzz` verde
- [ ] Validar `cargo check -p forge-core-store --features fuzz` verde

### R4.2 — `parse_signed_checkpoint` (mais isolado)

- [ ] Atualizar `fuzz/fuzz_targets/parse_signed_checkpoint.rs` pra chamar `forge_core_crypto::fuzz::parse_signed_checkpoint(data)` e usar o `Result` retornado (`.ok()` é suficiente)
- [ ] Gerar seed corpus estático de `crates/forge-core-cli/tests/validate.rs:366` → `fuzz/corpus/parse_signed_checkpoint/seed1.txt`
- [ ] **[`grill-with-docs`] ADR gate**: decisões hard-to-reverse/surprising/real-tradeoff?
  - Resposta: não — harness é reversível e óbvio. Pular.
- [ ] **[`improve-codebase-architecture`] Deletion test no harness**: se eu deletar o `.rs`, complexity reaparece no `Cargo.toml` `[[bin]]`. Mantém.
- [ ] `cargo fuzz run parse_signed_checkpoint -- -max_total_time=30` sem panic
- [ ] Commit `R4.2: parse_signed_checkpoint fuzz harness`

### R4.3 — `parse_rekor_log_entry` (JSON+base64 duplo)

- [ ] Mesmo padrão R4.2
- [ ] Seed de `validate.rs:327-403` (rekor_entry_fixture)
- [ ] **[`grill-with-docs`] ADR gate**: pular (igual R4.2)
- [ ] `cargo fuzz run parse_rekor_log_entry -- -max_total_time=30`
- [ ] Commit `R4.3: parse_rekor_log_entry fuzz harness`

### R4.4 — `decode_ocsp_response` (DER/ASN.1 via rasn)

- [ ] Mesmo padrão
- [ ] Seed de `validate.rs:654-699` (`ocsp_response_der`)
- [ ] **[`grill-with-docs`] ADR candidate**: usar `--sanitizer=address` por default, considerar `memory` depois. Real tradeoff (overhead vs cobertura), surpreendente sem contexto, hard to reverse (precisa rebuild). **Possível ADR-0002**.
  - Decidir depois de ver o primeiro crash (ou não-crash) no R4.6.
- [ ] `cargo fuzz run decode_ocsp_response -- -max_total_time=30`
- [ ] Commit `R4.4: decode_ocsp_response fuzz harness`

### R4.5 — `decode_prefix` (WAL binário com CRC)

- [ ] Harness já criado, valida
- [ ] Seed de `claim_wal.rs:103-135` (escreve records reais) → precisa dumpear pro corpus
- [ ] `cargo fuzz run decode_prefix -- -max_total_time=30`
- [ ] Commit `R4.5: decode_prefix (WAL) fuzz harness`

### R4.6 — DoD (definition of done)

- [ ] 4 targets rodando 60s cada sem panic
- [ ] `fuzz/corpus/*/` comitado (1+ seed por target)
- [ ] **[`grill-with-docs`] ADR-0002 decision**: sanitizer choice. Decidir基于 R4.6 results.
- [ ] Documentar `cargo fuzz` invocation no README de dev-docs
- [ ] Commit `R4.6: fuzz DoD + docs`

## Anti-padrões a evitar (lições da skill)

1. **Shallow wrappers** que só delegam — sempre retornar o tipo real do módulo-fonte
2. **Múltiplos wrappers pra mesma fn** com assinaturas diferentes — fragmenta a interface
3. **Feature `fuzz` que vaza tipos públicos** fora do namespace `fuzz::` — manter isolado
4. **Corpus dinâmico** gerado em runtime por testes — commitar estático pra reprodutibilidade

# R4 Fuzz Inventory — Alvos candidatos

**Date**: 2026-06-30
**Output of**: R4 planning sub-agent (session `b4e34de0-39b5-444c-ba96-b6cb2b6fb828`)
**Read-only**: nenhuma edição foi feita; só leitura.

## Sumário executivo

| Item | Status |
|---|---|
| Toolchain | `rustc 1.94.0` (stable) — compatível com `cargo-fuzz` |
| `cargo-fuzz` instalado | ❌ não (`error: no such command: fuzz`) |
| Dir `fuzz/` na raiz | ❌ não existe |
| `panic = "abort"` no release | ⚠️ conflita com cargo-fuzz, mas fuzz crate tem profile próprio |
| Visibilidade dos 4 alvos | ⚠️ todos `pub(crate)` ou privados — exige feature `fuzz` |

## Alvos candidatos (priorizados)

| # | Função | Arquivo:linha | Visibilidade | Input | Complexidade | Impacto |
|---|---|---|---|---|---|---|
| 1 | `parse_rekor_log_entry` | `rekor.rs:114` | `pub(crate)` | `&str` (JSON) | BAIXA | Alto |
| 2 | `parse_signed_checkpoint` | `rekor.rs:263` | `pub(crate)` | `&str` | BAIXA | Alto |
| 3 | `decode_ocsp_response` | `ocsp.rs:30` | `pub(crate)` | `&[u8]` + 2 mut Vec | MÉDIA | Alto |
| 4 | `decode_basic_ocsp_response` | `ocsp.rs:48` | `pub(crate)` | `&[u8]` + 2 mut Vec | MÉDIA | Alto |
| 5 | `decode_prefix` (WAL) | `claim_wal.rs:1818` | privada | `&Path` + `&[u8]` | BAIXA | Alto |
| 6 | `decode_record_frame` | `claim_wal.rs:1916` | privada | `&[u8]` + usize | BAIXA | Alto |
| 7 | `parse_effect_wal_records_for_recovery` | `lib.rs:1418` | privada | `&str` (NDJSON) | BAIXA | Médio |
| 8 | `validate_operation` | `forge-core-validate/lib.rs:1098` | `pub` | struct tipada | ALTA | Médio |
| 9 | `validate_command` | `forge-core-validate/lib.rs:1063` | `pub` | struct tipada | ALTA | Médio |
| 10 | `validate_claim` | `forge-core-validate/lib.rs:1194` | `pub` | struct tipada | ALTA | Médio |

**Observação**: `parse_workflow_graph_yaml` citado no roadmap **não existe**. O parsing YAML no CLI é indireto via `read_yaml_value` (`forge-core-store/src/lib.rs:2412`), privada e lê do disco. Não incluir na track R4 sem refactor.

## Detalhe dos 4 alvos do roadmap

### Nomenclatura — mapa roadmap → real

| Roadmap | Real | Status |
|---|---|---|
| `parse_rekor_log_entry` | `parse_rekor_log_entry` | ✅ existe com esse nome |
| `parse_signed_checkpoint` | `parse_signed_checkpoint` | ✅ existe com esse nome |
| `claim_wal_decode` | `decode_prefix` (privada) | ⚠️ ajustar nome; `recover_claim_wal` (pub) faz I/O |
| `ocsp_response_decode` | `decode_ocsp_response` | ⚠️ ordem invertida |

### Alvo 1 — `parse_rekor_log_entry` (`rekor.rs:114`)

```rust
pub(crate) fn parse_rekor_log_entry(text: &str) -> Result<ParsedRekorEntry, RekorParseError>
```

- **Input**: `&str` JSON com `body` (base64), `logID`, `logIndex` (i64), `integratedTime` (i64), `verification.inclusionProof.{hashes,logIndex,rootHash,treeSize,checkpoint}`.
- **Complexidade**: BAIXA — feed direto como `&str`.
- **Porque fuzzer acharia bugs**: parser JSON aninhado + base64 duplo-decoding. `logIndex`/`integratedTime` i64 podem ser negativos.
- **Panics arriscados downstream**: `rekor.rs:221` `u64::try_from(entry.proof.log_index).expect("log_index checked non-negative")` em `verify_rekor_entry_inclusion` — panicar se passar `logIndex` negativo pela cadeia completa. Fuzzar isoladamente.
- **Seed**: `crates/forge-core-cli/tests/validate.rs:327-403` (`rekor_entry_fixture`).

### Alvo 2 — `parse_signed_checkpoint` (`rekor.rs:263`)

```rust
pub(crate) fn parse_signed_checkpoint(checkpoint: &str) -> Result<ParsedCheckpoint, RekorParseError>
```

- **Input**: `&str` note-format: `<origin>\n<treeSize>\n<rootHash_b64>\n[extra]\n\n— <name> <sig_b64>`.
- **Complexidade**: BAIXA — feed direto como `&str`.
- **Porque fuzzer acharia bugs**: parsing manual por `split_once("\n\n")` + `split('\n')` + destructuring `[origin, tree_size, root_hash_b64, other @ ..]`. `decode_checkpoint_signature` faz `decoded[4..]` indexing (protegido por check `> 4`).
- **Panics**: nenhum — todo o caminho usa `?`, `ok_or`, slices pré-validadas. Bom alvo "limpo".
- **Seed**: `crates/forge-core-cli/tests/validate.rs:366-375` (checkpoint válido inline).

### Alvo 3 — `decode_ocsp_response` (`ocsp.rs:30`)

```rust
pub(crate) fn decode_ocsp_response(
    der: &[u8],
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<OcspResponse>
```

- **Input**: `&[u8]` DER + 2 `&mut Vec::new()`.
- **Complexidade**: MÉDIA.
- **Porque fuzzer acharia bugs**: parsing ASN.1/DER via `rasn::der::decode`. DER malformado, TLV truncado, OIDs inválidos, recursão ASN.1 profunda, integers com leading zeros — clássicos. `rasn` é relativamente novo.
- **Panics**: nenhum neste parser — só `match` em Ok/Err.
- **Seed**: `crates/forge-core-cli/tests/validate.rs:654-699` (`ocsp_response_der`).

### Alvo 4 — `decode_prefix` (WAL, `claim_wal.rs:1818`)

```rust
fn decode_prefix(wal_path: &Path, bytes: &[u8]) -> ClaimWalRecovery   // PRIVADA
```

- **Input**: `&[u8]` no formato binário frame-based: magic `FMW1` (4B) + version (1B) + record_type (1B) + flags u16 + seq u64 + payload_len u32 + header_crc u32 + payload JSON + payload_crc u32.
- **Complexidade**: BAIXA em si, **exige refactor prévio** — função privada.
- **Porque fuzzer acharia bugs**: parsing binário com checksums CRC32C, conversões `u32::from_le_bytes`/`u64::from_le_bytes`, `usize::try_from`, `checked_add` em offsets.
- **Panics (todos protegidos por checks anteriores)**:
  - `claim_wal.rs:1931` `.expect("4 byte payload length")` — protegido
  - `claim_wal.rs:1938` `.expect("4 byte header crc")` — protegido
  - `claim_wal.rs:1955` `.expect("4 byte payload crc")` — protegido
  - `claim_wal.rs:1963` `.expect("8 byte seq")` — protegido
- **Seed**: `crates/forge-core-store/tests/claim_wal.rs:103-135` (escreve records reais); `:303,329,370,405` (corruption cases já prontos).

## Estrutura de diretórios sugerida

```
fuzz/
├── Cargo.toml
├── targets/
│   ├── parse_rekor_log_entry.rs
│   ├── parse_signed_checkpoint.rs
│   ├── decode_ocsp_response.rs
│   └── decode_prefix.rs
└── corpus/
    ├── parse_rekor_log_entry/      ← dump de validate.rs:376
    ├── parse_signed_checkpoint/    ← dump de validate.rs:366
    ├── decode_ocsp_response/       ← dump de validate.rs:698
    └── decode_prefix/              ← dump de claim_wal.rs:103
```

Não existem fixtures estáticos committed em `docs/fixtures/` — todos gerados em runtime pelos testes. **R4.1 deve commitar uma vez** a saída dessas funções geradoras como arquivos estáticos em `fuzz/corpus/<target>/` para acelerar o primeiro ciclo.

## Bloqueadores a resolver antes de começar

1. **`panic = "abort"` em `profile.release`** (`Cargo.toml:70`): `cargo-fuzz` depende de unwind. O `fuzz/` crate tem seu próprio profile (gerado por `cargo fuzz init`) que sobrescreve isso localmente — validar com um fuzz target dummy antes dos 4 alvos.

2. **Visibilidade `pub(crate)` dos 4 alvos**: o `fuzz/` crate vive fora de `forge-core-crypto`/`forge-core-store`, então não vê `pub(crate)`. Opção recomendada (A): feature flag.
   ```rust
   #[cfg(feature = "fuzz")]
   pub use crate::rekor::{parse_rekor_log_entry, parse_signed_checkpoint};
   ```
   Para `decode_prefix` (privada): criar wrapper `pub fn recover_claim_wal_from_bytes(bytes: &[u8]) -> ClaimWalRecovery { decode_prefix(Path::new("<fuzz>"), bytes) }` sob a mesma feature.

3. **`cargo-fuzz` ausente**: rodar `cargo install cargo-fuzz` antes. Validar que está no mesmo ambiente WSL que compila os crates (rustc 1.94 stable).

## Sanitizers

- `--sanitizer=address|thread|memory|none` (default: `address`).
- ASan funciona bem em WSL/Linux. Recomendação: começar com `address` para os 4 alvos.
- Para `decode_ocsp_response` (DER), considerar `--sanitizer=memory` num segundo momento (stacks ASN.1 têm histórico de uninit-read).

## Convenções do projeto

- R4 **não adiciona** `anyhow`/`thiserror` — harnesses usam só `libfuzzer_sys::fuzz_target!` e descartam `Result`/`Option`.
- Harnesses não devem `panic!`/`unwrap` em código próprio — só chamar o alvo. Qualquer panic observado é bug do código sob fuzz.

## Ordem de execução recomendada

1. **R4.2** `parse_signed_checkpoint` (mais isolado, sem panic conhecido, harness de ~5 linhas) → valida infra end-to-end.
2. **R4.3** `parse_rekor_log_entry` (JSON+base64 duplo).
3. **R4.4** `decode_ocsp_response` (DER/ASN.1 via rasn).
4. **R4.5** `decode_prefix` (exige refactor feature flag, mas maior valor — parsing binário com CRC e offsets).

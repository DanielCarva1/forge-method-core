# R5 Crypto Inventory — Material to zeroize

**Date**: 2026-06-30
**Scope**: `crates/forge-core-crypto/src/{rekor.rs, ocsp.rs, host_adapter_verification.rs, lib.rs}`
**Output of**: R5 planning sub-agent (session `a819db1a-d30c-4c17-a085-95e84b14b3ed`)
**Read-only**: nenhuma edição foi feita; só leitura.

## 0. Estado das dependências

| Item | Status |
|---|---|
| `zeroize` no `Cargo.toml` do crate | **AUSENTE** |
| `zeroize` no `[workspace.dependencies]` | **AUSENTE** |
| `zeroize` transitivo no `Cargo.lock` | Presente v1.9.0 (via `curve25519-dalek 4.1.3`, `elliptic-curve 0.13.8`) |
| `subtle` no `Cargo.lock` | Presente v2.6.1 |
| `ed25519-dalek 2.2.0` | Tem feature `zeroize`, **mas desligada** no manifesto do crate |
| `p256 0.13.2` (features `ecdsa`, `pem`) | `elliptic-curve 0.13.8` puxa `zeroize` unconditional |

**Ação R5.1**: adicionar ao workspace:
```toml
zeroize = { version = "1.9", features = ["derive"] }
subtle = "2.6"
ed25519-dalek = { version = "2.2.0", features = ["zeroize"] }
p256 = { version = "0.13.2", features = ["ecdsa", "pem", "zeroize"] }
```

## 1. `rekor.rs`

| # | Linha | Tipo atual | Categoria | Wrapping recomendado | Breaking API? |
|---|---|---|---|---|---|
| R1 | `rekor.rs:111` `ParsedCheckpoint.signatures` field | `Vec<Vec<u8>>` | SIGNATURE_BYTES | `Vec<Zeroizing<Vec<u8>>>` | NÃO (`pub(crate)`) |
| R2 | `rekor.rs:250` `P256Signature::from_der` | `p256::ecdsa::Signature` | THIRD_PARTY | Não precisa wrap se feature ligada | NÃO (local) |
| R3 | `rekor.rs:188,237` `rekor_key: &P256VerifyingKey` | borrow | PUBLIC_KEY | Caller em host_adapter_verification.rs é ponto de wrapping | NÃO |
| R4 | `rekor.rs:127` `body_bytes` | `Vec<u8>` | PREHASH (baixa) | Opcional `Zeroizing<Vec<u8>>` | NÃO |
| R5 | `rekor.rs:196-203` `canonical_body` | `Vec<u8>` | PREHASH (baixa) | Opcional | NÃO |
| R6 | `rekor.rs:322-326` `rfc6962_leaf_hash` content | `Vec<u8>` | PREHASH (baixa) | Opcional | NÃO |
| R7 | `rekor.rs:368-372` `hash_merkle_node` content | `Vec<u8>` | PREHASH (baixa) | Opcional | NÃO |
| R8 | `rekor.rs:380-383` `hex_to_bytes` return | `Vec<u8>` | PREHASH (baixa) | Opcional | NÃO |

**Sites de comparação (R5.5)**:
- `rekor.rs:243` `checkpoint.root_hash != normalize_sha256_display(&proof.root_hash)` — hex String eq
- `rekor.rs:339` `leaf_hash == root_hash` (caso tree_size == 1) — hex String eq
- `rekor.rs:358` `computed == root_hash` — **verificação Merkle central** — usar `ConstantTimeEq` sobre bytes

## 2. `ocsp.rs`

| # | Linha | Tipo atual | Categoria | Wrapping recomendado | Breaking API? |
|---|---|---|---|---|---|
| O1 | `ocsp.rs:159` `issuer_key_hash` local | `Vec<u8>` SHA-1 SPKI | PUBLIC_KEY | `Zeroizing<Vec<u8>>` | NÃO (local) |
| O2 | `ocsp.rs:222-227` `issuer_name_hash`, `issuer_key_hash` | `Vec<u8>` digests | PUBLIC_KEY + PREHASH | `Zeroizing<Vec<u8>>` | NÃO (locais) |
| O3 | `ocsp.rs:368-388` `ocsp_digest_for_algorithm` return | `Vec<u8>` | PREHASH | `Zeroizing<Vec<u8>>` | SIM leve (privada) |
| O4 | `ocsp.rs:390-394` `sha1_digest` return | `Vec<u8>` 20 bytes | PREHASH | `Zeroizing<Vec<u8>>` | SIM leve (privada) |
| O5 | `ocsp.rs:72-94` `tbs_der`, `algorithm_der`, `signature_der` | `Vec<u8>` | SIGNATURE/PREHASH | `signature_der` → `Zeroizing<Vec<u8>>` (alta) | NÃO (locais) |
| O6 | `ocsp.rs:109` `signature: asn1_rs::BitString` | third-party | SIGNATURE | `asn1_rs::BitString` não impl `Zeroize`; extrair bytes p/ `Zeroizing<Box<[u8]>>` | NÃO |
| O7 | `ocsp.rs:35-45` `decode_ocsp_response` return | `rasn_ocsp::OcspResponse` | THIRD_PARTY | `rasn_ocsp` sem `Zeroize`; mitigação downstream | SIM `pub(crate)` |
| O8 | `ocsp.rs:53-63` `decode_basic_ocsp_response` return | `rasn_ocsp::BasicOcspResponse` | THIRD_PARTY | Mesma situação que O7 | SIM `pub(crate)` |

**Sites de comparação (R5.5)**:
- `ocsp.rs:148` `name_der == issuer.subject().as_raw()` — não secret-dependent, manter
- `ocsp.rs:160` fingerprint SHA-1 eq — não secret-dependent, manter
- `ocsp.rs:185` serial comparison como decimal **String** — **bug latente** (leading zeros, ASN.1 INTEGER signedness), sinalizar como bug de corretude + CT
- `ocsp.rs:229-230` issuer hashes eq — não secret-dependent, manter
- `ocsp.rs:327` `expected == observed` nonce OCSP em hex String — **candidato R5.5**, decodificar hex → bytes → `ConstantTimeEq`
- `ocsp.rs:398` OID arc match — não-cripto, manter

## 3. `host_adapter_verification.rs`

| # | Linha | Tipo atual | Categoria | Wrapping recomendado | Breaking API? |
|---|---|---|---|---|---|
| H1 | L64-73 `artifact_bytes` | `Option<Vec<u8>>` | PREHASH | `Zeroizing<Vec<u8>>` | NÃO (local) |
| H2 | L180-188 `signature_bytes`, `public_key_bytes`, etc | `Option<Vec<u8>>` | SIGNATURE/PUBLIC_KEY | `signature_bytes`/`public_key_bytes` → `Zeroizing<Vec<u8>>` | NÃO (locais); indireto via `read_signature_file` em file_io.rs |
| H4 | L302-303 `public_key_bytes` | `Option<Vec<u8>>` | PUBLIC_KEY | `Zeroizing<Vec<u8>>` | NÃO (local) |
| H5 | L316-328 `rekor_key: P256VerifyingKey` | third-party | PUBLIC_KEY | Com feature `zeroize` em p256 → já zeroiza | NÃO |
| H7 | L486-503 `leaf_der`, `issuer_ders` | `Option<Vec<u8>>` | PUBLIC_KEY | `Zeroizing<Vec<u8>>` | SIM leve via `read_certificate_der` |
| H8 | L614 `artifact_bytes`, L619 `bundle_bytes` | `Option<Vec<u8>>` | PREHASH/SIGNATURE | `bundle_bytes` → `Zeroizing<Vec<u8>>` | NÃO (locais) |
| H13 | L827-832 `certificate_der` (DSSE) | `Option<Vec<u8>>` | PUBLIC_KEY | `Zeroizing<Vec<u8>>` | NÃO (local) |
| H15 | L1103-1108 `certificate_der` (TSA) | `Option<Vec<u8>>` | PUBLIC_KEY | `Zeroizing<Vec<u8>>` | NÃO (local) |
| H16 | L1264-1269 `certificate_der` (CT SCT) | `Option<Vec<u8>>` | PUBLIC_KEY | `Zeroizing<Vec<u8>>` | NÃO (local) |
| H17 | L1282-1288 `sct_bytes: Vec<(PathBuf, Vec<u8>)>` | bytes SCT (contém sig ECDSA) | SIGNATURE | `Vec<(PathBuf, Zeroizing<Vec<u8>>)>` | NÃO (local) |
| H18 | L1290-1300 `ct_log_material` | struct com `key`/`id` | PUBLIC_KEY + PREHASH | `#[derive(Zeroize, ZeroizeOnDrop)]` em `CertificateTransparencyLogMaterial` | SIM `pub(crate)` |
| H20 | L1684-1701 `certificate_der`, `issuer_der`, `crl_der` | `Option<Vec<u8>>` | PUBLIC_KEY/SIGNATURE | `Zeroizing<Vec<u8>>` para os três | NÃO (locais) |
| H22 | L1886-1903 `certificate_der`, `issuer_der`, `ocsp_der` | `Option<Vec<u8>>` | PUBLIC_KEY/SIGNATURE | `Zeroizing<Vec<u8>>`; `ocsp_der` prioritário | NÃO (locais) |
| **H23** | **L1851-1854 `expected_nonce_hex: Option<String>`** | String hex | **SECRET_BYTES (nonce OCSP!)** | **`Option<Zeroizing<String>>`** — **alta prioridade** | **SIM field público** |
| **H24** | **L1855,1975-1979 `observed_nonce_hex: Option<String>`** | String hex | **SECRET_BYTES** | **`Option<Zeroizing<String>>`** — **alta prioridade** | **SIM field público** |

## 4. `lib.rs`

- L71 `pub use file_io::{read_public_key_file, read_required_file, read_signature_file};` — re-exporta helpers que retornam `Option<Vec<u8>>`. Mudar pra `Option<Zeroizing<Vec<u8>>>` é breaking para `forge-core-cli`, `tests/`, `forge-contract-validator` (track R5.7).
- L61-63 re-exporta `host_adapter_types::*` — fields `*_nonce_hex` são breaking (track R5.8).

## 5. Estratégia em 3 fases

### Fase A (não-breaking) — tracks R5.1 a R5.5
- Adicionar deps + features (R5.1)
- Wrap variáveis locais em rekor/ocsp/host_adapter_verification (R5.2-R5.4)
- Constant-time compares (R5.5)

### Fase B (`pub(crate)` breaking, sem bump externo) — track R5.6
- `ParsedCheckpoint.signatures`, `read_certificate_der`, structs internas

### Fase C (API pública breaking, pre-1.0 OK) — tracks R5.7, R5.8
- `read_signature_file`/`read_public_key_file`/`read_required_file` returns
- `HostAdapterCertificateOcspStatusVerification` fields `*_nonce_hex`

### Limites do inventário (FOLLOW-UP — tracks R5.10/R5.11)
Os 5 arquivos adjacentes devem ser inventariados antes de expandir:
- `sigstore.rs` (`ParsedBundle.signature`, `verify_ed25519_signature` internals, `CertificateTransparencyLogMaterial`, `read_certificate_der`)
- `file_io.rs` (`read_signature_file`, `read_public_key_file`, `read_required_file`)
- `hashing.rs` (`hex_sha256`, `hex_bytes`, `normalize_sha256_*`)
- `slsa_transparency.rs` (`verify_transparency_log_proof`)
- `tuf.rs` (`verify_tuf_metadata_freshness_role`)

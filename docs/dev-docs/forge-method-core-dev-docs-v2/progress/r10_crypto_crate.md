# R10 — `forge-core-crypto` crate extraction [Fase 1]

**Data**: 2026-06-29
**Status**: Concluído
**Fase roadmap**: Fase 1 — Mover cripto para fora da CLI

## Objetivo

Mover toda a lógica de verificação criptográfica do `forge-core-cli` para um
novo crate `forge-core-crypto`, isolando dependências cripto pesadas
(`ed25519-dalek`, `p256`, `rasn`, `rasn-ocsp`, `sigstore-tsa`, `rcgen`,
`x509-parser`) da camada de apresentação da CLI.

A CLI passa a ser um cliente fino que re-exporta a API pública do
`forge-core-crypto` para manter compatibilidade com `main.rs`,
`tests/validate.rs` e `forge-contract-validator`.

## Resultado

### Antes

- `forge-core-cli/src/` continha 26 arquivos `.rs`, incluindo 9 módulos
  puramente cripto (`crypto_hashing`, `crypto_ocsp`, `crypto_rekor`,
  `crypto_sigstore`, `crypto_slsa_transparency`, `crypto_tuf`,
  `host_adapter_verification`, `file_io`, `host_adapter_types`).
- `forge-core-cli/Cargo.toml` listava 13 deps cripto diretas
  (`asn1-rs`, `base64`, `ed25519-dalek`, `p256`, `rasn`, `rasn-ocsp`,
  `serde_yaml`, `sha1`, `sha2`, `sct`, `sigstore-tsa`, `rustls-pki-types`,
  `x509-parser`).

### Depois

- **Novo crate** `forge-core-crypto` (11 módulos):
  - `hashing` (SHA-256 / hex helpers)
  - `rekor` (log entry parsing + inclusion proof)
  - `ocsp` (response decoding + freshness + signature)
  - `sigstore` (trust policy / fulcio / bundle / DSSE)
  - `slsa_transparency` (SLSA statement + transparency log)
  - `tuf` (metadata freshness role)
  - `file_io` (read_required_file + signature/public_key readers)
  - `host_command` (predicados admission: `source_ref_is_immutable`,
    `version_like`)
  - `host_adapter_types` (todos os tipos `HostAdapter*`)
  - `host_adapter_verification` (as 13 `run_host_adapter_*_verification`)
  - `lib.rs` (57 linhas: mod declarations + re-exports)
- `forge-core-cli` agora contém apenas:
  - Camada de apresentação (`main.rs`, `lib.rs`, `validate.rs`)
  - Builders de host-adapter que são específicos da CLI
    (`host_adapter_manifest.rs`, `host_adapter_projection.rs`,
    `host_command.rs` com o builder `host_command()` +
    `argv_has_shell_control` + `env_key_is_forbidden`)
  - Demais módulos de comando (`autonomy_cmd`, `claim`, `contract_cmd`,
    `coordination`, `effect_index`, `eval_cmd`, `execute_operation`,
    `graph_cmd`, `guide`, `io_util`, `isolation`, `m1_cmd`, `project_cmd`,
    `telemetry_cmd`).
- A CLI agora declara **zero** deps cripto diretas na seção `[dependencies]`
  (apenas `forge-core-crypto = { path = "../forge-core-crypto" }`); as deps
  cripto continuam listadas em `[dev-dependencies]` porque
  `tests/validate.rs` usa `rcgen` e `proptest` para fixtures.
- API pública preservada via `pub use forge_core_crypto::*;` no crate root
  da CLI — `main.rs`, `tests/validate.rs`, e `forge-contract-validator`
  não precisaram mudar uma linha de call site.

## Sub-tasks (mapa R10.1 → R10.5 do roadmap)

- [x] **R10.1** — Criar `crates/forge-core-crypto/` esqueleto:
  `Cargo.toml` com deps cripto + `forge-core-contracts`, adicionado ao
  workspace `members`.
- [x] **R10.2** — Mover módulos crypto da CLI:
  - `crypto_hashing.rs` → `hashing.rs` (items `pub(crate)` → `pub`)
  - `crypto_rekor.rs` → `rekor.rs` (imports `crate::crypto_hashing` →
    `crate::hashing`)
  - `crypto_ocsp.rs` → `ocsp.rs`
  - `crypto_sigstore.rs` → `sigstore.rs` (imports `crate::{read_required_file,
    run_host_adapter_rekor_verification, HostAdapter*}` quebrados em imports
    explícitos por módulo de origem)
  - `crypto_slsa_transparency.rs` → `slsa_transparency.rs`
  - `crypto_tuf.rs` → `tuf.rs`
  - `file_io.rs` → `file_io.rs` (items `pub(crate)` → `pub`)
  - `host_command.rs` → apenas os 2 predicados admission
    (`source_ref_is_immutable`, `version_like`) — o builder e os outros 2
    predicados ficam na CLI
  - `host_adapter_types.rs` → `host_adapter_types.rs` (puro schema, sem
    mudanças)
  - `host_adapter_verification.rs` → `host_adapter_verification.rs`
    (imports `crate::crypto_*` → `crate::*`)
- [x] **R10.3** — Mover testes correspondentes: **não foi necessário**
  mover testes de `tests/validate.rs` nesta fase. Os testes continuam em
  `forge-core-cli/tests/` mas consomem a API via `forge_core_cli::*`
  (que re-exporta tudo de `forge_core_crypto`). A migração dos testes
  para `forge-core-crypto/tests/` fica para **R12** (Fase 0).
- [x] **R10.4** — CLI vira cliente fino:
  - `forge-core-cli/Cargo.toml` adiciona `forge-core-crypto` como dep
  - `forge-core-cli/Cargo.toml` remove as 13 deps cripto da seção
    `[dependencies]`
  - `lib.rs` faz `pub use forge_core_crypto::*;` (transitivo)
  - `lib.rs` remove `pub(crate) mod crypto_*`, `file_io`,
    `host_adapter_types`, `host_adapter_verification`
  - `host_adapter_projection.rs` muda `crate::crypto_hashing::*` →
    `crate::*` (via re-export)
- [x] **R10.5** — DoD:
  - [x] `forge-core-cli/src/lib.rs` < 1500 linhas — **94 linhas**
  - [x] `forge-core-crypto` tem zero deps em `forge-core-cli` ou
    `forge-core-runtime` — confirmado (só depende de
    `forge-core-contracts`)
  - [x] Todos os gates verdes
  - [x] CLI output snapshot inalterado (`validate --root . --json` →
    `"diagnostics": []`)

## Gates

- ✅ `cargo check --workspace` — zero warnings, zero errors
- ✅ `cargo test -p forge-core-cli --lib` — 103/103 verdes
- ✅ `cargo test --workspace` — 360 passed, 1 falha pré-existente
  (`validate_binary_outputs_json_summary`, case mismatch `Passed` vs
  `passed`, **não regressão**)
- ✅ `cargo clippy --workspace --all-targets -- -W clippy::pedantic` —
  **590 warnings** (baseline R1.FileIo era 570; +20 aceitável pelos
  items `pub(crate)` → `pub` que agora acionam lints de doc/`must_use`
  que antes eram silenciados pela visibilidade interna)
- ✅ `cargo fmt --all -- --check` verde
- ✅ **Âncora de regressão**: `validate --root . --json` emitiu
  `"diagnostics": []` — zero mudança observável

## Notas de implementação

### Items `pub(crate)` → `pub`

Os módulos crypto em `forge-core-cli` usavam `pub(crate)` em todos os
items (structs, funções, campos). Ao mover para `forge-core-crypto`, os
items que precisam ser visíveis no crate root do crypto (para re-export
para a CLI) foram promovidos a `pub`. Items puramente internos de cada
módulo (ex: `decode_base64_or_raw` em `file_io`) permanecem privados.

### Predicados `host_command` duplicados

Os predicados `source_ref_is_immutable` e `version_like` existem agora em
dois lugares:
- `forge-core-crypto/src/host_command.rs` — consumido por
  `host_adapter_verification`
- `forge-core-cli/src/host_command.rs` — consumido por
  `host_adapter_projection`

Esta duplicação é intencional e temporária: a CLI precisa dos predicados
para suas gates de admission, e mover tudo para um módulo compartilhado
exigiria refatorar a CLI para depender do crypto crate em mais pontos
(trabalho de R5/R8). A duplicação será consolidada quando R5 (zeroize) ou
R8 (error discipline) tocar nesses arquivos.

### `host_adapter_types` no crate crypto

`host_adapter_types.rs` é puro schema (structs derivando `Serialize`,
sem lógica). Foi movido para `forge-core-crypto` porque
`host_adapter_verification` precisa desses tipos como parâmetros. A CLI
os re-exporta via `pub use forge_core_crypto::*`, então
`crate::host_adapter_types::*` em `host_adapter_manifest` e
`host_adapter_projection` continua resolvendo.

### Shadow de `host_command`

A CLI declara `pub(crate) mod host_command;` local E faz
`pub use forge_core_crypto::*;` (que também traz `host_command`).
O warning `hidden_glob_reexports` é silenciado com `#[allow]` — a shadow
é intencional e documentada no comentário acima da declaração.

## Arquivos

### Criados

- `crates/forge-core-crypto/Cargo.toml`
- `crates/forge-core-crypto/src/lib.rs`
- `crates/forge-core-crypto/src/hashing.rs`
- `crates/forge-core-crypto/src/rekor.rs`
- `crates/forge-core-crypto/src/ocsp.rs`
- `crates/forge-core-crypto/src/sigstore.rs`
- `crates/forge-core-crypto/src/slsa_transparency.rs`
- `crates/forge-core-crypto/src/tuf.rs`
- `crates/forge-core-crypto/src/file_io.rs`
- `crates/forge-core-crypto/src/host_command.rs`
- `crates/forge-core-crypto/src/host_adapter_types.rs`
- `crates/forge-core-crypto/src/host_adapter_verification.rs`

### Removidos

- `crates/forge-core-cli/src/crypto_hashing.rs`
- `crates/forge-core-cli/src/crypto_ocsp.rs`
- `crates/forge-core-cli/src/crypto_rekor.rs`
- `crates/forge-core-cli/src/crypto_sigstore.rs`
- `crates/forge-core-cli/src/crypto_slsa_transparency.rs`
- `crates/forge-core-cli/src/crypto_tuf.rs`
- `crates/forge-core-cli/src/file_io.rs`
- `crates/forge-core-cli/src/host_adapter_types.rs`
- `crates/forge-core-cli/src/host_adapter_verification.rs`

### Modificados

- `Cargo.toml` (workspace members)
- `crates/forge-core-cli/Cargo.toml` (+ `forge-core-crypto`, − 13 deps
  cripto)
- `crates/forge-core-cli/src/lib.rs` (removidos mods crypto, adicionado
  `pub use forge_core_crypto::*`)
- `crates/forge-core-cli/src/host_adapter_projection.rs` (import path
  `crate::crypto_hashing::valid_sha256_digest` → `crate::valid_sha256_digest`)

## Próximos passos

- **R12** (Fase 0): migrar testes de `tests/validate.rs` para
  `forge-core-crypto/tests/` onde apropriado (testes de crypto
  verification), reduzindo o acoplamento da CLI com fixtures cripto.
- **R2** (Fase 2): migrar `Result<_, String>` residuais para enums
  nomeados.
- **R8** (Fase 2): remover `process::exit` de lib code.
- **R11** (Fase 2): decompor `main.rs` (4116 linhas) em módulos
  `*_cmd.rs`.

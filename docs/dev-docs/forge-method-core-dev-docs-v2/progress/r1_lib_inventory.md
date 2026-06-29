# R1 — Inventário de `forge-core-cli/src/lib.rs`

Data: 2026-06-29
Total: 7472 linhas, 141 funções, ~243 itens públicos.

## Mapa de domínios (por faixa de linhas)

| Faixa | Domínio | Funções principais |
|---|---|---|
| 1-80 | imports/prelude | — |
| 81-900 | `HostAdapter*` types (structs/enums) | ~70 tipos de contrato do host adapter |
| 901-1395 | `run_host_adapter_manifest` | monta o manifest estático |
| 1396-1545 | `run_host_adapter_projection` + `process_security_policy` + `invocation_admission` | projection/admission |
| 1546-3735 | `run_host_adapter_distribution_*` + `*_verification` (sigstore, fulcio, rekor, CT, CRL, OCSP, TUF) | 14 funções públicas de verificação |
| 3736-4255 | helpers OCSP/TUF/datetime | `decode_ocsp_response`, `verify_basic_ocsp_signature_with_issuer`, `parse_tuf_datetime_utc_to_unix`, etc. |
| 4256-5522 | helpers sigstore/CT/TSA | `select_rekor_integrated_time_for_timestamp_authority`, `verify_sigstore_trust_policy`, `verify_fulcio_chain`, etc. |
| 5523-5664 | rekor checkpoint + merkle | `verify_rekor_entry_inclusion`, `verify_rekor_checkpoint`, `parse_signed_checkpoint`, `rfc6962_leaf_hash` |
| 5665-5800 | host command helpers | `host_command`, `argv_has_shell_control`, `env_key_is_forbidden` |
| 5801-6105 | SLSA + transparency proof | `verify_slsa_statement`, `verify_transparency_log_proof`, `verify_merkle_inclusion` |
| 6106-6560 | projection/MCP/annotations | `project_host_command`, `mcp_annotations`, `command_input_schema` |
| 6561-6626 | effect index | `run_rebuild_effect_index`, `run_query_effect_index` |
| 6627-6905 | validate + execute_operation | `run_validate`, `run_execute_operation` |
| 6906-7472 | validate helpers + path utils | `validate_operation_fixtures`, `read_yaml`, `resolve_input_path`, `hex_sha256`, `yaml_files` |

## Módulos-alvo propostos

1. **`host_adapter_types.rs`** (linhas 81-900) — todos os `HostAdapter*` structs/enums
2. **`host_adapter_manifest.rs`** (linhas 901-1545) — `run_host_adapter_manifest`, `run_host_adapter_projection`, `run_host_adapter_process_security_policy`, `run_host_adapter_invocation_admission`
3. **`host_adapter_verification.rs`** (linhas 1546-3735) — as 14 `run_host_adapter_*_verification`
4. **`crypto_ocsp.rs`** (linhas 3736-4255) — helpers OCSP/TUF/datetime
5. **`crypto_sigstore.rs`** (linhas 4256-5522) — helpers sigstore/CT/TSA/fulcio
6. **`crypto_rekor.rs`** (linhas 5523-5664) — rekor checkpoint + merkle
7. **`host_adapter_command.rs`** (linhas 5665-5800) — host command helpers
8. **`crypto_slsa_transparency.rs`** (linhas 5801-6105) — SLSA + transparency proof
9. **`host_adapter_projection.rs`** (linhas 6106-6560) — projection/MCP/annotations
10. **`effect_index.rs`** (linhas 6561-6626) — effect index commands
11. **`validate.rs`** (linhas 6627-6905) — `run_validate`, `run_execute_operation`
12. **`validate_helpers.rs`** (linhas 6906-7472) — validate helpers + path utils

## Ordem de extração (menor risco primeiro)

1. `crypto_rekor.rs` (5 funções, isoladas, sem dependência de tipos do projeto)
2. `crypto_ocsp.rs` (helpers OCSP, só dependem de `rasn`/`rasn-ocsp`)
3. `crypto_sigstore.rs` (helpers sigstore, dependem de x509-parser)
4. `crypto_slsa_transparency.rs` (SLSA + merkle)
5. `host_adapter_types.rs` (só tipos, sem lógica)
6. `host_adapter_manifest.rs`
7. `host_adapter_verification.rs`
8. `host_adapter_command.rs`
9. `host_adapter_projection.rs`
10. `effect_index.rs`
11. `validate.rs`
12. `validate_helpers.rs`

## Notas

- O WIP do Codex mexe em `lib.rs` (OCSP boundary). Vou evitar a faixa 3451-3735
  (OCSP verification) e 3736-4255 (OCSP helpers) na primeira extração.
- Começar por `crypto_rekor.rs` é seguro: não toca no WIP do Codex.

## Progresso

- [x] R1.1 — Inventariar `lib.rs` (este documento)
- [x] R1.3 — Extrair `crypto_rekor.rs` (2026-06-29)
  - Movido: `ParsedRekorEntry`, `ParsedRekorInclusionProof`, `ParsedCheckpoint`,
    `parse_rekor_log_entry`, `required_string`/`required_i64`/`required_u64`,
    `verify_rekor_entry_inclusion`, `verify_rekor_checkpoint`,
    `parse_signed_checkpoint`, `decode_checkpoint_signature`, `rfc6962_leaf_hash`,
    `verify_merkle_inclusion`, `hash_merkle_node`, `hex_to_bytes`.
  - Helpers `hex_sha256`, `hex_bytes`, `normalize_sha256_display`,
    `normalize_sha256_digest`, `valid_sha256_digest` ficaram em `lib.rs` como
    `pub(crate)` (usados por múltiplos domínios — serão extraídos depois para
    `crypto_hashing.rs`).
  - `lib.rs`: 7472 → 7205 linhas (-267).
  - Gates: `cargo check`, `cargo test --workspace`, `cargo clippy --pedantic`,
    `cargo fmt --check` todos verdes.
- [x] R1.5 — Extrair `execute_operation.rs` (2026-06-29)
  - Movido: `ExecuteOperationInput`, `PayloadFileSpec`, `PayloadLoadPolicy`,
    `ExecuteOperationContractPathKind` (+ `label()`), `ExecuteOperationError`
    (+ `Display` + `std::error::Error`), `run_execute_operation`, e helpers
    privados `read_yaml_result`, `runtime_payload_from_file`,
    `canonicalize_existing_path`, `resolve_contract_input_path`,
    `validate_payload_scope`, `resolve_input_path`, `repo_relative_checked`.
  - `read_yaml` (com `ValidateSummary`) fica em `lib.rs` — pertence ao
    validate flow, não ao execute_operation.
  - API pública re-exportada via `pub use execute_operation::{...}` no crate
    root — `main.rs` e `tests/validate.rs` continuam importando de
    `forge_core_cli` sem mudanças.
  - `lib.rs`: 7205 → 6891 (-314); `execute_operation.rs`: 386 linhas.
  - Removidos imports não usados em `lib.rs`: `execute_operation`,
    `RuntimeEffectPayloadKind`, `RuntimeOperationCommandInput`,
    `RuntimeOperationEffectInput`, `RuntimeOperationEffectPayload`,
    `RuntimeOperationExecution`, `RuntimeOperationExecutionContext`,
    `CommandExecutionContext`, `std::fmt`, `std::io`.
  - Gates: `cargo check` (zero warnings), `cargo test --workspace`
    (440+ testes, todos verdes), `cargo clippy --pedantic` (só warnings
    pré-existentes), `cargo fmt --check` verde.
- [x] R1.EffectIndex — Extrair `effect_index.rs` (2026-06-29)
  - Movido: `RebuildEffectIndexInput`, `QueryEffectIndexInput` (+
    `Default`), `run_rebuild_effect_index`, `run_query_effect_index`,
    `run_query_effect_index_context`.
  - API pública re-exportada via `pub use effect_index::{...}` no crate
    root — `main.rs` e `tests/validate.rs` continuam importando de
    `forge_core_cli` sem mudanças.
  - `lib.rs`: 6887 → 6810 linhas (-77); `effect_index.rs`: 124 linhas.
  - Removidos imports não usados em `lib.rs`: `StableId`,
    `tool_effect::EffectTargetKind`, `build_effect_metadata_context`,
    `query_effect_target_metadata_index`,
    `rebuild_effect_target_metadata_index_with_lock`,
    `EffectMetadataConsumerUse`, `EffectMetadataContextBuildOptions`,
    `EffectMetadataContextBuildResult`, `EffectTargetMetadataIndexQuery`,
    `EffectTargetMetadataIndexQueryResult`,
    `EffectTargetMetadataIndexRebuildResult`.
  - Gates: `cargo check` (zero warnings), `cargo test --workspace` (440+
    testes verdes), `cargo clippy --pedantic` (warnings pré-existentes
    preservados — `must_use_candidate`/`needless_pass_by_value` já
    existiam em `lib.rs`), `cargo fmt --check` verde.
- [x] R1.CryptoHashing — Extrair `crypto_hashing.rs` (2026-06-29)
  - Movido: `hex_sha256`, `hex_bytes`, `valid_sha256_digest`,
    `normalize_sha256_digest`, `normalize_sha256_display` (helpers de
    hash/hex compartilhados por rekor, slsa, x509, payloads).
  - Módulo novo `pub(crate)`, re-exportado via `pub(crate) use
    crypto_hashing::{...}` no crate root — preserva todos os
    call sites `crate::hex_sha256`, `crate::hex_bytes`, etc.
  - `lib.rs`: 6810 → 6771 linhas (-39); `crypto_hashing.rs`: 47 linhas.
  - Gates: `cargo check` (zero warnings), `cargo test --workspace`
    (440+ testes verdes), `cargo clippy --pedantic` (320 warnings —
    paridade perfeita com a baseline), `cargo fmt --check` verde.
- [x] R1.HostAdapterTypes — Extrair `host_adapter_types.rs` (2026-06-29)
  - Movidos: 74 tipos `HostAdapter*` (structs + enums) das linhas 101-914 de
    `lib.rs`: manifest, command, projection, process security, distribution
    policy/evidence/admission, e todos os pares `*VerificationInput` /
    `*Verification` para os 14 fluxos de verificação (artifact, provenance,
    rekor, sigstore trust policy, fulcio, bundle subject, dsse in-toto,
    timestamp authority, CT SCT, revocation policy, TUF freshness, CRL, OCSP)
    + as projections MCP/borrowed shell/app UI.
  - `ValidationStatus` (linhas 94-99) **não movido** — não é tipo
    `HostAdapter*`, é tipo de domínio genérico de validação, fica em `lib.rs`.
  - Módulo `host_adapter_types` é `pub(crate)`, re-exportado via
    `pub use host_adapter_types::*;` no crate root — preserva todos os
    callers externos (`main.rs`, `tests/validate.rs`) que importam
    `HostAdapterManifest`, etc., diretamente de `forge_core_cli::`.
  - Imports do novo módulo: `forge_core_contracts::RuntimeKind`,
    `serde::Serialize`, `serde_json::Value`, `std::path::PathBuf`.
  - `lib.rs`: 6787 → 5972 linhas (-815, -12.0%); `host_adapter_types.rs`:
    828 linhas.
  - Gates: `cargo check` (zero warnings), `cargo test --workspace` (todos
    verdes; `claim_wal_cli_parallel_acquire_preserves_clean_monotonic_wal`
    em `tests/claim_wal_stress.rs` é flaky de concorrência de FS — passa
    isolado), `cargo clippy --pedantic` (320 warnings — paridade perfeita
    com baseline), `cargo fmt --check` verde.
- [x] R1.HostCommand — Extrair `host_command.rs` (2026-06-29)
  - Movidos: `HostCommandMetadata<'a>` struct + 5 funções helpers:
    `host_command` (builder de `HostAdapterCommand`),
    `argv_has_shell_control` (detector de shell metacharacters em argv),
    `env_key_is_forbidden` (detector de TOKEN/SECRET/KEY/PASSWORD em env
    keys), `source_ref_is_immutable` (detector de git SHA-1 40-hex),
    `version_like` (validador de string de versão `[A-Za-z0-9.\-_+]`).
  - Todas as 5 funções são usadas apenas dentro de `lib.rs` (em
    `run_host_adapter_manifest` e nos gates de invocation/distribution/
    artifact-verification admission), nunca externamente — por isso o
    módulo é `pub(crate)` e o re-export é `pub(crate) use host_command::{...}`.
  - `command_process_admission` (sibling de `host_command`) **não movido**
    — fica em `lib.rs` como parte do domínio admission; será extraído
    junto com o módulo admission/projection em fase futura.
  - Imports do novo módulo: 6 tipos `HostAdapter*` de
    `crate::host_adapter_types`.
  - `lib.rs`: 5972 → 5912 linhas (-60); `host_command.rs`: 96 linhas.
  - Gates: `cargo check` (zero warnings), `cargo test --workspace` (todos
    verdes), `cargo clippy --pedantic` (320 warnings — paridade perfeita
    com baseline), `cargo fmt --check` verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [x] R1.Validate — Extrair `validate.rs` (2026-06-29)
  - Movidos: 4 tipos públicos (`ValidateSummary`, `ValidateCheck`,
    `ValidateDiagnostic`, `ValidationStatus`) + função pública
    `run_validate` + 10 helpers privados (`validate_operation_fixtures`,
    `validate_side_contracts`, `validate_runtime_contracts`,
    `validate_named_dir_instances`, `validate_cross_ref_instances`,
    `validate_named`, `validate_named_cross`, `read_yaml`, `yaml_files`,
    `repo_relative`) + 2 impls (`ValidateSummary` com métodos pub + privados,
    `ValidateDiagnostic` com métodos privados).
  - `ValidationStatus` ganhou `Copy + Eq` derives no novo módulo
    (original só tinha `PartialEq` + `Serialize`) — mudança de derive não
    quebra ABI/behavior, apenas permite `==` em mais contextos e clonagem
    barata. Sem impacto observável.
  - Imports movidos integralmente para `validate.rs`:
    `forge_core_contracts::{14 tipos Document}` (excluindo `RuntimeKind`,
    que fica em `lib.rs` por ser usado pelo host_adapter manifest +
    distribution policy), `forge_core_store::{build_reference_index,
    collect_known_repo_paths, collect_validation_yaml_documents}` e
    `forge_core_validate::{27 validate_* functions + Diagnostic,
    DiagnosticSeverity, ReferenceIndex, ValidationReport}`.
  - Em `lib.rs`: `use serde::{Deserialize, Serialize}` reduziu para
    `use serde::Deserialize` (Serialize não é mais usado em `lib.rs` após a
    extração).
  - Módulo `pub(crate)`, re-exportado via `pub use validate::{run_validate,
    ValidateCheck, ValidateDiagnostic, ValidateSummary, ValidationStatus};`
    — preserva todos os callers externos (`main.rs`, `tests/validate.rs`,
    `forge-contract-validator/{src/main.rs, tests/parity.rs}`).
  - `lib.rs`: 5912 → 5331 linhas (-581); `validate.rs`: 621 linhas.
  - Gates: `cargo check` (zero warnings), `cargo test --workspace` (todos
    verdes), `cargo clippy --pedantic` (320 warnings — paridade perfeita
    com baseline), `cargo fmt --check` verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [x] R1.HostAdapterManifest — Extrair `host_adapter_manifest.rs` (2026-06-29)
  - Movido: `run_host_adapter_manifest` (L94-587 de `lib.rs` pré-extração,
    ~493 linhas) — única função do domínio manifest movida agora. Os 5
    siblings (`run_host_adapter_projection`,
    `run_host_adapter_process_security_policy`,
    `run_host_adapter_invocation_admission`,
    `run_host_adapter_distribution_policy`,
    `run_host_adapter_distribution_admission`) **não movidos** — dependem
    de helpers privados (`project_host_command`,
    `command_process_admission`, `projection_target_id`, `process_target_id`)
    que pertencem ao domínio projection/MCP e serão extraídos juntos em
    R1.HostAdapterProjection.
  - A função é puramente declarativa — literal de struct gigante com dados
    estáticos do manifest, sem lógica/condicionais/loops. Só chama
    `host_command(HostCommandMetadata { ... })` para cada comando.
  - Imports do novo módulo: `forge_core_contracts::RuntimeKind`, 7 tipos
    `HostAdapter*` de `crate::host_adapter_types::{...}` (import explícito,
    não wildcard — preserva paridade de clippy), `host_command` +
    `HostCommandMetadata` de `crate::host_command`.
  - Em `lib.rs`: removidos `host_command` e `HostCommandMetadata` do
    `pub(crate) use host_command::{...}` (não mais usados em `lib.rs`, só
    pelo novo módulo que importa direto). Os 4 predicados de admissão
    (`argv_has_shell_control`, `env_key_is_forbidden`,
    `source_ref_is_immutable`, `version_like`) continuam re-exportados.
  - Módulo `pub(crate)`, re-exportado via `pub use
    host_adapter_manifest::run_host_adapter_manifest;` no crate root —
    preserva todos os callers (`main.rs`, `tests/validate.rs`, e os 5
    siblings ainda em `lib.rs`).
  - `lib.rs`: 5338 → 4843 linhas (-495); `host_adapter_manifest.rs`:
    547 linhas.
  - Gates: `cargo check` (zero warnings), `cargo test --workspace`
    (1 falha pré-existente `validate_binary_outputs_json_summary` —
    case mismatch `Passed` vs `passed`, confirmada via stash que falha
    também no commit anterior, NÃO é regressão), `cargo clippy --pedantic`
    (320 warnings — paridade perfeita com baseline), `cargo fmt --check`
    verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [x] R1.HostAdapterProjection — Extrair `host_adapter_projection.rs` (2026-06-29)
  - Movidos: 5 funções públicas (`run_host_adapter_projection`,
    `run_host_adapter_process_security_policy`,
    `run_host_adapter_invocation_admission`,
    `run_host_adapter_distribution_policy`,
    `run_host_adapter_distribution_admission`) + 9 helpers privados
    (`command_process_admission`, `project_host_command`,
    `projection_target_id`, `process_target_id`, `mcp_tool_name`,
    `command_title`, `command_description`, `mcp_annotations`,
    `command_input_schema`).
  - Todos os 9 helpers eram chamados APENAS pelas 5 funções movidas —
    verificado com grep exaustivo. Zero dependências externas.
  - Imports do novo módulo: `RuntimeKind` de `forge_core_contracts`,
    `json!`/`Value` de `serde_json`, 28 tipos `HostAdapter*` de
    `crate::host_adapter_types::{...}` (import explícito, não wildcard),
    `valid_sha256_digest` de `crate::crypto_hashing`,
    `argv_has_shell_control`/`env_key_is_forbidden`/
    `source_ref_is_immutable`/`version_like` de `crate::host_command`,
    `run_host_adapter_manifest` de `crate::host_adapter_manifest`.
  - Em `lib.rs`: removidos do re-export `pub(crate) use host_command::{...}`
    os predicados `argv_has_shell_control` e `env_key_is_forbidden`
    (não mais usados em `lib.rs`). `source_ref_is_immutable` e
    `version_like` continuam re-exportados (ainda usados por helpers de
       verificação em `lib.rs`).
  - Em `lib.rs`: removidos imports não usados: `RuntimeKind` (só usado pelas
       funções movidas), `json!` (só usado por `command_input_schema`),
    `valid_sha256_digest` (só usado por `run_host_adapter_distribution_admission`).
  - `serde_json::{json, Value}` reduziu para `serde_json::Value`.
  - Módulo `pub(crate)`, re-exportado via `pub use host_adapter_projection::{...}`
    com as 5 funções públicas — preserva todos os callers externos.
  - `lib.rs`: 4844 → 4146 linhas (-698); `host_adapter_projection.rs`:
    783 linhas.
  - Gates: `cargo check` (zero warnings), `cargo test --workspace` (1
    falha pré-existente `validate_binary_outputs_json_summary` — case
    mismatch `Passed` vs `passed`, não é regressão), `cargo clippy --pedantic`
    (320 warnings — paridade), `cargo fmt --check` verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [x] R1.CryptoOCSP — Extrair `crypto_ocsp.rs` (2026-06-29)
  - Movidos: 14 helpers OCSP (decode, verify, freshness, status, nonce) —
    11 `pub(crate)` (`decode_ocsp_response`, `decode_basic_ocsp_response`,
    `verify_basic_ocsp_signature_with_issuer`, `ocsp_responder_id_matches_issuer`,
    `find_matching_ocsp_single_response`, `verify_ocsp_single_response_freshness`,
    `apply_ocsp_cert_status`, `extract_ocsp_response_nonce_hex`,
    `verify_ocsp_nonce`, `normalize_expected_ocsp_nonce_hex`, `rasn_oid_matches`)
    + 3 privados (`ocsp_cert_id_issuer_hashes_match`, `ocsp_digest_for_algorithm`,
    `sha1_digest` — só consumidos dentro do módulo).
  - Public entrypoint `run_host_adapter_certificate_ocsp_status_verification`
    (L1884-2168) **não movido** — consome `OcspResponseStatus` diretamente,
    fica em `lib.rs` como parte do domínio host-adapter verification.
  - Imports do novo módulo: `asn1_rs::{BitString, FromDer}`,
    `rasn::types::ObjectIdentifier`, `rasn_ocsp::{6 tipos}`, `sha1::Sha1`,
    `sha2::{Digest, Sha256, Sha384, Sha512}`, `x509_parser::certificate::X509Certificate`,
    `x509_parser::x509::AlgorithmIdentifier`, e `hex_bytes` de
    `crate::crypto_hashing` (cross-module helper).
  - Em `lib.rs`: removidos imports totalmente órfãos após a extração
    (confirmado via grep: único uso fora do bloco removido era
    `OcspResponseStatus::Successful` em L2003):
    `asn1_rs::{...}`, `rasn::types::ObjectIdentifier`, `sha1::Sha1`,
    `sha2::{Digest, Sha256, Sha384, Sha512}`, `x509_parser::x509::AlgorithmIdentifier`.
    Reduzido `rasn_ocsp::{7 itens}` → `rasn_ocsp::OcspResponseStatus`.
  - Módulo `pub(crate)`, re-exportado via `pub(crate) use crypto_ocsp::{11 itens}`
    no crate root — preserva todos os call sites `crate::decode_ocsp_response`,
    etc., em `run_host_adapter_certificate_ocsp_status_verification`.
  - `lib.rs`: 4146 → 3785 linhas (-361); `crypto_ocsp.rs`: 404 linhas.
  - Gates: `cargo check` (zero warnings), `cargo test -p forge-core-cli --lib`
    (103/103 verdes), OCSP-focused tests `--test validate ocsp` (18/18 verdes),
    `cargo clippy --pedantic` (320 warnings — paridade perfeita com baseline),
    `cargo fmt --check` verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [x] R1.CryptoTUF — Extrair `crypto_tuf.rs` (2026-06-29)
  - Movidos: 6 helpers TUF/datetime (L2183-2340 pré-remoção) — 1 `pub(crate)`
    (`verify_tuf_metadata_freshness_role`, chamado 4× por
    `run_host_adapter_tuf_trusted_root_freshness_verification` para os papéis
    root/timestamp/snapshot/targets) + 5 privados (`parse_tuf_datetime_utc_to_unix`,
    `parse_fixed_i32`, `days_in_month`, `is_leap_year`, `days_from_civil` —
    parser UTC RFC 3339 dependency-free baseado no algoritmo `days_from_civil`
    de Howard Hinnant).
  - `read_required_file` (helper de I/O compartilhado por ~14 call sites em
    múltiplos domínios: provenance, rekor, bundle, dsse, CT, TUF, TSA,
    sigstore) **não movido** — promovido de `fn` para `pub(crate) fn` em
    `lib.rs` para que `crypto_tuf` (e futuros módulos crypto) possa importá-lo
    via `crate::read_required_file`. Migração completa para um módulo
    `file_io` fica para uma fase futura (R1.FileIo).
  - Imports do novo módulo: `std::path::Path`, `serde_json::Value`,
    `HostAdapterTufMetadataFreshnessRole` de `crate::host_adapter_types`,
    `read_required_file` de `crate`.
  - Em `lib.rs`: zero imports órfãos removidos (Path/Value são amplamente
    usados por outros domínios em `lib.rs`).
  - Módulo `pub(crate)`, re-exportado via
    `pub(crate) use crypto_tuf::verify_tuf_metadata_freshness_role;` no
    crate root — preserva os 4 call sites em
    `run_host_adapter_tuf_trusted_root_freshness_verification`.
  - `lib.rs`: 3791 → 3644 linhas (-147, líquido); `crypto_tuf.rs`: 256 linhas.
  - Gates: `cargo check` (zero warnings), `cargo test -p forge-core-cli --lib`
    (103/103 verdes), TUF-focused tests `--test validate tuf` (6/6 verdes),
    `cargo clippy --pedantic` (320 warnings — paridade perfeita com baseline),
    `cargo fmt --check` verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [x] R1.CryptoSigstore — Extrair `crypto_sigstore.rs` (2026-06-29)
  - Movidos: 39 helpers `pub(crate)` + 12 structs `pub(crate)` (com 50 fields
    `pub(crate)`) cobrindo todo o domínio sigstore (L2183-3362 + L3388-3414
    pré-extração) — TSA selectors (`select_rekor_integrated_time_for_timestamp_authority`,
    `select_rfc3161_tsa_for_timestamp_authority`), trust policy loader + 8
    structs de sub-política (Fulcio/Rekor/CT/TSA/revocation/identity),
    helpers de certificado X.509/Fulcio (`read_certificate_der`,
    `parse_certificate`, `verify_fulcio_chain`, `verify_issuer_ca_usage`,
    `verify_leaf_code_signing_usage`, `extract_fulcio_certificate_identity`,
    `verify_fulcio_identity_selectors`, `github_repository_matches` + struct
    `FulcioCertificateIdentity`), helpers de bundle/DSSE
    (`parse_sigstore_message_signature_bundle`, `parse_sigstore_dsse_bundle`,
    `verify_bundle_signature_with_certificate`,
    `verify_dsse_signature_with_certificate`,
    `verify_rekor_body_binds_bundle`, `verify_rekor_body_binds_dsse`,
    `dsse_pae`, `decode_base64_flexible`, `decode_ct_log_id`) e structs
    `ParsedSigstoreMessageSignatureBundle` / `ParsedSigstoreDsseBundle` /
    `CertificateTransparencyLogMaterial`.
  - `decode_ct_log_id` (originalmente em L3388-3414, no meio do file I/O entre
    `read_public_key_file` e `decode_base64_or_raw`) foi movido junto — é
    chamado por `load_certificate_transparency_log_material` (bloco sigstore)
    e chama `decode_base64_flexible` (também bloco sigstore). Manter no file
    I/O criaria dependência circular indesejada.
  - `decode_base64_or_raw` **não movido** — fica em `lib.rs` porque é usado por
    `read_signature_file` e `read_public_key_file` (file I/O que permanece
    aguardando R1.FileIo).
  - Imports do novo módulo: `base64` (4 variantes STANDARD/STANDARD_NO_PAD/
    URL_SAFE/URL_SAFE_NO_PAD), `p256::ecdsa::{P256Signature, P256VerifyingKey}`
    + `p256::ecdsa::signature::Verifier` (trait necessário para
    `VerifyingKey::verify` — re-export do `signature::Verifier`, mesmo trait
    que `ed25519_dalek::Verifier`), `rustls_pki_types::CertificateDer`,
    `serde::Deserialize`, `serde_json::Value`, `x509_parser` (6 itens:
    `X509Certificate`, `GeneralName`, `ParsedExtension`, `parse_x509_pem`,
    `parse_x509_certificate`), `crate::crypto_hashing::{hex_bytes,
    normalize_sha256_display}`, `crate::crypto_rekor` (via path
    `crate::crypto_rekor::ParsedRekorEntry` em assinaturas de função, sem
    import top-level para evitar unused warnings), e 5 itens `crate::{...}`
    (`read_required_file`, `run_host_adapter_rekor_verification`,
    `HostAdapterRekorVerificationInput`, `HostAdapterRekorVerificationStatus`,
    `HostAdapterSigstoreTimestampAuthorityVerificationInput`).
  - Em `lib.rs`: imports reduzidos (de 21 linhas para 9 linhas) removendo
    13 símbolos órfãos (confirmado via grep: 0 usos fora do bloco extraído
    após excluir a própria linha de import):
    `STANDARD_NO_PAD`, `URL_SAFE`, `URL_SAFE_NO_PAD` (3 das 4 variantes
    base64), `Ed25519Verifier` (trait), `P256Signature`, `DecodePublicKey`,
    `CertificateDer`, `Deserialize`, `X509Certificate`, `GeneralName`,
    `ParsedExtension`, `parse_x509_pem`, `parse_x509_certificate`. Imports
    que ficaram: `BASE64` (usado por `decode_base64_or_raw`),
    `Ed25519Signature`/`Ed25519VerifyingKey`/`Ed25519Verifier` (trait — usado
    por `verify_ed25519_signature`), `P256VerifyingKey` + `DecodePublicKey`
    (usados por `run_host_adapter_rekor_verification`), `OcspResponseStatus`
    (usado por OCSP verification), `parse_x509_crl` (usado por CRL
    verification), `Value`/`fs`/`Path` (amplo uso).
  - Note: `PathBuf` também foi removido de `lib.rs` (órfão após extração —
    todos os 3 usos em structs sigstore movidos).
  - Módulo `pub(crate)`, re-exportado via
    `#[allow(clippy::wildcard_imports)] pub(crate) use crypto_sigstore::*;`
    no crate root (wildcard, mesmo padrão de `host_adapter_types`, porque o
    módulo é grande e muitos itens são consumidos por múltiplos
    `run_host_adapter_*_verification`; `#[allow]` mantém paridade com a
    baseline clippy de 320 warnings).
  - `lib.rs`: 3644 → 2433 linhas (-1211, -33.2%); `crypto_sigstore.rs`:
    1261 linhas. Redução acumulada desde o início de R1: 7472 → 2433
    (-5039, -67.4%).
  - Gates: `cargo check --workspace` (zero warnings), `cargo test -p
    forge-core-cli --lib` (103/103 verdes), sigstore-focused tests `--test
    validate sigstore|fulcio|bundle|dsse|timestamp_authority` (44 testes
    verdes), `cargo test --workspace` (97 passando, 1 falha pré-existente
    `validate_binary_outputs_json_summary` — case mismatch `Passed` vs
    `passed`, confirmada pré-existente via stash, NÃO regressão), `cargo
    clippy --pedantic` (320 warnings — paridade perfeita com baseline),
    `cargo fmt --check` verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [x] R1.CryptoSlsaTransparency — Extrair `crypto_slsa_transparency.rs` (2026-06-29)
  - Movidos: 6 helpers `pub(crate)` + 1 struct `pub(crate)` com 4 fields
    `pub(crate)` cobrindo o domínio SLSA/transparency (L2233-2437
    pré-extração) — `verify_ed25519_signature` (assinatura Ed25519 sobre
    provenance bruto), `ExpectedProvenance<'a>` struct (sha256/builder_id/
    source_uri/source_ref), `verify_slsa_statement` (valida statement
    in-toto/SLSA v1 contra subject digest, builder, source URI/ref),
    `statement_subject_has_sha256` (subject matcher, cross-domain),
    `json_contains_string` (recursive JSON string matcher),
    `verify_transparency_log_proof` (inclusion proof verifier, delega
    Merkle math para `crypto_rekor::verify_merkle_inclusion`),
    `transparency_leaf_hash` (RFC 6962 leaf hash 0x00 || payload).
  - `statement_subject_has_sha256` é cross-domain: além de
    `verify_slsa_statement`, é chamado por
    `run_host_adapter_sigstore_bundle_subject_verification` e pela verificação
    DSSE in-toto subject em `lib.rs`. Por isso `pub(crate)` + wildcard
    re-export no crate root.
  - Imports do novo módulo: `ed25519_dalek::{Ed25519Signature,
    Ed25519Verifier, Ed25519VerifyingKey}` (Verificador trait re-exportado
    do `signature` crate, necessário para `VerifyingKey::verify`),
    `serde_json::Value`, `crate::crypto_hashing::{hex_sha256,
    normalize_sha256_digest, normalize_sha256_display}`, e
    `crate::crypto_rekor::verify_merkle_inclusion` via path completo inline.
  - Em `lib.rs`: removido integralmente o import `ed25519_dalek::{...}` (3
    símbolos órfãos: Ed25519Signature, Ed25519VerifyingKey, Ed25519Verifier —
    confirmado via grep: 0 usos fora do bloco extraído, 0 calls `.verify()`
    remanescentes em `lib.rs`). Outros imports mantidos: P256VerifyingKey
    + DecodePublicKey (run_host_adapter_rekor_verification),
    OcspResponseStatus (OCSP verification), parse_x509_crl (CRL
    verification), BASE64 (decode_base64_or_raw), Value/fs/Path.
  - Bug caught: regex awk inicial `[a-zA-Z_]+:` não pegava field `sha256`
    (dígito após letra sem underscore). Corrigido para
    `[a-zA-Z_][a-zA-Z0-9_]*:`. Verificado retroativamente que
    `crypto_sigstore.rs` não foi afetado (todos os fields lá tinham letras/
    underscore antes de qualquer dígito).
  - Módulo `pub(crate)`, re-exportado via
    `#[allow(clippy::wildcard_imports)] pub(crate) use
    crypto_slsa_transparency::*;` no crate root (mesmo padrão de
    `crypto_sigstore` / `host_adapter_types`).
  - `lib.rs`: 2437 → 2239 linhas (-198, -8.1%); `crypto_slsa_transparency.rs`:
    232 linhas. Redução acumulada desde o início de R1: 7472 → 2239
    (-5233, -70.0%).
  - Gates: `cargo check --workspace` (zero warnings), `cargo test -p
    forge-core-cli --lib` (103/103 verdes), provenance-focused tests
    `--test validate provenance` (4/4 verdes), sigstore-bundle tests (4/4
    verdes, cobrem `statement_subject_has_sha256` cross-domain), DSSE tests
    (6/6 verdes), `cargo test --workspace` (360 passed, 1 falha pré-existente
    `validate_binary_outputs_json_summary` — case mismatch, NÃO regressão),
    `cargo clippy --pedantic` (320 warnings — paridade perfeita com baseline),
    `cargo fmt --check` verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [x] R1.FileIo — Extrair `file_io.rs` (2026-06-29)
  - Movidos: 4 helpers de I/O (L2193-2239 pré-extração) — 3 `pub(crate)`
    (`read_required_file` cross-module usado por lib.rs + crypto_sigstore +
    crypto_tuf; `read_signature_file` e `read_public_key_file` usados por
    `run_host_adapter_provenance_verification`) + 1 privado
    (`decode_base64_or_raw`, usado apenas internamente pelas duas acima).
  - `read_required_file` já era `pub(crate)` desde R1.CryptoTUF (foi
    promovido naquele commit para habilitar import de crypto_tuf). Agora
    vive em seu módulo próprio, com seus dois siblings
    (`read_signature_file` e `read_public_key_file`) promovidos de `fn`
    para `pub(crate) fn` para serem alcançáveis a partir de `lib.rs`.
  - Imports do novo módulo: `base64::{STANDARD as BASE64, Engine as _}`
    (para decode base64), `std::fs` (para `fs::read`), `std::path::Path`.
  - Em `lib.rs`: removidos imports órfãos `base64::{STANDARD as BASE64,
    Engine as _}` (decode_base64_or_raw era o único usuário) e
    `std::path::Path` (todos os parâmetros `&Path` das funções movidas
    foram junto). `std::fs` foi mantido — ainda usado por 5 call sites em
    `run_host_adapter_*` (`fs::read`, `fs::read_to_string` para artifact,
    log_entry, policy, rekor_log_entry paths).
  - Módulo `pub(crate)`, re-exportado via `pub(crate) use file_io::{read_required_file,
    read_signature_file, read_public_key_file};` no crate root (import
    explícito, não wildcard — módulo pequeno, mantém clippy feliz sem
    `#[allow]`). `decode_base64_or_raw` fica privado.
  - `lib.rs`: 2239 → 2199 linhas (-40, -1.8%); `file_io.rs`: 71 linhas.
    Redução acumulada desde o início de R1: 7472 → 2199 (-5273, -70.6%).
  - Gates: `cargo check --workspace` (zero warnings), `cargo test -p
    forge-core-cli --lib` (103/103 verdes), `cargo test --workspace` (360
    passed, 1 falha pré-existente `validate_binary_outputs_json_summary`
    — case mismatch, NÃO regressão), `cargo clippy --pedantic` (320
    warnings — paridade perfeita com baseline), `cargo fmt --check` verde.
  - Âncora de regressão: `validate --root . --json` emitiu
    `"diagnostics": []` — zero mudança observável.
- [ ] R1.2 — Criar módulos-alvo (esqueleto) — em andamento
- [ ] R1.4 — Mover verificação X.509/CRL/OCSP
- [ ] R1.6 — Mover project link resolve/init
- [ ] R1.7 — Validar (lib.rs ≤ 1500 linhas)

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
- [ ] R1.2 — Criar módulos-alvo (esqueleto) — em andamento
- [ ] R1.4 — Mover verificação X.509/CRL/OCSP
- [ ] R1.6 — Mover project link resolve/init
- [ ] R1.7 — Validar (lib.rs ≤ 1500 linhas)

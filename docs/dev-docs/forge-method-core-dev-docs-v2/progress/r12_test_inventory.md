# R12.1 — Inventory of `crates/forge-core-cli/tests/validate.rs`

Date: 2026-06-29
Total tests: **98**
Total lines: **5215**
  - Test bodies: ~3593 lines
  - Non-test scaffolding (imports, fixtures, helpers): ~1622 lines

## Summary by category

| Category     | Tests | Lines (body) | % of test body |
|--------------|-------|--------------|----------------|
| contract-flow| 14    | ~753         | ~21.0%         |
| crypto-flow  | 54    | ~1665        | ~46.4%         |
| cli-flow     | 30    | ~1175        | ~32.7%         |
| mixed        | 0     | 0            | 0%             |

Notes on classification rules used:

- A test counts as **cli-flow** when its primary assertion is about CLI presentation:
  the `--json` envelope shape, exit codes, argv parsing, or error formatting produced by
  spawning the `forge-core` binary via `Command::new(env!("CARGO_BIN_EXE_forge-core"))`.
  Several of these also assert contract-field values in the JSON (e.g.
  `host_adapter_distribution_policy_binary_outputs_json` checks `default_admission == "blocked"`);
  they are still classified cli-flow because the dominant concern is the CLI surface, not the
  underlying policy.
- A test counts as **crypto-flow** when it exercises real verification logic against fixtures
  (Ed25519 / p256 signatures, x509 chains, OCSP DER parsing, CRL parsing, CT SCT signatures,
  Rekor inclusion proofs / signed checkpoints, RFC3161 timestamp tokens, TUF metadata freshness,
  Sigstore bundle subject binding). This is the cluster targeted for the future
  `forge-core-crypto` crate (post-R10).
- A test counts as **contract-flow** when it asserts non-crypto policy/admission boundaries:
  `run_validate`, `run_host_adapter_manifest`, `run_host_adapter_projection`,
  `run_host_adapter_process_security_policy`, `run_host_adapter_invocation_admission`,
  `run_host_adapter_distribution_policy`, `run_host_adapter_distribution_admission`,
  `run_host_adapter_artifact_verification` (digest + metadata policy only; signature is deferred),
  `run_execute_operation` payload policy, and effect-index rebuild/query.
- **No test was classified `mixed`.** Every test had a single dominant concern under the rules
  above. The closest to "mixed" are the cli-flow tests that pin contract-field values in JSON,
  but the dominant concern there is unambiguously CLI presentation.

## Top 10 heaviest tests

| Function                                                                                                          | Lines | Category      | Summary                                                                                              |
|-------------------------------------------------------------------------------------------------------------------|-------|---------------|------------------------------------------------------------------------------------------------------|
| `host_adapter_manifest_library_classifies_command_authority`                                                      | 305   | contract-flow | Asserts every host-adapter command's `command_kind` / `mutation_class` / `authority_class` / policy_refs. |
| `host_adapter_invocation_admission_enforces_target_and_process_controls`                                          | 67    | contract-flow | `run_host_adapter_invocation_admission` across MCP-stdio / borrowed-shell, shell-token / env-key rejection. |
| `host_adapter_distribution_admission_requires_canary_opt_in_and_blocks_dev`                                       | 63    | contract-flow | Distribution admission: canary requires opt-in, dev channel always blocked.                          |
| `query_effect_index_binary_outputs_json`                                                                          | 60    | cli-flow      | Binary `query-effect-index --json` envelope shape incl. authority_boundary fields.                   |
| `host_adapter_verify_sigstore_dsse_in_toto_subject_binary_outputs_json_and_exit_status`                           | 55    | cli-flow      | Binary DSSE in-toto subject verification `--json` envelope.                                          |
| `host_adapter_distribution_admission_blocks_weak_evidence_and_allows_complete_evidence`                           | 53    | contract-flow | Distribution admission: weak evidence blocked with named reasons; complete evidence allowed.         |
| `host_adapter_verify_provenance_binary_outputs_json_and_exit_status`                                              | 52    | cli-flow      | Binary provenance verification `--json` envelope + success exit.                                     |
| `host_adapter_verify_sigstore_bundle_subject_binary_outputs_json_and_exit_status`                                 | 51    | cli-flow      | Binary sigstore bundle subject verification `--json` envelope.                                       |
| `host_adapter_verify_sigstore_timestamp_authority_binary_outputs_json_for_rfc3161`                                | 50    | cli-flow      | Binary TSA verification `--json` envelope for RFC3161 path.                                          |
| `query_effect_index_context_outputs_bounded_json`                                                                 | 77    | cli-flow      | Binary `query-effect-index --context --json` bounded context presentation.                           |

(Sorted by line count after the table; the manifest test at 305 lines is by far the largest
and is the obvious first split candidate — see R12.2 notes.)

## Inline helpers / fixtures

All non-test scaffolding lives in this single file. Line ranges below are 1-based inclusive.
"Should move to" is a recommendation for R12.2; the canonical split is:
crypto-cluster helpers → `forge-core-crypto/tests/common/` (post-R10);
contract/effect-index helpers → `forge-core-contracts/tests/common/` or `forge-core-store/tests/common/`;
`temp_payload_file` / `temp_repo_root` / `repo_root` → `forge-core-cli/tests/common/` (they pin CLI binaries).

| Name                                                  | Lines      | Used by                                                                                                       | Should move to                                            |
|-------------------------------------------------------|------------|---------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------|
| `repo_root()`                                         | 93-99      | validate_library_passes_current_repo, validate_binary_outputs_json_summary, execute_operation_*              | forge-core-cli/tests/common (CLI-pinning helper)          |
| `temp_payload_file()`                                 | 101-105    | artifact verification tests, execute_operation_* tests                                                        | forge-core-cli/tests/common                               |
| `temp_repo_root()`                                    | 107-118    | every fixture builder; rebuild/query effect-index tests                                                       | forge-core-cli/tests/common (or shared workspace util)    |
| `SidecarCliFixture` struct + `temp_sidecar_cli_fixture()` | 120-143 | rebuild_effect_index_binary_outputs_json, query_effect_index_*                                                | forge-core-cli/tests/common (pins CLI sidecar layout)     |
| `SignedProvenanceFixture` struct                      | 145-155    | provenance verification tests                                                                                 | forge-core-crypto/tests/common                            |
| `RekorEntryFixture` struct                           | 157-161    | rekor / sigstore bundle / dsse / TSA tests                                                                    | forge-core-crypto/tests/common                            |
| `SigstoreTrustPolicyFixture` struct                  | 163-165    | all sigstore tests (transitively, via `sigstore_trust_policy_fixture`)                                        | forge-core-crypto/tests/common                            |
| `FulcioCertificateFixture` struct                    | 167-175    | fulcio / sigstore bundle / dsse / TSA / revocation / CRL / OCSP tests                                         | forge-core-crypto/tests/common                            |
| `Rfc3161TimestampFixture` struct                     | 177-180    | RFC3161 TSA tests                                                                                             | forge-core-crypto/tests/common                            |
| `CertificateTransparencySctFixture` struct           | 182-187    | CT SCT tests                                                                                                  | forge-core-crypto/tests/common                            |
| `SigstoreBundleSubjectFixture` struct                | 189-198    | sigstore bundle subject tests                                                                                 | forge-core-crypto/tests/common                            |
| `SigstoreDsseInTotoSubjectFixture` struct            | 200-210    | sigstore DSSE in-toto subject tests                                                                           | forge-core-crypto/tests/common                            |
| `hex_bytes()`                                         | 212-217    | rekor / CT / OCSP nonce tests                                                                                 | forge-core-crypto/tests/common                            |
| `rekor_leaf_hash()`                                   | 219-224    | all rekor fixture builders                                                                                    | forge-core-crypto/tests/common                            |
| `signed_provenance_fixture()`                         | 226-325    | provenance verification tests                                                                                 | forge-core-crypto/tests/common                            |
| `rekor_entry_fixture()`                               | 327-403    | rekor / TSA tests                                                                                             | forge-core-crypto/tests/common                            |
| `sigstore_trust_policy_fixture()`                     | 405-455    | nearly every sigstore/fulcio/CRL/OCSP/TUF/CT test                                                             | forge-core-crypto/tests/common                            |
| `set_sigstore_revocation_policy()`                    | 457-470    | revocation / CRL / OCSP tests                                                                                 | forge-core-crypto/tests/common                            |
| `write_tuf_metadata()`                                | 472-489    | TUF freshness tests                                                                                           | forge-core-crypto/tests/common                            |
| `write_crl_fixture()`                                 | 491-522    | CRL status tests                                                                                              | forge-core-crypto/tests/common                            |
| `OcspCertificateFixture` struct                       | 524-532    | all OCSP tests                                                                                                | forge-core-crypto/tests/common                            |
| `OcspFixtureCertStatus` enum                          | 534-539    | OCSP response builders                                                                                        | forge-core-crypto/tests/common                            |
| `OcspResponseFixtureOptions` struct + `good()`        | 541-567    | all OCSP tests                                                                                                | forge-core-crypto/tests/common                            |
| `ocsp_certificate_fixture()`                          | 569-601    | all OCSP tests                                                                                                | forge-core-crypto/tests/common                            |
| `test_ocsp_ca()`                                      | 603-622    | OCSP fixture + bad-target-cert test                                                                           | forge-core-crypto/tests/common                            |
| `test_ocsp_leaf()`                                    | 624-640    | OCSP fixture + bad-target-cert test                                                                           | forge-core-crypto/tests/common                            |
| `write_ocsp_response_fixture()`                       | 642-652    | all OCSP tests                                                                                                | forge-core-crypto/tests/common                            |
| `ocsp_response_der()`                                 | 654-699    | OCSP response writer                                                                                          | forge-core-crypto/tests/common                            |
| `der_ocsp_single_response()`                          | 701-722    | OCSP response writer                                                                                          | forge-core-crypto/tests/common                            |
| `der_ocsp_cert_id()`                                  | 724-746    | OCSP response writer                                                                                          | forge-core-crypto/tests/common                            |
| `der_ocsp_cert_status()`                              | 748-754    | OCSP response writer                                                                                          | forge-core-crypto/tests/common                            |
| `der_ocsp_nonce_extension()`                          | 756-761    | OCSP response writer                                                                                          | forge-core-crypto/tests/common                            |
| `x509_subject_der()`                                  | 763-767    | OCSP fixture / responder-mismatch test                                                                        | forge-core-crypto/tests/common                            |
| `der_sequence()`                                      | 769-776    | all DER builders                                                                                              | forge-core-crypto/tests/common                            |
| `der_context_explicit()`                              | 778-780    | all DER builders                                                                                              | forge-core-crypto/tests/common                            |
| `der_context_primitive()`                             | 782-784    | OCSP cert-status encoder                                                                                      | forge-core-crypto/tests/common                            |
| `der_algorithm_identifier()`                          | 786-788    | OCSP / cert-id encoder                                                                                        | forge-core-crypto/tests/common                            |
| `der_oid()`                                           | 790-805    | all DER builders                                                                                              | forge-core-crypto/tests/common                            |
| `der_octet_string()`                                  | 807-809    | OCSP / cert-id encoder                                                                                        | forge-core-crypto/tests/common                            |
| `der_bit_string()`                                    | 811-816    | OCSP signature encoder                                                                                        | forge-core-crypto/tests/common                            |
| `der_integer_positive()`                              | 818-828    | OCSP cert-id serial encoder                                                                                   | forge-core-crypto/tests/common                            |
| `der_enumerated()`                                    | 830-837    | OCSP response-status encoder                                                                                  | forge-core-crypto/tests/common                            |
| `der_generalized_time()`                              | 839-841    | OCSP / TSA time encoders                                                                                      | forge-core-crypto/tests/common                            |
| `der()`                                               | 843-849    | all DER builders                                                                                              | forge-core-crypto/tests/common                            |
| `der_length()`                                        | 851-864    | all DER builders                                                                                              | forge-core-crypto/tests/common                            |
| `fulcio_certificate_fixture()`                        | 866-868    | wrapper used by fulcio / revocation / CRL / bundle / dsse / TSA tests                                         | forge-core-crypto/tests/common                            |
| `fulcio_certificate_fixture_with_validity()`          | 870-899    | fulcio / RFC3161 TSA tests                                                                                    | forge-core-crypto/tests/common                            |
| `test_fulcio_ca_with_validity()`                      | 901-919    | fulcio fixture builder                                                                                        | forge-core-crypto/tests/common                            |
| `test_fulcio_leaf_with_validity()`                    | 921-993    | fulcio fixture builder                                                                                        | forge-core-crypto/tests/common                            |
| `der_utf8()`                                          | 995-997    | fulcio extension encoder                                                                                      | forge-core-crypto/tests/common                            |
| `install_rfc3161_timestamp_fixture()`                 | 999-1041   | RFC3161 TSA tests                                                                                             | forge-core-crypto/tests/common                            |
| `certificate_transparency_sct_fixture()`              | 1043-1074  | CT SCT tests                                                                                                  | forge-core-crypto/tests/common                            |
| `extract_rfc3161_timestamp_token()`                   | 1076-1085  | `install_rfc3161_timestamp_fixture`                                                                           | forge-core-crypto/tests/common                            |
| `extract_rfc3161_signature()`                         | 1087-1095  | `install_rfc3161_timestamp_fixture`                                                                           | forge-core-crypto/tests/common                            |
| `extract_rfc3161_tsa_certificates()`                  | 1097-1116  | `install_rfc3161_timestamp_fixture`                                                                           | forge-core-crypto/tests/common                            |
| `sigstore_bundle_subject_fixture()`                   | 1118-1162  | sigstore bundle subject tests                                                                                 | forge-core-crypto/tests/common                            |
| `rekor_entry_fixture_for_bundle()`                    | 1164-1243  | sigstore bundle subject fixture                                                                               | forge-core-crypto/tests/common                            |
| `dsse_pae()`                                          | 1245-1257  | DSSE subject fixture                                                                                          | forge-core-crypto/tests/common                            |
| `sigstore_dsse_in_toto_subject_fixture()`             | 1259-1329  | sigstore DSSE in-toto subject tests                                                                           | forge-core-crypto/tests/common                            |
| `rekor_entry_fixture_for_dsse()`                      | 1331-1416  | DSSE subject fixture                                                                                          | forge-core-crypto/tests/common                            |
| `ocsp_verification_input()`                           | 3983-3996  | all OCSP tests                                                                                                | forge-core-crypto/tests/common                            |
| `write_effect_index_record()`                         | 5124-5153  | rebuild / query effect-index tests                                                                            | forge-core-store/tests/common (pins store types)          |
| `write_committed_metadata_wal()`                      | 5155-5215  | rebuild_effect_index_library_rebuilds_from_committed_wal, rebuild_effect_index_binary_outputs_json | forge-core-store/tests/common (pins store types)     |

Also note the `include_str!` / `include_bytes!` constants at lines 74-91 (RFC3161 bundles,
CT Google cert/SCT/log-id binaries, log-id tables). These are crypto-cluster fixtures and
should move with the crypto helpers to `forge-core-crypto/tests/fixtures/`.

## Full inventory

Line ranges are 1-based inclusive, starting at the `#[test]` attribute and ending at the
function's closing brace. "Depends on" lists symbols pulled from the `forge_core_cli::`
crate surface (either via the `use` block at lines 3-47 or via fully-qualified
`forge_core_cli::...` paths in the body). Symbols from `forge_core_contracts::`,
`forge_core_store::`, and third-party crates are not listed per-test (they are project-wide
shared surfaces).

| Function | Line range | Category | Summary | Depends on (from `forge_core_cli::`) |
|---|---|---|---|---|
| `validate_library_passes_current_repo` | 1418-1428 | contract-flow | `run_validate(repo_root())` passes with no diagnostics on the current repo. | `run_validate`, `ValidationStatus` |
| `validate_binary_outputs_json_summary` | 1430-1463 | cli-flow | Binary `validate --root ... --json` produces the expected envelope (status / checks / diagnostics). | (binary only; uses `env!("CARGO_BIN_EXE_forge-core")`) |
| `host_adapter_manifest_library_classifies_command_authority` | 1465-1769 | contract-flow | Asserts manifest's `command_kind`/`mutation_class`/`authority_class`/`policy_refs` for every host-adapter command (execute-operation, query-effect-index, validate, verify-artifact, verify-provenance, verify-rekor, verify-sigstore-trust-policy, verify-fulcio-certificate-identity, verify-sigstore-bundle-subject, verify-sigstore-dsse-in-toto-subject, verify-sigstore-timestamp-authority, verify-certificate-transparency-sct, verify-certificate-revocation-policy, verify-tuf-trusted-root-freshness, verify-certificate-crl-status). | `run_host_adapter_manifest`, `HostAdapterCommandKind`, `HostAdapterMutationClass`, `HostAdapterAuthorityClass`, `HostAdapterAutoTrigger` |
| `host_adapter_manifest_binary_outputs_json` | 1771-1798 | cli-flow | Binary `host-adapter-manifest --json` envelope shape (`manifest_id`, commands array, execute-operation fields). | (binary only) |
| `host_adapter_projection_library_preserves_non_authority_boundary` | 1800-1838 | contract-flow | `run_host_adapter_projection(McpTools)` keeps projection non-authoritative; execute-operation MCP hints destructive, query-effect-index read-only. | `run_host_adapter_projection`, `HostAdapterProjectionTarget`, `HostAdapterMutationClass`, `HostAdapterAuthorityClass` |
| `host_adapter_projection_binary_outputs_mcp_json` | 1840-1863 | cli-flow | Binary `host-adapter-projection --target mcp_tools --json` envelope shape. | (binary only) |
| `host_adapter_process_policy_blocks_mcp_mutating_operations` | 1865-1888 | contract-flow | `run_host_adapter_process_security_policy(McpStdio)` blocks mutating ops by default; argv/env/stdio policies tightened. | `run_host_adapter_process_security_policy`, `HostAdapterProcessTarget` |
| `host_adapter_invocation_admission_enforces_target_and_process_controls` | 1890-1956 | contract-flow | `run_host_adapter_invocation_admission` for MCP-stdio / borrowed-shell: explicit-invocation required, shell control tokens rejected, forbidden env keys rejected. | `run_host_adapter_invocation_admission`, `HostAdapterInvocationRequest`, `HostAdapterInvocationAdmissionStatus`, `HostAdapterProcessTarget` |
| `host_adapter_process_policy_binary_outputs_json` | 1958-1981 | cli-flow | Binary `host-adapter-process-policy --target mcp_stdio --json` envelope shape. | (binary only) |
| `host_adapter_admit_invocation_binary_blocks_mcp_mutation` | 1983-2007 | cli-flow | Binary `host-adapter-admit-invocation ... --json` for execute-operation on mcp_stdio exits non-zero with `status=blocked`, `reasons=[mcp_stdio_mutating_operation_deferred]`. | (binary only) |
| `host_adapter_distribution_policy_requires_supply_chain_evidence` | 2009-2023 | contract-flow | `run_host_adapter_distribution_policy` requires immutable source ref, artifact checksum, provenance, version compat, rollback; dev channel not for general install. | `run_host_adapter_distribution_policy`, `HostAdapterDistributionAdmissionStatus` |
| `host_adapter_distribution_admission_blocks_weak_evidence_and_allows_complete_evidence` | 2025-2077 | contract-flow | `run_host_adapter_distribution_admission`: weak evidence blocked with named reasons; complete stable evidence allowed. | `run_host_adapter_distribution_admission`, `HostAdapterDistributionEvidence`, `HostAdapterDistributionAdmissionStatus`, `HostAdapterUpdateChannel` |
| `host_adapter_distribution_admission_requires_canary_opt_in_and_blocks_dev` | 2079-2141 | contract-flow | Canary channel requires explicit opt-in; dev channel always blocked regardless of opt-in. | `run_host_adapter_distribution_admission`, `HostAdapterDistributionEvidence`, `HostAdapterDistributionAdmissionStatus`, `HostAdapterUpdateChannel` |
| `host_adapter_distribution_policy_binary_outputs_json` | 2143-2159 | cli-flow | Binary `host-adapter-distribution-policy --json` envelope shape. | (binary only) |
| `host_adapter_admit_distribution_binary_blocks_missing_evidence` | 2161-2184 | cli-flow | Binary `host-adapter-admit-distribution ... --json` exits non-zero with `status=blocked`, `reasons` contains `source_ref_required`. | (binary only) |
| `host_adapter_artifact_verification_passes_for_matching_digest_and_metadata` | 2186-2216 | contract-flow | `run_host_adapter_artifact_verification` passes when SHA256 matches and all metadata refs present; signature crypto verification deferred. | `run_host_adapter_artifact_verification`, `HostAdapterArtifactVerificationInput`, `HostAdapterArtifactVerificationStatus` |
| `host_adapter_artifact_verification_fails_for_mismatch_or_missing_metadata` | 2218-2250 | contract-flow | Same entrypoint fails with named reasons when SHA256 mismatches or required refs missing. | `run_host_adapter_artifact_verification`, `HostAdapterArtifactVerificationInput`, `HostAdapterArtifactVerificationStatus` |
| `host_adapter_verify_artifact_binary_outputs_json_and_exit_status` | 2252-2288 | cli-flow | Binary `host-adapter-verify-artifact ... --json` success envelope (`status=passed`, `computed_sha256`). | (binary only) |
| `host_adapter_verify_artifact_binary_blocks_mismatched_digest` | 2290-2325 | cli-flow | Binary verify-artifact with mismatched SHA256 exits non-zero with `status=failed`, `reasons=[sha256_mismatch]`. | (binary only) |
| `host_adapter_provenance_verification_passes_signed_slsa_statement` | 2327-2356 | crypto-flow | `run_host_adapter_provenance_verification` passes: ed25519 sig valid, subject matches artifact, transparency inclusion proof valid. | `run_host_adapter_provenance_verification`, `HostAdapterProvenanceVerificationInput`, `HostAdapterProvenanceVerificationStatus` |
| `host_adapter_provenance_verification_fails_when_source_ref_mismatches` | 2358-2384 | crypto-flow | Same entrypoint fails with `source_ref_missing` while signature remains valid. | `run_host_adapter_provenance_verification`, `HostAdapterProvenanceVerificationInput`, `HostAdapterProvenanceVerificationStatus` |
| `host_adapter_verify_provenance_binary_outputs_json_and_exit_status` | 2386-2437 | cli-flow | Binary `host-adapter-verify-provenance ... --json` success envelope. | (binary only) |
| `host_adapter_verify_provenance_binary_blocks_bad_signature` | 2439-2485 | cli-flow | Binary verify-provenance with replaced signature exits non-zero with `status=failed`, `reasons=[provenance_signature_invalid]`. | (binary only) |
| `host_adapter_rekor_verification_passes_signed_checkpoint_and_inclusion` | 2487-2509 | crypto-flow | `run_host_adapter_rekor_verification` passes: entry parsed, signed checkpoint valid, inclusion proof valid. | `run_host_adapter_rekor_verification`, `HostAdapterRekorVerificationInput`, `HostAdapterRekorVerificationStatus` |
| `host_adapter_rekor_verification_fails_when_log_id_mismatches` | 2511-2531 | crypto-flow | Same entrypoint fails with `rekor_log_id_mismatch`; inclusion proof still valid. | `run_host_adapter_rekor_verification`, `HostAdapterRekorVerificationInput`, `HostAdapterRekorVerificationStatus` |
| `host_adapter_verify_rekor_entry_binary_outputs_json_and_exit_status` | 2533-2567 | cli-flow | Binary `host-adapter-verify-rekor-entry ... --json` success envelope. | (binary only) |
| `host_adapter_verify_rekor_entry_binary_blocks_wrong_key` | 2569-2610 | cli-flow | Binary verify-rekor-entry with mismatched public key exits non-zero; reason starts with `rekor_inclusion_verification_failed:`. | (binary only) |
| `host_adapter_sigstore_trust_policy_verification_passes_complete_policy` | 2612-2635 | crypto-flow | `run_host_adapter_sigstore_trust_policy_verification` passes: fulcio CA refs present, identity selector present, timestamp policy has source. | `run_host_adapter_sigstore_trust_policy_verification` (fully qualified), `HostAdapterSigstoreTrustPolicyVerificationInput`, `HostAdapterSigstoreTrustPolicyVerificationStatus` |
| `host_adapter_sigstore_trust_policy_verification_fails_missing_fulcio_refs` | 2637-2653 | crypto-flow | Same entrypoint fails with `sigstore_fulcio_ca_refs_missing` when `fulcio_refs=[]`. | `run_host_adapter_sigstore_trust_policy_verification` (fully qualified), `HostAdapterSigstoreTrustPolicyVerificationInput`, `HostAdapterSigstoreTrustPolicyVerificationStatus` |
| `host_adapter_verify_sigstore_trust_policy_binary_outputs_json_and_exit_status` | 2655-2683 | cli-flow | Binary `host-adapter-verify-sigstore-trust-policy ... --json` success envelope. | (binary only) |
| `host_adapter_verify_sigstore_trust_policy_binary_blocks_missing_tsa_source` | 2685-2717 | cli-flow | Binary verify-sigstore-trust-policy in `rfc3161_tsa` mode with empty certificate_refs exits non-zero with `sigstore_timestamp_policy_requires_tsa_certs`. | (binary only) |
| `host_adapter_fulcio_certificate_identity_verification_passes_matching_policy` | 2719-2749 | crypto-flow | `run_host_adapter_fulcio_certificate_identity_verification` passes: OIDC issuer match, SAN match, chain signature verified. | `run_host_adapter_fulcio_certificate_identity_verification`, `HostAdapterFulcioCertificateIdentityVerificationInput`, `HostAdapterFulcioCertificateIdentityVerificationStatus` |
| `host_adapter_fulcio_certificate_identity_verification_fails_oidc_issuer_mismatch` | 2751-2782 | crypto-flow | Same entrypoint fails with `fulcio_identity_oidc_issuer_mismatch`. | (same as above) |
| `host_adapter_fulcio_certificate_identity_verification_fails_undeclared_root` | 2784-2806 | crypto-flow | Same entrypoint fails with `fulcio_chain_declared_ca_ref_missing` when issuer not in declared refs. | (same as above) |
| `host_adapter_verify_fulcio_certificate_identity_binary_outputs_json_and_exit_status` | 2808-2853 | cli-flow | Binary `host-adapter-verify-fulcio-certificate-identity ... --json` success envelope. | (binary only) |
| `host_adapter_sigstore_bundle_subject_verification_passes_matching_bundle` | 2855-2887 | crypto-flow | `run_host_adapter_sigstore_bundle_subject_verification` passes: digest matches artifact, signature verified with cert key, fulcio identity verified at rekor time, rekor body binds signature. | `run_host_adapter_sigstore_bundle_subject_verification`, `HostAdapterSigstoreBundleSubjectVerificationInput`, `HostAdapterSigstoreBundleSubjectVerificationStatus` |
| `host_adapter_sigstore_bundle_subject_verification_fails_digest_mismatch` | 2889-2913 | crypto-flow | Same entrypoint fails with `bundle_message_digest_mismatch` when artifact mutated. | (same as above) |
| `host_adapter_sigstore_bundle_subject_verification_fails_rekor_body_mismatch` | 2915-2953 | crypto-flow | Same entrypoint fails with `rekor_body_artifact_digest_mismatch` when rekor body hash tampered. | (same as above) |
| `host_adapter_verify_sigstore_bundle_subject_binary_outputs_json_and_exit_status` | 2955-3005 | cli-flow | Binary `host-adapter-verify-sigstore-bundle-subject ... --json` success envelope. | (binary only) |
| `host_adapter_sigstore_dsse_in_toto_subject_verification_passes_matching_bundle` | 3007-3040 | crypto-flow | `run_host_adapter_sigstore_dsse_in_toto_subject_verification` passes: DSSE PAE sig verified, in-toto subject matches artifact, rekor body binds payload hash, fulcio identity verified at rekor time. | `run_host_adapter_sigstore_dsse_in_toto_subject_verification`, `HostAdapterSigstoreDsseInTotoSubjectVerificationInput`, `HostAdapterSigstoreDsseInTotoSubjectVerificationStatus` |
| `host_adapter_sigstore_dsse_in_toto_subject_verification_fails_payload_type_mismatch` | 3042-3078 | crypto-flow | Same entrypoint fails with `dsse_payload_type_unsupported` + `dsse_signature_verification_failed`. | (same as above) |
| `host_adapter_sigstore_dsse_in_toto_subject_verification_fails_subject_mismatch` | 3080-3105 | crypto-flow | Same entrypoint fails with `dsse_intoto_subject_sha256_missing` when artifact mutated. | (same as above) |
| `host_adapter_sigstore_dsse_in_toto_subject_verification_fails_multiple_signatures` | 3107-3145 | crypto-flow | Same entrypoint fails with `dsse_signature_count_invalid` when signatures array has >1 entry. | (same as above) |
| `host_adapter_sigstore_dsse_in_toto_subject_verification_fails_rekor_body_mismatch` | 3147-3187 | crypto-flow | Same entrypoint fails with `rekor_body_dsse_payload_hash_mismatch` when rekor body hash tampered. | (same as above) |
| `host_adapter_verify_sigstore_dsse_in_toto_subject_binary_outputs_json_and_exit_status` | 3189-3243 | cli-flow | Binary `host-adapter-verify-sigstore-dsse-in-toto-subject ... --json` success envelope. | (binary only) |
| `host_adapter_sigstore_timestamp_authority_verification_passes_rekor_integrated_time` | 3245-3280 | crypto-flow | `run_host_adapter_sigstore_timestamp_authority_verification` selects `rekor_integrated_time` source; timestamp within cert validity. | `run_host_adapter_sigstore_timestamp_authority_verification`, `HostAdapterSigstoreTimestampAuthorityVerificationInput`, `HostAdapterSigstoreTimestampAuthorityVerificationStatus`, `HostAdapterRekorVerificationStatus` |
| `host_adapter_sigstore_timestamp_authority_verification_fails_outside_certificate_window` | 3282-3321 | crypto-flow | Same entrypoint fails with `timestamp_outside_certificate_validity` when rekor integratedTime moved to epoch. | (same as above) |
| `host_adapter_sigstore_timestamp_authority_verification_passes_rfc3161_tsa_token` | 3323-3369 | crypto-flow | Same entrypoint selects `rfc3161_tsa` source: token verified, message imprint verified, CMS signature verified, TSA chain verified, timestamp within cert validity. | (same as above) |
| `host_adapter_sigstore_timestamp_authority_verification_fails_rfc3161_payload_mismatch` | 3371-3403 | crypto-flow | Same entrypoint fails with reason prefix `timestamp_rfc3161_verification_failed:Timestamp message hash mismatch`. | (same as above) |
| `host_adapter_sigstore_timestamp_authority_verification_fails_missing_rfc3161_tsa_refs` | 3405-3451 | crypto-flow | Same entrypoint fails with `timestamp_rfc3161_tsa_certificate_refs_missing` when policy's certificate_refs stripped. | (same as above) |
| `host_adapter_verify_sigstore_timestamp_authority_binary_outputs_json_and_exit_status` | 3453-3498 | cli-flow | Binary `host-adapter-verify-sigstore-timestamp-authority ... --json` success envelope for rekor-integrated-time path. | (binary only) |
| `host_adapter_verify_sigstore_timestamp_authority_binary_outputs_json_for_rfc3161` | 3500-3549 | cli-flow | Binary TSA verification `--json` success envelope for RFC3161 path (`selected_timestamp_source=rfc3161_tsa`). | (binary only) |
| `host_adapter_certificate_transparency_sct_verification_passes_supplied_scts` | 3551-3578 | crypto-flow | `run_host_adapter_certificate_transparency_sct_verification` passes: 2 SCTs verified against Google Pilot + Symantec log ids. | `run_host_adapter_certificate_transparency_sct_verification`, `HostAdapterCertificateTransparencySctVerificationInput`, `HostAdapterCertificateTransparencySctVerificationStatus` |
| `host_adapter_certificate_transparency_sct_verification_fails_unknown_log` | 3580-3604 | crypto-flow | Same entrypoint fails with reason containing `UnknownLog` when log id removed from policy. | (same as above) |
| `host_adapter_certificate_transparency_sct_verification_fails_future_sct` | 3606-3626 | crypto-flow | Same entrypoint fails with reason containing `TimestampInFuture` at verification_time=1. | (same as above) |
| `host_adapter_verify_certificate_transparency_sct_binary_outputs_json` | 3628-3664 | cli-flow | Binary `host-adapter-verify-certificate-transparency-sct ... --json` success envelope; deferred revocation_status noted. | (binary only) |
| `host_adapter_certificate_revocation_policy_verification_passes_short_lived_policy` | 3666-3698 | crypto-flow | `run_host_adapter_certificate_revocation_policy_verification` selects `implicit_short_lived_certificate` strategy; trusted signing time within cert validity. | `run_host_adapter_certificate_revocation_policy_verification`, `HostAdapterCertificateRevocationPolicyVerificationInput`, `HostAdapterCertificateRevocationPolicyVerificationStatus` |
| `host_adapter_certificate_revocation_policy_verification_fails_excessive_lifetime` | 3700-3721 | crypto-flow | Same entrypoint fails with `certificate_revocation_certificate_lifetime_exceeds_policy` when policy max too low. | (same as above) |
| `host_adapter_certificate_revocation_policy_verification_fails_explicit_status_required` | 3723-3746 | crypto-flow | Same entrypoint fails with `certificate_revocation_explicit_status_not_implemented`; defers `explicit_ocsp_status`. | (same as above) |
| `host_adapter_verify_certificate_revocation_policy_binary_outputs_json` | 3748-3785 | cli-flow | Binary `host-adapter-verify-certificate-revocation-policy ... --json` success envelope for `short_lived_certificate` mode. | (binary only) |
| `host_adapter_certificate_crl_status_verification_passes_good_by_supplied_crl` | 3787-3822 | crypto-flow | `run_host_adapter_certificate_crl_status_verification` returns `good_by_supplied_crl` with CRL signature verified. | `run_host_adapter_certificate_crl_status_verification`, `HostAdapterCertificateCrlStatusVerificationInput`, `HostAdapterCertificateCrlStatusVerificationStatus` |
| `host_adapter_certificate_crl_status_verification_fails_revoked_certificate` | 3824-3859 | crypto-flow | Same entrypoint returns `revoked_by_supplied_crl` with `crl_status_certificate_revoked`. | (same as above) |
| `host_adapter_certificate_crl_status_verification_fails_expired_crl` | 3861-3896 | crypto-flow | Same entrypoint fails with `crl_status_crl_expired` when CRL next_update in the past. | (same as above) |
| `host_adapter_certificate_crl_status_verification_fails_without_explicit_policy` | 3898-3933 | crypto-flow | Same entrypoint fails with `crl_status_policy_not_explicit_status_required` when policy is `short_lived_certificate`. | (same as above) |
| `host_adapter_verify_certificate_crl_status_binary_outputs_json` | 3935-3981 | cli-flow | Binary `host-adapter-verify-certificate-crl-status ... --json` success envelope. | (binary only) |
| `host_adapter_certificate_ocsp_status_verification_passes_good_by_supplied_ocsp` | 3998-4022 | crypto-flow | `run_host_adapter_certificate_ocsp_status_verification` returns `good_by_supplied_ocsp` with response signature verified; nonce-not-supplied evidence. | `run_host_adapter_certificate_ocsp_status_verification`, `HostAdapterCertificateOcspStatusVerificationInput`, `HostAdapterCertificateOcspStatusVerificationStatus` |
| `host_adapter_certificate_ocsp_status_verification_fails_revoked_by_supplied_ocsp` | 4024-4051 | crypto-flow | Same entrypoint returns `revoked_by_supplied_ocsp` with `ocsp_status_certificate_revoked`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_unknown_by_supplied_ocsp` | 4053-4080 | crypto-flow | Same entrypoint returns `unknown_by_supplied_ocsp` with `ocsp_status_certificate_unknown`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_expired_response` | 4082-4109 | crypto-flow | Same entrypoint fails with `ocsp_status_response_expired` when next_update in the past. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_future_this_update` | 4111-4139 | crypto-flow | Same entrypoint fails with `ocsp_status_this_update_in_future`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_future_produced_at` | 4141-4169 | crypto-flow | Same entrypoint fails with `ocsp_status_produced_at_in_future`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_missing_next_update` | 4171-4198 | crypto-flow | Same entrypoint fails with `ocsp_status_next_update_missing`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_cert_id_serial_mismatch` | 4200-4230 | crypto-flow | Same entrypoint fails with `ocsp_status_certificate_serial_not_found` + `ocsp_status_single_response_match_missing`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_unsupported_cert_id_hash` | 4232-4262 | crypto-flow | Same entrypoint fails with `ocsp_status_cert_id_hash_algorithm_unsupported` when hash OID set to `[1,2,3,4]`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_bad_target_certificate_signature` | 4264-4299 | crypto-flow | Same entrypoint fails with reason prefix `ocsp_status_certificate_signature_failed:` when target leaf re-signed by wrong CA. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_responder_mismatch` | 4301-4328 | crypto-flow | Same entrypoint fails with `ocsp_status_responder_unauthorized` when responder name mismatches. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_bad_signature` | 4330-4357 | crypto-flow | Same entrypoint fails with `ocsp_status_response_signature_invalid` when signature byte tampered. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_does_not_trust_revoked_bad_signature` | 4359-4391 | crypto-flow | Same entrypoint, revoked status + tampered signature: does not trust `revoked_at_unix`, surfaces both `ocsp_status_response_signature_invalid` and `ocsp_status_certificate_revoked`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_does_not_trust_unknown_bad_signature` | 4393-4424 | crypto-flow | Same entrypoint, unknown status + tampered signature: surfaces both `ocsp_status_response_signature_invalid` and `ocsp_status_certificate_unknown`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_passes_matching_nonce` | 4426-4454 | crypto-flow | Same entrypoint passes with `ocsp_status_nonce_verified` when expected nonce matches. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_nonce_mismatch` | 4456-4486 | crypto-flow | Same entrypoint fails with `ocsp_status_nonce_mismatch`. | (same as above) |
| `host_adapter_certificate_ocsp_status_verification_fails_missing_nonce_when_expected` | 4488-4515 | crypto-flow | Same entrypoint fails with `ocsp_status_nonce_missing` when expected nonce not present. | (same as above) |
| `host_adapter_verify_certificate_ocsp_status_binary_outputs_json` | 4517-4563 | cli-flow | Binary `host-adapter-verify-certificate-ocsp-status ... --json` success envelope. | (binary only) |
| `host_adapter_tuf_trusted_root_freshness_verification_passes_fresh_metadata` | 4565-4600 | crypto-flow | `run_host_adapter_tuf_trusted_root_freshness_verification` passes: root/timestamp/snapshot roles verified, root expires after update_start, defers signature-threshold check. | `run_host_adapter_tuf_trusted_root_freshness_verification`, `HostAdapterTufTrustedRootFreshnessVerificationInput`, `HostAdapterTufTrustedRootFreshnessVerificationStatus` |
| `host_adapter_tuf_trusted_root_freshness_verification_fails_expired_root` | 4602-4631 | crypto-flow | Same entrypoint fails with `tuf_root_metadata_expired`. | (same as above) |
| `host_adapter_tuf_trusted_root_freshness_verification_fails_rollback_version` | 4633-4662 | crypto-flow | Same entrypoint fails with `tuf_root_version_below_floor` when root version < min. | (same as above) |
| `host_adapter_tuf_trusted_root_freshness_verification_fails_invalid_expiry_format` | 4664-4693 | crypto-flow | Same entrypoint fails with `tuf_root_expires_format_invalid` when expiry not ISO-8601. | (same as above) |
| `host_adapter_tuf_trusted_root_freshness_verification_fails_non_tuf_policy` | 4695-4729 | crypto-flow | Same entrypoint fails with `tuf_freshness_root_source_not_tuf` when policy `root_source=manual`. | (same as above) |
| `host_adapter_verify_tuf_trusted_root_freshness_binary_outputs_json` | 4731-4764 | cli-flow | Binary `host-adapter-verify-tuf-trusted-root-freshness ... --json` success envelope. | (binary only) |
| `execute_operation_binary_outputs_json_even_when_awaiting_human` | 4766-4803 | cli-flow | Binary `execute-operation ... --json` returns `status=awaiting_human` with empty command_executions / effect_applications. | (binary only) |
| `execute_operation_rejects_payload_outside_root_by_default` | 4805-4827 | contract-flow | `run_execute_operation` rejects payload outside root with "outside root" error under default `PayloadLoadPolicy`. | `run_execute_operation`, `ExecuteOperationInput`, `PayloadFileSpec`, `PayloadLoadPolicy` |
| `execute_operation_rejects_payload_larger_than_policy` | 4829-4854 | contract-flow | Same entrypoint rejects oversized payload with "too large" when `max_payload_bytes=1`. | (same as above) |
| `rebuild_effect_index_library_rebuilds_from_committed_wal` | 4856-4880 | contract-flow | `run_rebuild_effect_index` rebuilds index from a 3-stage WAL; redacts payload-secret; reports `Rebuilt` with rebuilt=1, appended=1. | `run_rebuild_effect_index`, `RebuildEffectIndexInput` |
| `rebuild_effect_index_binary_outputs_json` | 4882-4929 | cli-flow | Binary `rebuild-effect-index --root <app> --json` envelope shape; verifies no consumer-local `.forge-method` is created; index lands in sidecar state root. | (binary only) |
| `query_effect_index_library_filters_metadata_records` | 4931-4968 | contract-flow | `run_query_effect_index` filters by logical_ref, dedups latest_per_target, redacts content_hash; non-workflow authority. | `run_query_effect_index`, `QueryEffectIndexInput` |
| `query_effect_index_binary_outputs_json` | 4970-5029 | cli-flow | Binary `query-effect-index ... --json` envelope for `handoff_context` consumer use; asserts authority_boundary fields. | (binary only) |
| `query_effect_index_context_outputs_bounded_json` | 5031-5107 | cli-flow | Binary `query-effect-index --context --max-context-groups 1 --adapter-kind codex ...` envelope; total/returned/omitted groups; adapter_presentation fields. | (binary only) |
| `query_effect_index_rejects_workflow_authority_consumer_use` | 5109-5122 | cli-flow | Binary `query-effect-index --consumer-use workflow_authority --json` exits with code 2. | (binary only) |

## Notes for R12.2 (migration)

### Big-picture split

The file is essentially three concerns interleaved by command:

1. **CLI presentation tests (30 tests, ~1175 lines)** — every test whose body is dominated by
   `Command::new(env!("CARGO_BIN_EXE_forge-core"))` and `serde_json::from_slice` assertions on
   the `--json` envelope. **These stay in `forge-core-cli/tests/validate.rs`** (or get split
   into per-command files: `validate_cli.rs`, `host_adapter_cli.rs`, `effect_index_cli.rs`,
   `execute_operation_cli.rs`).

2. **Crypto / verification tests (54 tests, ~1665 lines)** — every test that calls
   `run_host_adapter_*_verification` against real fixtures (provenance sig, rekor checkpoint,
   sigstore bundle / DSSE / fulcio / TSA / CT / CRL / OCSP / TUF). **These move to
   `forge-core-crypto/tests/`** once R10 lands the crate. They currently depend on
   `forge_core_cli::run_host_adapter_*_verification` entry points plus their
   `HostAdapter*VerificationInput` / `HostAdapter*VerificationStatus` types — those entry points
   and types will need to be relocated (or re-exported) as part of R10/R12.2.

3. **Contract / policy tests (14 tests, ~753 lines)** — `run_validate`, manifest / projection /
   process-policy / invocation-admission / distribution-policy / distribution-admission /
   artifact-verification (digest-only) / execute-operation payload policy / effect-index
   rebuild & query. **These move to `forge-core-contracts/tests/` (the policy/admission ones)
   and `forge-core-store/tests/` (the effect-index rebuild/query ones).**

### Specific migration observations

- **The 305-line `host_adapter_manifest_library_classifies_command_authority` test (lines
  1465-1769) is the single biggest test in the file and should be split first.** It asserts
  one block per host-adapter command (~25 lines each × 15 commands). It belongs in
  `forge-core-contracts/tests/host_adapter_manifest.rs` and should be split into one test per
  command (or grouped by command_kind) so failures localize.

- **The OCSP cluster (tests #66-#82, lines 3998-4515, 17 tests, ~500 lines) is the densest
  single cluster** and depends on a large fixture builder stack (`ocsp_certificate_fixture`,
  `OcspResponseFixtureOptions` + `good()`, `test_ocsp_ca`, `test_ocsp_leaf`,
  `write_ocsp_response_fixture`, `ocsp_response_der`, `der_ocsp_*`, all `der_*` primitives,
  `x509_subject_der`, `ocsp_verification_input`). This entire stack (lines 524-567, 569-622,
  624-699, 701-789, 807-841, 843-864, 3983-3996) moves as a unit to
  `forge-core-crypto/tests/common/ocsp.rs`. Without moving the helpers atomically with the
  tests, the OCSP cluster will not compile standalone.

- **The CRL cluster (#61-#64, lines 3787-3933) shares `write_crl_fixture` (lines 491-522) and
  the `fulcio_certificate_fixture` builder.** Move `write_crl_fixture` to
  `forge-core-crypto/tests/common/crl.rs`. The fulcio fixture is shared across many clusters
  — see below.

- **The `sigstore_trust_policy_fixture` (lines 405-455) + `set_sigstore_revocation_policy`
  (lines 457-470) helpers are load-bearing across nearly every sigstore/fulcio/CRL/OCSP/TUF/CT
  test.** They must move to a shared `forge-core-crypto/tests/common/policy.rs` *before* any
  crypto test cluster is moved individually, otherwise every cluster will fail to compile.

- **The `fulcio_certificate_fixture` / `fulcio_certificate_fixture_with_validity` /
  `test_fulcio_ca_with_validity` / `test_fulcio_leaf_with_validity` / `der_utf8` stack
  (lines 866-997) is shared by fulcio / sigstore bundle / sigstore DSSE / TSA / revocation /
  CRL / OCSP tests.** This is the second must-move-first helper stack alongside the policy
  builder.

- **`rekor_entry_fixture` (lines 327-403), `rekor_entry_fixture_for_bundle` (1164-1243),
  `rekor_entry_fixture_for_dsse` (1331-1416), `rekor_leaf_hash` (219-224), `hex_bytes`
  (212-217), `dsse_pae` (1245-1257) form the rekor/DSSE cluster helpers.** Move together to
  `forge-core-crypto/tests/common/rekor.rs`.

- **The DER primitive library (`der`, `der_length`, `der_sequence`, `der_oid`, `der_octet_string`,
  `der_bit_string`, `der_integer_positive`, `der_enumerated`, `der_generalized_time`,
  `der_context_explicit`, `der_context_primitive`, `der_algorithm_identifier`, `der_utf8`,
  lines 743-997) is general-purpose and should become `forge-core-crypto/tests/common/der.rs`.**
  It is currently only used by the OCSP / fulcio fixture builders, but it's a clean reusable
  primitive set.

- **The contract-flow cluster (manifest / projection / process-policy / invocation-admission /
  distribution-policy / distribution-admission / artifact-verification / execute-operation) all
  depend on `HostAdapter*` enum types defined in `forge_core_cli`** (e.g.
  `HostAdapterCommandKind`, `HostAdapterMutationClass`, `HostAdapterAuthorityClass`,
  `HostAdapterAutoTrigger`, `HostAdapterProcessTarget`, `HostAdapterInvocationAdmissionStatus`,
  `HostAdapterDistributionAdmissionStatus`, `HostAdapterDistributionEvidence`,
  `HostAdapterUpdateChannel`, `HostAdapterArtifactVerificationStatus`,
  `HostAdapterArtifactVerificationInput`, `ExecuteOperationInput`, `PayloadFileSpec`,
  `PayloadLoadPolicy`). Before these tests can move to `forge-core-contracts/tests/`,
  **those types and the `run_host_adapter_*` policy entry points must either move to
  `forge-core-contracts`** (preferred, since they describe contract policy) **or be re-exported
  in a way that lets `forge-core-contracts` depend on them.** This is a coupling point worth
  resolving in R12.2 design before any contract-flow test moves.

- **The effect-index rebuild & query tests (#93, #95) plus their binary counterparts (#94, #96,
  #97, #98) depend on `write_effect_index_record` (lines 5124-5153) and
  `write_committed_metadata_wal` (lines 5155-5215)** which use `forge_core_store::` types
  (`EffectTargetMetadataRecord`, `EffectWalRecord`, `EffectWalStage`,
  `EffectWalTargetMetadata`, `append_json_line`). The library tests (#93, #95) should move to
  `forge-core-store/tests/` together with these two helpers; the binary tests stay in
  `forge-core-cli/tests/`.

- **`temp_sidecar_cli_fixture` / `SidecarCliFixture` (lines 120-143) is the only fixture that
  pins the sidecar project-link layout** (`.forge-method.yaml`, `sidecar_root`,
  `state_root`). It's used by the rebuild/query binary tests. **Stays in
  `forge-core-cli/tests/common/`** because it encodes the CLI's project-link contract.

- **Two tests (#28, #29) use the fully-qualified path
  `forge_core_cli::run_host_adapter_sigstore_trust_policy_verification`** rather than the
  glob-import at lines 3-47. This is the only entry point referenced that way; if/when the
  glob-import is reshuffled during R12.2, these two tests need to keep their explicit path
  (or be updated to a new glob) to avoid breakage.

- **The `validate_library_passes_current_repo` test (lines 1418-1428) calls `run_validate` on
  `repo_root()`** — i.e. it validates the live repo. If it moves to `forge-core-contracts`,
  the helper `repo_root()` (lines 93-99) and `env!("CARGO_MANIFEST_DIR")` resolution must be
  re-evaluated, because the manifest dir will change when the test moves.

### Suggested move order for R12.2

1. Move DER primitives + `hex_bytes` + `x509_subject_der` → `forge-core-crypto/tests/common/`
   (pure utilities, no dependencies on the rest of the file).
2. Move `sigstore_trust_policy_fixture` + `set_sigstore_revocation_policy` →
   `forge-core-crypto/tests/common/policy.rs` (load-bearing for nearly every crypto test).
3. Move fulcio fixture stack → `forge-core-crypto/tests/common/fulcio.rs`.
4. Move rekor fixture stack → `forge-core-crypto/tests/common/rekor.rs`.
5. Move the 54 crypto-flow tests in command-sized chunks (provenance, rekor, sigstore-trust-policy,
   fulcio, sigstore-bundle, sigstore-dsse, timestamp-authority, CT, revocation, CRL, OCSP, TUF).
   Move OCSP cluster last (its helper stack is the largest).
6. Once `HostAdapter*` policy types move to `forge-core-contracts` (separate decision),
   move the 14 contract-flow tests.
7. Split `host_adapter_manifest_library_classifies_command_authority` into per-command tests
   as part of step 6.
8. Move effect-index library tests (#93, #95) + their two `write_*` helpers to
   `forge-core-store/tests/`. Keep their binary counterparts in `forge-core-cli/tests/`.

## Blockers / open questions

- **The single biggest blocker for moving any contract-flow or crypto-flow test out of
  `forge-core-cli` is the location of the `run_host_adapter_*` entry points and the
  `HostAdapter*` input/status enums.** They all currently live in `forge_core_cli` (lines 3-47
  import them). R12.2 needs to decide: do these move to `forge-core-contracts` (for the policy
  ones) and `forge-core-crypto` (for the verification ones), or do they stay in
  `forge-core-cli` and the moved tests re-import them across crate boundaries? The recommended
  path is to move them — but that decision is out of scope for R12.1.
- **No blocker on classification itself.** Every test was classifiable into a single category;
  none required `mixed`.

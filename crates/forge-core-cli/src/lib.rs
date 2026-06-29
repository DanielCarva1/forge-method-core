pub mod autonomy_cmd;
pub mod claim;
pub mod contract_cmd;
pub mod coordination;
pub(crate) mod crypto_hashing;
pub(crate) mod crypto_ocsp;
pub(crate) mod crypto_rekor;
pub(crate) mod effect_index;
pub mod eval_cmd;
pub(crate) mod execute_operation;
pub mod graph_cmd;
pub mod guide;
pub(crate) mod host_adapter_manifest;
pub(crate) mod host_adapter_projection;
pub(crate) mod host_adapter_types;
pub(crate) mod host_command;
pub mod io_util;
pub mod isolation;
pub mod m1_cmd;
pub mod project_cmd;
pub mod telemetry_cmd;
pub(crate) mod validate;

// Re-export the execute-operation API at the crate root so that the binary
// entrypoint (`main.rs`) and integration tests (`tests/validate.rs`) keep
// importing `ExecuteOperationInput`, `PayloadFileSpec`, `PayloadLoadPolicy`,
// `run_execute_operation` directly from `forge_core_cli`.
pub use execute_operation::{
    run_execute_operation, ExecuteOperationContractPathKind, ExecuteOperationError,
    ExecuteOperationInput, PayloadFileSpec, PayloadLoadPolicy,
};
// Re-export the effect-index API at the crate root so `main.rs` and
// `tests/validate.rs` keep importing the input structs and entrypoints
// directly from `forge_core_cli`.
pub use effect_index::{
    run_query_effect_index, run_query_effect_index_context, run_rebuild_effect_index,
    QueryEffectIndexInput, RebuildEffectIndexInput,
};
// Re-export the shared hashing helpers at the crate root so existing
// call sites (`crate::hex_sha256`, `crate::hex_bytes`, ...) inside `lib.rs`
// and the crypto submodules keep resolving after the helpers moved into
// `crypto_hashing`. `valid_sha256_digest` is now consumed only by
// `host_adapter_projection`, which imports it directly from
// `crate::crypto_hashing`.
pub(crate) use crypto_hashing::{
    hex_bytes, hex_sha256, normalize_sha256_digest, normalize_sha256_display,
};
// Re-export the OCSP verification helpers at the crate root so the
// host-adapter OCSP status verification function in `lib.rs` keeps
// resolving after the helpers moved into `crypto_ocsp`. The three
// purely internal helpers (`ocsp_cert_id_issuer_hashes_match`,
// `ocsp_digest_for_algorithm`, `sha1_digest`) are consumed only inside
// `crypto_ocsp` and therefore stay private to that module.
pub(crate) use crypto_ocsp::{
    apply_ocsp_cert_status, decode_basic_ocsp_response, decode_ocsp_response,
    extract_ocsp_response_nonce_hex, find_matching_ocsp_single_response,
    normalize_expected_ocsp_nonce_hex, ocsp_responder_id_matches_issuer, rasn_oid_matches,
    verify_basic_ocsp_signature_with_issuer, verify_ocsp_nonce,
    verify_ocsp_single_response_freshness,
};
// Re-export the admission safety predicates at the crate root so existing
// call sites inside `lib.rs` keep resolving after the helpers moved into
// `host_command`. The `host_command` builder, `HostCommandMetadata`,
// `argv_has_shell_control`, and `env_key_is_forbidden` are now consumed
// only by `host_adapter_projection`, which imports them directly from
// `crate::host_command`. The remaining two predicates below are still
// used by verification helpers in `lib.rs`.
pub(crate) use host_command::{source_ref_is_immutable, version_like};
// Re-export the public validate API at the crate root so `main.rs`,
// `tests/validate.rs`, and `forge-contract-validator` keep importing
// `run_validate`, `ValidateSummary`, `ValidateCheck`, `ValidateDiagnostic`,
// and `ValidationStatus` directly from `forge_core_cli`.
pub use validate::{
    run_validate, ValidateCheck, ValidateDiagnostic, ValidateSummary, ValidationStatus,
};
// Re-export all host-adapter types at the crate root so `main.rs` and
// `tests/validate.rs` keep importing `HostAdapterManifest`, etc., directly
// from `forge_core_cli` after the types moved into `host_adapter_types`.
pub use host_adapter_types::*;
// Re-export the host adapter manifest builder at the crate root so
// `main.rs`, `tests/validate.rs`, and the projection/policy/admission
// builders still in `lib.rs` keep calling `run_host_adapter_manifest()`
// directly from `forge_core_cli` after the builder moved into the
// `host_adapter_manifest` module.
pub use host_adapter_manifest::run_host_adapter_manifest;
// Re-export the host adapter projection/policy/admission builders at the
// crate root so `main.rs`, `tests/validate.rs`, and
// `forge-contract-validator` keep importing `run_host_adapter_projection`,
// `run_host_adapter_process_security_policy`,
// `run_host_adapter_invocation_admission`,
// `run_host_adapter_distribution_policy`, and
// `run_host_adapter_distribution_admission` directly from
// `forge_core_cli` after the builders moved into the
// `host_adapter_projection` module.
pub use host_adapter_projection::{
    run_host_adapter_distribution_admission, run_host_adapter_distribution_policy,
    run_host_adapter_invocation_admission, run_host_adapter_process_security_policy,
    run_host_adapter_projection,
};

use base64::{
    engine::general_purpose::{STANDARD as BASE64, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use ed25519_dalek::{
    Signature as Ed25519Signature, Verifier as Ed25519Verifier, VerifyingKey as Ed25519VerifyingKey,
};
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use rasn_ocsp::OcspResponseStatus;
use rustls_pki_types::CertificateDer;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use x509_parser::certificate::X509Certificate;
use x509_parser::extensions::{GeneralName, ParsedExtension};
use x509_parser::pem::parse_x509_pem;
use x509_parser::{parse_x509_certificate, parse_x509_crl};

pub fn run_host_adapter_artifact_verification(
    input: HostAdapterArtifactVerificationInput,
) -> HostAdapterArtifactVerification {
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let deferred_verification = vec![
        "signature_cryptographic_verification".to_string(),
        "provenance_predicate_semantic_verification".to_string(),
        "transparency_log_inclusion_verification".to_string(),
    ];
    let artifact_path = input.artifact_path.to_string_lossy().to_string();

    let normalized_expected = normalize_sha256_digest(&input.expected_sha256);
    if normalized_expected.is_none() {
        reasons.push("expected_sha256_invalid".to_string());
    }

    let artifact_bytes = match fs::read(&input.artifact_path) {
        Ok(bytes) => {
            verified_evidence.push("artifact_readable".to_string());
            Some(bytes)
        }
        Err(err) => {
            reasons.push(format!("artifact_read_failed:{:?}", err.kind()));
            None
        }
    };

    let computed_sha256 = artifact_bytes
        .as_deref()
        .map(|bytes| format!("sha256:{}", hex_sha256(bytes)));
    let byte_len = artifact_bytes.as_ref().map(Vec::len);

    match (normalized_expected.as_deref(), computed_sha256.as_deref()) {
        (Some(expected), Some(computed))
            if expected == normalize_sha256_display(computed).as_str() =>
        {
            verified_evidence.push("sha256_match".to_string());
        }
        (Some(_), Some(_)) => reasons.push("sha256_mismatch".to_string()),
        _ => {}
    }

    if input
        .signature_ref
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        verified_evidence.push("signature_ref_present".to_string());
    }

    if input
        .provenance_ref
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        verified_evidence.push("provenance_ref_present".to_string());
    } else {
        reasons.push("provenance_ref_required".to_string());
    }

    match input.source_ref.as_deref() {
        Some(source_ref) if source_ref_is_immutable(source_ref) => {
            verified_evidence.push("immutable_source_ref".to_string());
        }
        Some(_) => reasons.push("immutable_source_ref_required".to_string()),
        None => reasons.push("source_ref_required".to_string()),
    }

    if input.version.as_deref().is_some_and(version_like)
        && input
            .compatible_core_version
            .as_deref()
            .is_some_and(version_like)
    {
        verified_evidence.push("version_compatibility".to_string());
    } else {
        reasons.push("version_compatibility_required".to_string());
    }

    if input
        .rollback_ref
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        verified_evidence.push("rollback_ref_present".to_string());
    } else {
        reasons.push("rollback_ref_required".to_string());
    }

    if input
        .update_summary_ref
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        verified_evidence.push("update_summary_ref_present".to_string());
    } else {
        reasons.push("update_summary_ref_required".to_string());
    }

    HostAdapterArtifactVerification {
        status: if reasons.is_empty() {
            HostAdapterArtifactVerificationStatus::Passed
        } else {
            HostAdapterArtifactVerificationStatus::Failed
        },
        artifact_path,
        byte_len,
        expected_sha256: input.expected_sha256,
        computed_sha256,
        reasons,
        verified_evidence,
        deferred_verification,
    }
}

pub fn run_host_adapter_provenance_verification(
    input: HostAdapterProvenanceVerificationInput,
) -> HostAdapterProvenanceVerification {
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let artifact_path = input.artifact_path.to_string_lossy().to_string();
    let provenance_path = input.provenance_path.to_string_lossy().to_string();
    let signature_path = input.signature_path.to_string_lossy().to_string();
    let public_key_path = input.public_key_path.to_string_lossy().to_string();
    let transparency_log_path = input.transparency_log_path.to_string_lossy().to_string();

    let normalized_expected = normalize_sha256_digest(&input.expected_sha256);
    if normalized_expected.is_none() {
        reasons.push("expected_sha256_invalid".to_string());
    }

    let artifact_bytes = read_required_file(&input.artifact_path, "artifact", &mut reasons);
    let provenance_bytes = read_required_file(&input.provenance_path, "provenance", &mut reasons);
    let signature_bytes = read_signature_file(&input.signature_path, &mut reasons);
    let public_key_bytes = read_public_key_file(&input.public_key_path, &mut reasons);
    let transparency_log_bytes = read_required_file(
        &input.transparency_log_path,
        "transparency_log",
        &mut reasons,
    );

    let computed_artifact_sha256 = artifact_bytes
        .as_deref()
        .map(|bytes| format!("sha256:{}", hex_sha256(bytes)));
    let provenance_sha256 = provenance_bytes
        .as_deref()
        .map(|bytes| format!("sha256:{}", hex_sha256(bytes)));
    let signature_sha256 = signature_bytes
        .as_deref()
        .map(|bytes| format!("sha256:{}", hex_sha256(bytes)));

    match (
        normalized_expected.as_deref(),
        computed_artifact_sha256.as_deref(),
    ) {
        (Some(expected), Some(computed))
            if expected == normalize_sha256_display(computed).as_str() =>
        {
            verified_evidence.push("artifact_sha256_match".to_string());
        }
        (Some(_), Some(_)) => reasons.push("artifact_sha256_mismatch".to_string()),
        _ => {}
    }

    if let (Some(provenance), Some(signature), Some(public_key)) = (
        provenance_bytes.as_deref(),
        signature_bytes.as_deref(),
        public_key_bytes.as_deref(),
    ) {
        if verify_ed25519_signature(public_key, signature, provenance) {
            verified_evidence.push("provenance_signature_valid".to_string());
        } else {
            reasons.push("provenance_signature_invalid".to_string());
        }
    }

    let mut predicate_type = None;
    let mut builder_id = None;
    if let (Some(provenance), Some(expected_sha256)) =
        (provenance_bytes.as_deref(), normalized_expected.as_deref())
    {
        match serde_json::from_slice::<Value>(provenance) {
            Ok(statement) => {
                verify_slsa_statement(
                    &statement,
                    ExpectedProvenance {
                        sha256: expected_sha256,
                        builder_id: &input.expected_builder_id,
                        source_uri: &input.expected_source_uri,
                        source_ref: &input.expected_source_ref,
                    },
                    &mut predicate_type,
                    &mut builder_id,
                    &mut verified_evidence,
                    &mut reasons,
                );
            }
            Err(err) => reasons.push(format!("provenance_json_invalid:{err}")),
        }
    }

    if let (Some(provenance_sha256), Some(signature_sha256), Some(transparency_log)) = (
        provenance_sha256.as_deref(),
        signature_sha256.as_deref(),
        transparency_log_bytes.as_deref(),
    ) {
        verify_transparency_log_proof(
            provenance_sha256,
            signature_sha256,
            transparency_log,
            &mut verified_evidence,
            &mut reasons,
        );
    }

    HostAdapterProvenanceVerification {
        status: if reasons.is_empty() {
            HostAdapterProvenanceVerificationStatus::Passed
        } else {
            HostAdapterProvenanceVerificationStatus::Failed
        },
        artifact_path,
        provenance_path,
        signature_path,
        public_key_path,
        transparency_log_path,
        computed_artifact_sha256,
        provenance_sha256,
        signature_sha256,
        predicate_type,
        builder_id,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies detached Ed25519 provenance signature, SLSA/in-toto statement semantics, artifact/source/builder expectations, and Forge transparency proof inclusion. It does not yet verify Sigstore Fulcio certificate chains or public Rekor checkpoints.".to_string(),
    }
}

pub fn run_host_adapter_rekor_verification(
    input: HostAdapterRekorVerificationInput,
) -> HostAdapterRekorVerification {
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let log_entry_path = input.log_entry_path.to_string_lossy().to_string();
    let public_key_path = input.public_key_path.to_string_lossy().to_string();

    let log_entry_text = match fs::read_to_string(&input.log_entry_path) {
        Ok(value) => Some(value),
        Err(err) => {
            reasons.push(format!("rekor_log_entry_read_failed:{:?}", err.kind()));
            None
        }
    };
    let public_key_bytes =
        read_required_file(&input.public_key_path, "rekor_public_key", &mut reasons);

    let mut log_entry: Option<crypto_rekor::ParsedRekorEntry> = None;
    if let Some(text) = log_entry_text.as_deref() {
        match crypto_rekor::parse_rekor_log_entry(text) {
            Ok(entry) => {
                verified_evidence.push("rekor_log_entry_parsed".to_string());
                log_entry = Some(entry);
            }
            Err(reason) => reasons.push(reason),
        }
    }

    let rekor_key = public_key_bytes.as_deref().and_then(|bytes| {
        let pem = String::from_utf8_lossy(bytes);
        match P256VerifyingKey::from_public_key_pem(&pem) {
            Ok(key) => {
                verified_evidence.push("rekor_public_key_parsed".to_string());
                Some(key)
            }
            Err(err) => {
                reasons.push(format!("rekor_public_key_invalid:{err}"));
                None
            }
        }
    });

    if let Some(entry) = log_entry.as_ref() {
        if entry.log_id == input.expected_log_id {
            verified_evidence.push("rekor_log_id_match".to_string());
        } else {
            reasons.push("rekor_log_id_mismatch".to_string());
        }

        if let Some(key) = rekor_key.as_ref() {
            crypto_rekor::verify_rekor_entry_inclusion(
                entry,
                key,
                &mut verified_evidence,
                &mut reasons,
            );
        }
    }

    HostAdapterRekorVerification {
        status: if reasons.is_empty() {
            HostAdapterRekorVerificationStatus::Passed
        } else {
            HostAdapterRekorVerificationStatus::Failed
        },
        log_entry_path,
        public_key_path,
        expected_log_id: input.expected_log_id,
        observed_log_id: log_entry.as_ref().map(|entry| entry.log_id.clone()),
        log_index: log_entry.as_ref().map(|entry| entry.log_index),
        integrated_time: log_entry.as_ref().map(|entry| entry.integrated_time),
        reasons,
        verified_evidence,
        inference_boundary: "Verifies a Rekor log entry inclusion proof and signed checkpoint with an expected Rekor public key and log id. It does not by itself verify Fulcio identity, certificate chain policy, Sigstore bundle subject semantics, revocation, or release install authority.".to_string(),
    }
}

pub fn run_host_adapter_sigstore_trust_policy_verification(
    input: HostAdapterSigstoreTrustPolicyVerificationInput,
) -> HostAdapterSigstoreTrustPolicyVerification {
    let policy_path = input.policy_path.to_string_lossy().to_string();
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();

    let policy_text = match fs::read_to_string(&input.policy_path) {
        Ok(value) => Some(value),
        Err(err) => {
            reasons.push(format!(
                "sigstore_trust_policy_read_failed:{:?}",
                err.kind()
            ));
            None
        }
    };

    let policy_document = policy_text.as_deref().and_then(|text| {
        match serde_yaml::from_str::<SigstoreTrustedRootPolicyDocument>(text) {
            Ok(value) => {
                verified_evidence.push("sigstore_trust_policy_parsed".to_string());
                Some(value)
            }
            Err(err) => {
                reasons.push(format!("sigstore_trust_policy_parse_failed:{err}"));
                None
            }
        }
    });

    let mut schema_version = None;
    let mut root_source = None;
    let mut trusted_root_ref = None;
    let mut timestamp_mode = None;
    let mut expected_oidc_issuer = None;
    let mut expected_certificate_identity = None;
    let mut expected_github_repository = None;
    let mut expected_github_ref = None;
    let mut expected_github_sha = None;

    if let Some(document) = policy_document.as_ref() {
        schema_version = Some(document.schema_version.clone());
        let policy = &document.sigstore_trusted_root_policy;
        root_source = Some(policy.root_source.clone());
        trusted_root_ref = Some(policy.trusted_root_ref.clone());
        timestamp_mode = Some(policy.timestamp_authority.mode.clone());
        expected_oidc_issuer = Some(policy.identity_policy.expected_oidc_issuer.clone());
        expected_certificate_identity =
            policy.identity_policy.expected_certificate_identity.clone();
        expected_github_repository = policy.identity_policy.expected_github_repository.clone();
        expected_github_ref = policy.identity_policy.expected_github_ref.clone();
        expected_github_sha = policy.identity_policy.expected_github_sha.clone();

        verify_sigstore_trust_policy(document, &mut verified_evidence, &mut reasons);
    }

    HostAdapterSigstoreTrustPolicyVerification {
        status: if reasons.is_empty() {
            HostAdapterSigstoreTrustPolicyVerificationStatus::Passed
        } else {
            HostAdapterSigstoreTrustPolicyVerificationStatus::Failed
        },
        policy_path,
        schema_version,
        root_source,
        trusted_root_ref,
        timestamp_mode,
        expected_oidc_issuer,
        expected_certificate_identity,
        expected_github_repository,
        expected_github_ref,
        expected_github_sha,
        reasons,
        verified_evidence,
        inference_boundary: "Validates Forge's Sigstore trusted-root policy shape, required trust material, identity policy, and timestamp source consistency. It does not verify a Fulcio certificate chain, OIDC certificate extensions, Sigstore bundle subject binding, Rekor inclusion, RFC3161 timestamp signatures, revocation status, TUF metadata freshness, or release install authority.".to_string(),
    }
}

pub fn run_host_adapter_fulcio_certificate_identity_verification(
    input: HostAdapterFulcioCertificateIdentityVerificationInput,
) -> HostAdapterFulcioCertificateIdentityVerification {
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let certificate_path = input.certificate_path.to_string_lossy().to_string();
    let issuer_certificate_paths = input
        .issuer_certificate_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut expected_oidc_issuer = None;
    let mut expected_certificate_identity = None;
    let mut expected_github_repository = None;
    let mut expected_github_ref = None;
    let mut expected_github_sha = None;
    let mut observed_subject_alt_names = Vec::new();
    let mut observed_oidc_issuer = None;
    let mut observed_build_signer_uri = None;
    let mut observed_build_signer_digest = None;
    let mut observed_source_repository_uri = None;
    let mut observed_source_repository_digest = None;
    let mut observed_source_repository_ref = None;
    let mut observed_token_subject = None;

    let trust_policy = read_sigstore_trust_policy_document(
        &input.trust_policy_path,
        "fulcio_identity_trust_policy",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(document) = trust_policy.as_ref() {
        verify_sigstore_trust_policy(document, &mut verified_evidence, &mut reasons);
        let identity_policy = &document.sigstore_trusted_root_policy.identity_policy;
        expected_oidc_issuer = Some(identity_policy.expected_oidc_issuer.clone());
        expected_certificate_identity = identity_policy.expected_certificate_identity.clone();
        expected_github_repository = identity_policy.expected_github_repository.clone();
        expected_github_ref = identity_policy.expected_github_ref.clone();
        expected_github_sha = identity_policy.expected_github_sha.clone();
    }

    if input.issuer_certificate_paths.is_empty() {
        reasons.push("fulcio_issuer_certificate_paths_missing".to_string());
    }

    let leaf_der = read_certificate_der(
        &input.certificate_path,
        "leaf_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    let issuer_ders = input
        .issuer_certificate_paths
        .iter()
        .map(|path| {
            read_certificate_der(
                path,
                "issuer_certificate",
                &mut verified_evidence,
                &mut reasons,
            )
        })
        .collect::<Vec<_>>();

    if let (Some(document), Some(leaf_der)) = (trust_policy.as_ref(), leaf_der.as_ref()) {
        let issuer_der_refs = issuer_ders
            .iter()
            .filter_map(Option::as_ref)
            .collect::<Vec<_>>();
        if issuer_der_refs.len() == input.issuer_certificate_paths.len()
            && !issuer_der_refs.is_empty()
        {
            if let Some(leaf) = parse_certificate(
                leaf_der,
                "leaf_certificate",
                &mut verified_evidence,
                &mut reasons,
            ) {
                let issuers = issuer_der_refs
                    .iter()
                    .enumerate()
                    .filter_map(|(index, der)| {
                        parse_certificate(
                            der,
                            &format!("issuer_certificate_{index}"),
                            &mut verified_evidence,
                            &mut reasons,
                        )
                    })
                    .collect::<Vec<_>>();
                if issuers.len() == issuer_der_refs.len() {
                    verify_fulcio_chain(
                        &leaf,
                        &issuers,
                        &input.issuer_certificate_paths,
                        document,
                        input.verification_time_unix,
                        &mut verified_evidence,
                        &mut reasons,
                    );
                    let identity = extract_fulcio_certificate_identity(&leaf);
                    observed_subject_alt_names = identity.subject_alt_names.clone();
                    observed_oidc_issuer = identity.oidc_issuer.clone();
                    observed_build_signer_uri = identity.build_signer_uri.clone();
                    observed_build_signer_digest = identity.build_signer_digest.clone();
                    observed_source_repository_uri = identity.source_repository_uri.clone();
                    observed_source_repository_digest = identity.source_repository_digest.clone();
                    observed_source_repository_ref = identity.source_repository_ref.clone();
                    observed_token_subject = identity.token_subject.clone();
                    verify_fulcio_identity_selectors(
                        document,
                        &identity,
                        &mut verified_evidence,
                        &mut reasons,
                    );
                }
            }
        }
    }

    HostAdapterFulcioCertificateIdentityVerification {
        status: if reasons.is_empty() {
            HostAdapterFulcioCertificateIdentityVerificationStatus::Passed
        } else {
            HostAdapterFulcioCertificateIdentityVerificationStatus::Failed
        },
        trust_policy_path,
        certificate_path,
        issuer_certificate_paths,
        verification_time_unix: input.verification_time_unix,
        expected_oidc_issuer,
        expected_certificate_identity,
        expected_github_repository,
        expected_github_ref,
        expected_github_sha,
        observed_subject_alt_names,
        observed_oidc_issuer,
        observed_build_signer_uri,
        observed_build_signer_digest,
        observed_source_repository_uri,
        observed_source_repository_digest,
        observed_source_repository_ref,
        observed_token_subject,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies a supplied Fulcio-style certificate chain, leaf certificate validity window, code-signing usage when declared, SAN identity, OIDC issuer extension, and declared workflow identity selectors against Forge's Sigstore trusted-root policy. It does not verify Sigstore bundle subject binding, artifact signature binding, Rekor inclusion or signed checkpoints, certificate transparency SCTs, RFC3161 TSA signatures, revocation status, TUF metadata freshness, installer mutation authority, or future FIDO assurance.".to_string(),
    }
}

pub fn run_host_adapter_sigstore_bundle_subject_verification(
    input: HostAdapterSigstoreBundleSubjectVerificationInput,
) -> HostAdapterSigstoreBundleSubjectVerification {
    let bundle_path = input.bundle_path.to_string_lossy().to_string();
    let artifact_path = input.artifact_path.to_string_lossy().to_string();
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let certificate_path = input.certificate_path.to_string_lossy().to_string();
    let issuer_certificate_paths = input
        .issuer_certificate_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let rekor_log_entry_path = input.rekor_log_entry_path.to_string_lossy().to_string();
    let rekor_public_key_path = input.rekor_public_key_path.to_string_lossy().to_string();
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut media_type = None;
    let mut bundle_message_digest_sha256 = None;
    let mut bundle_signature_sha256 = None;
    let mut rekor_integrated_time = None;
    let mut fulcio_status = None;

    let artifact_bytes = read_required_file(&input.artifact_path, "artifact", &mut reasons);
    let computed_artifact_sha256 = artifact_bytes
        .as_deref()
        .map(|bytes| format!("sha256:{}", hex_sha256(bytes)));

    let bundle_bytes = read_required_file(&input.bundle_path, "sigstore_bundle", &mut reasons);
    let bundle = bundle_bytes
        .as_deref()
        .and_then(|bytes| parse_sigstore_message_signature_bundle(bytes, &mut reasons));

    let certificate_der = read_certificate_der(
        &input.certificate_path,
        "bundle_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    let leaf_certificate = certificate_der.as_deref().and_then(|der| {
        parse_certificate(
            der,
            "bundle_certificate",
            &mut verified_evidence,
            &mut reasons,
        )
    });

    let parsed_rekor_entry = fs::read_to_string(&input.rekor_log_entry_path)
        .map_err(|err| {
            reasons.push(format!(
                "bundle_rekor_log_entry_read_failed:{:?}",
                err.kind()
            ))
        })
        .ok()
        .and_then(|text| match crypto_rekor::parse_rekor_log_entry(&text) {
            Ok(entry) => {
                rekor_integrated_time = Some(entry.integrated_time);
                verified_evidence.push("bundle_rekor_log_entry_parsed".to_string());
                Some(entry)
            }
            Err(reason) => {
                reasons.push(reason);
                None
            }
        });

    if let Some(bundle) = bundle.as_ref() {
        media_type = bundle.media_type.clone();
        bundle_message_digest_sha256 =
            Some(format!("sha256:{}", hex_bytes(&bundle.message_digest)));
        bundle_signature_sha256 = Some(format!("sha256:{}", hex_sha256(&bundle.signature)));

        if bundle.media_type.as_deref() == Some("application/vnd.dev.sigstore.bundle.v0.3+json") {
            verified_evidence.push("bundle_media_type_v03".to_string());
        } else {
            reasons.push("bundle_media_type_unsupported".to_string());
        }

        if bundle.message_digest_algorithm == "sha256"
            || bundle.message_digest_algorithm == "sha2_256"
            || bundle.message_digest_algorithm == "sha-256"
        {
            verified_evidence.push("bundle_message_digest_algorithm_sha256".to_string());
        } else {
            reasons.push("bundle_message_digest_algorithm_unsupported".to_string());
        }

        if let Some(computed) = computed_artifact_sha256.as_deref() {
            if normalize_sha256_display(computed) == hex_bytes(&bundle.message_digest) {
                verified_evidence.push("bundle_message_digest_matches_artifact".to_string());
            } else {
                reasons.push("bundle_message_digest_mismatch".to_string());
            }
        }

        if let Some(certificate_der) = certificate_der.as_deref() {
            if bundle.certificate_der == certificate_der {
                verified_evidence.push("bundle_certificate_matches_input".to_string());
            } else {
                reasons.push("bundle_certificate_mismatch".to_string());
            }
        }

        if let Some(certificate) = leaf_certificate.as_ref() {
            verify_bundle_signature_with_certificate(
                certificate,
                &bundle.message_digest,
                &bundle.signature,
                &mut verified_evidence,
                &mut reasons,
            );
        }

        if let Some(rekor_entry) = parsed_rekor_entry.as_ref() {
            verify_rekor_body_binds_bundle(
                rekor_entry,
                &bundle.message_digest,
                &bundle.signature,
                &mut verified_evidence,
                &mut reasons,
            );
        }
    }

    if let Some(integrated_time) = rekor_integrated_time {
        let fulcio_verification = run_host_adapter_fulcio_certificate_identity_verification(
            HostAdapterFulcioCertificateIdentityVerificationInput {
                trust_policy_path: input.trust_policy_path,
                certificate_path: input.certificate_path,
                issuer_certificate_paths: input.issuer_certificate_paths,
                verification_time_unix: integrated_time,
            },
        );
        fulcio_status = Some(fulcio_verification.status);
        if fulcio_verification.status
            == HostAdapterFulcioCertificateIdentityVerificationStatus::Passed
        {
            verified_evidence.push("bundle_fulcio_identity_verified_at_rekor_time".to_string());
        } else {
            reasons.extend(
                fulcio_verification
                    .reasons
                    .into_iter()
                    .map(|reason| format!("fulcio_identity:{reason}")),
            );
        }
    } else {
        reasons.push("bundle_rekor_integrated_time_missing".to_string());
    }

    let rekor_verification =
        run_host_adapter_rekor_verification(HostAdapterRekorVerificationInput {
            log_entry_path: input.rekor_log_entry_path,
            public_key_path: input.rekor_public_key_path,
            expected_log_id: input.expected_rekor_log_id.clone(),
        });
    let rekor_status = Some(rekor_verification.status);
    if rekor_verification.status == HostAdapterRekorVerificationStatus::Passed {
        verified_evidence.push("bundle_rekor_entry_verified".to_string());
    } else {
        reasons.extend(
            rekor_verification
                .reasons
                .into_iter()
                .map(|reason| format!("rekor:{reason}")),
        );
    }

    HostAdapterSigstoreBundleSubjectVerification {
        status: if reasons.is_empty() {
            HostAdapterSigstoreBundleSubjectVerificationStatus::Passed
        } else {
            HostAdapterSigstoreBundleSubjectVerificationStatus::Failed
        },
        bundle_path,
        artifact_path,
        trust_policy_path,
        certificate_path,
        issuer_certificate_paths,
        rekor_log_entry_path,
        rekor_public_key_path,
        expected_rekor_log_id: input.expected_rekor_log_id,
        media_type,
        computed_artifact_sha256,
        bundle_message_digest_sha256,
        bundle_signature_sha256,
        rekor_integrated_time,
        fulcio_status,
        rekor_status,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies Sigstore bundle subject binding for a v0.3 messageSignature bundle by binding artifact SHA-256 digest, certificate-carried P-256 signing key, bundle signature, Fulcio certificate identity, Rekor body, and Rekor inclusion evidence. It does not verify DSSE envelopes, RFC3161 TSA signatures, certificate transparency SCTs, revocation, TUF trusted-root freshness, policy thresholds, installer mutation authority, or post-quantum algorithms.".to_string(),
    }
}

pub fn run_host_adapter_sigstore_dsse_in_toto_subject_verification(
    input: HostAdapterSigstoreDsseInTotoSubjectVerificationInput,
) -> HostAdapterSigstoreDsseInTotoSubjectVerification {
    let bundle_path = input.bundle_path.to_string_lossy().to_string();
    let artifact_path = input.artifact_path.to_string_lossy().to_string();
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let certificate_path = input.certificate_path.to_string_lossy().to_string();
    let issuer_certificate_paths = input
        .issuer_certificate_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let rekor_log_entry_path = input.rekor_log_entry_path.to_string_lossy().to_string();
    let rekor_public_key_path = input.rekor_public_key_path.to_string_lossy().to_string();
    let expected_rekor_log_id = input.expected_rekor_log_id.clone();
    let expected_predicate_type = input.expected_predicate_type.clone();
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut media_type = None;
    let mut payload_type = None;
    let mut dsse_payload_sha256 = None;
    let mut dsse_envelope_sha256 = None;
    let mut dsse_signature_sha256 = None;
    let mut statement_type = None;
    let mut predicate_type = None;
    let mut rekor_integrated_time = None;
    let mut fulcio_status = None;

    let artifact_bytes = read_required_file(&input.artifact_path, "artifact", &mut reasons);
    let computed_artifact_hex = artifact_bytes.as_deref().map(hex_sha256);
    let computed_artifact_sha256 = computed_artifact_hex
        .as_ref()
        .map(|digest| format!("sha256:{digest}"));

    let bundle_bytes = read_required_file(&input.bundle_path, "sigstore_dsse_bundle", &mut reasons);
    let bundle = bundle_bytes
        .as_deref()
        .and_then(|bytes| parse_sigstore_dsse_bundle(bytes, &mut reasons));

    let certificate_der = read_certificate_der(
        &input.certificate_path,
        "dsse_bundle_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    let leaf_certificate = certificate_der.as_deref().and_then(|der| {
        parse_certificate(
            der,
            "dsse_bundle_certificate",
            &mut verified_evidence,
            &mut reasons,
        )
    });

    let parsed_rekor_entry = fs::read_to_string(&input.rekor_log_entry_path)
        .map_err(|err| reasons.push(format!("dsse_rekor_log_entry_read_failed:{:?}", err.kind())))
        .ok()
        .and_then(|text| match crypto_rekor::parse_rekor_log_entry(&text) {
            Ok(entry) => {
                rekor_integrated_time = Some(entry.integrated_time);
                verified_evidence.push("dsse_rekor_log_entry_parsed".to_string());
                Some(entry)
            }
            Err(reason) => {
                reasons.push(reason);
                None
            }
        });

    if let Some(bundle) = bundle.as_ref() {
        media_type = bundle.media_type.clone();
        payload_type = Some(bundle.payload_type.clone());
        let payload_hash = hex_sha256(&bundle.payload);
        let envelope_hash = match serde_json_canonicalizer::to_vec(&bundle.envelope) {
            Ok(bytes) => Some(hex_sha256(&bytes)),
            Err(err) => {
                reasons.push(format!("dsse_envelope_canonicalization_failed:{err}"));
                None
            }
        };
        dsse_payload_sha256 = Some(format!("sha256:{payload_hash}"));
        dsse_envelope_sha256 = envelope_hash
            .as_ref()
            .map(|digest| format!("sha256:{digest}"));
        dsse_signature_sha256 = Some(format!("sha256:{}", hex_sha256(&bundle.signature)));

        if bundle.media_type.as_deref() == Some("application/vnd.dev.sigstore.bundle.v0.3+json") {
            verified_evidence.push("dsse_bundle_media_type_v03".to_string());
        } else {
            reasons.push("dsse_bundle_media_type_unsupported".to_string());
        }

        if bundle.payload_type == "application/vnd.in-toto+json" {
            verified_evidence.push("dsse_payload_type_in_toto_json".to_string());
        } else {
            reasons.push("dsse_payload_type_unsupported".to_string());
        }

        if let Some(certificate_der) = certificate_der.as_deref() {
            if bundle.certificate_der == certificate_der {
                verified_evidence.push("dsse_bundle_certificate_matches_input".to_string());
            } else {
                reasons.push("dsse_bundle_certificate_mismatch".to_string());
            }
        }

        if let Some(certificate) = leaf_certificate.as_ref() {
            verify_dsse_signature_with_certificate(
                certificate,
                &bundle.payload_type,
                &bundle.payload,
                &bundle.signature,
                &mut verified_evidence,
                &mut reasons,
            );
        }

        match serde_json::from_slice::<Value>(&bundle.payload) {
            Ok(statement) => {
                verified_evidence.push("dsse_payload_json_parsed".to_string());
                statement_type = statement
                    .get("_type")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                predicate_type = statement
                    .get("predicateType")
                    .and_then(Value::as_str)
                    .map(str::to_string);

                match statement_type.as_deref() {
                    Some(value) if value.starts_with("https://in-toto.io/Statement/v") => {
                        verified_evidence.push("dsse_intoto_statement_type".to_string());
                    }
                    Some(_) => reasons.push("dsse_intoto_statement_type_invalid".to_string()),
                    None => reasons.push("dsse_intoto_statement_type_missing".to_string()),
                }

                match predicate_type.as_deref() {
                    Some(value) => {
                        verified_evidence.push("dsse_intoto_predicate_type_present".to_string());
                        if let Some(expected) = expected_predicate_type.as_deref() {
                            if value == expected {
                                verified_evidence
                                    .push("dsse_intoto_predicate_type_expected".to_string());
                            } else {
                                reasons.push("dsse_intoto_predicate_type_mismatch".to_string());
                            }
                        }
                    }
                    None => reasons.push("dsse_intoto_predicate_type_missing".to_string()),
                }

                if let Some(computed) = computed_artifact_hex.as_deref() {
                    if statement_subject_has_sha256(&statement, computed) {
                        verified_evidence.push("dsse_intoto_subject_matches_artifact".to_string());
                    } else {
                        reasons.push("dsse_intoto_subject_sha256_missing".to_string());
                    }
                }
            }
            Err(err) => reasons.push(format!("dsse_payload_json_invalid:{err}")),
        }

        if let (Some(rekor_entry), Some(envelope_hash)) =
            (parsed_rekor_entry.as_ref(), envelope_hash.as_deref())
        {
            verify_rekor_body_binds_dsse(
                rekor_entry,
                &payload_hash,
                envelope_hash,
                &bundle.signature,
                &mut verified_evidence,
                &mut reasons,
            );
        }
    }

    if let Some(integrated_time) = rekor_integrated_time {
        let fulcio_verification = run_host_adapter_fulcio_certificate_identity_verification(
            HostAdapterFulcioCertificateIdentityVerificationInput {
                trust_policy_path: input.trust_policy_path,
                certificate_path: input.certificate_path,
                issuer_certificate_paths: input.issuer_certificate_paths,
                verification_time_unix: integrated_time,
            },
        );
        fulcio_status = Some(fulcio_verification.status);
        if fulcio_verification.status
            == HostAdapterFulcioCertificateIdentityVerificationStatus::Passed
        {
            verified_evidence.push("dsse_fulcio_identity_verified_at_rekor_time".to_string());
        } else {
            reasons.extend(
                fulcio_verification
                    .reasons
                    .into_iter()
                    .map(|reason| format!("fulcio_identity:{reason}")),
            );
        }
    } else {
        reasons.push("dsse_rekor_integrated_time_missing".to_string());
    }

    let rekor_verification =
        run_host_adapter_rekor_verification(HostAdapterRekorVerificationInput {
            log_entry_path: input.rekor_log_entry_path,
            public_key_path: input.rekor_public_key_path,
            expected_log_id: expected_rekor_log_id.clone(),
        });
    let rekor_status = Some(rekor_verification.status);
    if rekor_verification.status == HostAdapterRekorVerificationStatus::Passed {
        verified_evidence.push("dsse_rekor_entry_verified".to_string());
    } else {
        reasons.extend(
            rekor_verification
                .reasons
                .into_iter()
                .map(|reason| format!("rekor:{reason}")),
        );
    }

    HostAdapterSigstoreDsseInTotoSubjectVerification {
        status: if reasons.is_empty() {
            HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Passed
        } else {
            HostAdapterSigstoreDsseInTotoSubjectVerificationStatus::Failed
        },
        bundle_path,
        artifact_path,
        trust_policy_path,
        certificate_path,
        issuer_certificate_paths,
        rekor_log_entry_path,
        rekor_public_key_path,
        expected_rekor_log_id,
        expected_predicate_type,
        media_type,
        payload_type,
        computed_artifact_sha256,
        dsse_payload_sha256,
        dsse_envelope_sha256,
        dsse_signature_sha256,
        statement_type,
        predicate_type,
        rekor_integrated_time,
        fulcio_status,
        rekor_status,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies Sigstore DSSE/in-toto subject binding for a v0.3 bundle by binding payloadType, DSSE PAE signature, in-toto statement subject SHA-256 digest, certificate-carried P-256 signing key, Fulcio certificate identity, Rekor body, and Rekor inclusion evidence. It does not verify messageSignature bundles, RFC3161 TSA signatures, certificate transparency SCTs, revocation, TUF trusted-root freshness, multi-signature threshold policy, installer mutation authority, or post-quantum algorithms.".to_string(),
    }
}

pub fn run_host_adapter_sigstore_timestamp_authority_verification(
    input: HostAdapterSigstoreTimestampAuthorityVerificationInput,
) -> HostAdapterSigstoreTimestampAuthorityVerification {
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let certificate_path = input.certificate_path.to_string_lossy().to_string();
    let rekor_log_entry_path = input
        .rekor_log_entry_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let rekor_public_key_path = input
        .rekor_public_key_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let expected_rekor_log_id = input.expected_rekor_log_id.clone();
    let rfc3161_timestamp_token_path = input
        .rfc3161_timestamp_token_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let rfc3161_timestamped_signature_path = input
        .rfc3161_timestamped_signature_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let deferred_verification = vec![
        "certificate_transparency_sct".to_string(),
        "revocation_status".to_string(),
        "tuf_metadata_freshness".to_string(),
        "release_install_update_authority".to_string(),
    ];
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut policy_mode = None;
    let mut selected_timestamp_source = None;
    let mut observed_timestamp_unix = None;
    let mut certificate_not_before_unix = None;
    let mut certificate_not_after_unix = None;
    let mut rekor_status = None;
    let mut rfc3161_tsa_certificate_refs = Vec::new();

    let trust_policy = read_sigstore_trust_policy_document(
        &input.trust_policy_path,
        "timestamp_authority_trust_policy",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(document) = trust_policy.as_ref() {
        verify_sigstore_trust_policy(document, &mut verified_evidence, &mut reasons);
        policy_mode = Some(
            document
                .sigstore_trusted_root_policy
                .timestamp_authority
                .mode
                .clone(),
        );
        rfc3161_tsa_certificate_refs = document
            .sigstore_trusted_root_policy
            .timestamp_authority
            .certificate_refs
            .clone();
    }

    let certificate_der = read_certificate_der(
        &input.certificate_path,
        "timestamp_authority_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(certificate_der) = certificate_der.as_ref() {
        if let Some(certificate) = parse_certificate(
            certificate_der,
            "timestamp_authority_certificate",
            &mut verified_evidence,
            &mut reasons,
        ) {
            let validity = certificate.validity();
            certificate_not_before_unix = Some(validity.not_before.timestamp());
            certificate_not_after_unix = Some(validity.not_after.timestamp());
            verified_evidence.push("timestamp_certificate_validity_window_loaded".to_string());
        }
    }

    match policy_mode.as_deref() {
        Some("rekor_integrated_time") => {
            select_rekor_integrated_time_for_timestamp_authority(
                &input,
                &mut selected_timestamp_source,
                &mut observed_timestamp_unix,
                &mut rekor_status,
                &mut verified_evidence,
                &mut reasons,
            );
        }
        Some("either") => {
            if input.rekor_log_entry_path.is_some()
                && input.rekor_public_key_path.is_some()
                && input.expected_rekor_log_id.is_some()
            {
                select_rekor_integrated_time_for_timestamp_authority(
                    &input,
                    &mut selected_timestamp_source,
                    &mut observed_timestamp_unix,
                    &mut rekor_status,
                    &mut verified_evidence,
                    &mut reasons,
                );
            } else if input.rfc3161_timestamp_token_path.is_some()
                || input.rfc3161_timestamped_signature_path.is_some()
            {
                select_rfc3161_tsa_for_timestamp_authority(
                    &input,
                    trust_policy.as_ref(),
                    &mut selected_timestamp_source,
                    &mut observed_timestamp_unix,
                    &mut verified_evidence,
                    &mut reasons,
                );
            } else {
                reasons.push("timestamp_source_missing".to_string());
            }
        }
        Some("rfc3161_tsa") => {
            select_rfc3161_tsa_for_timestamp_authority(
                &input,
                trust_policy.as_ref(),
                &mut selected_timestamp_source,
                &mut observed_timestamp_unix,
                &mut verified_evidence,
                &mut reasons,
            );
        }
        Some(_) => reasons.push("timestamp_policy_mode_unknown".to_string()),
        None => reasons.push("timestamp_policy_mode_missing".to_string()),
    }

    if let (Some(timestamp), Some(not_before), Some(not_after)) = (
        observed_timestamp_unix,
        certificate_not_before_unix,
        certificate_not_after_unix,
    ) {
        if timestamp >= not_before && timestamp <= not_after {
            verified_evidence.push("timestamp_within_certificate_validity".to_string());
        } else {
            reasons.push("timestamp_outside_certificate_validity".to_string());
        }
    } else if selected_timestamp_source.is_some() {
        reasons.push("timestamp_certificate_validity_window_missing".to_string());
    }

    HostAdapterSigstoreTimestampAuthorityVerification {
        status: if reasons.is_empty() {
            HostAdapterSigstoreTimestampAuthorityVerificationStatus::Passed
        } else {
            HostAdapterSigstoreTimestampAuthorityVerificationStatus::Failed
        },
        trust_policy_path,
        certificate_path,
        rekor_log_entry_path,
        rekor_public_key_path,
        expected_rekor_log_id,
        rfc3161_timestamp_token_path,
        rfc3161_timestamped_signature_path,
        rfc3161_tsa_certificate_refs,
        policy_mode,
        selected_timestamp_source,
        observed_timestamp_unix,
        certificate_not_before_unix,
        certificate_not_after_unix,
        rekor_status,
        deferred_verification,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies Sigstore trusted-time source selection and certificate validity-window evaluation using verified Rekor integrated time or verified RFC3161 TSA token evidence. RFC3161 verification covers token parsing, message imprint, CMS signature, TSA certificate chain, and timestamp extraction for supplied signature bytes. It does not verify certificate transparency SCTs, revocation status, TUF trusted-root freshness, release install/update authority, or post-quantum algorithms.".to_string(),
    }
}

pub fn run_host_adapter_certificate_transparency_sct_verification(
    input: HostAdapterCertificateTransparencySctVerificationInput,
) -> HostAdapterCertificateTransparencySctVerification {
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let certificate_path = input.certificate_path.to_string_lossy().to_string();
    let sct_paths = input
        .sct_paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let deferred_verification = vec![
        "embedded_sct_extension_extraction".to_string(),
        "ct_log_inclusion_proof_fetch".to_string(),
        "ct_log_mmd_audit".to_string(),
        "revocation_status".to_string(),
        "tuf_trusted_root_freshness".to_string(),
        "release_install_update_authority".to_string(),
    ];
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut policy_log_ids = Vec::new();
    let mut ct_public_key_refs = Vec::new();
    let mut verified_log_ids = Vec::new();

    let trust_policy = read_sigstore_trust_policy_document(
        &input.trust_policy_path,
        "ct_sct_trust_policy",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(document) = trust_policy.as_ref() {
        verify_sigstore_trust_policy(document, &mut verified_evidence, &mut reasons);
        policy_log_ids = document
            .sigstore_trusted_root_policy
            .certificate_transparency
            .log_ids
            .clone();
        ct_public_key_refs = document
            .sigstore_trusted_root_policy
            .certificate_transparency
            .public_key_refs
            .clone();
    }

    let certificate_der = read_certificate_der(
        &input.certificate_path,
        "ct_sct_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(certificate_der) = certificate_der.as_ref() {
        parse_certificate(
            certificate_der,
            "ct_sct_certificate",
            &mut verified_evidence,
            &mut reasons,
        );
    }

    if input.sct_paths.is_empty() {
        reasons.push("ct_sct_paths_missing".to_string());
    }
    let mut sct_bytes = Vec::new();
    for path in &input.sct_paths {
        if let Some(bytes) = read_required_file(path, "ct_sct", &mut reasons) {
            verified_evidence.push("ct_sct_bytes_loaded".to_string());
            sct_bytes.push((path, bytes));
        }
    }

    let ct_log_material = trust_policy
        .as_ref()
        .map(|document| {
            load_certificate_transparency_log_material(
                &input.trust_policy_path,
                document,
                &mut verified_evidence,
                &mut reasons,
            )
        })
        .unwrap_or_default();

    let ct_logs = ct_log_material
        .iter()
        .map(|material| sct::Log {
            description: "",
            url: "",
            operated_by: "",
            key: material.key.as_slice(),
            id: material.id,
            max_merge_delay: 0,
        })
        .collect::<Vec<_>>();
    let ct_log_refs = ct_logs.iter().collect::<Vec<_>>();

    if let Some(certificate_der) = certificate_der.as_deref() {
        for (path, sct) in sct_bytes {
            match sct::verify_sct(
                certificate_der,
                sct.as_slice(),
                input.verification_time_unix_ms,
                &ct_log_refs,
            ) {
                Ok(index) => {
                    if let Some(material) = ct_log_material.get(index) {
                        verified_log_ids.push(material.id_hex.clone());
                        verified_evidence
                            .push(format!("ct_sct_signature_verified:{}", material.id_hex));
                    } else {
                        reasons.push(format!(
                            "ct_sct_verified_log_index_missing:{}",
                            path.to_string_lossy()
                        ));
                    }
                }
                Err(err) => reasons.push(format!(
                    "ct_sct_verification_failed:{}:{err:?}",
                    path.to_string_lossy()
                )),
            }
        }
    }

    HostAdapterCertificateTransparencySctVerification {
        status: if reasons.is_empty() {
            HostAdapterCertificateTransparencySctVerificationStatus::Passed
        } else {
            HostAdapterCertificateTransparencySctVerificationStatus::Failed
        },
        trust_policy_path,
        certificate_path,
        sct_paths,
        verification_time_unix_ms: input.verification_time_unix_ms,
        policy_log_ids,
        ct_public_key_refs,
        verified_sct_count: verified_log_ids.len(),
        verified_log_ids,
        deferred_verification,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies supplied RFC6962 Signed Certificate Timestamp bytes offline against a supplied DER certificate and policy-declared raw CT log verification keys. It does not extract embedded SCT extensions, fetch CT inclusion proofs, audit maximum merge delay, check revocation, refresh TUF trusted roots, mutate installations, or decide release update authority.".to_string(),
    }
}

pub fn run_host_adapter_certificate_revocation_policy_verification(
    input: HostAdapterCertificateRevocationPolicyVerificationInput,
) -> HostAdapterCertificateRevocationPolicyVerification {
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let certificate_path = input.certificate_path.to_string_lossy().to_string();
    let deferred_verification = vec![
        "explicit_crl_status".to_string(),
        "explicit_ocsp_status".to_string(),
        "revocation_distribution_point_fetch".to_string(),
        "ocsp_responder_network_fetch".to_string(),
        "tuf_trusted_root_freshness".to_string(),
        "release_install_update_authority".to_string(),
    ];
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut policy_mode = None;
    let mut max_certificate_lifetime_seconds = None;
    let mut certificate_not_before_unix = None;
    let mut certificate_not_after_unix = None;
    let mut certificate_lifetime_seconds = None;
    let mut revocation_strategy = None;
    let mut revocation_status = None;

    let trust_policy = read_sigstore_trust_policy_document(
        &input.trust_policy_path,
        "certificate_revocation_trust_policy",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(document) = trust_policy.as_ref() {
        verify_sigstore_trust_policy(document, &mut verified_evidence, &mut reasons);
        if let Some(revocation) = document.sigstore_trusted_root_policy.revocation.as_ref() {
            policy_mode = Some(revocation.mode.clone());
            max_certificate_lifetime_seconds = revocation.max_certificate_lifetime_seconds;
            verified_evidence.push("certificate_revocation_policy_declared".to_string());
        } else {
            reasons.push("certificate_revocation_policy_missing".to_string());
        }
    }

    let certificate_der = read_certificate_der(
        &input.certificate_path,
        "certificate_revocation_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(certificate_der) = certificate_der.as_ref() {
        if let Some(certificate) = parse_certificate(
            certificate_der,
            "certificate_revocation_certificate",
            &mut verified_evidence,
            &mut reasons,
        ) {
            let validity = certificate.validity();
            let not_before = validity.not_before.timestamp();
            let not_after = validity.not_after.timestamp();
            certificate_not_before_unix = Some(not_before);
            certificate_not_after_unix = Some(not_after);
            certificate_lifetime_seconds = Some(not_after - not_before);
            verified_evidence.push("certificate_revocation_validity_window_loaded".to_string());
        }
    }

    match policy_mode.as_deref() {
        Some("short_lived_certificate") => {
            revocation_strategy = Some("implicit_short_lived_certificate".to_string());
            revocation_status = Some("not_checked_by_short_lived_policy".to_string());
            verified_evidence.push("certificate_revocation_policy_short_lived".to_string());
            match (
                max_certificate_lifetime_seconds,
                certificate_lifetime_seconds,
            ) {
                (Some(max_lifetime), Some(actual_lifetime))
                    if max_lifetime > 0 && actual_lifetime <= max_lifetime =>
                {
                    verified_evidence.push(
                        "certificate_revocation_certificate_lifetime_within_policy".to_string(),
                    );
                }
                (Some(max_lifetime), Some(_)) if max_lifetime <= 0 => {
                    reasons.push("certificate_revocation_max_lifetime_invalid".to_string());
                }
                (Some(_), Some(_)) => {
                    reasons.push(
                        "certificate_revocation_certificate_lifetime_exceeds_policy".to_string(),
                    );
                }
                (None, _) => {
                    reasons.push("certificate_revocation_max_lifetime_missing".to_string());
                }
                (_, None) => {
                    reasons.push("certificate_revocation_certificate_lifetime_missing".to_string());
                }
            }

            if let (Some(not_before), Some(not_after)) =
                (certificate_not_before_unix, certificate_not_after_unix)
            {
                if input.trusted_signing_time_unix >= not_before
                    && input.trusted_signing_time_unix <= not_after
                {
                    verified_evidence.push(
                        "certificate_revocation_trusted_signing_time_within_certificate_validity"
                            .to_string(),
                    );
                } else {
                    reasons.push(
                        "certificate_revocation_trusted_signing_time_outside_certificate_validity"
                            .to_string(),
                    );
                }
            } else {
                reasons.push("certificate_revocation_certificate_validity_missing".to_string());
            }
        }
        Some("explicit_status_required") => {
            revocation_strategy = Some("explicit_crl_or_ocsp_required".to_string());
            revocation_status = Some("not_checked_explicit_status_required".to_string());
            reasons.push("certificate_revocation_explicit_status_not_implemented".to_string());
        }
        Some(_) => reasons.push("certificate_revocation_policy_mode_unknown".to_string()),
        None => reasons.push("certificate_revocation_policy_mode_missing".to_string()),
    }

    HostAdapterCertificateRevocationPolicyVerification {
        status: if reasons.is_empty() {
            HostAdapterCertificateRevocationPolicyVerificationStatus::Passed
        } else {
            HostAdapterCertificateRevocationPolicyVerificationStatus::Failed
        },
        trust_policy_path,
        certificate_path,
        trusted_signing_time_unix: input.trusted_signing_time_unix,
        policy_mode,
        max_certificate_lifetime_seconds,
        certificate_not_before_unix,
        certificate_not_after_unix,
        certificate_lifetime_seconds,
        revocation_strategy,
        revocation_status,
        deferred_verification,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies a policy boundary for Sigstore-style short-lived certificate revocation strategy by checking declared revocation mode, certificate lifetime limit, and trusted signing time inside the certificate validity window. It does not fetch or verify CRLs, query OCSP, claim the certificate is not revoked, refresh TUF trusted roots, mutate installations, or decide release update authority.".to_string(),
    }
}

pub fn run_host_adapter_tuf_trusted_root_freshness_verification(
    input: HostAdapterTufTrustedRootFreshnessVerificationInput,
) -> HostAdapterTufTrustedRootFreshnessVerification {
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let root_metadata_path = input.root_metadata_path.to_string_lossy().to_string();
    let timestamp_metadata_path = input
        .timestamp_metadata_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let snapshot_metadata_path = input
        .snapshot_metadata_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let targets_metadata_path = input
        .targets_metadata_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let deferred_verification = vec![
        "tuf_metadata_signature_thresholds".to_string(),
        "tuf_root_key_rotation_chain".to_string(),
        "tuf_timestamp_snapshot_hash_binding".to_string(),
        "tuf_target_hash_binding".to_string(),
        "tuf_repository_download".to_string(),
        "release_install_update_authority".to_string(),
    ];
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut root_source = None;
    let mut trusted_root_ref = None;
    let mut verified_roles = Vec::new();

    let trust_policy = read_sigstore_trust_policy_document(
        &input.trust_policy_path,
        "tuf_freshness_trust_policy",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(document) = trust_policy.as_ref() {
        verify_sigstore_trust_policy(document, &mut verified_evidence, &mut reasons);
        let policy = &document.sigstore_trusted_root_policy;
        root_source = Some(policy.root_source.clone());
        trusted_root_ref = Some(policy.trusted_root_ref.clone());
        if policy.root_source == "tuf" {
            verified_evidence.push("tuf_freshness_root_source_tuf".to_string());
        } else {
            reasons.push("tuf_freshness_root_source_not_tuf".to_string());
        }
    }

    verify_tuf_metadata_freshness_role(
        "root",
        &input.root_metadata_path,
        input.min_root_version,
        input.update_start_time_unix,
        &mut verified_roles,
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(path) = input.timestamp_metadata_path.as_ref() {
        verify_tuf_metadata_freshness_role(
            "timestamp",
            path,
            input.min_timestamp_version,
            input.update_start_time_unix,
            &mut verified_roles,
            &mut verified_evidence,
            &mut reasons,
        );
    }
    if let Some(path) = input.snapshot_metadata_path.as_ref() {
        verify_tuf_metadata_freshness_role(
            "snapshot",
            path,
            input.min_snapshot_version,
            input.update_start_time_unix,
            &mut verified_roles,
            &mut verified_evidence,
            &mut reasons,
        );
    }
    if let Some(path) = input.targets_metadata_path.as_ref() {
        verify_tuf_metadata_freshness_role(
            "targets",
            path,
            input.min_targets_version,
            input.update_start_time_unix,
            &mut verified_roles,
            &mut verified_evidence,
            &mut reasons,
        );
    }

    HostAdapterTufTrustedRootFreshnessVerification {
        status: if reasons.is_empty() {
            HostAdapterTufTrustedRootFreshnessVerificationStatus::Passed
        } else {
            HostAdapterTufTrustedRootFreshnessVerificationStatus::Failed
        },
        trust_policy_path,
        root_metadata_path,
        timestamp_metadata_path,
        snapshot_metadata_path,
        targets_metadata_path,
        update_start_time_unix: input.update_start_time_unix,
        root_source,
        trusted_root_ref,
        verified_roles,
        deferred_verification,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies TUF trusted-root freshness inputs by checking supplied local metadata role type, version floors, and ISO 8601 UTC expiration timestamps against a fixed update start time. It does not verify TUF metadata signatures or thresholds, walk root key rotation, fetch repository metadata, bind timestamp/snapshot/target hashes, mutate installations, or decide release update authority.".to_string(),
    }
}

pub fn run_host_adapter_certificate_crl_status_verification(
    input: HostAdapterCertificateCrlStatusVerificationInput,
) -> HostAdapterCertificateCrlStatusVerification {
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let certificate_path = input.certificate_path.to_string_lossy().to_string();
    let issuer_certificate_path = input.issuer_certificate_path.to_string_lossy().to_string();
    let crl_path = input.crl_path.to_string_lossy().to_string();
    let deferred_verification = vec![
        "ocsp_response_status".to_string(),
        "crl_distribution_point_fetch".to_string(),
        "delta_crl_processing".to_string(),
        "indirect_crl_processing".to_string(),
        "tuf_trusted_root_freshness".to_string(),
        "release_install_update_authority".to_string(),
    ];
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut policy_mode = None;
    let mut certificate_serial_hex = None;
    let mut issuer_subject = None;
    let mut crl_issuer = None;
    let mut crl_this_update_unix = None;
    let mut crl_next_update_unix = None;
    let mut revocation_status = None;
    let mut revoked_at_unix = None;
    let mut revocation_reason = None;

    let trust_policy = read_sigstore_trust_policy_document(
        &input.trust_policy_path,
        "crl_status_trust_policy",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(document) = trust_policy.as_ref() {
        verify_sigstore_trust_policy(document, &mut verified_evidence, &mut reasons);
        let policy = &document.sigstore_trusted_root_policy;
        if let Some(revocation) = policy.revocation.as_ref() {
            policy_mode = Some(revocation.mode.clone());
            if revocation.mode == "explicit_status_required" {
                verified_evidence.push("crl_status_explicit_revocation_policy".to_string());
            } else {
                reasons.push("crl_status_policy_not_explicit_status_required".to_string());
            }
        } else {
            reasons.push("crl_status_revocation_policy_missing".to_string());
        }
        if path_matches_any_ref(
            &input.issuer_certificate_path,
            &policy.fulcio.certificate_authority_refs,
        ) {
            verified_evidence.push("crl_status_issuer_declared_ca_ref_matched".to_string());
        } else {
            reasons.push("crl_status_issuer_declared_ca_ref_missing".to_string());
        }
    }

    let certificate_der = read_certificate_der(
        &input.certificate_path,
        "crl_status_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    let issuer_der = read_certificate_der(
        &input.issuer_certificate_path,
        "crl_status_issuer_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    let crl_der = read_certificate_der(
        &input.crl_path,
        "crl_status_crl",
        &mut verified_evidence,
        &mut reasons,
    );

    if let (Some(certificate_der), Some(issuer_der), Some(crl_der)) = (
        certificate_der.as_ref(),
        issuer_der.as_ref(),
        crl_der.as_ref(),
    ) {
        let certificate = parse_certificate(
            certificate_der,
            "crl_status_certificate",
            &mut verified_evidence,
            &mut reasons,
        );
        let issuer = parse_certificate(
            issuer_der,
            "crl_status_issuer_certificate",
            &mut verified_evidence,
            &mut reasons,
        );
        let crl = match parse_x509_crl(crl_der) {
            Ok((_remaining, crl)) => {
                verified_evidence.push("crl_status_crl_parsed".to_string());
                Some(crl)
            }
            Err(err) => {
                reasons.push(format!("crl_status_crl_parse_failed:{err}"));
                None
            }
        };

        if let (Some(certificate), Some(issuer), Some(crl)) =
            (certificate.as_ref(), issuer.as_ref(), crl.as_ref())
        {
            certificate_serial_hex = Some(hex_bytes(certificate.tbs_certificate.raw_serial()));
            issuer_subject = Some(format!("{}", issuer.subject()));
            crl_issuer = Some(format!("{}", crl.issuer()));
            crl_this_update_unix = Some(crl.last_update().timestamp());
            crl_next_update_unix = crl.next_update().map(|time| time.timestamp());

            if certificate.issuer() == issuer.subject() {
                verified_evidence.push("crl_status_certificate_issuer_subject_match".to_string());
            } else {
                reasons.push("crl_status_certificate_issuer_subject_mismatch".to_string());
            }
            match certificate.verify_signature(Some(issuer.public_key())) {
                Ok(()) => {
                    verified_evidence.push("crl_status_certificate_signature_verified".to_string())
                }
                Err(err) => reasons.push(format!("crl_status_certificate_signature_failed:{err}")),
            }

            if crl.issuer() == issuer.subject() {
                verified_evidence.push("crl_status_crl_issuer_subject_match".to_string());
            } else {
                reasons.push("crl_status_crl_issuer_subject_mismatch".to_string());
            }
            match crl.verify_signature(issuer.public_key()) {
                Ok(()) => verified_evidence.push("crl_status_crl_signature_verified".to_string()),
                Err(err) => reasons.push(format!("crl_status_crl_signature_failed:{err}")),
            }

            let this_update = crl.last_update().timestamp();
            if input.verification_time_unix >= this_update {
                verified_evidence.push("crl_status_crl_this_update_not_in_future".to_string());
            } else {
                reasons.push("crl_status_crl_this_update_in_future".to_string());
            }
            if let Some(next_update) = crl.next_update().map(|time| time.timestamp()) {
                if input.verification_time_unix <= next_update {
                    verified_evidence.push("crl_status_crl_next_update_not_expired".to_string());
                } else {
                    reasons.push("crl_status_crl_expired".to_string());
                }
            } else {
                reasons.push("crl_status_crl_next_update_missing".to_string());
            }

            if let Some(revoked) = crl
                .iter_revoked_certificates()
                .find(|revoked| revoked.serial() == &certificate.tbs_certificate.serial)
            {
                revocation_status = Some("revoked_by_supplied_crl".to_string());
                revoked_at_unix = Some(revoked.revocation_date.timestamp());
                revocation_reason = revoked
                    .reason_code()
                    .map(|(_critical, reason)| format!("{reason}"));
                reasons.push("crl_status_certificate_revoked".to_string());
            } else if reasons.is_empty() {
                revocation_status = Some("good_by_supplied_crl".to_string());
                verified_evidence.push("crl_status_certificate_serial_absent_from_crl".to_string());
            } else {
                revocation_status = Some("unknown_due_to_failed_crl_verification".to_string());
            }
        }
    }

    HostAdapterCertificateCrlStatusVerification {
        status: if reasons.is_empty() {
            HostAdapterCertificateCrlStatusVerificationStatus::Passed
        } else {
            HostAdapterCertificateCrlStatusVerificationStatus::Failed
        },
        trust_policy_path,
        certificate_path,
        issuer_certificate_path,
        crl_path,
        verification_time_unix: input.verification_time_unix,
        policy_mode,
        certificate_serial_hex,
        issuer_subject,
        crl_issuer,
        crl_this_update_unix,
        crl_next_update_unix,
        revocation_status,
        revoked_at_unix,
        revocation_reason,
        deferred_verification,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies explicit local CRL revocation status by checking supplied certificate and issuer binding, CRL issuer binding, CRL signature, CRL freshness window, and whether the certificate serial is present in the CRL. It does not fetch CRL distribution points, process delta or indirect CRLs, verify OCSP responses, refresh TUF trusted roots, mutate installations, or decide release update authority.".to_string(),
    }
}

pub fn run_host_adapter_certificate_ocsp_status_verification(
    input: HostAdapterCertificateOcspStatusVerificationInput,
) -> HostAdapterCertificateOcspStatusVerification {
    let trust_policy_path = input.trust_policy_path.to_string_lossy().to_string();
    let certificate_path = input.certificate_path.to_string_lossy().to_string();
    let issuer_certificate_path = input.issuer_certificate_path.to_string_lossy().to_string();
    let ocsp_response_path = input.ocsp_response_path.to_string_lossy().to_string();
    let deferred_verification = vec![
        "network_ocsp_fetch".to_string(),
        "crl_status".to_string(),
        "tuf_freshness".to_string(),
        "install_update_authority".to_string(),
    ];
    let mut reasons = Vec::new();
    let mut verified_evidence = Vec::new();
    let mut policy_mode = None;
    let mut certificate_serial_hex = None;
    let mut issuer_subject = None;
    let mut ocsp_response_status = None;
    let mut responder_authority = None;
    let mut ocsp_produced_at_unix = None;
    let mut ocsp_this_update_unix = None;
    let mut ocsp_next_update_unix = None;
    let mut revocation_status = None;
    let mut revoked_at_unix = None;
    let mut revocation_reason = None;
    let expected_nonce_hex = input
        .expected_nonce_hex
        .as_deref()
        .and_then(|value| normalize_expected_ocsp_nonce_hex(value, &mut reasons));
    let mut observed_nonce_hex = None;

    let trust_policy = read_sigstore_trust_policy_document(
        &input.trust_policy_path,
        "ocsp_status_trust_policy",
        &mut verified_evidence,
        &mut reasons,
    );
    if let Some(document) = trust_policy.as_ref() {
        verify_sigstore_trust_policy(document, &mut verified_evidence, &mut reasons);
        let policy = &document.sigstore_trusted_root_policy;
        if let Some(revocation) = policy.revocation.as_ref() {
            policy_mode = Some(revocation.mode.clone());
            if revocation.mode == "explicit_status_required" {
                verified_evidence.push("ocsp_status_explicit_revocation_policy".to_string());
            } else {
                reasons.push("ocsp_status_policy_not_explicit_status_required".to_string());
            }
        } else {
            reasons.push("ocsp_status_revocation_policy_missing".to_string());
        }
        if path_matches_any_ref(
            &input.issuer_certificate_path,
            &policy.fulcio.certificate_authority_refs,
        ) {
            verified_evidence.push("ocsp_status_issuer_declared_ca_ref_matched".to_string());
        } else {
            reasons.push("ocsp_status_issuer_declared_ca_ref_missing".to_string());
        }
    }

    let certificate_der = read_certificate_der(
        &input.certificate_path,
        "ocsp_status_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    let issuer_der = read_certificate_der(
        &input.issuer_certificate_path,
        "ocsp_status_issuer_certificate",
        &mut verified_evidence,
        &mut reasons,
    );
    let ocsp_der = read_certificate_der(
        &input.ocsp_response_path,
        "ocsp_status_response",
        &mut verified_evidence,
        &mut reasons,
    );

    if let (Some(certificate_der), Some(issuer_der), Some(ocsp_der)) = (
        certificate_der.as_ref(),
        issuer_der.as_ref(),
        ocsp_der.as_ref(),
    ) {
        let certificate = parse_certificate(
            certificate_der,
            "ocsp_status_certificate",
            &mut verified_evidence,
            &mut reasons,
        );
        let issuer = parse_certificate(
            issuer_der,
            "ocsp_status_issuer_certificate",
            &mut verified_evidence,
            &mut reasons,
        );
        let ocsp_response = decode_ocsp_response(ocsp_der, &mut verified_evidence, &mut reasons);

        if let (Some(certificate), Some(issuer)) = (certificate.as_ref(), issuer.as_ref()) {
            certificate_serial_hex = Some(hex_bytes(certificate.tbs_certificate.raw_serial()));
            issuer_subject = Some(format!("{}", issuer.subject()));

            if certificate.issuer() == issuer.subject() {
                verified_evidence.push("ocsp_status_certificate_issuer_subject_match".to_string());
            } else {
                reasons.push("ocsp_status_certificate_issuer_subject_mismatch".to_string());
            }
            match certificate.verify_signature(Some(issuer.public_key())) {
                Ok(()) => {
                    verified_evidence.push("ocsp_status_certificate_signature_verified".to_string())
                }
                Err(err) => reasons.push(format!("ocsp_status_certificate_signature_failed:{err}")),
            }
        }

        if let Some(ocsp_response) = ocsp_response.as_ref() {
            ocsp_response_status = Some(format!("{:?}", ocsp_response.status));
            match ocsp_response.status {
                OcspResponseStatus::Successful => {
                    verified_evidence.push("ocsp_status_response_successful".to_string());
                }
                status => reasons.push(format!("ocsp_status_response_not_successful:{status:?}")),
            }

            if let Some(response_bytes) = ocsp_response.bytes.as_ref() {
                if rasn_oid_matches(&response_bytes.r#type, &[1, 3, 6, 1, 5, 5, 7, 48, 1, 1]) {
                    verified_evidence.push("ocsp_status_basic_response_type".to_string());
                } else {
                    reasons.push(format!(
                        "ocsp_status_response_type_unsupported:{}",
                        response_bytes.r#type
                    ));
                }

                if let Some(basic_response) = decode_basic_ocsp_response(
                    response_bytes.response.as_ref(),
                    &mut verified_evidence,
                    &mut reasons,
                ) {
                    ocsp_produced_at_unix =
                        Some(basic_response.tbs_response_data.produced_at.timestamp());
                    if input.verification_time_unix
                        >= basic_response.tbs_response_data.produced_at.timestamp()
                    {
                        verified_evidence.push("ocsp_status_produced_at_not_in_future".to_string());
                    } else {
                        reasons.push("ocsp_status_produced_at_in_future".to_string());
                    }
                    observed_nonce_hex = extract_ocsp_response_nonce_hex(
                        &basic_response,
                        &mut verified_evidence,
                        &mut reasons,
                    );
                    verify_ocsp_nonce(
                        expected_nonce_hex.as_deref(),
                        observed_nonce_hex.as_deref(),
                        &mut verified_evidence,
                        &mut reasons,
                    );

                    if let Some(issuer) = issuer.as_ref() {
                        if ocsp_responder_id_matches_issuer(
                            &basic_response.tbs_response_data.responder_id,
                            issuer,
                            &mut verified_evidence,
                            &mut reasons,
                        ) {
                            responder_authority = Some("issuer_certificate_direct".to_string());
                        } else {
                            reasons.push("ocsp_status_responder_unauthorized".to_string());
                        }

                        let signature_verified = verify_basic_ocsp_signature_with_issuer(
                            &basic_response,
                            issuer,
                            &mut verified_evidence,
                            &mut reasons,
                        );
                        if !signature_verified
                            && basic_response
                                .certs
                                .as_ref()
                                .is_some_and(|certs| !certs.is_empty())
                        {
                            reasons.push(
                                "ocsp_status_delegated_responder_certificate_unsupported"
                                    .to_string(),
                            );
                        }
                    }

                    if let (Some(certificate), Some(issuer)) =
                        (certificate.as_ref(), issuer.as_ref())
                    {
                        if let Some(single_response) = find_matching_ocsp_single_response(
                            &basic_response,
                            certificate,
                            issuer,
                            &mut verified_evidence,
                            &mut reasons,
                        ) {
                            ocsp_this_update_unix = Some(single_response.this_update.timestamp());
                            ocsp_next_update_unix = single_response
                                .next_update
                                .as_ref()
                                .map(|time| time.timestamp());
                            verify_ocsp_single_response_freshness(
                                single_response,
                                input.verification_time_unix,
                                &mut verified_evidence,
                                &mut reasons,
                            );
                            let verification_had_reasons_before_status = !reasons.is_empty();
                            apply_ocsp_cert_status(
                                &single_response.cert_status,
                                &mut revocation_status,
                                &mut revoked_at_unix,
                                &mut revocation_reason,
                                &mut verified_evidence,
                                &mut reasons,
                            );
                            if verification_had_reasons_before_status
                                && matches!(
                                    revocation_status.as_deref(),
                                    Some("revoked_by_supplied_ocsp")
                                        | Some("unknown_by_supplied_ocsp")
                                )
                            {
                                revocation_status =
                                    Some("unknown_due_to_failed_ocsp_verification".to_string());
                                revoked_at_unix = None;
                                revocation_reason = None;
                            }
                        } else {
                            revocation_status =
                                Some("unknown_due_to_failed_ocsp_verification".to_string());
                        }
                    }
                }
            } else {
                reasons.push("ocsp_status_response_bytes_missing".to_string());
            }
        }
    }

    if matches!(
        revocation_status.as_deref(),
        Some("good_by_supplied_ocsp") | None
    ) && !reasons.is_empty()
    {
        revocation_status = Some("unknown_due_to_failed_ocsp_verification".to_string());
    }

    HostAdapterCertificateOcspStatusVerification {
        status: if reasons.is_empty() {
            HostAdapterCertificateOcspStatusVerificationStatus::Passed
        } else {
            HostAdapterCertificateOcspStatusVerificationStatus::Failed
        },
        trust_policy_path,
        certificate_path,
        issuer_certificate_path,
        ocsp_response_path,
        verification_time_unix: input.verification_time_unix,
        expected_nonce_hex,
        observed_nonce_hex,
        policy_mode,
        certificate_serial_hex,
        issuer_subject,
        ocsp_response_status,
        responder_authority,
        ocsp_produced_at_unix,
        ocsp_this_update_unix,
        ocsp_next_update_unix,
        revocation_status,
        revoked_at_unix,
        revocation_reason,
        deferred_verification,
        reasons,
        verified_evidence,
        inference_boundary: "Verifies explicit offline OCSP revocation status from a supplied RFC6960 DER OCSP response by checking successful OCSPResponse, BasicOCSPResponse, direct issuer responder authority, OCSP signature, CertID serial and issuer hashes, thisUpdate/nextUpdate freshness, and optional nonce equality. It does not fetch OCSP over the network, infer OCSP from CRL, CT, Rekor, TUF, or short-lived policy, refresh TUF trusted roots, mutate installations, or decide release update authority.".to_string(),
    }
}

fn verify_tuf_metadata_freshness_role(
    expected_role: &str,
    metadata_path: &Path,
    min_version: Option<i64>,
    update_start_time_unix: i64,
    verified_roles: &mut Vec<HostAdapterTufMetadataFreshnessRole>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let metadata_path_string = metadata_path.to_string_lossy().to_string();
    let Some(bytes) = read_required_file(metadata_path, "tuf_metadata", reasons) else {
        verified_roles.push(HostAdapterTufMetadataFreshnessRole {
            role: expected_role.to_string(),
            metadata_path: metadata_path_string,
            version: None,
            min_version,
            expires: None,
            expires_unix: None,
        });
        return;
    };
    verified_evidence.push(format!("tuf_{expected_role}_metadata_loaded"));

    let value = match serde_json::from_slice::<Value>(&bytes) {
        Ok(value) => value,
        Err(err) => {
            reasons.push(format!("tuf_{expected_role}_metadata_json_invalid:{err}"));
            verified_roles.push(HostAdapterTufMetadataFreshnessRole {
                role: expected_role.to_string(),
                metadata_path: metadata_path_string,
                version: None,
                min_version,
                expires: None,
                expires_unix: None,
            });
            return;
        }
    };

    let signed = value.get("signed").and_then(Value::as_object);
    let observed_role = signed
        .and_then(|signed| signed.get("_type"))
        .and_then(Value::as_str);
    let version = signed
        .and_then(|signed| signed.get("version"))
        .and_then(Value::as_i64);
    let expires = signed
        .and_then(|signed| signed.get("expires"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let expires_unix = expires
        .as_deref()
        .and_then(|value| parse_tuf_datetime_utc_to_unix(value, expected_role, reasons));

    if observed_role == Some(expected_role) {
        verified_evidence.push(format!("tuf_{expected_role}_role_type_matches"));
    } else {
        reasons.push(format!("tuf_{expected_role}_role_type_mismatch"));
    }

    match (version, min_version) {
        (Some(observed), Some(minimum)) if observed >= minimum => {
            verified_evidence.push(format!("tuf_{expected_role}_version_at_or_above_floor"));
        }
        (Some(_), Some(_)) => reasons.push(format!("tuf_{expected_role}_version_below_floor")),
        (Some(_), None) => verified_evidence.push(format!("tuf_{expected_role}_version_present")),
        (None, _) => reasons.push(format!("tuf_{expected_role}_version_missing")),
    }

    if let Some(expires_unix) = expires_unix {
        if expires_unix > update_start_time_unix {
            verified_evidence.push(format!("tuf_{expected_role}_expires_after_update_start"));
        } else {
            reasons.push(format!("tuf_{expected_role}_metadata_expired"));
        }
    } else if expires.is_none() {
        reasons.push(format!("tuf_{expected_role}_expires_missing"));
    }

    verified_roles.push(HostAdapterTufMetadataFreshnessRole {
        role: expected_role.to_string(),
        metadata_path: metadata_path_string,
        version,
        min_version,
        expires,
        expires_unix,
    });
}

fn parse_tuf_datetime_utc_to_unix(
    value: &str,
    role: &str,
    reasons: &mut Vec<String>,
) -> Option<i64> {
    if value.len() != 20 || !value.ends_with('Z') {
        reasons.push(format!("tuf_{role}_expires_format_invalid"));
        return None;
    }
    if value.as_bytes().get(4) != Some(&b'-')
        || value.as_bytes().get(7) != Some(&b'-')
        || value.as_bytes().get(10) != Some(&b'T')
        || value.as_bytes().get(13) != Some(&b':')
        || value.as_bytes().get(16) != Some(&b':')
    {
        reasons.push(format!("tuf_{role}_expires_format_invalid"));
        return None;
    }
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        parse_fixed_i32(value, 0, 4),
        parse_fixed_i32(value, 5, 7),
        parse_fixed_i32(value, 8, 10),
        parse_fixed_i32(value, 11, 13),
        parse_fixed_i32(value, 14, 16),
        parse_fixed_i32(value, 17, 19),
    ) else {
        reasons.push(format!("tuf_{role}_expires_format_invalid"));
        return None;
    };
    if !(1..=12).contains(&month)
        || !(1..=days_in_month(year, month)).contains(&day)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        reasons.push(format!("tuf_{role}_expires_format_invalid"));
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + i64::from(hour * 3_600 + minute * 60 + second))
}

fn parse_fixed_i32(value: &str, start: usize, end: usize) -> Option<i32> {
    value.get(start..end)?.parse::<i32>().ok()
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era - 719_468)
}

fn select_rekor_integrated_time_for_timestamp_authority(
    input: &HostAdapterSigstoreTimestampAuthorityVerificationInput,
    selected_timestamp_source: &mut Option<String>,
    observed_timestamp_unix: &mut Option<i64>,
    rekor_status: &mut Option<HostAdapterRekorVerificationStatus>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let (Some(rekor_log_entry_path), Some(rekor_public_key_path), Some(expected_rekor_log_id)) = (
        input.rekor_log_entry_path.as_ref(),
        input.rekor_public_key_path.as_ref(),
        input.expected_rekor_log_id.as_ref(),
    ) else {
        reasons.push("timestamp_rekor_evidence_missing".to_string());
        return;
    };

    let rekor = run_host_adapter_rekor_verification(HostAdapterRekorVerificationInput {
        log_entry_path: rekor_log_entry_path.clone(),
        public_key_path: rekor_public_key_path.clone(),
        expected_log_id: expected_rekor_log_id.clone(),
    });
    *rekor_status = Some(rekor.status);
    if rekor.status == HostAdapterRekorVerificationStatus::Passed {
        let text = match fs::read_to_string(rekor_log_entry_path) {
            Ok(text) => text,
            Err(err) => {
                reasons.push(format!(
                    "timestamp_rekor_log_entry_read_failed:{:?}",
                    err.kind()
                ));
                return;
            }
        };
        match crypto_rekor::parse_rekor_log_entry(&text) {
            Ok(entry) => {
                *selected_timestamp_source = Some("rekor_integrated_time".to_string());
                *observed_timestamp_unix = Some(entry.integrated_time);
                verified_evidence.push("timestamp_rekor_integrated_time_verified".to_string());
            }
            Err(reason) => reasons.push(format!("timestamp_rekor_log_entry_parse_failed:{reason}")),
        }
    } else {
        reasons.extend(
            rekor
                .reasons
                .into_iter()
                .map(|reason| format!("rekor:{reason}")),
        );
    }
}

fn select_rfc3161_tsa_for_timestamp_authority(
    input: &HostAdapterSigstoreTimestampAuthorityVerificationInput,
    trust_policy: Option<&SigstoreTrustedRootPolicyDocument>,
    selected_timestamp_source: &mut Option<String>,
    observed_timestamp_unix: &mut Option<i64>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let Some(document) = trust_policy else {
        reasons.push("timestamp_rfc3161_trust_policy_missing".to_string());
        return;
    };
    let (Some(token_path), Some(signature_path)) = (
        input.rfc3161_timestamp_token_path.as_ref(),
        input.rfc3161_timestamped_signature_path.as_ref(),
    ) else {
        if input.rfc3161_timestamp_token_path.is_none() {
            reasons.push("timestamp_rfc3161_token_path_missing".to_string());
        }
        if input.rfc3161_timestamped_signature_path.is_none() {
            reasons.push("timestamp_rfc3161_signature_path_missing".to_string());
        }
        return;
    };

    let token_bytes = read_required_file(token_path, "timestamp_rfc3161_token", reasons);
    if token_bytes.is_some() {
        verified_evidence.push("timestamp_rfc3161_token_loaded".to_string());
    }
    let signature_bytes =
        read_required_file(signature_path, "timestamp_rfc3161_signature", reasons);
    if signature_bytes.is_some() {
        verified_evidence.push("timestamp_rfc3161_signature_loaded".to_string());
    }

    let tsa_refs = &document
        .sigstore_trusted_root_policy
        .timestamp_authority
        .certificate_refs;
    if tsa_refs.is_empty() {
        reasons.push("timestamp_rfc3161_tsa_certificate_refs_missing".to_string());
        return;
    }

    let mut tsa_certificates = Vec::new();
    for cert_ref in tsa_refs {
        let cert_path = resolve_policy_relative_path(&input.trust_policy_path, cert_ref);
        if let Some(certificate_der) = read_certificate_der(
            &cert_path,
            "timestamp_rfc3161_tsa_certificate",
            verified_evidence,
            reasons,
        ) {
            tsa_certificates.push(CertificateDer::from(certificate_der));
        }
    }
    if tsa_certificates.len() != tsa_refs.len() {
        reasons.push("timestamp_rfc3161_tsa_certificate_load_failed".to_string());
        return;
    }
    if tsa_certificates.is_empty() {
        reasons.push("timestamp_rfc3161_tsa_certificate_refs_missing".to_string());
        return;
    }
    verified_evidence.push("timestamp_rfc3161_tsa_certificate_refs_loaded".to_string());

    let (Some(token_bytes), Some(signature_bytes)) =
        (token_bytes.as_deref(), signature_bytes.as_deref())
    else {
        return;
    };

    let root = tsa_certificates
        .last()
        .expect("tsa certificates nonempty")
        .clone();
    let intermediates = if tsa_certificates.len() > 1 {
        tsa_certificates[..tsa_certificates.len() - 1].to_vec()
    } else {
        Vec::new()
    };
    let opts = sigstore_tsa::VerifyOpts::new()
        .with_root(root)
        .with_intermediates(intermediates)
        .with_tsa_certificates(tsa_certificates);

    match sigstore_tsa::verify_timestamp_response(token_bytes, signature_bytes, opts) {
        Ok(result) => {
            *selected_timestamp_source = Some("rfc3161_tsa".to_string());
            *observed_timestamp_unix = Some(result.time.as_second());
            verified_evidence.push("timestamp_rfc3161_token_verified".to_string());
            verified_evidence.push("timestamp_rfc3161_message_imprint_verified".to_string());
            verified_evidence.push("timestamp_rfc3161_cms_signature_verified".to_string());
            verified_evidence.push("timestamp_rfc3161_tsa_chain_verified".to_string());
        }
        Err(err) => reasons.push(format!("timestamp_rfc3161_verification_failed:{err}")),
    }
}

fn resolve_policy_relative_path(policy_path: &Path, path_ref: &str) -> PathBuf {
    let path = PathBuf::from(path_ref);
    if path.is_absolute() {
        path
    } else {
        policy_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SigstoreTrustedRootPolicyDocument {
    schema_version: String,
    sigstore_trusted_root_policy: SigstoreTrustedRootPolicy,
}

#[derive(Debug, Clone, Deserialize)]
struct SigstoreTrustedRootPolicy {
    root_source: String,
    trusted_root_ref: String,
    offline_allowed: bool,
    fulcio: SigstoreFulcioTrustPolicy,
    rekor: SigstoreRekorTrustPolicy,
    certificate_transparency: SigstoreCertificateTransparencyTrustPolicy,
    timestamp_authority: SigstoreTimestampAuthorityPolicy,
    #[serde(default)]
    revocation: Option<SigstoreRevocationPolicy>,
    identity_policy: SigstoreIdentityPolicy,
}

#[derive(Debug, Clone, Deserialize)]
struct SigstoreFulcioTrustPolicy {
    required: bool,
    #[serde(default)]
    certificate_authority_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SigstoreRekorTrustPolicy {
    required: bool,
    #[serde(default)]
    log_ids: Vec<String>,
    #[serde(default)]
    public_key_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SigstoreCertificateTransparencyTrustPolicy {
    required: bool,
    #[serde(default)]
    log_ids: Vec<String>,
    #[serde(default)]
    public_key_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SigstoreTimestampAuthorityPolicy {
    mode: String,
    #[serde(default)]
    certificate_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SigstoreRevocationPolicy {
    mode: String,
    #[serde(default)]
    max_certificate_lifetime_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct SigstoreIdentityPolicy {
    expected_oidc_issuer: String,
    #[serde(default)]
    expected_certificate_identity: Option<String>,
    #[serde(default)]
    expected_github_repository: Option<String>,
    #[serde(default)]
    expected_github_ref: Option<String>,
    #[serde(default)]
    expected_github_sha: Option<String>,
}

struct CertificateTransparencyLogMaterial {
    id: [u8; 32],
    id_hex: String,
    key: Vec<u8>,
}

fn load_certificate_transparency_log_material(
    policy_path: &Path,
    document: &SigstoreTrustedRootPolicyDocument,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Vec<CertificateTransparencyLogMaterial> {
    let policy = &document
        .sigstore_trusted_root_policy
        .certificate_transparency;
    if !policy.required {
        verified_evidence.push("ct_sct_not_required_by_policy".to_string());
    }
    if policy.log_ids.len() != policy.public_key_refs.len() {
        reasons.push("ct_sct_log_id_public_key_ref_count_mismatch".to_string());
        return Vec::new();
    }
    if policy.log_ids.is_empty() {
        reasons.push("ct_sct_log_ids_missing".to_string());
        return Vec::new();
    }

    policy
        .log_ids
        .iter()
        .zip(policy.public_key_refs.iter())
        .filter_map(|(log_id, public_key_ref)| {
            let id = decode_ct_log_id(log_id, reasons)?;
            let public_key_path = resolve_policy_relative_path(policy_path, public_key_ref);
            let key = read_required_file(&public_key_path, "ct_sct_log_public_key", reasons)?;
            if key.is_empty() {
                reasons.push(format!("ct_sct_log_public_key_empty:{public_key_ref}"));
                return None;
            }
            verified_evidence.push("ct_sct_log_public_key_loaded".to_string());
            Some(CertificateTransparencyLogMaterial {
                id,
                id_hex: hex_bytes(&id),
                key,
            })
        })
        .collect()
}

fn verify_sigstore_trust_policy(
    document: &SigstoreTrustedRootPolicyDocument,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    if document.schema_version == "0.1" {
        verified_evidence.push("sigstore_trust_policy_schema_version_supported".to_string());
    } else {
        reasons.push("sigstore_trust_policy_schema_version_unsupported".to_string());
    }

    let policy = &document.sigstore_trusted_root_policy;
    match policy.root_source.as_str() {
        "tuf" | "pinned" | "manual" => {
            verified_evidence.push("sigstore_trust_root_source_supported".to_string());
        }
        _ => reasons.push("sigstore_trust_root_source_unknown".to_string()),
    }

    if non_empty_string(&policy.trusted_root_ref) {
        verified_evidence.push("sigstore_trusted_root_ref_present".to_string());
    } else {
        reasons.push("sigstore_trusted_root_ref_missing".to_string());
    }

    if policy.fulcio.required && non_empty_items(&policy.fulcio.certificate_authority_refs) {
        verified_evidence.push("sigstore_fulcio_ca_refs_present".to_string());
    } else if policy.fulcio.required {
        reasons.push("sigstore_fulcio_ca_refs_missing".to_string());
    } else {
        reasons.push("sigstore_fulcio_required_false".to_string());
    }

    if policy.rekor.required {
        if non_empty_items(&policy.rekor.log_ids) && non_empty_items(&policy.rekor.public_key_refs)
        {
            verified_evidence.push("sigstore_rekor_trust_material_present".to_string());
        } else {
            reasons.push("sigstore_rekor_trust_material_missing".to_string());
        }
    } else {
        verified_evidence.push("sigstore_rekor_not_required_by_policy".to_string());
    }

    if policy.certificate_transparency.required {
        if non_empty_items(&policy.certificate_transparency.log_ids)
            && non_empty_items(&policy.certificate_transparency.public_key_refs)
        {
            verified_evidence.push("sigstore_ct_trust_material_present".to_string());
        } else {
            reasons.push("sigstore_ct_trust_material_missing".to_string());
        }
    } else {
        verified_evidence.push("sigstore_ct_not_required_by_policy".to_string());
    }

    verify_sigstore_timestamp_policy(policy, verified_evidence, reasons);
    verify_sigstore_identity_policy(&policy.identity_policy, verified_evidence, reasons);

    if policy.offline_allowed && policy.root_source == "tuf" {
        verified_evidence.push("sigstore_tuf_offline_policy_declared".to_string());
    }
}

fn verify_sigstore_timestamp_policy(
    policy: &SigstoreTrustedRootPolicy,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    match policy.timestamp_authority.mode.as_str() {
        "rekor_integrated_time" => {
            if policy.rekor.required
                && non_empty_items(&policy.rekor.log_ids)
                && non_empty_items(&policy.rekor.public_key_refs)
            {
                verified_evidence
                    .push("sigstore_timestamp_policy_rekor_integrated_time_ready".to_string());
            } else {
                reasons.push("sigstore_timestamp_policy_requires_rekor_material".to_string());
            }
        }
        "rfc3161_tsa" => {
            if non_empty_items(&policy.timestamp_authority.certificate_refs) {
                verified_evidence.push("sigstore_timestamp_policy_tsa_ready".to_string());
            } else {
                reasons.push("sigstore_timestamp_policy_requires_tsa_certs".to_string());
            }
        }
        "either" => {
            let rekor_ready = policy.rekor.required
                && non_empty_items(&policy.rekor.log_ids)
                && non_empty_items(&policy.rekor.public_key_refs);
            let tsa_ready = non_empty_items(&policy.timestamp_authority.certificate_refs);
            if rekor_ready || tsa_ready {
                verified_evidence.push("sigstore_timestamp_policy_has_source".to_string());
            } else {
                reasons.push("sigstore_timestamp_policy_missing_source".to_string());
            }
        }
        _ => reasons.push("sigstore_timestamp_policy_mode_unknown".to_string()),
    }
}

fn verify_sigstore_identity_policy(
    policy: &SigstoreIdentityPolicy,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    if non_empty_string(&policy.expected_oidc_issuer) {
        verified_evidence.push("sigstore_identity_oidc_issuer_present".to_string());
    } else {
        reasons.push("sigstore_identity_oidc_issuer_missing".to_string());
    }

    let has_identity_selector = optional_non_empty(&policy.expected_certificate_identity)
        || optional_non_empty(&policy.expected_github_repository)
        || optional_non_empty(&policy.expected_github_ref)
        || optional_non_empty(&policy.expected_github_sha);
    if has_identity_selector {
        verified_evidence.push("sigstore_identity_selector_present".to_string());
    } else {
        reasons.push("sigstore_identity_selector_missing".to_string());
    }

    if let Some(github_sha) = policy.expected_github_sha.as_deref() {
        if is_git_sha(github_sha) {
            verified_evidence.push("sigstore_identity_github_sha_immutable".to_string());
        } else {
            reasons.push("sigstore_identity_github_sha_invalid".to_string());
        }
    }
}

fn non_empty_string(value: &str) -> bool {
    !value.trim().is_empty()
}

fn optional_non_empty(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

fn non_empty_items(values: &[String]) -> bool {
    !values.is_empty() && values.iter().all(|value| !value.trim().is_empty())
}

fn is_git_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn read_sigstore_trust_policy_document(
    policy_path: &Path,
    evidence_prefix: &str,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<SigstoreTrustedRootPolicyDocument> {
    let policy_text = match fs::read_to_string(policy_path) {
        Ok(value) => value,
        Err(err) => {
            reasons.push(format!("{evidence_prefix}_read_failed:{:?}", err.kind()));
            return None;
        }
    };

    match serde_yaml::from_str::<SigstoreTrustedRootPolicyDocument>(&policy_text) {
        Ok(value) => {
            verified_evidence.push(format!("{evidence_prefix}_parsed"));
            Some(value)
        }
        Err(err) => {
            reasons.push(format!("{evidence_prefix}_parse_failed:{err}"));
            None
        }
    }
}

fn read_certificate_der(
    path: &Path,
    evidence_prefix: &str,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<Vec<u8>> {
    let bytes = match fs::read(path) {
        Ok(value) => value,
        Err(err) => {
            reasons.push(format!("{evidence_prefix}_read_failed:{:?}", err.kind()));
            return None;
        }
    };

    if bytes.starts_with(b"-----BEGIN") {
        match parse_x509_pem(&bytes) {
            Ok((_remaining, pem)) => {
                verified_evidence.push(format!("{evidence_prefix}_pem_decoded"));
                Some(pem.contents)
            }
            Err(err) => {
                reasons.push(format!("{evidence_prefix}_pem_decode_failed:{err}"));
                None
            }
        }
    } else {
        verified_evidence.push(format!("{evidence_prefix}_der_loaded"));
        Some(bytes)
    }
}

fn parse_certificate<'a>(
    der: &'a [u8],
    evidence_prefix: &str,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<X509Certificate<'a>> {
    match parse_x509_certificate(der) {
        Ok((_remaining, certificate)) => {
            verified_evidence.push(format!("{evidence_prefix}_parsed"));
            Some(certificate)
        }
        Err(err) => {
            reasons.push(format!("{evidence_prefix}_parse_failed:{err}"));
            None
        }
    }
}

fn verify_fulcio_chain(
    leaf: &X509Certificate<'_>,
    issuers: &[X509Certificate<'_>],
    issuer_paths: &[PathBuf],
    document: &SigstoreTrustedRootPolicyDocument,
    verification_time_unix: i64,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    if issuer_paths.iter().any(|path| {
        path_matches_any_ref(
            path,
            &document
                .sigstore_trusted_root_policy
                .fulcio
                .certificate_authority_refs,
        )
    }) {
        verified_evidence.push("fulcio_chain_declared_ca_ref_matched".to_string());
    } else {
        reasons.push("fulcio_chain_declared_ca_ref_missing".to_string());
    }

    let mut child = leaf;
    for (index, issuer) in issuers.iter().enumerate() {
        if child.issuer() == issuer.subject() {
            verified_evidence.push(format!("fulcio_chain_issuer_subject_match_{index}"));
        } else {
            reasons.push(format!("fulcio_chain_issuer_subject_mismatch_{index}"));
        }

        match child.verify_signature(Some(issuer.public_key())) {
            Ok(()) => {
                verified_evidence.push(format!("fulcio_chain_signature_verified_{index}"));
            }
            Err(err) => {
                reasons.push(format!("fulcio_chain_signature_failed_{index}:{err}"));
            }
        }

        verify_issuer_ca_usage(issuer, index, verified_evidence, reasons);
        child = issuer;
    }

    if let Some(root) = issuers.last() {
        if root.issuer() == root.subject() {
            verified_evidence.push("fulcio_chain_root_subject_self_issued".to_string());
        } else {
            reasons.push("fulcio_chain_root_not_self_issued".to_string());
        }

        match root.verify_signature(None) {
            Ok(()) => verified_evidence.push("fulcio_chain_root_signature_verified".to_string()),
            Err(err) => reasons.push(format!("fulcio_chain_root_signature_failed:{err}")),
        }
    }

    let validity = leaf.validity();
    if verification_time_unix >= validity.not_before.timestamp()
        && verification_time_unix <= validity.not_after.timestamp()
    {
        verified_evidence.push("fulcio_leaf_valid_at_verification_time".to_string());
    } else {
        reasons.push("fulcio_leaf_not_valid_at_verification_time".to_string());
    }

    verify_leaf_code_signing_usage(leaf, verified_evidence, reasons);
}

fn verify_issuer_ca_usage(
    issuer: &X509Certificate<'_>,
    index: usize,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let mut saw_basic_constraints = false;
    for extension in issuer.extensions() {
        if let ParsedExtension::BasicConstraints(basic_constraints) = extension.parsed_extension() {
            saw_basic_constraints = true;
            if basic_constraints.ca {
                verified_evidence.push(format!("fulcio_chain_issuer_ca_basic_constraints_{index}"));
            } else {
                reasons.push(format!("fulcio_chain_issuer_not_ca_{index}"));
            }
        }
    }
    if !saw_basic_constraints {
        reasons.push(format!(
            "fulcio_chain_issuer_basic_constraints_missing_{index}"
        ));
    }
}

fn verify_leaf_code_signing_usage(
    leaf: &X509Certificate<'_>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let mut saw_eku = false;
    for extension in leaf.extensions() {
        if let ParsedExtension::ExtendedKeyUsage(extended_key_usage) = extension.parsed_extension()
        {
            saw_eku = true;
            if extended_key_usage.code_signing || extended_key_usage.any {
                verified_evidence.push("fulcio_leaf_code_signing_usage_allowed".to_string());
            } else {
                reasons.push("fulcio_leaf_code_signing_usage_missing".to_string());
            }
        }
    }
    if !saw_eku {
        verified_evidence.push("fulcio_leaf_extended_key_usage_absent".to_string());
    }
}

fn path_matches_any_ref(path: &Path, refs: &[String]) -> bool {
    let path_text = normalize_ref_path(&path.to_string_lossy());
    let file_name = path
        .file_name()
        .map(|name| normalize_ref_path(&name.to_string_lossy()));
    refs.iter().any(|reference| {
        let reference = normalize_ref_path(reference);
        !reference.is_empty()
            && (path_text == reference
                || path_text.ends_with(&format!("/{reference}"))
                || file_name.as_ref().is_some_and(|name| name == &reference))
    })
}

fn normalize_ref_path(value: &str) -> String {
    value.trim().replace('\\', "/")
}

#[derive(Debug, Clone, Default)]
struct FulcioCertificateIdentity {
    subject_alt_names: Vec<String>,
    oidc_issuer: Option<String>,
    build_signer_uri: Option<String>,
    build_signer_digest: Option<String>,
    source_repository_uri: Option<String>,
    source_repository_digest: Option<String>,
    source_repository_ref: Option<String>,
    token_subject: Option<String>,
}

fn extract_fulcio_certificate_identity(
    certificate: &X509Certificate<'_>,
) -> FulcioCertificateIdentity {
    let mut identity = FulcioCertificateIdentity::default();
    for extension in certificate.extensions() {
        if let ParsedExtension::SubjectAlternativeName(subject_alt_name) =
            extension.parsed_extension()
        {
            for name in &subject_alt_name.general_names {
                if let Some(value) = general_name_identity_value(name) {
                    identity.subject_alt_names.push(value);
                }
            }
        }

        let Some(text) = parse_der_text(extension.value) else {
            continue;
        };
        match extension.oid.to_string().as_str() {
            "1.3.6.1.4.1.57264.1.8" => identity.oidc_issuer = Some(text),
            "1.3.6.1.4.1.57264.1.9" => identity.build_signer_uri = Some(text),
            "1.3.6.1.4.1.57264.1.10" => identity.build_signer_digest = Some(text),
            "1.3.6.1.4.1.57264.1.12" => identity.source_repository_uri = Some(text),
            "1.3.6.1.4.1.57264.1.13" => identity.source_repository_digest = Some(text),
            "1.3.6.1.4.1.57264.1.14" => identity.source_repository_ref = Some(text),
            "1.3.6.1.4.1.57264.1.24" => identity.token_subject = Some(text),
            _ => {}
        }
    }
    identity
}

fn general_name_identity_value(name: &GeneralName<'_>) -> Option<String> {
    match name {
        GeneralName::URI(value) | GeneralName::RFC822Name(value) | GeneralName::DNSName(value) => {
            Some((*value).to_string())
        }
        GeneralName::OtherName(oid, value) => {
            parse_der_text(value).map(|text| format!("{oid}:{text}"))
        }
        _ => None,
    }
}

fn parse_der_text(value: &[u8]) -> Option<String> {
    if value.len() >= 2 && matches!(value[0], 0x0c | 0x16 | 0x13) {
        let (length, offset) = parse_der_length(&value[1..])?;
        let start = 1 + offset;
        let end = start.checked_add(length)?;
        if end == value.len() {
            return String::from_utf8(value[start..end].to_vec()).ok();
        }
    }

    let text = String::from_utf8(value.to_vec()).ok()?;
    if text.chars().all(|character| {
        character == '\n' || character == '\r' || character == '\t' || !character.is_ascii_control()
    }) {
        Some(text)
    } else {
        None
    }
}

fn parse_der_length(value: &[u8]) -> Option<(usize, usize)> {
    let first = *value.first()?;
    if first & 0x80 == 0 {
        return Some((usize::from(first), 1));
    }
    let byte_count = usize::from(first & 0x7f);
    if byte_count == 0 || byte_count > std::mem::size_of::<usize>() || value.len() < 1 + byte_count
    {
        return None;
    }
    let mut length = 0usize;
    for byte in &value[1..=byte_count] {
        length = length.checked_mul(256)?.checked_add(usize::from(*byte))?;
    }
    Some((length, 1 + byte_count))
}

fn verify_fulcio_identity_selectors(
    document: &SigstoreTrustedRootPolicyDocument,
    identity: &FulcioCertificateIdentity,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let policy = &document.sigstore_trusted_root_policy.identity_policy;

    match identity.oidc_issuer.as_deref() {
        Some(value) if value == policy.expected_oidc_issuer => {
            verified_evidence.push("fulcio_identity_oidc_issuer_match".to_string());
        }
        Some(_) => reasons.push("fulcio_identity_oidc_issuer_mismatch".to_string()),
        None => reasons.push("fulcio_identity_oidc_issuer_missing".to_string()),
    }

    if let Some(expected) = policy.expected_certificate_identity.as_deref() {
        if identity
            .subject_alt_names
            .iter()
            .any(|observed| observed == expected)
        {
            verified_evidence.push("fulcio_identity_san_match".to_string());
        } else {
            reasons.push("fulcio_identity_san_mismatch".to_string());
        }
    }

    if let Some(expected) = policy.expected_github_repository.as_deref() {
        match identity.source_repository_uri.as_deref() {
            Some(observed) if github_repository_matches(expected, observed) => {
                verified_evidence.push("fulcio_identity_github_repository_match".to_string());
            }
            Some(_) => reasons.push("fulcio_identity_github_repository_mismatch".to_string()),
            None => reasons.push("fulcio_identity_github_repository_missing".to_string()),
        }
    }

    if let Some(expected) = policy.expected_github_ref.as_deref() {
        match identity.source_repository_ref.as_deref() {
            Some(observed) if observed == expected => {
                verified_evidence.push("fulcio_identity_github_ref_match".to_string());
            }
            Some(_) => reasons.push("fulcio_identity_github_ref_mismatch".to_string()),
            None => reasons.push("fulcio_identity_github_ref_missing".to_string()),
        }
    }

    if let Some(expected) = policy.expected_github_sha.as_deref() {
        let digest_match = identity
            .source_repository_digest
            .as_deref()
            .is_some_and(|observed| observed == expected)
            || identity
                .build_signer_digest
                .as_deref()
                .is_some_and(|observed| observed == expected);
        if digest_match {
            verified_evidence.push("fulcio_identity_github_sha_match".to_string());
        } else {
            reasons.push("fulcio_identity_github_sha_mismatch".to_string());
        }
    }
}

fn github_repository_matches(expected: &str, observed: &str) -> bool {
    normalize_github_repository(expected) == normalize_github_repository(observed)
}

fn normalize_github_repository(value: &str) -> String {
    let mut normalized = value
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("github.com/")
        .trim_start_matches("www.github.com/")
        .trim_end_matches(".git")
        .to_string();
    if normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

#[derive(Debug, Clone)]
struct ParsedSigstoreMessageSignatureBundle {
    media_type: Option<String>,
    certificate_der: Vec<u8>,
    message_digest_algorithm: String,
    message_digest: Vec<u8>,
    signature: Vec<u8>,
}

#[derive(Debug, Clone)]
struct ParsedSigstoreDsseBundle {
    media_type: Option<String>,
    certificate_der: Vec<u8>,
    payload_type: String,
    payload: Vec<u8>,
    signature: Vec<u8>,
    envelope: Value,
}

fn parse_sigstore_message_signature_bundle(
    bytes: &[u8],
    reasons: &mut Vec<String>,
) -> Option<ParsedSigstoreMessageSignatureBundle> {
    let value = match serde_json::from_slice::<Value>(bytes) {
        Ok(value) => value,
        Err(err) => {
            reasons.push(format!("bundle_json_invalid:{err}"));
            return None;
        }
    };

    let media_type = value
        .get("mediaType")
        .and_then(Value::as_str)
        .map(str::to_string);
    let certificate_der = required_json_string(
        &value,
        &["verificationMaterial", "certificate", "rawBytes"],
        "bundle_certificate_raw_bytes_missing",
        reasons,
    )
    .and_then(|raw| decode_base64(raw, "bundle_certificate_raw_bytes_invalid", reasons))?;
    let message_signature = value.get("messageSignature").unwrap_or(&Value::Null);
    let message_digest = required_json_string(
        message_signature,
        &["messageDigest", "digest"],
        "bundle_message_digest_missing",
        reasons,
    )
    .and_then(|digest| decode_base64(digest, "bundle_message_digest_invalid", reasons))?;
    let message_digest_algorithm = required_json_string(
        message_signature,
        &["messageDigest", "algorithm"],
        "bundle_message_digest_algorithm_missing",
        reasons,
    )
    .map(|value| value.to_ascii_lowercase())?;
    let signature = required_json_string(
        message_signature,
        &["signature"],
        "bundle_signature_missing",
        reasons,
    )
    .and_then(|signature| decode_base64(signature, "bundle_signature_invalid", reasons))?;

    Some(ParsedSigstoreMessageSignatureBundle {
        media_type,
        certificate_der,
        message_digest_algorithm,
        message_digest,
        signature,
    })
}

fn parse_sigstore_dsse_bundle(
    bytes: &[u8],
    reasons: &mut Vec<String>,
) -> Option<ParsedSigstoreDsseBundle> {
    let value = match serde_json::from_slice::<Value>(bytes) {
        Ok(value) => value,
        Err(err) => {
            reasons.push(format!("dsse_bundle_json_invalid:{err}"));
            return None;
        }
    };

    let media_type = value
        .get("mediaType")
        .and_then(Value::as_str)
        .map(str::to_string);
    let certificate_der = required_json_string(
        &value,
        &["verificationMaterial", "certificate", "rawBytes"],
        "dsse_bundle_certificate_raw_bytes_missing",
        reasons,
    )
    .and_then(|raw| {
        decode_base64_flexible(raw, "dsse_bundle_certificate_raw_bytes_invalid", reasons)
    })?;
    let envelope = value.get("dsseEnvelope").cloned().unwrap_or(Value::Null);
    if envelope.is_null() {
        reasons.push("dsse_envelope_missing".to_string());
        return None;
    }
    let payload_type = required_json_string(
        &envelope,
        &["payloadType"],
        "dsse_payload_type_missing",
        reasons,
    )?
    .to_string();
    let payload = required_json_string(&envelope, &["payload"], "dsse_payload_missing", reasons)
        .and_then(|payload| decode_base64_flexible(payload, "dsse_payload_invalid", reasons))?;
    let signatures = envelope
        .get("signatures")
        .and_then(Value::as_array)
        .ok_or_else(|| "dsse_signatures_missing".to_string())
        .map_err(|reason| reasons.push(reason))
        .ok()?;
    if signatures.len() != 1 {
        reasons.push("dsse_signature_count_invalid".to_string());
        return None;
    }
    let signature =
        required_json_string(&signatures[0], &["sig"], "dsse_signature_missing", reasons)
            .and_then(|signature| {
                decode_base64_flexible(signature, "dsse_signature_invalid", reasons)
            })?;

    Some(ParsedSigstoreDsseBundle {
        media_type,
        certificate_der,
        payload_type,
        payload,
        signature,
        envelope,
    })
}

fn required_json_string<'a>(
    value: &'a Value,
    path: &[&str],
    reason: &str,
    reasons: &mut Vec<String>,
) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = match current.get(*segment) {
            Some(value) => value,
            None => {
                reasons.push(reason.to_string());
                return None;
            }
        };
    }
    match current.as_str() {
        Some(value) => Some(value),
        None => {
            reasons.push(reason.to_string());
            None
        }
    }
}

fn decode_base64(value: &str, reason: &str, reasons: &mut Vec<String>) -> Option<Vec<u8>> {
    match BASE64.decode(value.as_bytes()) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            reasons.push(format!("{reason}:{err}"));
            None
        }
    }
}

fn decode_base64_flexible(value: &str, reason: &str, reasons: &mut Vec<String>) -> Option<Vec<u8>> {
    for engine in [&BASE64, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(value.as_bytes()) {
            return Some(bytes);
        }
    }
    reasons.push(reason.to_string());
    None
}

fn dsse_pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
    let payload_type = payload_type.as_bytes();
    let mut out = Vec::new();
    out.extend_from_slice(b"DSSEv1 ");
    out.extend_from_slice(payload_type.len().to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(payload_type);
    out.push(b' ');
    out.extend_from_slice(payload.len().to_string().as_bytes());
    out.push(b' ');
    out.extend_from_slice(payload);
    out
}

fn verify_bundle_signature_with_certificate(
    certificate: &X509Certificate<'_>,
    message_digest: &[u8],
    signature: &[u8],
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let verifying_key = match P256VerifyingKey::from_sec1_bytes(
        certificate.public_key().subject_public_key.data.as_ref(),
    ) {
        Ok(key) => key,
        Err(err) => {
            reasons.push(format!("bundle_certificate_public_key_p256_invalid:{err}"));
            return;
        }
    };
    let signature = match P256Signature::from_der(signature) {
        Ok(signature) => signature,
        Err(err) => {
            reasons.push(format!("bundle_signature_der_invalid:{err}"));
            return;
        }
    };
    if verifying_key.verify(message_digest, &signature).is_ok() {
        verified_evidence.push("bundle_signature_verified_with_certificate_key".to_string());
    } else {
        reasons.push("bundle_signature_verification_failed".to_string());
    }
}

fn verify_dsse_signature_with_certificate(
    certificate: &X509Certificate<'_>,
    payload_type: &str,
    payload: &[u8],
    signature: &[u8],
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let verifying_key = match P256VerifyingKey::from_sec1_bytes(
        certificate.public_key().subject_public_key.data.as_ref(),
    ) {
        Ok(key) => key,
        Err(err) => {
            reasons.push(format!("dsse_certificate_public_key_p256_invalid:{err}"));
            return;
        }
    };
    let signature = match P256Signature::from_der(signature) {
        Ok(signature) => signature,
        Err(err) => {
            reasons.push(format!("dsse_signature_der_invalid:{err}"));
            return;
        }
    };
    let pae = dsse_pae(payload_type, payload);
    if verifying_key.verify(&pae, &signature).is_ok() {
        verified_evidence.push("dsse_signature_verified_with_certificate_key".to_string());
    } else {
        reasons.push("dsse_signature_verification_failed".to_string());
    }
}

fn verify_rekor_body_binds_bundle(
    entry: &crypto_rekor::ParsedRekorEntry,
    message_digest: &[u8],
    signature: &[u8],
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let expected_digest = hex_bytes(message_digest);
    let observed_digest = entry
        .body
        .pointer("/spec/data/hash/value")
        .and_then(Value::as_str)
        .map(normalize_sha256_display);
    if observed_digest.as_deref() == Some(expected_digest.as_str()) {
        verified_evidence.push("rekor_body_binds_bundle_artifact_digest".to_string());
    } else {
        reasons.push("rekor_body_artifact_digest_mismatch".to_string());
    }

    let expected_signature = BASE64.encode(signature);
    let observed_signature = entry
        .body
        .pointer("/spec/signature/content")
        .and_then(Value::as_str);
    if observed_signature == Some(expected_signature.as_str()) {
        verified_evidence.push("rekor_body_binds_bundle_signature".to_string());
    } else {
        reasons.push("rekor_body_signature_mismatch".to_string());
    }
}

fn verify_rekor_body_binds_dsse(
    entry: &crypto_rekor::ParsedRekorEntry,
    expected_payload_hash: &str,
    expected_envelope_hash: &str,
    signature: &[u8],
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    match entry.body.get("kind").and_then(Value::as_str) {
        Some("dsse") => verified_evidence.push("rekor_body_kind_dsse".to_string()),
        Some(_) => reasons.push("rekor_body_kind_not_dsse".to_string()),
        None => reasons.push("rekor_body_kind_missing".to_string()),
    }

    let observed_payload_hash = first_json_string(
        &entry.body,
        &["/spec/dsseObj/payloadHash/value", "/spec/payloadHash/value"],
    )
    .map(|value| normalize_sha256_display(&value));
    if observed_payload_hash.as_deref() == Some(expected_payload_hash) {
        verified_evidence.push("rekor_body_binds_dsse_payload_hash".to_string());
    } else {
        reasons.push("rekor_body_dsse_payload_hash_mismatch".to_string());
    }

    let observed_envelope_hash = first_json_string(
        &entry.body,
        &[
            "/spec/dsseObj/envelopeHash/value",
            "/spec/envelopeHash/value",
        ],
    )
    .map(|value| normalize_sha256_display(&value));
    if observed_envelope_hash.as_deref() == Some(expected_envelope_hash) {
        verified_evidence.push("rekor_body_binds_dsse_envelope_hash".to_string());
    } else {
        reasons.push("rekor_body_dsse_envelope_hash_mismatch".to_string());
    }

    let expected_signature = BASE64.encode(signature);
    let observed_signature = first_dsse_rekor_signature(&entry.body);
    if observed_signature.as_deref() == Some(expected_signature.as_str()) {
        verified_evidence.push("rekor_body_binds_dsse_signature".to_string());
    } else {
        reasons.push("rekor_body_dsse_signature_mismatch".to_string());
    }
}

fn first_json_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers.iter().find_map(|pointer| {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn first_dsse_rekor_signature(value: &Value) -> Option<String> {
    ["/spec/dsseObj/signatures", "/spec/signatures"]
        .iter()
        .filter_map(|pointer| value.pointer(pointer).and_then(Value::as_array))
        .flat_map(|items| items.iter())
        .find_map(|item| {
            item.get("signature")
                .or_else(|| item.get("sig"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn read_required_file(path: &Path, label: &str, reasons: &mut Vec<String>) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            reasons.push(format!("{label}_read_failed:{:?}", err.kind()));
            None
        }
    }
}

fn read_signature_file(path: &Path, reasons: &mut Vec<String>) -> Option<Vec<u8>> {
    read_required_file(path, "signature", reasons)
        .and_then(|bytes| decode_base64_or_raw(bytes, 64, "signature", reasons))
}

fn read_public_key_file(path: &Path, reasons: &mut Vec<String>) -> Option<Vec<u8>> {
    read_required_file(path, "public_key", reasons)
        .and_then(|bytes| decode_base64_or_raw(bytes, 32, "public_key", reasons))
}

fn decode_ct_log_id(value: &str, reasons: &mut Vec<String>) -> Option<[u8; 32]> {
    let trimmed = value.trim();
    let maybe_digest = trimmed.strip_prefix("sha256:").unwrap_or(trimmed);
    let hex_candidate = maybe_digest.replace(':', "");
    if hex_candidate.len() == 64
        && hex_candidate
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        let mut bytes = [0u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let start = index * 2;
            *byte =
                u8::from_str_radix(&hex_candidate[start..start + 2], 16).expect("valid hex pair");
        }
        return Some(bytes);
    }

    if let Some(decoded) = decode_base64_flexible(trimmed, "ct_sct_log_id_invalid", reasons) {
        if let Ok(bytes) = <[u8; 32]>::try_from(decoded.as_slice()) {
            return Some(bytes);
        }
        reasons.push(format!("ct_sct_log_id_length_invalid:{}", decoded.len()));
        return None;
    }
    None
}

fn decode_base64_or_raw(
    bytes: Vec<u8>,
    raw_len: usize,
    label: &str,
    reasons: &mut Vec<String>,
) -> Option<Vec<u8>> {
    if bytes.len() == raw_len {
        return Some(bytes);
    }
    let text = String::from_utf8_lossy(&bytes);
    let compact = text.split_whitespace().collect::<String>();
    match BASE64.decode(compact.as_bytes()) {
        Ok(decoded) if decoded.len() == raw_len => Some(decoded),
        Ok(decoded) => {
            reasons.push(format!("{label}_length_invalid:{}", decoded.len()));
            None
        }
        Err(_) => {
            reasons.push(format!("{label}_base64_invalid"));
            None
        }
    }
}

fn verify_ed25519_signature(public_key: &[u8], signature: &[u8], message: &[u8]) -> bool {
    let Ok(public_key_bytes) = <&[u8; 32]>::try_from(public_key) else {
        return false;
    };
    let Ok(signature_bytes) = <&[u8; 64]>::try_from(signature) else {
        return false;
    };
    let Ok(verifying_key) = Ed25519VerifyingKey::from_bytes(public_key_bytes) else {
        return false;
    };
    let signature = Ed25519Signature::from_bytes(signature_bytes);
    verifying_key.verify(message, &signature).is_ok()
}

struct ExpectedProvenance<'a> {
    sha256: &'a str,
    builder_id: &'a str,
    source_uri: &'a str,
    source_ref: &'a str,
}

fn verify_slsa_statement(
    statement: &Value,
    expected: ExpectedProvenance<'_>,
    predicate_type: &mut Option<String>,
    builder_id: &mut Option<String>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    match statement.get("_type").and_then(Value::as_str) {
        Some(value) if value.starts_with("https://in-toto.io/Statement/v") => {
            verified_evidence.push("intoto_statement_type".to_string());
        }
        Some(_) => reasons.push("intoto_statement_type_invalid".to_string()),
        None => reasons.push("intoto_statement_type_missing".to_string()),
    }

    match statement.get("predicateType").and_then(Value::as_str) {
        Some("https://slsa.dev/provenance/v1") => {
            *predicate_type = Some("https://slsa.dev/provenance/v1".to_string());
            verified_evidence.push("slsa_predicate_type".to_string());
        }
        Some(_) => reasons.push("slsa_predicate_type_invalid".to_string()),
        None => reasons.push("slsa_predicate_type_missing".to_string()),
    }

    if statement_subject_has_sha256(statement, expected.sha256) {
        verified_evidence.push("provenance_subject_matches_artifact".to_string());
    } else {
        reasons.push("provenance_subject_sha256_missing".to_string());
    }

    let Some(predicate) = statement.get("predicate") else {
        reasons.push("slsa_predicate_missing".to_string());
        return;
    };

    match predicate
        .get("builder")
        .and_then(|item| item.get("id"))
        .and_then(Value::as_str)
    {
        Some(value) if value == expected.builder_id => {
            *builder_id = Some(value.to_string());
            verified_evidence.push("builder_id_match".to_string());
        }
        Some(value) => {
            *builder_id = Some(value.to_string());
            reasons.push("builder_id_mismatch".to_string());
        }
        None => reasons.push("builder_id_missing".to_string()),
    }

    if json_contains_string(predicate, expected.source_uri) {
        verified_evidence.push("source_uri_match".to_string());
    } else {
        reasons.push("source_uri_missing".to_string());
    }

    if json_contains_string(predicate, expected.source_ref) {
        verified_evidence.push("source_ref_match".to_string());
    } else {
        reasons.push("source_ref_missing".to_string());
    }
}

fn statement_subject_has_sha256(statement: &Value, expected_sha256: &str) -> bool {
    statement
        .get("subject")
        .and_then(Value::as_array)
        .is_some_and(|subjects| {
            subjects.iter().any(|subject| {
                subject
                    .get("digest")
                    .and_then(|digest| digest.get("sha256"))
                    .and_then(Value::as_str)
                    .is_some_and(|value| normalize_sha256_display(value) == expected_sha256)
            })
        })
}

fn json_contains_string(value: &Value, expected: &str) -> bool {
    match value {
        Value::String(actual) => actual == expected,
        Value::Array(items) => items
            .iter()
            .any(|item| json_contains_string(item, expected)),
        Value::Object(map) => map
            .values()
            .any(|item| json_contains_string(item, expected)),
        _ => false,
    }
}

fn verify_transparency_log_proof(
    provenance_sha256: &str,
    signature_sha256: &str,
    transparency_log: &[u8],
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let proof = match serde_json::from_slice::<Value>(transparency_log) {
        Ok(value) => value,
        Err(err) => {
            reasons.push(format!("transparency_log_json_invalid:{err}"));
            return;
        }
    };

    let expected_leaf_hash = transparency_leaf_hash(provenance_sha256, signature_sha256);
    let leaf_hash = proof
        .get("leaf_hash")
        .and_then(Value::as_str)
        .and_then(normalize_sha256_digest);
    let root_hash = proof
        .get("root_hash")
        .and_then(Value::as_str)
        .and_then(normalize_sha256_digest);
    let hashes = proof
        .get("hashes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(normalize_sha256_digest)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let log_index = proof.get("log_index").and_then(Value::as_u64);
    let tree_size = proof.get("tree_size").and_then(Value::as_u64);

    if proof
        .get("log_id")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        verified_evidence.push("transparency_log_id_present".to_string());
    } else {
        reasons.push("transparency_log_id_missing".to_string());
    }

    match leaf_hash.as_deref() {
        Some(value) if value == expected_leaf_hash => {
            verified_evidence.push("transparency_leaf_binds_signature_and_provenance".to_string());
        }
        Some(_) => reasons.push("transparency_leaf_hash_mismatch".to_string()),
        None => reasons.push("transparency_leaf_hash_missing".to_string()),
    }

    let Some(leaf_hash) = leaf_hash else {
        return;
    };
    let Some(root_hash) = root_hash else {
        reasons.push("transparency_root_hash_missing".to_string());
        return;
    };
    let Some(log_index) = log_index else {
        reasons.push("transparency_log_index_missing".to_string());
        return;
    };
    let Some(tree_size) = tree_size else {
        reasons.push("transparency_tree_size_missing".to_string());
        return;
    };

    if crypto_rekor::verify_merkle_inclusion(&leaf_hash, &hashes, log_index, tree_size, &root_hash)
    {
        verified_evidence.push("transparency_inclusion_proof_valid".to_string());
    } else {
        reasons.push("transparency_inclusion_proof_invalid".to_string());
    }
}

fn transparency_leaf_hash(provenance_sha256: &str, signature_sha256: &str) -> String {
    let payload = format!(
        "{}\n{}",
        normalize_sha256_display(provenance_sha256),
        normalize_sha256_display(signature_sha256)
    );
    let mut content = Vec::with_capacity(payload.len() + 1);
    content.push(0);
    content.extend_from_slice(payload.as_bytes());
    hex_sha256(&content)
}

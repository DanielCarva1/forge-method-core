//! Host-adapter verification entrypoints.
//!
//! Groups together all of the `run_host_adapter_*_verification` functions
//! that the `forge` CLI (`main.rs`) and integration tests (`tests/validate.rs`)
//! invoke to verify artifact provenance, signatures, sigstore bundles, DSSE
//! payloads, fulcio certificate identity, certificate transparency SCTs,
//! CRL/OCSP revocation status, TUF trusted root freshness, and rekor log
//! inclusion. The functions stay `pub` and are re-exported at the crate
//! root via `pub use host_adapter_verification::*;` so external callers
//! keep importing `forge_core_cli::run_host_adapter_*_verification`
//! unchanged after the extraction.

use p256::ecdsa::VerifyingKey as P256VerifyingKey;
use p256::pkcs8::DecodePublicKey;
use rasn_ocsp::OcspResponseStatus;
use serde_json::Value;
use std::fs;
use x509_parser::parse_x509_crl;

use crate::hashing::{hex_bytes, hex_sha256, normalize_sha256_digest, normalize_sha256_display};
use crate::ocsp::{
    apply_ocsp_cert_status, decode_basic_ocsp_response, decode_ocsp_response,
    extract_ocsp_response_nonce_hex, find_matching_ocsp_single_response,
    normalize_expected_ocsp_nonce_hex, ocsp_responder_id_matches_issuer, rasn_oid_matches,
    verify_basic_ocsp_signature_with_issuer, verify_ocsp_nonce,
    verify_ocsp_single_response_freshness,
};
use crate::rekor;
// Items below are accessed via the qualified `rekor::*` path used by
// the verification functions (kept verbatim from the original `lib.rs`).
use crate::file_io::{read_public_key_file, read_required_file, read_signature_file};
use crate::host_command::{source_ref_is_immutable, version_like};
#[allow(unused_imports)]
use crate::rekor::{parse_rekor_log_entry, verify_rekor_entry_inclusion, ParsedRekorEntry};
use crate::tuf::verify_tuf_metadata_freshness_role;
// Re-export wildcard for the sigstore / SLSA helpers and host-adapter types
// (same pattern as the original crate-root re-exports in `lib.rs`).
#[allow(clippy::wildcard_imports)]
use crate::host_adapter_types::*;
#[allow(clippy::wildcard_imports)]
use crate::sigstore::*;
#[allow(clippy::wildcard_imports)]
use crate::slsa_transparency::*;

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

    let mut log_entry: Option<rekor::ParsedRekorEntry> = None;
    if let Some(text) = log_entry_text.as_deref() {
        match rekor::parse_rekor_log_entry(text) {
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
            rekor::verify_rekor_entry_inclusion(entry, key, &mut verified_evidence, &mut reasons);
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
        .and_then(|text| match rekor::parse_rekor_log_entry(&text) {
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
        .and_then(|text| match rekor::parse_rekor_log_entry(&text) {
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

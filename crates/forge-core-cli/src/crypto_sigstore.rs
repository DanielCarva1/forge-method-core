//! Sigstore trust policy, certificate/Fulcio chain verification, and
//! bundle/DSSE signature helpers.
//!
//! This module groups together all of the helpers that the
//! `run_host_adapter_sigstore_*_verification`,
//! `run_host_adapter_fulcio_certificate_identity_verification`,
//! `run_host_adapter_certificate_transparency_sct_verification`, and
//! `run_host_adapter_sigstore_timestamp_authority_verification` functions
//! in `lib.rs` rely on to:
//!
//! - load and apply a Sigstore trusted-root policy document (Fulcio, Rekor,
//!   CT, TSA, identity, revocation sub-policies),
//! - parse and verify Fulcio leaf/issuer certificate chains (basic
//!   constraints, code-signing EKU, SAN identity extraction),
//! - verify Sigstore bundles (message signature + DSSE) against their
//!   signing certificate, including rekor-entry body binding checks,
//! - resolve RFC 3161 vs Rekor-integrated-time timestamp authorities, and
//! - decode Certificate Transparency SCT log IDs (hex or base64).
//!
//! The helpers are `pub(crate)` and re-exported at the crate root via
//! `pub(crate) use crypto_sigstore::*;` so existing call sites inside
//! `lib.rs` keep resolving unchanged after the extraction.

use base64::{
    engine::general_purpose::{STANDARD as BASE64, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use rustls_pki_types::CertificateDer;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use x509_parser::certificate::X509Certificate;
use x509_parser::extensions::{GeneralName, ParsedExtension};
use x509_parser::parse_x509_certificate;
use x509_parser::pem::parse_x509_pem;

use crate::crypto_hashing::{hex_bytes, normalize_sha256_display};
use crate::{
    read_required_file, run_host_adapter_rekor_verification, HostAdapterRekorVerificationInput,
    HostAdapterRekorVerificationStatus, HostAdapterSigstoreTimestampAuthorityVerificationInput,
};

pub(crate) fn select_rekor_integrated_time_for_timestamp_authority(
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
        match crate::crypto_rekor::parse_rekor_log_entry(&text) {
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

pub(crate) fn select_rfc3161_tsa_for_timestamp_authority(
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

pub(crate) fn resolve_policy_relative_path(policy_path: &Path, path_ref: &str) -> PathBuf {
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
pub(crate) struct SigstoreTrustedRootPolicyDocument {
    pub(crate) schema_version: String,
    pub(crate) sigstore_trusted_root_policy: SigstoreTrustedRootPolicy,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SigstoreTrustedRootPolicy {
    pub(crate) root_source: String,
    pub(crate) trusted_root_ref: String,
    pub(crate) offline_allowed: bool,
    pub(crate) fulcio: SigstoreFulcioTrustPolicy,
    pub(crate) rekor: SigstoreRekorTrustPolicy,
    pub(crate) certificate_transparency: SigstoreCertificateTransparencyTrustPolicy,
    pub(crate) timestamp_authority: SigstoreTimestampAuthorityPolicy,
    #[serde(default)]
    pub(crate) revocation: Option<SigstoreRevocationPolicy>,
    pub(crate) identity_policy: SigstoreIdentityPolicy,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SigstoreFulcioTrustPolicy {
    pub(crate) required: bool,
    #[serde(default)]
    pub(crate) certificate_authority_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SigstoreRekorTrustPolicy {
    pub(crate) required: bool,
    #[serde(default)]
    pub(crate) log_ids: Vec<String>,
    #[serde(default)]
    pub(crate) public_key_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SigstoreCertificateTransparencyTrustPolicy {
    pub(crate) required: bool,
    #[serde(default)]
    pub(crate) log_ids: Vec<String>,
    #[serde(default)]
    pub(crate) public_key_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SigstoreTimestampAuthorityPolicy {
    pub(crate) mode: String,
    #[serde(default)]
    pub(crate) certificate_refs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SigstoreRevocationPolicy {
    pub(crate) mode: String,
    #[serde(default)]
    pub(crate) max_certificate_lifetime_seconds: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SigstoreIdentityPolicy {
    pub(crate) expected_oidc_issuer: String,
    #[serde(default)]
    pub(crate) expected_certificate_identity: Option<String>,
    #[serde(default)]
    pub(crate) expected_github_repository: Option<String>,
    #[serde(default)]
    pub(crate) expected_github_ref: Option<String>,
    #[serde(default)]
    pub(crate) expected_github_sha: Option<String>,
}

pub(crate) struct CertificateTransparencyLogMaterial {
    pub(crate) id: [u8; 32],
    pub(crate) id_hex: String,
    pub(crate) key: Vec<u8>,
}

pub(crate) fn load_certificate_transparency_log_material(
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

pub(crate) fn verify_sigstore_trust_policy(
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

pub(crate) fn verify_sigstore_timestamp_policy(
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

pub(crate) fn verify_sigstore_identity_policy(
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

pub(crate) fn non_empty_string(value: &str) -> bool {
    !value.trim().is_empty()
}

pub(crate) fn optional_non_empty(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

pub(crate) fn non_empty_items(values: &[String]) -> bool {
    !values.is_empty() && values.iter().all(|value| !value.trim().is_empty())
}

pub(crate) fn is_git_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

pub(crate) fn read_sigstore_trust_policy_document(
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

pub(crate) fn read_certificate_der(
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

pub(crate) fn parse_certificate<'a>(
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

pub(crate) fn verify_fulcio_chain(
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

pub(crate) fn verify_issuer_ca_usage(
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

pub(crate) fn verify_leaf_code_signing_usage(
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

pub(crate) fn path_matches_any_ref(path: &Path, refs: &[String]) -> bool {
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

pub(crate) fn normalize_ref_path(value: &str) -> String {
    value.trim().replace('\\', "/")
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FulcioCertificateIdentity {
    pub(crate) subject_alt_names: Vec<String>,
    pub(crate) oidc_issuer: Option<String>,
    pub(crate) build_signer_uri: Option<String>,
    pub(crate) build_signer_digest: Option<String>,
    pub(crate) source_repository_uri: Option<String>,
    pub(crate) source_repository_digest: Option<String>,
    pub(crate) source_repository_ref: Option<String>,
    pub(crate) token_subject: Option<String>,
}

pub(crate) fn extract_fulcio_certificate_identity(
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

pub(crate) fn general_name_identity_value(name: &GeneralName<'_>) -> Option<String> {
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

pub(crate) fn parse_der_text(value: &[u8]) -> Option<String> {
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

pub(crate) fn parse_der_length(value: &[u8]) -> Option<(usize, usize)> {
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

pub(crate) fn verify_fulcio_identity_selectors(
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

pub(crate) fn github_repository_matches(expected: &str, observed: &str) -> bool {
    normalize_github_repository(expected) == normalize_github_repository(observed)
}

pub(crate) fn normalize_github_repository(value: &str) -> String {
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
pub(crate) struct ParsedSigstoreMessageSignatureBundle {
    pub(crate) media_type: Option<String>,
    pub(crate) certificate_der: Vec<u8>,
    pub(crate) message_digest_algorithm: String,
    pub(crate) message_digest: Vec<u8>,
    pub(crate) signature: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedSigstoreDsseBundle {
    pub(crate) media_type: Option<String>,
    pub(crate) certificate_der: Vec<u8>,
    pub(crate) payload_type: String,
    pub(crate) payload: Vec<u8>,
    pub(crate) signature: Vec<u8>,
    pub(crate) envelope: Value,
}

pub(crate) fn parse_sigstore_message_signature_bundle(
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

pub(crate) fn parse_sigstore_dsse_bundle(
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

pub(crate) fn required_json_string<'a>(
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

pub(crate) fn decode_base64(
    value: &str,
    reason: &str,
    reasons: &mut Vec<String>,
) -> Option<Vec<u8>> {
    match BASE64.decode(value.as_bytes()) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            reasons.push(format!("{reason}:{err}"));
            None
        }
    }
}

pub(crate) fn decode_base64_flexible(
    value: &str,
    reason: &str,
    reasons: &mut Vec<String>,
) -> Option<Vec<u8>> {
    for engine in [&BASE64, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = engine.decode(value.as_bytes()) {
            return Some(bytes);
        }
    }
    reasons.push(reason.to_string());
    None
}

pub(crate) fn dsse_pae(payload_type: &str, payload: &[u8]) -> Vec<u8> {
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

pub(crate) fn verify_bundle_signature_with_certificate(
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

pub(crate) fn verify_dsse_signature_with_certificate(
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

pub(crate) fn verify_rekor_body_binds_bundle(
    entry: &crate::crypto_rekor::ParsedRekorEntry,
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

pub(crate) fn verify_rekor_body_binds_dsse(
    entry: &crate::crypto_rekor::ParsedRekorEntry,
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

pub(crate) fn first_json_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers.iter().find_map(|pointer| {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

pub(crate) fn first_dsse_rekor_signature(value: &Value) -> Option<String> {
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

pub(crate) fn decode_ct_log_id(value: &str, reasons: &mut Vec<String>) -> Option<[u8; 32]> {
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

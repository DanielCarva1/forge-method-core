//! OCSP (Online Certificate Status Protocol) verification helpers.
//!
//! This module owns the low-level helpers that decode OCSP responses, verify
//! the responder signature against an issuer certificate, locate the matching
//! single response for a target certificate, check freshness windows, apply
//! the certificate status, and verify response nonces.
//!
//! The public entrypoint that consumes these helpers is
//! [`crate::run_host_adapter_certificate_ocsp_status_verification`], which
//! stays in `lib.rs` as part of the host-adapter verification domain.
//!
//! ## Visibility
//!
//! Helpers called from outside this module are `pub(crate)` and re-exported
//! at the crate root via `pub(crate) use`. Helpers only called within this
//! module (`ocsp_cert_id_issuer_hashes_match`, `ocsp_digest_for_algorithm`,
//! `sha1_digest`) remain private.

use asn1_rs::{BitString as Asn1BitString, FromDer as _};
use rasn::types::ObjectIdentifier as RasnObjectIdentifier;
use rasn_ocsp::{BasicOcspResponse, CertId, CertStatus, OcspResponse, ResponderId, SingleResponse};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::collections::HashSet;
use x509_parser::certificate::X509Certificate;
use x509_parser::extensions::ParsedExtension;
use x509_parser::x509::{AlgorithmIdentifier as X509AlgorithmIdentifier, X509Name};
use zeroize::Zeroizing;

use crate::hashing::hex_bytes;

/// Decode a DER-encoded OCSP response.
///
/// Public so the `fuzz/` workspace can drive the parser with attacker-controlled
/// DER bytes; all other OCSP helpers stay `pub(crate)`.
pub fn decode_ocsp_response(
    der: &[u8],
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<OcspResponse> {
    match rasn::der::decode::<OcspResponse>(der) {
        Ok(response) => {
            verified_evidence.push("ocsp_status_response_parsed".to_string());
            Some(response)
        }
        Err(err) => {
            reasons.push(format!("ocsp_status_response_parse_failed:{err}"));
            None
        }
    }
}

/// Decode the basic OCSP response embedded in an OCSP response.
pub(crate) fn decode_basic_ocsp_response(
    der: &[u8],
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<BasicOcspResponse> {
    match rasn::der::decode::<BasicOcspResponse>(der) {
        Ok(response) => {
            verified_evidence.push("ocsp_status_basic_response_parsed".to_string());
            Some(response)
        }
        Err(err) => {
            reasons.push(format!("ocsp_status_basic_response_parse_failed:{err}"));
            None
        }
    }
}

/// Verify that the basic OCSP response signature was produced by the selected
/// responder certificate. The caller must authorize that certificate before
/// invoking this helper.
pub(crate) fn verify_basic_ocsp_signature_with_certificate(
    basic_response: &BasicOcspResponse,
    responder: &X509Certificate<'_>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> bool {
    let tbs_der = match rasn::der::encode(&basic_response.tbs_response_data) {
        Ok(value) => Zeroizing::new(value),
        Err(err) => {
            reasons.push(format!("ocsp_status_tbs_response_encode_failed:{err}"));
            return false;
        }
    };
    let algorithm_der = match rasn::der::encode(&basic_response.signature_algorithm) {
        Ok(value) => Zeroizing::new(value),
        Err(err) => {
            reasons.push(format!(
                "ocsp_status_signature_algorithm_encode_failed:{err}"
            ));
            return false;
        }
    };
    let signature_der = match rasn::der::encode(&basic_response.signature) {
        Ok(value) => Zeroizing::new(value),
        Err(err) => {
            reasons.push(format!("ocsp_status_signature_encode_failed:{err}"));
            return false;
        }
    };

    let algorithm = match X509AlgorithmIdentifier::from_der(&algorithm_der) {
        Ok(([], algorithm)) => algorithm,
        Ok((_remaining, _algorithm)) => {
            reasons.push("ocsp_status_signature_algorithm_trailing_der".to_string());
            return false;
        }
        Err(err) => {
            reasons.push(format!(
                "ocsp_status_signature_algorithm_parse_failed:{err}"
            ));
            return false;
        }
    };
    let signature = match Asn1BitString::from_der(&signature_der) {
        Ok(([], signature)) => signature,
        Ok((_remaining, _signature)) => {
            reasons.push("ocsp_status_signature_trailing_der".to_string());
            return false;
        }
        Err(err) => {
            reasons.push(format!("ocsp_status_signature_parse_failed:{err}"));
            return false;
        }
    };

    match x509_parser::verify::verify_signature(
        responder.public_key(),
        &algorithm,
        &signature,
        &tbs_der,
    ) {
        Ok(()) => {
            verified_evidence.push("ocsp_status_response_signature_verified".to_string());
            true
        }
        Err(err) => {
            reasons.push("ocsp_status_response_signature_invalid".to_string());
            reasons.push(format!("ocsp_status_signature_failed:{err}"));
            false
        }
    }
}

/// Compatibility wrapper for direct-issuer OCSP responses.
pub(crate) fn verify_basic_ocsp_signature_with_issuer(
    basic_response: &BasicOcspResponse,
    issuer: &X509Certificate<'_>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> bool {
    verify_basic_ocsp_signature_with_certificate(basic_response, issuer, verified_evidence, reasons)
}

/// Check whether the OCSP responder ID matches the issuer certificate.
pub(crate) fn ocsp_responder_id_matches_issuer(
    responder_id: &ResponderId,
    issuer: &X509Certificate<'_>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> bool {
    match responder_id {
        ResponderId::ByName(name) => match rasn::der::encode(name) {
            Ok(name_der) if distinguished_names_match(&name_der, issuer.subject()) => {
                verified_evidence.push("ocsp_status_responder_name_matches_issuer".to_string());
                true
            }
            Ok(_name_der) => false,
            Err(err) => {
                reasons.push(format!("ocsp_status_responder_name_encode_failed:{err}"));
                false
            }
        },
        ResponderId::ByKey(key_hash) => {
            let issuer_key_hash = sha1_digest(issuer.public_key().subject_public_key.data.as_ref());
            if key_hash.as_ref() == issuer_key_hash.as_slice() {
                verified_evidence.push("ocsp_status_responder_key_matches_issuer".to_string());
                true
            } else {
                false
            }
        }
    }
}

/// Check whether the responder ID matches an already-authorized responder
/// certificate, returning the identity form that matched.
pub(crate) fn ocsp_responder_id_matches_certificate(
    responder_id: &ResponderId,
    responder: &X509Certificate<'_>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<crate::host_adapter_types::HostAdapterOcspResponderIdMode> {
    use crate::host_adapter_types::HostAdapterOcspResponderIdMode;

    match responder_id {
        ResponderId::ByName(name) => match rasn::der::encode(name) {
            Ok(name_der) if distinguished_names_match(&name_der, responder.subject()) => {
                verified_evidence
                    .push("ocsp_status_responder_name_matches_selected_certificate".to_string());
                Some(HostAdapterOcspResponderIdMode::ByName)
            }
            Ok(_) => {
                reasons.push("ocsp_status_responder_name_mismatch".to_string());
                None
            }
            Err(err) => {
                reasons.push(format!("ocsp_status_responder_name_encode_failed:{err}"));
                None
            }
        },
        ResponderId::ByKey(key_hash) => {
            let responder_key_hash =
                sha1_digest(responder.public_key().subject_public_key.data.as_ref());
            if key_hash.as_ref() == responder_key_hash.as_slice() {
                verified_evidence
                    .push("ocsp_status_responder_key_matches_selected_certificate".to_string());
                Some(HostAdapterOcspResponderIdMode::ByKey)
            } else {
                reasons.push("ocsp_status_responder_key_mismatch".to_string());
                None
            }
        }
    }
}

/// Return the RFC 6960 responder key hash used by responderID byKey.
pub(crate) fn ocsp_responder_public_key_sha1_hex(responder: &X509Certificate<'_>) -> String {
    hex_bytes(&sha1_digest(
        responder.public_key().subject_public_key.data.as_ref(),
    ))
}

fn distinguished_names_match(encoded_name: &[u8], certificate_name: &X509Name<'_>) -> bool {
    let Ok((remaining, responder_name)) = X509Name::from_der(encoded_name) else {
        return false;
    };
    remaining.is_empty()
        && normalized_distinguished_name(&responder_name)
            == normalized_distinguished_name(certificate_name)
}

fn normalized_distinguished_name(name: &X509Name<'_>) -> Vec<Vec<(String, String)>> {
    name.iter_rdn()
        .map(|rdn| {
            let mut attributes = rdn
                .iter()
                .map(|attribute| {
                    let value = attribute.as_str().map_or_else(
                        |_| hex_bytes(attribute.as_slice()),
                        normalize_directory_string,
                    );
                    (attribute.attr_type().to_id_string(), value)
                })
                .collect::<Vec<_>>();
            attributes.sort_unstable();
            attributes
        })
        .collect()
}

fn normalize_directory_string(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Validate supplied delegated-responder authority before its key can verify a
/// `BasicOCSPResponse`. The supplied chain is exact and ordered from the
/// responder's immediate issuer toward, but excluding, the target issuer.
pub(crate) fn verify_delegated_ocsp_responder_authority(
    basic_response: &BasicOcspResponse,
    responder: &X509Certificate<'_>,
    issuer_chain: &[X509Certificate<'_>],
    target_issuer: &X509Certificate<'_>,
    verification_time_unix: i64,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<crate::host_adapter_types::HostAdapterOcspResponderIdMode> {
    let reason_count_before = reasons.len();
    let responder_validity = responder.validity();
    if verification_time_unix >= responder_validity.not_before.timestamp()
        && verification_time_unix <= responder_validity.not_after.timestamp()
    {
        verified_evidence
            .push("ocsp_status_delegated_responder_valid_at_verification_time".to_string());
    } else {
        reasons.push("ocsp_status_delegated_responder_not_valid_at_verification_time".to_string());
    }

    match responder.extended_key_usage() {
        Ok(Some(eku)) if eku.value.ocsp_signing => {
            verified_evidence.push("ocsp_status_delegated_responder_ocsp_signing_eku".to_string());
        }
        Ok(Some(_)) => {
            reasons.push("ocsp_status_delegated_responder_ocsp_signing_eku_missing".to_string());
        }
        Ok(None) => reasons.push("ocsp_status_delegated_responder_eku_missing".to_string()),
        Err(err) => reasons.push(format!("ocsp_status_delegated_responder_eku_invalid:{err}")),
    }

    match responder.key_usage() {
        Ok(Some(usage)) if usage.value.digital_signature() => verified_evidence
            .push("ocsp_status_delegated_responder_digital_signature_usage".to_string()),
        Ok(Some(_)) => reasons
            .push("ocsp_status_delegated_responder_digital_signature_usage_missing".to_string()),
        Ok(None) => {
            verified_evidence.push("ocsp_status_delegated_responder_key_usage_absent".to_string());
        }
        Err(err) => reasons.push(format!(
            "ocsp_status_delegated_responder_key_usage_invalid:{err}"
        )),
    }

    validate_ocsp_path_certificate_extensions(responder, "delegated_responder", 0, reasons);

    if let Some(certs) = basic_response.certs.as_ref() {
        if certs.len() == 1 {
            match rasn::der::encode(&certs[0]) {
                Ok(embedded_der) if embedded_der == responder.as_raw() => verified_evidence.push(
                    "ocsp_status_embedded_responder_certificate_matches_supplied".to_string(),
                ),
                Ok(_) => {
                    reasons.push("ocsp_status_embedded_responder_certificate_mismatch".to_string());
                }
                Err(err) => reasons.push(format!(
                    "ocsp_status_embedded_responder_certificate_encode_failed:{err}"
                )),
            }
        } else {
            reasons
                .push("ocsp_status_embedded_responder_certificate_count_unsupported".to_string());
        }
    } else {
        verified_evidence
            .push("ocsp_status_responder_certificate_supplied_out_of_band".to_string());
    }

    if issuer_chain
        .iter()
        .any(|certificate| certificate.as_raw() == responder.as_raw())
        || issuer_chain.iter().enumerate().any(|(index, certificate)| {
            issuer_chain
                .iter()
                .skip(index + 1)
                .any(|other| other.as_raw() == certificate.as_raw())
        })
    {
        reasons.push("ocsp_status_delegated_responder_path_duplicate_certificate".to_string());
    }
    if issuer_chain
        .iter()
        .any(|certificate| certificate.as_raw() == target_issuer.as_raw())
    {
        reasons.push("ocsp_status_delegated_responder_path_includes_terminal_issuer".to_string());
    }

    let mut child = responder;
    for (index, chain_issuer) in issuer_chain.iter().enumerate() {
        let subordinate_ca_count = issuer_chain[..index]
            .iter()
            .filter(|certificate| certificate.subject() != certificate.issuer())
            .count();
        verify_ocsp_responder_path_link(
            child,
            chain_issuer,
            index,
            subordinate_ca_count,
            verification_time_unix,
            verified_evidence,
            reasons,
        );
        child = chain_issuer;
    }
    let subordinate_ca_count = issuer_chain
        .iter()
        .filter(|certificate| certificate.subject() != certificate.issuer())
        .count();
    verify_ocsp_responder_path_link(
        child,
        target_issuer,
        issuer_chain.len(),
        subordinate_ca_count,
        verification_time_unix,
        verified_evidence,
        reasons,
    );

    let responder_id_mode = ocsp_responder_id_matches_certificate(
        &basic_response.tbs_response_data.responder_id,
        responder,
        verified_evidence,
        reasons,
    );
    if responder_id_mode.is_some() && reasons.len() == reason_count_before {
        responder_id_mode
    } else {
        None
    }
}

fn verify_ocsp_responder_path_link(
    child: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
    index: usize,
    subordinate_ca_count: usize,
    verification_time_unix: i64,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    if child.issuer() == issuer.subject() {
        verified_evidence.push(format!(
            "ocsp_status_delegated_path_issuer_subject_match_{index}"
        ));
    } else {
        reasons.push(format!(
            "ocsp_status_delegated_path_issuer_subject_mismatch_{index}"
        ));
    }
    match child.verify_signature(Some(issuer.public_key())) {
        Ok(()) => verified_evidence.push(format!(
            "ocsp_status_delegated_path_signature_verified_{index}"
        )),
        Err(err) => reasons.push(format!(
            "ocsp_status_delegated_path_signature_failed_{index}:{err}"
        )),
    }

    let validity = issuer.validity();
    if verification_time_unix >= validity.not_before.timestamp()
        && verification_time_unix <= validity.not_after.timestamp()
    {
        verified_evidence.push(format!("ocsp_status_delegated_path_issuer_valid_{index}"));
    } else {
        reasons.push(format!(
            "ocsp_status_delegated_path_issuer_not_valid_{index}"
        ));
    }

    match issuer.basic_constraints() {
        Ok(Some(constraints)) if constraints.value.ca => {
            verified_evidence.push(format!("ocsp_status_delegated_path_issuer_ca_{index}"));
            if constraints
                .value
                .path_len_constraint
                .is_some_and(|limit| subordinate_ca_count > limit as usize)
            {
                reasons.push(format!(
                    "ocsp_status_delegated_path_length_exceeded_{index}"
                ));
            }
        }
        Ok(Some(_)) => reasons.push(format!("ocsp_status_delegated_path_issuer_not_ca_{index}")),
        Ok(None) => reasons.push(format!(
            "ocsp_status_delegated_path_issuer_basic_constraints_missing_{index}"
        )),
        Err(err) => reasons.push(format!(
            "ocsp_status_delegated_path_issuer_basic_constraints_invalid_{index}:{err}"
        )),
    }
    validate_ocsp_path_certificate_extensions(
        issuer,
        &format!("delegated_path_issuer_{index}"),
        subordinate_ca_count,
        reasons,
    );

    match issuer.key_usage() {
        Ok(Some(usage)) if usage.value.key_cert_sign() => verified_evidence.push(format!(
            "ocsp_status_delegated_path_issuer_key_cert_sign_{index}"
        )),
        Ok(Some(_)) => reasons.push(format!(
            "ocsp_status_delegated_path_issuer_key_cert_sign_missing_{index}"
        )),
        Ok(None) => reasons.push(format!(
            "ocsp_status_delegated_path_issuer_key_usage_missing_{index}"
        )),
        Err(err) => reasons.push(format!(
            "ocsp_status_delegated_path_issuer_key_usage_invalid_{index}:{err}"
        )),
    }
}

fn validate_ocsp_path_certificate_extensions(
    certificate: &X509Certificate<'_>,
    label: &str,
    _subordinate_ca_count: usize,
    reasons: &mut Vec<String>,
) {
    let mut seen_oids = HashSet::new();
    for extension in certificate.extensions() {
        let oid = extension.oid.to_id_string();
        if !seen_oids.insert(oid.clone()) {
            reasons.push(format!("ocsp_status_{label}_duplicate_extension:{oid}"));
        }
        if matches!(
            extension.parsed_extension(),
            ParsedExtension::NameConstraints(_)
        ) {
            reasons.push(format!("ocsp_status_{label}_name_constraints_unsupported"));
        }
        if extension.critical
            && (extension.parsed_extension().unsupported()
                || extension.parsed_extension().error().is_some())
        {
            reasons.push(format!(
                "ocsp_status_{label}_critical_extension_unsupported:{oid}"
            ));
        }
    }
}

/// Find the single response in the basic OCSP response matching the certificate.
pub(crate) fn find_matching_ocsp_single_response<'a>(
    basic_response: &'a BasicOcspResponse,
    certificate: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<&'a SingleResponse> {
    let certificate_serial_decimal = certificate.tbs_certificate.serial.to_string();
    let mut saw_serial = false;
    let mut saw_supported_hash = false;
    let mut saw_issuer_hash_match = false;

    for single_response in &basic_response.tbs_response_data.responses {
        let serial_matches =
            single_response.cert_id.serial_number.to_string() == certificate_serial_decimal;
        if serial_matches {
            saw_serial = true;
        }
        match ocsp_cert_id_issuer_hashes_match(&single_response.cert_id, issuer) {
            Some(true) => {
                saw_supported_hash = true;
                saw_issuer_hash_match = true;
                if serial_matches {
                    verified_evidence
                        .push("ocsp_status_cert_id_serial_and_issuer_hash_match".to_string());
                    return Some(single_response);
                }
            }
            Some(false) => saw_supported_hash = true,
            None => {}
        }
    }

    if !saw_supported_hash {
        reasons.push("ocsp_status_cert_id_hash_algorithm_unsupported".to_string());
    }
    if !saw_serial {
        reasons.push("ocsp_status_certificate_serial_not_found".to_string());
    }
    if !saw_issuer_hash_match {
        reasons.push("ocsp_status_issuer_hash_mismatch".to_string());
    }
    reasons.push("ocsp_status_single_response_match_missing".to_string());
    None
}

/// Check whether the cert ID issuer name/key hashes match the issuer certificate.
fn ocsp_cert_id_issuer_hashes_match(
    cert_id: &CertId,
    issuer: &X509Certificate<'_>,
) -> Option<bool> {
    let issuer_name_hash =
        ocsp_digest_for_algorithm(&cert_id.hash_algorithm.algorithm, issuer.subject().as_raw())?;
    let issuer_key_hash = ocsp_digest_for_algorithm(
        &cert_id.hash_algorithm.algorithm,
        issuer.public_key().subject_public_key.data.as_ref(),
    )?;
    Some(
        cert_id.issuer_name_hash.as_ref() == issuer_name_hash.as_slice()
            && cert_id.issuer_key_hash.as_ref() == issuer_key_hash.as_slice(),
    )
}

/// Verify that the single response freshness window covers the verification time.
pub(crate) fn verify_ocsp_single_response_freshness(
    single_response: &SingleResponse,
    verification_time_unix: i64,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let this_update = single_response.this_update.timestamp();
    if verification_time_unix >= this_update {
        verified_evidence.push("ocsp_status_this_update_not_in_future".to_string());
    } else {
        reasons.push("ocsp_status_this_update_in_future".to_string());
    }
    if let Some(next_update) = single_response
        .next_update
        .as_ref()
        .map(|time| time.timestamp())
    {
        if verification_time_unix <= next_update {
            verified_evidence.push("ocsp_status_next_update_not_expired".to_string());
        } else {
            reasons.push("ocsp_status_response_expired".to_string());
        }
    } else {
        reasons.push("ocsp_status_next_update_missing".to_string());
    }
}

/// Apply the OCSP certificate status to the output revocation fields.
pub(crate) fn apply_ocsp_cert_status(
    cert_status: &CertStatus,
    revocation_status: &mut Option<String>,
    revoked_at_unix: &mut Option<i64>,
    revocation_reason: &mut Option<String>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    match cert_status {
        CertStatus::Good => {
            *revocation_status = Some("good_by_supplied_ocsp".to_string());
            verified_evidence.push("ocsp_status_certificate_good".to_string());
        }
        CertStatus::Revoked(info) => {
            *revocation_status = Some("revoked_by_supplied_ocsp".to_string());
            *revoked_at_unix = Some(info.revocation_time.timestamp());
            *revocation_reason = info
                .revocation_reason
                .as_ref()
                .map(|reason| format!("{reason:?}"));
            reasons.push("ocsp_status_certificate_revoked".to_string());
        }
        CertStatus::Unknown(()) => {
            *revocation_status = Some("unknown_by_supplied_ocsp".to_string());
            reasons.push("ocsp_status_certificate_unknown".to_string());
        }
    }
}

/// Extract the nonce hex from the OCSP response extensions, if present.
pub(crate) fn extract_ocsp_response_nonce_hex(
    basic_response: &BasicOcspResponse,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> Option<String> {
    let Some(extensions) = basic_response
        .tbs_response_data
        .response_extensions
        .as_ref()
    else {
        return None;
    };
    let mut seen_oids = HashSet::new();
    let mut nonce = None;
    for extension in extensions.iter() {
        let oid = extension.extn_id.to_string();
        if !seen_oids.insert(oid.clone()) {
            reasons.push(format!("ocsp_status_response_extension_duplicate:{oid}"));
            continue;
        }
        if rasn_oid_matches(&extension.extn_id, &[1, 3, 6, 1, 5, 5, 7, 48, 1, 2]) {
            nonce = match rasn::der::decode::<rasn_ocsp::Nonce>(extension.extn_value.as_ref()) {
                Ok(value) => {
                    verified_evidence.push("ocsp_status_nonce_observed".to_string());
                    Some(hex_bytes(value.as_ref()))
                }
                Err(err) => {
                    reasons.push(format!("ocsp_status_nonce_parse_failed:{err}"));
                    None
                }
            };
        } else if extension.critical {
            reasons.push(format!(
                "ocsp_status_response_critical_extension_unsupported:{oid}"
            ));
        }
    }
    nonce
}

pub(crate) fn validate_ocsp_single_response_extensions(
    single_response: &SingleResponse,
    reasons: &mut Vec<String>,
) {
    let Some(extensions) = single_response.single_extensions.as_ref() else {
        return;
    };
    let mut seen_oids = HashSet::new();
    for extension in extensions.iter() {
        let oid = extension.extn_id.to_string();
        if !seen_oids.insert(oid.clone()) {
            reasons.push(format!("ocsp_status_single_extension_duplicate:{oid}"));
        }
        if extension.critical {
            reasons.push(format!(
                "ocsp_status_single_critical_extension_unsupported:{oid}"
            ));
        }
    }
}

/// Verify the OCSP nonce against the expected value.
pub(crate) fn verify_ocsp_nonce(
    expected_nonce_hex: Option<&str>,
    observed_nonce_hex: Option<&str>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    match (expected_nonce_hex, observed_nonce_hex) {
        (Some(expected), Some(observed))
            if crate::hashing::constant_time_eq_hex(expected, observed) =>
        {
            verified_evidence.push("ocsp_status_nonce_verified".to_string());
        }
        (Some(_expected), Some(_observed)) => {
            reasons.push("ocsp_status_nonce_mismatch".to_string());
        }
        (Some(_expected), None) => {
            reasons.push("ocsp_status_nonce_missing".to_string());
        }
        (None, Some(_observed)) => {
            verified_evidence.push("ocsp_status_nonce_present_without_expectation".to_string());
        }
        (None, None) => {
            verified_evidence.push("ocsp_status_nonce_not_supplied".to_string());
        }
    }
}

/// Normalize an expected OCSP nonce hex string (strip separators, lowercase).
#[allow(clippy::manual_is_multiple_of)] // `is_multiple_of` is unstable on the Rust 1.85 MSRV.
pub(crate) fn normalize_expected_ocsp_nonce_hex(
    value: &str,
    reasons: &mut Vec<String>,
) -> Option<String> {
    let mut normalized = String::new();
    for character in value.chars() {
        if character.is_ascii_hexdigit() {
            normalized.push(character.to_ascii_lowercase());
        } else if !matches!(character, ':' | '-' | ' ' | '\t' | '\n' | '\r') {
            reasons.push("ocsp_status_expected_nonce_hex_invalid".to_string());
            return None;
        }
    }
    if normalized.is_empty() || normalized.len() % 2 != 0 {
        reasons.push("ocsp_status_expected_nonce_hex_invalid".to_string());
        None
    } else {
        Some(normalized)
    }
}

/// Compute a digest using the algorithm identified by the OID.
fn ocsp_digest_for_algorithm(
    algorithm: &RasnObjectIdentifier,
    content: &[u8],
) -> Option<Zeroizing<Vec<u8>>> {
    match algorithm.as_ref() {
        [1, 3, 14, 3, 2, 26] => Some(sha1_digest(content)),
        [2, 16, 840, 1, 101, 3, 4, 2, 1] => {
            let mut hasher = Sha256::new();
            hasher.update(content);
            Some(Zeroizing::new(hasher.finalize().to_vec()))
        }
        [2, 16, 840, 1, 101, 3, 4, 2, 2] => {
            let mut hasher = Sha384::new();
            hasher.update(content);
            Some(Zeroizing::new(hasher.finalize().to_vec()))
        }
        [2, 16, 840, 1, 101, 3, 4, 2, 3] => {
            let mut hasher = Sha512::new();
            hasher.update(content);
            Some(Zeroizing::new(hasher.finalize().to_vec()))
        }
        _ => None,
    }
}

fn sha1_digest(content: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut hasher = Sha1::new();
    hasher.update(content);
    Zeroizing::new(hasher.finalize().to_vec())
}

/// Check whether a RASN OID matches the expected OID arc.
pub(crate) fn rasn_oid_matches(oid: &RasnObjectIdentifier, expected: &[u32]) -> bool {
    oid.as_ref() == expected
}

#[cfg(test)]
mod tests {
    //! Unit coverage for the OCSP helpers in this module.
    //!
    //! Coverage strategy: the public entrypoint
    //! `run_host_adapter_certificate_ocsp_status_verification` (in
    //! `host_adapter_verification.rs`) is already covered end-to-end by 17
    //! integration tests in `forge-core-cli/tests/validate.rs` (good/revoked/
    //! unknown/expired/future/nonce/sig/responder-mismatch). Those tests mint
    //! real DER OCSP responses signed via rcgen. We do NOT duplicate that
    //! here. Instead these unit tests exercise each `pub(crate)` helper's
    //! contract in isolation by constructing `rasn-ocsp` structs directly in
    //! Rust (no DER signing), so a regression in a specific helper fails close
    //! to the point of failure rather than in a far-off E2E assertion.
    //!
    //! Error style of this module: `Vec<String>` accumulation (neither `Result`
    //! nor an enum). Tests assert on the returned `Option`/`bool` AND on the
    //! contents of the `verified_evidence` / `reasons` vectors.

    use super::{
        apply_ocsp_cert_status, decode_basic_ocsp_response, decode_ocsp_response,
        extract_ocsp_response_nonce_hex, find_matching_ocsp_single_response,
        normalize_expected_ocsp_nonce_hex, ocsp_responder_id_matches_issuer,
        ocsp_responder_public_key_sha1_hex, rasn_oid_matches,
        validate_ocsp_single_response_extensions, verify_basic_ocsp_signature_with_certificate,
        verify_basic_ocsp_signature_with_issuer, verify_delegated_ocsp_responder_authority,
        verify_ocsp_nonce, verify_ocsp_single_response_freshness,
    };
    use asn1_rs::FromDer as _;
    use chrono::TimeZone as _;
    use rasn::types::{
        BitString, GeneralizedTime, ObjectIdentifier as RasnOid, OctetString as RasnOctetString,
    };
    use rasn_ocsp::{
        BasicOcspResponse, CertId, CertStatus, OcspResponse, OcspResponseStatus, ResponderId,
        ResponseBytes, ResponseData, RevokedInfo, SingleResponse,
    };
    use rasn_pkix::{
        AlgorithmIdentifier as RasnAlgorithmIdentifier, CrlReason, Extension, Extensions,
    };
    use rcgen::{
        CertificateParams, CustomExtension, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
        KeyUsagePurpose, SigningKey as _,
    };
    use sha1::Digest as _;
    use x509_parser::certificate::X509Certificate;

    /// OCSP nonce OID: `1.3.6.1.5.5.7.48.1.2`.
    const NONCE_OID: [u32; 10] = [1, 3, 6, 1, 5, 5, 7, 48, 1, 2];
    /// SHA-256 OID: `2.16.840.1.101.3.4.2.1`.
    const SHA256_OID: [u32; 9] = [2, 16, 840, 1, 101, 3, 4, 2, 1];

    // ---- fixture builders ------------------------------------------------

    /// Construct a `GeneralizedTime` (= `chrono::DateTime<FixedOffset>`) from a
    /// unix timestamp. OCSP times are UTC.
    fn generalized_time(unix_secs: i64) -> GeneralizedTime {
        chrono::Utc
            .timestamp_opt(unix_secs, 0)
            .single()
            .expect("valid unix timestamp")
            .fixed_offset()
    }

    /// A `CertId` with synthetic hashes (real responses carry SHA-1/SHA-256 of
    /// the issuer name/key; tests use fixed bytes for determinism).
    fn synthetic_cert_id(
        hash_oid: &[u32],
        name_hash: &[u8],
        key_hash: &[u8],
        serial_decimal: u64,
    ) -> CertId {
        CertId {
            hash_algorithm: RasnAlgorithmIdentifier {
                algorithm: RasnOid::new_unchecked(hash_oid.to_vec().into()),
                parameters: None,
            },
            issuer_name_hash: RasnOctetString::from(name_hash.to_vec()),
            issuer_key_hash: RasnOctetString::from(key_hash.to_vec()),
            serial_number: serial_decimal.into(),
        }
    }

    /// A `SingleResponse` over `[this_update, next_update]`.
    fn single_response(
        cert_id: CertId,
        status: CertStatus,
        this_update_unix: i64,
        next_update_unix: Option<i64>,
    ) -> SingleResponse {
        SingleResponse {
            cert_id,
            cert_status: status,
            this_update: generalized_time(this_update_unix),
            next_update: next_update_unix.map(generalized_time),
            single_extensions: None,
        }
    }

    /// A minimal `BasicOcspResponse` with `ResponderId::ByKey` (avoids needing
    /// the complex X.500 `Name` type) and a synthetic (invalid) signature.
    fn basic_ocsp_response(
        responder_key_hash: Vec<u8>,
        responses: Vec<SingleResponse>,
        response_extensions: Option<Extensions>,
    ) -> BasicOcspResponse {
        BasicOcspResponse {
            tbs_response_data: ResponseData {
                version: 0u32.into(),
                responder_id: ResponderId::ByKey(RasnOctetString::from(responder_key_hash)),
                produced_at: generalized_time(1_783_391_200),
                responses,
                response_extensions,
            },
            signature_algorithm: RasnAlgorithmIdentifier {
                algorithm: RasnOid::new_unchecked(vec![1, 2, 840, 10045, 4, 3, 2].into()),
                parameters: None,
            },
            signature: BitString::from_vec(vec![0u8; 64]),
            certs: None,
        }
    }

    /// Mint a self-signed CA cert via rcgen and parse it back. Returns the
    /// parsed `X509Certificate` borrowing a leaked (static) DER slice.
    fn test_issuer_cert(common_name: &str) -> X509Certificate<'static> {
        let mut params = CertificateParams::new(Vec::new()).expect("empty SAN CA params");
        params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, common_name);
        params.key_usages.push(KeyUsagePurpose::DigitalSignature);
        params.key_usages.push(KeyUsagePurpose::KeyCertSign);
        // Pin a small serial so tests that compare the serial against a u64
        // (the `find_matching_ocsp_single_response` happy path) don't overflow.
        // rcgen's default is a large random serial.
        params.serial_number = Some(rcgen::SerialNumber::from(0x1234_u64));
        params.not_before = rcgen::date_time_ymd(2026, 1, 1);
        params.not_after = rcgen::date_time_ymd(2027, 1, 1);
        let key_pair = KeyPair::generate().expect("generate CA key");
        let certificate = params.self_signed(&key_pair).expect("self-sign CA");
        let der = certificate.der().to_vec();
        let leaked: &'static [u8] = Box::leak(der.into_boxed_slice());
        let (_, parsed) = X509Certificate::from_der(leaked).expect("parse CA DER");
        parsed
    }

    fn test_delegated_responder(
        eku: ExtendedKeyUsagePurpose,
        not_before: (i32, u8, u8),
        not_after: (i32, u8, u8),
    ) -> (X509Certificate<'static>, X509Certificate<'static>, KeyPair) {
        let mut issuer_params =
            CertificateParams::new(Vec::new()).expect("empty SAN issuer params");
        issuer_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "Forge Delegated OCSP Issuer");
        issuer_params
            .key_usages
            .push(KeyUsagePurpose::DigitalSignature);
        issuer_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
        issuer_params.not_before = rcgen::date_time_ymd(2026, 1, 1);
        issuer_params.not_after = rcgen::date_time_ymd(2028, 1, 1);
        let issuer_key = KeyPair::generate().expect("generate delegated issuer key");
        let issuer_certificate = issuer_params
            .self_signed(&issuer_key)
            .expect("self-sign delegated issuer");

        let mut responder_params =
            CertificateParams::new(Vec::new()).expect("empty SAN responder params");
        responder_params
            .distinguished_name
            .push(DnType::CommonName, "Forge Delegated OCSP Responder");
        responder_params
            .key_usages
            .push(KeyUsagePurpose::DigitalSignature);
        responder_params.extended_key_usages.push(eku);
        responder_params.not_before =
            rcgen::date_time_ymd(not_before.0, not_before.1, not_before.2);
        responder_params.not_after = rcgen::date_time_ymd(not_after.0, not_after.1, not_after.2);
        let responder_key = KeyPair::generate().expect("generate delegated responder key");
        let issuer = Issuer::from_params(&issuer_params, &issuer_key);
        let responder_certificate = responder_params
            .signed_by(&responder_key, &issuer)
            .expect("sign delegated responder");

        let issuer_der: &'static [u8] =
            Box::leak(issuer_certificate.der().to_vec().into_boxed_slice());
        let responder_der: &'static [u8] =
            Box::leak(responder_certificate.der().to_vec().into_boxed_slice());
        let (_, issuer) = X509Certificate::from_der(issuer_der).expect("parse delegated issuer");
        let (_, responder) =
            X509Certificate::from_der(responder_der).expect("parse delegated responder");
        (issuer, responder, responder_key)
    }

    fn sha1_of(content: &[u8]) -> Vec<u8> {
        let mut hasher = sha1::Sha1::new();
        hasher.update(content);
        hasher.finalize().to_vec()
    }

    fn sha256_of(content: &[u8]) -> Vec<u8> {
        let mut hasher = sha2::Sha256::new();
        hasher.update(content);
        hasher.finalize().to_vec()
    }

    // ---- rasn_oid_matches ------------------------------------------------

    #[test]
    fn oid_matches_for_identical_arcs() {
        let oid = RasnOid::new_unchecked(NONCE_OID.to_vec().into());
        assert!(rasn_oid_matches(&oid, &NONCE_OID));
    }

    #[test]
    fn oid_rejects_prefix_only_match() {
        let oid = RasnOid::new_unchecked(NONCE_OID.to_vec().into());
        assert!(!rasn_oid_matches(&oid, &NONCE_OID[..NONCE_OID.len() - 1]));
    }

    #[test]
    fn oid_rejects_different_arcs() {
        let oid = RasnOid::new_unchecked(SHA256_OID.to_vec().into());
        assert!(!rasn_oid_matches(&oid, &[1, 3, 14, 3, 2, 26]));
    }

    // ---- decode_ocsp_response / decode_basic_ocsp_response ---------------

    #[test]
    fn decode_ocsp_response_rejects_invalid_der() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let result = decode_ocsp_response(b"not valid der", &mut evidence, &mut reasons);

        assert!(result.is_none());
        assert!(reasons
            .iter()
            .any(|r| r.starts_with("ocsp_status_response_parse_failed")));
        assert!(evidence.is_empty());
    }

    #[test]
    fn decode_ocsp_response_round_trips_valid_response() {
        let response = OcspResponse {
            status: OcspResponseStatus::Successful,
            bytes: None,
        };
        let der = rasn::der::encode(&response).expect("encode OcspResponse");

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let decoded = decode_ocsp_response(&der, &mut evidence, &mut reasons).expect("decodes");

        assert_eq!(decoded.status, OcspResponseStatus::Successful);
        assert!(evidence.contains(&"ocsp_status_response_parsed".to_string()));
        assert!(reasons.is_empty());
    }

    #[test]
    fn decode_ocsp_response_with_response_bytes_round_trips() {
        let response = OcspResponse {
            status: OcspResponseStatus::Successful,
            bytes: Some(ResponseBytes {
                r#type: RasnOid::new_unchecked(vec![1, 3, 6, 1, 5, 5, 7, 48, 1, 1].into()),
                response: RasnOctetString::from(b"placeholder".as_slice()),
            }),
        };
        let der = rasn::der::encode(&response).expect("encode OcspResponse with bytes");

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let decoded = decode_ocsp_response(&der, &mut evidence, &mut reasons).expect("decodes");

        assert!(decoded.bytes.is_some());
        assert_eq!(decoded.status, OcspResponseStatus::Successful);
    }

    #[test]
    fn decode_basic_ocsp_response_rejects_invalid_der() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let result = decode_basic_ocsp_response(b"", &mut evidence, &mut reasons);

        assert!(result.is_none());
        assert!(reasons
            .iter()
            .any(|r| r.starts_with("ocsp_status_basic_response_parse_failed")));
    }

    #[test]
    fn decode_basic_ocsp_response_round_trips_valid_response() {
        let cert_id = synthetic_cert_id(&SHA256_OID, &[1; 32], &[2; 32], 42);
        let response = single_response(
            cert_id,
            CertStatus::Good,
            1_783_391_200,
            Some(1_783_477_600),
        );
        let basic = basic_ocsp_response(vec![0xaa; 20], vec![response], None);
        let der = rasn::der::encode(&basic).expect("encode BasicOcspResponse");

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let decoded =
            decode_basic_ocsp_response(&der, &mut evidence, &mut reasons).expect("decodes");

        assert_eq!(decoded.tbs_response_data.responses.len(), 1);
        assert!(evidence.contains(&"ocsp_status_basic_response_parsed".to_string()));
    }

    // ---- verify_ocsp_single_response_freshness ---------------------------

    #[test]
    fn freshness_passes_when_verification_time_is_in_window() {
        let cert_id = synthetic_cert_id(&SHA256_OID, &[0; 32], &[0; 32], 1);
        let response = single_response(cert_id, CertStatus::Good, 1000, Some(2000));

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_single_response_freshness(&response, 1500, &mut evidence, &mut reasons);

        assert!(evidence.contains(&"ocsp_status_this_update_not_in_future".to_string()));
        assert!(evidence.contains(&"ocsp_status_next_update_not_expired".to_string()));
        assert!(reasons.is_empty());
    }

    #[test]
    fn freshness_fails_when_this_update_is_in_future() {
        let cert_id = synthetic_cert_id(&SHA256_OID, &[0; 32], &[0; 32], 1);
        let response = single_response(cert_id, CertStatus::Good, 2000, Some(3000));

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_single_response_freshness(&response, 1000, &mut evidence, &mut reasons);

        assert!(reasons.contains(&"ocsp_status_this_update_in_future".to_string()));
    }

    #[test]
    fn freshness_fails_when_next_update_expired() {
        let cert_id = synthetic_cert_id(&SHA256_OID, &[0; 32], &[0; 32], 1);
        let response = single_response(cert_id, CertStatus::Good, 1000, Some(2000));

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_single_response_freshness(&response, 2500, &mut evidence, &mut reasons);

        assert!(reasons.contains(&"ocsp_status_response_expired".to_string()));
    }

    #[test]
    fn freshness_fails_when_next_update_missing() {
        let cert_id = synthetic_cert_id(&SHA256_OID, &[0; 32], &[0; 32], 1);
        let response = single_response(cert_id, CertStatus::Good, 1000, None);

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_single_response_freshness(&response, 1500, &mut evidence, &mut reasons);

        assert!(reasons.contains(&"ocsp_status_next_update_missing".to_string()));
    }

    // ---- apply_ocsp_cert_status ------------------------------------------

    #[test]
    fn apply_cert_status_good_marks_revocation_status_good() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let mut revocation_status = None;
        let mut revoked_at = None;
        let mut reason = None;

        apply_ocsp_cert_status(
            &CertStatus::Good,
            &mut revocation_status,
            &mut revoked_at,
            &mut reason,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(revocation_status.as_deref(), Some("good_by_supplied_ocsp"));
        assert!(evidence.contains(&"ocsp_status_certificate_good".to_string()));
        assert!(revoked_at.is_none());
        assert!(reason.is_none());
    }

    #[test]
    fn apply_cert_status_revoked_sets_revoked_at_and_reason() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let mut revocation_status = None;
        let mut revoked_at = None;
        let mut reason = None;

        apply_ocsp_cert_status(
            &CertStatus::Revoked(RevokedInfo {
                revocation_time: generalized_time(1_763_232_400),
                revocation_reason: Some(CrlReason::KeyCompromise),
            }),
            &mut revocation_status,
            &mut revoked_at,
            &mut reason,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(
            revocation_status.as_deref(),
            Some("revoked_by_supplied_ocsp")
        );
        assert_eq!(revoked_at, Some(1_763_232_400));
        assert!(reasons.contains(&"ocsp_status_certificate_revoked".to_string()));
        assert!(reason
            .as_deref()
            .is_some_and(|r| r.contains("KeyCompromise")));
    }

    #[test]
    fn apply_cert_status_unknown_marks_unknown() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let mut revocation_status = None;
        let mut revoked_at = None;
        let mut reason = None;

        apply_ocsp_cert_status(
            &CertStatus::Unknown(()),
            &mut revocation_status,
            &mut revoked_at,
            &mut reason,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(
            revocation_status.as_deref(),
            Some("unknown_by_supplied_ocsp")
        );
        assert!(reasons.contains(&"ocsp_status_certificate_unknown".to_string()));
        assert!(revoked_at.is_none());
    }

    // ---- extract_ocsp_response_nonce_hex ---------------------------------

    fn nonce_extension(nonce_bytes: &[u8]) -> Extension {
        // The production decoder reads the nonce via
        // `rasn::der::decode::<Nonce>(extn_value)` where `Nonce = OctetString`.
        // That means the extn_value itself must be the DER encoding of an
        // OCTET STRING wrapping the raw nonce bytes (double-wrapping, matching
        // `der_ocsp_nonce_extension` in validate.rs:766-771). We encode the
        // inner OCTET STRING here so the decode in production succeeds.
        let inner = RasnOctetString::from(nonce_bytes.to_vec());
        let encoded = rasn::der::encode(&inner).expect("encode nonce octet string");
        Extension {
            extn_id: RasnOid::new_unchecked(NONCE_OID.to_vec().into()),
            critical: false,
            extn_value: RasnOctetString::from(encoded),
        }
    }

    #[test]
    fn extract_nonce_returns_hex_when_nonce_extension_present() {
        let basic = basic_ocsp_response(
            vec![0xaa; 20],
            Vec::new(),
            Some(Extensions::from(vec![nonce_extension(&[
                0xde, 0xad, 0xbe, 0xef,
            ])])),
        );

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let nonce_hex =
            extract_ocsp_response_nonce_hex(&basic, &mut evidence, &mut reasons).expect("nonce");

        assert_eq!(nonce_hex, "deadbeef");
        assert!(evidence.contains(&"ocsp_status_nonce_observed".to_string()));
    }

    #[test]
    fn extract_nonce_returns_none_when_extensions_absent() {
        let basic = basic_ocsp_response(vec![0xaa; 20], Vec::new(), None);

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        assert!(extract_ocsp_response_nonce_hex(&basic, &mut evidence, &mut reasons).is_none());
    }

    #[test]
    fn extract_nonce_rejects_unknown_critical_response_extension() {
        let extension = Extension {
            extn_id: RasnOid::new_unchecked(vec![1, 2, 3].into()),
            critical: true,
            extn_value: RasnOctetString::from(b"misc".as_slice()),
        };
        let basic = basic_ocsp_response(
            vec![0xaa; 20],
            Vec::new(),
            Some(Extensions::from(vec![extension])),
        );
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        assert!(extract_ocsp_response_nonce_hex(&basic, &mut evidence, &mut reasons).is_none());
        assert!(reasons.iter().any(
            |reason| reason.starts_with("ocsp_status_response_critical_extension_unsupported:")
        ));
    }

    #[test]
    fn single_response_rejects_unknown_critical_extension() {
        let mut response = single_response(
            synthetic_cert_id(&SHA256_OID, &[0; 32], &[0; 32], 1),
            CertStatus::Good,
            1000,
            Some(2000),
        );
        response.single_extensions = Some(Extensions::from(vec![Extension {
            extn_id: RasnOid::new_unchecked(vec![1, 2, 3].into()),
            critical: true,
            extn_value: RasnOctetString::from(b"misc".as_slice()),
        }]));
        let mut reasons = Vec::new();

        validate_ocsp_single_response_extensions(&response, &mut reasons);

        assert!(
            reasons
                .iter()
                .any(|reason| reason
                    .starts_with("ocsp_status_single_critical_extension_unsupported:"))
        );
    }

    #[test]
    fn extract_nonce_returns_none_when_no_nonce_oid_present() {
        let extension = Extension {
            extn_id: RasnOid::new_unchecked(vec![1, 2, 3].into()),
            critical: false,
            extn_value: RasnOctetString::from(b"misc".as_slice()),
        };
        let basic = basic_ocsp_response(
            vec![0xaa; 20],
            Vec::new(),
            Some(Extensions::from(vec![extension])),
        );

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        assert!(extract_ocsp_response_nonce_hex(&basic, &mut evidence, &mut reasons).is_none());
    }

    // ---- verify_ocsp_nonce -----------------------------------------------

    #[test]
    fn nonce_verify_match_pushes_verified_evidence() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_nonce(
            Some("deadbeef"),
            Some("deadbeef"),
            &mut evidence,
            &mut reasons,
        );

        assert!(evidence.contains(&"ocsp_status_nonce_verified".to_string()));
        assert!(reasons.is_empty());
    }

    #[test]
    fn nonce_verify_mismatch_pushes_mismatch_reason() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_nonce(
            Some("deadbeef"),
            Some("cafef00d"),
            &mut evidence,
            &mut reasons,
        );

        assert!(reasons.contains(&"ocsp_status_nonce_mismatch".to_string()));
        assert!(evidence.is_empty());
    }

    #[test]
    fn nonce_verify_expected_but_missing_pushes_missing_reason() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_nonce(Some("deadbeef"), None, &mut evidence, &mut reasons);

        assert!(reasons.contains(&"ocsp_status_nonce_missing".to_string()));
    }

    #[test]
    fn nonce_verify_observed_without_expectation_is_inert() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_nonce(None, Some("deadbeef"), &mut evidence, &mut reasons);

        assert!(evidence.contains(&"ocsp_status_nonce_present_without_expectation".to_string()));
        assert!(reasons.is_empty());
    }

    #[test]
    fn nonce_verify_neither_supplied_is_inert() {
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        verify_ocsp_nonce(None, None, &mut evidence, &mut reasons);

        assert!(evidence.contains(&"ocsp_status_nonce_not_supplied".to_string()));
        assert!(reasons.is_empty());
    }

    // ---- normalize_expected_ocsp_nonce_hex -------------------------------

    #[test]
    fn normalize_nonce_lowercases_valid_hex() {
        let mut reasons = Vec::new();
        let normalized = normalize_expected_ocsp_nonce_hex("DEADBEEF", &mut reasons);
        assert_eq!(normalized.as_deref(), Some("deadbeef"));
        assert!(reasons.is_empty());
    }

    #[test]
    fn normalize_nonce_strips_separators() {
        let mut reasons = Vec::new();
        let normalized = normalize_expected_ocsp_nonce_hex("de:ad-be ef", &mut reasons);
        assert_eq!(normalized.as_deref(), Some("deadbeef"));
        assert!(reasons.is_empty());
    }

    #[test]
    fn normalize_nonce_rejects_odd_length() {
        let mut reasons = Vec::new();
        let normalized = normalize_expected_ocsp_nonce_hex("abc", &mut reasons);
        assert!(normalized.is_none());
        assert!(reasons.contains(&"ocsp_status_expected_nonce_hex_invalid".to_string()));
    }

    #[test]
    fn normalize_nonce_rejects_invalid_character() {
        let mut reasons = Vec::new();
        let normalized = normalize_expected_ocsp_nonce_hex("de:xy", &mut reasons);
        assert!(normalized.is_none());
        assert!(reasons.contains(&"ocsp_status_expected_nonce_hex_invalid".to_string()));
    }

    #[test]
    fn normalize_nonce_rejects_empty() {
        let mut reasons = Vec::new();
        let normalized = normalize_expected_ocsp_nonce_hex("", &mut reasons);
        assert!(normalized.is_none());
        assert!(reasons.contains(&"ocsp_status_expected_nonce_hex_invalid".to_string()));
    }

    // ---- ocsp_responder_id_matches_issuer --------------------------------

    #[test]
    fn responder_id_by_key_matches_issuer_public_key_hash() {
        let issuer = test_issuer_cert("Forge Test OCSP Root");
        let issuer_key_hash = sha1_of(issuer.public_key().subject_public_key.data.as_ref());
        let responder = ResponderId::ByKey(RasnOctetString::from(issuer_key_hash));

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let matches =
            ocsp_responder_id_matches_issuer(&responder, &issuer, &mut evidence, &mut reasons);

        assert!(matches);
        assert!(evidence.contains(&"ocsp_status_responder_key_matches_issuer".to_string()));
    }

    #[test]
    fn responder_id_by_key_rejects_mismatched_hash() {
        let issuer = test_issuer_cert("Forge Test OCSP Root");
        let responder = ResponderId::ByKey(RasnOctetString::from(vec![0xff; 20]));

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let matches =
            ocsp_responder_id_matches_issuer(&responder, &issuer, &mut evidence, &mut reasons);

        assert!(!matches);
        // ByKey mismatch is silent: only the bool differs, no reason pushed.
        assert!(reasons.is_empty());
    }

    // ---- find_matching_ocsp_single_response ------------------------------

    #[test]
    fn find_single_response_finds_matching_serial_and_hash() {
        let issuer = test_issuer_cert("Forge Test OCSP Root");
        let name_hash = sha256_of(issuer.subject().as_raw());
        let key_hash = sha256_of(issuer.public_key().subject_public_key.data.as_ref());
        // The issuer's serial is whatever rcgen assigned (1 for self-signed
        // root); read it so the CertID matches what the production code
        // derives. The issuer is used as both the "certificate" and the issuer.
        let issuer_serial: u64 = issuer
            .tbs_certificate
            .serial
            .to_string()
            .parse()
            .expect("issuer serial parses as u64");
        let cert_id = synthetic_cert_id(&SHA256_OID, &name_hash, &key_hash, issuer_serial);
        let response = single_response(cert_id, CertStatus::Good, 1000, Some(2000));
        let basic = basic_ocsp_response(vec![0xaa; 20], vec![response], None);

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let found = find_matching_ocsp_single_response(
            &basic,
            &issuer,
            &issuer,
            &mut evidence,
            &mut reasons,
        );

        assert!(found.is_some());
        assert!(evidence.contains(&"ocsp_status_cert_id_serial_and_issuer_hash_match".to_string()));
    }

    #[test]
    fn find_single_response_rejects_serial_mismatch() {
        let issuer = test_issuer_cert("Forge Test OCSP Root");
        let name_hash = sha256_of(issuer.subject().as_raw());
        let key_hash = sha256_of(issuer.public_key().subject_public_key.data.as_ref());
        // CertID claims serial 0x9999, but the issuer's serial differs.
        let cert_id = synthetic_cert_id(&SHA256_OID, &name_hash, &key_hash, 0x9999);
        let response = single_response(cert_id, CertStatus::Good, 1000, Some(2000));
        let basic = basic_ocsp_response(vec![0xaa; 20], vec![response], None);

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let found = find_matching_ocsp_single_response(
            &basic,
            &issuer,
            &issuer,
            &mut evidence,
            &mut reasons,
        );

        assert!(found.is_none());
        assert!(reasons.contains(&"ocsp_status_certificate_serial_not_found".to_string()));
    }

    #[test]
    fn find_single_response_rejects_unsupported_hash_algorithm() {
        let issuer = test_issuer_cert("Forge Test OCSP Root");
        // MD5 OID 1.2.840.113549.2.5 — not in the supported set.
        let cert_id = synthetic_cert_id(&[1, 2, 840, 113_549, 2, 5], &[0; 16], &[1; 16], 0x1234);
        let response = single_response(cert_id, CertStatus::Good, 1000, Some(2000));
        let basic = basic_ocsp_response(vec![0xaa; 20], vec![response], None);

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let found = find_matching_ocsp_single_response(
            &basic,
            &issuer,
            &issuer,
            &mut evidence,
            &mut reasons,
        );

        assert!(found.is_none());
        assert!(reasons.contains(&"ocsp_status_cert_id_hash_algorithm_unsupported".to_string()));
    }

    // ---- delegated responder authority -----------------------------------

    #[test]
    fn delegated_responder_authority_accepts_exact_issuer_eku_time_and_by_key() {
        let (issuer, responder, _key) = test_delegated_responder(
            ExtendedKeyUsagePurpose::OcspSigning,
            (2026, 1, 1),
            (2027, 1, 1),
        );
        let responder_hash = sha1_of(responder.public_key().subject_public_key.data.as_ref());
        let basic = basic_ocsp_response(responder_hash, Vec::new(), None);
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        let mode = verify_delegated_ocsp_responder_authority(
            &basic,
            &responder,
            &[],
            &issuer,
            1_783_391_200,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(mode, Some(crate::HostAdapterOcspResponderIdMode::ByKey));
        assert!(reasons.is_empty());
        assert!(!evidence.contains(&"ocsp_status_delegated_responder_authorized".to_string()));
        assert_eq!(ocsp_responder_public_key_sha1_hex(&responder).len(), 40);
    }

    #[test]
    fn delegated_responder_authority_accepts_responder_id_by_name() {
        let (issuer, responder, _key) = test_delegated_responder(
            ExtendedKeyUsagePurpose::OcspSigning,
            (2026, 1, 1),
            (2027, 1, 1),
        );
        let mut basic = basic_ocsp_response(vec![0; 20], Vec::new(), None);
        basic.tbs_response_data.responder_id = ResponderId::ByName(
            rasn::der::decode(responder.subject().as_raw()).expect("decode responder subject"),
        );
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        let mode = verify_delegated_ocsp_responder_authority(
            &basic,
            &responder,
            &[],
            &issuer,
            1_783_391_200,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(mode, Some(crate::HostAdapterOcspResponderIdMode::ByName));
        assert!(reasons.is_empty());
    }

    #[test]
    fn delegated_responder_authority_rejects_wrong_eku() {
        let (issuer, responder, _key) = test_delegated_responder(
            ExtendedKeyUsagePurpose::CodeSigning,
            (2026, 1, 1),
            (2027, 1, 1),
        );
        let basic = basic_ocsp_response(
            sha1_of(responder.public_key().subject_public_key.data.as_ref()),
            Vec::new(),
            None,
        );
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        assert!(verify_delegated_ocsp_responder_authority(
            &basic,
            &responder,
            &[],
            &issuer,
            1_783_391_200,
            &mut evidence,
            &mut reasons,
        )
        .is_none());
        assert!(reasons
            .contains(&"ocsp_status_delegated_responder_ocsp_signing_eku_missing".to_string()));
    }

    #[test]
    fn delegated_responder_authority_rejects_time_responder_id_and_path_mismatch() {
        let (issuer, responder, _key) = test_delegated_responder(
            ExtendedKeyUsagePurpose::OcspSigning,
            (2028, 1, 1),
            (2029, 1, 1),
        );
        let wrong_issuer = test_issuer_cert("Wrong Delegated OCSP Issuer");
        let basic = basic_ocsp_response(vec![0xff; 20], Vec::new(), None);
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        assert!(verify_delegated_ocsp_responder_authority(
            &basic,
            &responder,
            &[],
            &wrong_issuer,
            1_783_391_200,
            &mut evidence,
            &mut reasons,
        )
        .is_none());
        assert!(reasons.contains(
            &"ocsp_status_delegated_responder_not_valid_at_verification_time".to_string()
        ));
        assert!(reasons.contains(&"ocsp_status_responder_key_mismatch".to_string()));
        assert!(reasons.iter().any(
            |reason| reason.starts_with("ocsp_status_delegated_path_issuer_subject_mismatch_0")
        ));
        assert_ne!(issuer.subject(), wrong_issuer.subject());
    }

    #[test]
    fn delegated_responder_authority_rejects_unsupported_critical_issuer_extension() {
        let mut issuer_params = CertificateParams::new(Vec::new()).expect("issuer params");
        issuer_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "Critical Extension Issuer");
        issuer_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
        let mut unsupported = CustomExtension::from_oid_content(&[1, 2, 3, 4], vec![5, 0]);
        unsupported.set_criticality(true);
        issuer_params.custom_extensions.push(unsupported);
        let issuer_key = KeyPair::generate().expect("issuer key");
        let issuer_certificate = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");

        let mut responder_params = CertificateParams::new(Vec::new()).expect("responder params");
        responder_params
            .distinguished_name
            .push(DnType::CommonName, "Delegated Responder");
        responder_params
            .key_usages
            .push(KeyUsagePurpose::DigitalSignature);
        responder_params
            .extended_key_usages
            .push(ExtendedKeyUsagePurpose::OcspSigning);
        let responder_key = KeyPair::generate().expect("responder key");
        let issuer = Issuer::from_params(&issuer_params, &issuer_key);
        let responder_certificate = responder_params
            .signed_by(&responder_key, &issuer)
            .expect("responder certificate");
        let issuer_der: &'static [u8] =
            Box::leak(issuer_certificate.der().to_vec().into_boxed_slice());
        let responder_der: &'static [u8] =
            Box::leak(responder_certificate.der().to_vec().into_boxed_slice());
        let (_, issuer) = X509Certificate::from_der(issuer_der).expect("parse issuer");
        let (_, responder) = X509Certificate::from_der(responder_der).expect("parse responder");
        let basic = basic_ocsp_response(
            sha1_of(responder.public_key().subject_public_key.data.as_ref()),
            Vec::new(),
            None,
        );
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        assert!(verify_delegated_ocsp_responder_authority(
            &basic,
            &responder,
            &[],
            &issuer,
            1_783_391_200,
            &mut evidence,
            &mut reasons,
        )
        .is_none());
        assert!(reasons.iter().any(|reason| reason
            .starts_with("ocsp_status_delegated_path_issuer_0_critical_extension_unsupported:")));
    }

    #[test]
    fn delegated_responder_authority_rejects_path_len_zero_with_intermediate() {
        let mut root_params = CertificateParams::new(Vec::new()).expect("root params");
        root_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Constrained(0));
        root_params
            .distinguished_name
            .push(DnType::CommonName, "Path Length Zero Root");
        root_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
        let root_key = KeyPair::generate().expect("root key");
        let root_certificate = root_params
            .self_signed(&root_key)
            .expect("root certificate");

        let mut intermediate_params =
            CertificateParams::new(Vec::new()).expect("intermediate params");
        intermediate_params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        intermediate_params
            .distinguished_name
            .push(DnType::CommonName, "Intermediate CA");
        intermediate_params
            .key_usages
            .push(KeyUsagePurpose::KeyCertSign);
        let intermediate_key = KeyPair::generate().expect("intermediate key");
        let root_issuer = Issuer::from_params(&root_params, &root_key);
        let intermediate_certificate = intermediate_params
            .signed_by(&intermediate_key, &root_issuer)
            .expect("intermediate certificate");

        let mut responder_params = CertificateParams::new(Vec::new()).expect("responder params");
        responder_params
            .distinguished_name
            .push(DnType::CommonName, "Delegated Responder");
        responder_params
            .key_usages
            .push(KeyUsagePurpose::DigitalSignature);
        responder_params
            .extended_key_usages
            .push(ExtendedKeyUsagePurpose::OcspSigning);
        let responder_key = KeyPair::generate().expect("responder key");
        let intermediate_issuer = Issuer::from_params(&intermediate_params, &intermediate_key);
        let responder_certificate = responder_params
            .signed_by(&responder_key, &intermediate_issuer)
            .expect("responder certificate");

        let root_der: &'static [u8] = Box::leak(root_certificate.der().to_vec().into_boxed_slice());
        let intermediate_der: &'static [u8] =
            Box::leak(intermediate_certificate.der().to_vec().into_boxed_slice());
        let responder_der: &'static [u8] =
            Box::leak(responder_certificate.der().to_vec().into_boxed_slice());
        let (_, root) = X509Certificate::from_der(root_der).expect("parse root");
        let (_, intermediate) =
            X509Certificate::from_der(intermediate_der).expect("parse intermediate");
        let (_, responder) = X509Certificate::from_der(responder_der).expect("parse responder");
        let basic = basic_ocsp_response(
            sha1_of(responder.public_key().subject_public_key.data.as_ref()),
            Vec::new(),
            None,
        );
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        assert!(verify_delegated_ocsp_responder_authority(
            &basic,
            &responder,
            &[intermediate],
            &root,
            1_783_391_200,
            &mut evidence,
            &mut reasons,
        )
        .is_none());
        assert!(reasons.contains(&"ocsp_status_delegated_path_length_exceeded_1".to_string()));
    }

    #[test]
    fn delegated_responder_signature_verifies_only_with_delegated_key() {
        let (issuer, responder, responder_key) = test_delegated_responder(
            ExtendedKeyUsagePurpose::OcspSigning,
            (2026, 1, 1),
            (2027, 1, 1),
        );
        let responder_hash = sha1_of(responder.public_key().subject_public_key.data.as_ref());
        let mut basic = basic_ocsp_response(responder_hash, Vec::new(), None);
        let tbs_der = rasn::der::encode(&basic.tbs_response_data).expect("encode response data");
        basic.signature = BitString::from_vec(
            responder_key
                .sign(&tbs_der)
                .expect("sign response data with delegated key"),
        );

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        assert!(verify_basic_ocsp_signature_with_certificate(
            &basic,
            &responder,
            &mut evidence,
            &mut reasons,
        ));

        let mut wrong_key_evidence = Vec::new();
        let mut wrong_key_reasons = Vec::new();
        assert!(!verify_basic_ocsp_signature_with_certificate(
            &basic,
            &issuer,
            &mut wrong_key_evidence,
            &mut wrong_key_reasons,
        ));
        assert!(wrong_key_reasons.contains(&"ocsp_status_response_signature_invalid".to_string()));
    }

    // ---- verify_basic_ocsp_signature_with_issuer (negative path) ---------

    #[test]
    fn verify_signature_rejects_synthetic_invalid_signature() {
        let issuer = test_issuer_cert("Forge Test OCSP Root");
        // The fixture's signature is 64 zero bytes — not a valid signature.
        // The happy path of this helper is covered by the 17 E2E tests.
        let basic = basic_ocsp_response(vec![0xaa; 20], Vec::new(), None);

        let mut evidence = Vec::new();
        let mut reasons = Vec::new();
        let verified =
            verify_basic_ocsp_signature_with_issuer(&basic, &issuer, &mut evidence, &mut reasons);

        assert!(!verified);
        assert!(reasons.contains(&"ocsp_status_response_signature_invalid".to_string()));
    }
}

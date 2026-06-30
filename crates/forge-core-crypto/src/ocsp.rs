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
use x509_parser::certificate::X509Certificate;
use x509_parser::x509::AlgorithmIdentifier as X509AlgorithmIdentifier;
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

/// Verify that the basic OCSP response signature was produced by the issuer.
pub(crate) fn verify_basic_ocsp_signature_with_issuer(
    basic_response: &BasicOcspResponse,
    issuer: &X509Certificate<'_>,
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
        issuer.public_key(),
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

/// Check whether the OCSP responder ID matches the issuer certificate.
pub(crate) fn ocsp_responder_id_matches_issuer(
    responder_id: &ResponderId,
    issuer: &X509Certificate<'_>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) -> bool {
    match responder_id {
        ResponderId::ByName(name) => match rasn::der::encode(name) {
            Ok(name_der) if name_der == issuer.subject().as_raw() => {
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
    let extensions = basic_response
        .tbs_response_data
        .response_extensions
        .as_ref()?;
    for extension in extensions.iter() {
        if rasn_oid_matches(&extension.extn_id, &[1, 3, 6, 1, 5, 5, 7, 48, 1, 2]) {
            return match rasn::der::decode::<rasn_ocsp::Nonce>(extension.extn_value.as_ref()) {
                Ok(nonce) => {
                    verified_evidence.push("ocsp_status_nonce_observed".to_string());
                    Some(hex_bytes(nonce.as_ref()))
                }
                Err(err) => {
                    reasons.push(format!("ocsp_status_nonce_parse_failed:{err}"));
                    None
                }
            };
        }
    }
    None
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
    if normalized.is_empty() || !normalized.len().is_multiple_of(2) {
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

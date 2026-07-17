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
        normalize_expected_ocsp_nonce_hex, ocsp_responder_id_matches_issuer, rasn_oid_matches,
        verify_basic_ocsp_signature_with_issuer, verify_ocsp_nonce,
        verify_ocsp_single_response_freshness,
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
    use rcgen::{CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose};
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

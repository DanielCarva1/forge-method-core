//! SLSA provenance statement verification, transparency log inclusion proof
//! verification, and related signature helpers.
//!
//! This module groups together the helpers that
//! `run_host_adapter_provenance_verification` (in `lib.rs`) relies on to:
//!
//! - verify an Ed25519 signature over a raw provenance document
//!   (`verify_ed25519_signature`),
//! - validate an in-toto/SLSA v1 provenance statement against expected
//!   subject digest, builder id, source URI and source ref
//!   (`verify_slsa_statement` + `ExpectedProvenance` +
//!   `statement_subject_has_sha256` + `json_contains_string`),
//! - verify a transparency log inclusion proof
//!   (`verify_transparency_log_proof` + `transparency_leaf_hash`, delegating
//!   the Merkle math to `crypto_rekor::verify_merkle_inclusion`).
//!
//! `statement_subject_has_sha256` is also consumed by
//! `run_host_adapter_sigstore_bundle_subject_verification` and by the DSSE
//! subject verification, so it is `pub(crate)` and re-exported at the crate
//! root. The remaining helpers are `pub(crate)` as well for consistency and
//! re-exported via `pub(crate) use crypto_slsa_transparency::*;`.

use ed25519_dalek::{
    Signature as Ed25519Signature, Verifier as Ed25519Verifier, VerifyingKey as Ed25519VerifyingKey,
};
use serde_json::Value;

use crate::hashing::{hex_sha256, normalize_sha256_digest, normalize_sha256_display};

pub(crate) fn verify_ed25519_signature(
    public_key: &[u8],
    signature: &[u8],
    message: &[u8],
) -> bool {
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

pub(crate) struct ExpectedProvenance<'a> {
    pub(crate) sha256: &'a str,
    pub(crate) builder_id: &'a str,
    pub(crate) source_uri: &'a str,
    pub(crate) source_ref: &'a str,
}

pub(crate) fn verify_slsa_statement(
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

pub(crate) fn statement_subject_has_sha256(statement: &Value, expected_sha256: &str) -> bool {
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

pub(crate) fn json_contains_string(value: &Value, expected: &str) -> bool {
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

pub(crate) fn verify_transparency_log_proof(
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

    if crate::rekor::verify_merkle_inclusion(&leaf_hash, &hashes, log_index, tree_size, &root_hash)
    {
        verified_evidence.push("transparency_inclusion_proof_valid".to_string());
    } else {
        reasons.push("transparency_inclusion_proof_invalid".to_string());
    }
}

pub(crate) fn transparency_leaf_hash(provenance_sha256: &str, signature_sha256: &str) -> String {
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

#[cfg(test)]
mod tests {
    //! Unit coverage for `verify_ed25519_signature`.
    //!
    //! This is a security-critical path: a silent bug here means a forged
    //! provenance signature passes verification. The crate had zero unit
    //! coverage on this function (only the indirect CLI E2E in
    //! `forge-core-cli/tests/validate.rs` exercised it). These tests pin the
    //! contract directly: round-trip Ok, every tamper variant fail-closed,
    //! malformed inputs return `false` rather than panicking, and a
    //! deterministic KAT pins the exact canonical bytes + signature so a
    //! dalek/canonicalization drift surfaces as a clear diff.

    use super::verify_ed25519_signature;
    use ed25519_dalek::{Signer, SigningKey};

    /// Fixed 32-byte seed reused across tests for determinism.
    /// NOT a secret — it is a test vector. Mirrors `validate.rs:288`
    /// (`SigningKey::from_bytes(&[7u8; 32])`) so the two suites agree on the
    /// same key material.
    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn hex_encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write as _;
            let _ = write!(out, "{b:02x}");
        }
        out
    }

    #[test]
    fn ed25519_roundtrip_ok() {
        let signing_key = test_signing_key();
        let verifying_bytes = signing_key.verifying_key().to_bytes();
        let message = b"forge provenance payload";
        let signature = signing_key.sign(message).to_bytes();

        assert!(
            verify_ed25519_signature(&verifying_bytes, &signature, message),
            "valid signature must verify"
        );
    }

    #[test]
    fn ed25519_tampered_signature_invalid() {
        let signing_key = test_signing_key();
        let verifying_bytes = signing_key.verifying_key().to_bytes();
        let message = b"forge provenance payload";
        let mut signature = signing_key.sign(message).to_bytes();
        // Flip a single bit in the first signature byte.
        signature[0] ^= 0x01;

        assert!(
            !verify_ed25519_signature(&verifying_bytes, &signature, message),
            "tampered signature must fail-closed"
        );
    }

    #[test]
    fn ed25519_tampered_message_invalid() {
        let signing_key = test_signing_key();
        let verifying_bytes = signing_key.verifying_key().to_bytes();
        let signed_message = b"forge provenance payload";
        let signature = signing_key.sign(signed_message).to_bytes();
        let tampered_message = b"forge provenance payload EVIL";

        assert!(
            !verify_ed25519_signature(&verifying_bytes, &signature, tampered_message),
            "signature over a different message must fail-closed"
        );
    }

    #[test]
    fn ed25519_wrong_key_invalid() {
        let signing_key = test_signing_key();
        let message = b"forge provenance payload";
        let signature = signing_key.sign(message).to_bytes();

        // A different key (different seed) must reject the signature.
        let wrong_key = SigningKey::from_bytes(&[8u8; 32]);
        let wrong_verifying_bytes = wrong_key.verifying_key().to_bytes();

        assert!(
            !verify_ed25519_signature(&wrong_verifying_bytes, &signature, message),
            "signature verified with the wrong public key must fail-closed"
        );
    }

    #[test]
    fn ed25519_malformed_inputs_return_false_not_panic() {
        // The function's contract is: lengths are enforced (32-byte key,
        // 64-byte signature); anything else returns `false` without
        // panicking. It does NOT promise to reject every degenerate-but-
        // well-formed key/sig pair (ed25519 admits valid encodings of the
        // identity point), only structural failures.
        let valid_signature = [0u8; 64];

        // Public key too short: returns false, does not panic.
        assert!(
            !verify_ed25519_signature(&[0u8; 31], &valid_signature, b"msg"),
            "31-byte public key must be rejected without panicking"
        );

        // Signature wrong length: returns false.
        assert!(
            !verify_ed25519_signature(&[0u8; 32], &[0u8; 63], b"msg"),
            "63-byte signature must be rejected without panicking"
        );

        // Public key too long: returns false.
        assert!(
            !verify_ed25519_signature(&[0u8; 33], &valid_signature, b"msg"),
            "33-byte public key must be rejected without panicking"
        );

        // Signature too long: returns false.
        assert!(
            !verify_ed25519_signature(&[0u8; 32], &[0u8; 65], b"msg"),
            "65-byte signature must be rejected without panicking"
        );
    }

    /// Deterministic known-answer test.
    ///
    /// Pins (a) the 32-byte verifying key derived from the fixed seed, and
    /// (b) the 64-byte signature over a fixed message. Any drift in the
    /// ed25519-dalek crate, the seed derivation, or the signature encoding
    /// surfaces here as a clear before/after hex diff rather than as a flaky
    /// verify failure elsewhere. If this fails after an intentional upgrade,
    /// recompute the two pinned values from the new outputs and update them
    /// here — that update IS the review that the change is intentional.
    #[test]
    fn ed25519_deterministic_kat_pins_key_and_signature() {
        // Pinned values live at the top of the function so clippy's
        // `items_after_statements` stays quiet and the constants read as
        // "the contract this test pins", not "an incidental local".
        const PINNED_VERIFYING_KEY_HEX: &str =
            "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c";
        // Ed25519 is deterministic, so the same seed + message always yields
        // the same signature.
        const PINNED_SIGNATURE_HEX: &str = "\
            b0bc8e6733416ebd9cb1ebd1c596896f2cfaa4dcbdecaf392b3024d2394dff4b\
            def737a4b392ca71597ab34530ca7c56d6688e4221cbeb7bffe675620d4d9604";

        let signing_key = test_signing_key();
        let verifying_key = signing_key.verifying_key();

        // Pin the verifying key (hex). If the seed→key derivation or the
        // dalek encoding changes, this changes.
        let actual_verify_hex = hex_encode(&verifying_key.to_bytes());
        assert_eq!(
            actual_verify_hex, PINNED_VERIFYING_KEY_HEX,
            "pinned verifying key drifted; if the dalek version or seed changed, \
             recompute and update this constant"
        );

        let message = b"deterministic-kat-message";
        let signature = signing_key.sign(message);
        let signature_bytes = signature.to_bytes();

        // Pin the signature (hex).
        let actual_sig_hex = hex_encode(&signature_bytes);
        assert_eq!(
            actual_sig_hex, PINNED_SIGNATURE_HEX,
            "pinned signature drifted; if the dalek version or seed changed, \
             recompute and update this constant"
        );

        // The pinned signature must verify against the pinned key.
        assert!(
            verify_ed25519_signature(&verifying_key.to_bytes(), &signature_bytes, message),
            "pinned KAT signature must verify"
        );
    }

    /// Property test: for any message length in 0..256 bytes, sign→verify Ok,
    /// and a single-bit flip in the signature flips the verdict to Invalid.
    /// Guards against length-dependent edge cases and fail-open regressions.
    #[test]
    fn ed25519_proptest_sign_verify_tamper() {
        use proptest::prelude::*;

        proptest!(|(message in proptest::collection::vec(0u8..=255, 0..256))| {
            let signing_key = test_signing_key();
            let verifying_bytes = signing_key.verifying_key().to_bytes();
            let signature = signing_key.sign(&message).to_bytes();

            prop_assert!(
                verify_ed25519_signature(&verifying_bytes, &signature, &message),
                "valid signature must verify for any message length"
            );

            let mut tampered = signature;
            tampered[0] ^= 0x01;
            prop_assert!(
                !verify_ed25519_signature(&verifying_bytes, &tampered, &message),
                "single-bit-flipped signature must fail-closed"
            );
        });
    }
}

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

use crate::crypto_hashing::{hex_sha256, normalize_sha256_digest, normalize_sha256_display};

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

    if crate::crypto_rekor::verify_merkle_inclusion(
        &leaf_hash, &hashes, log_index, tree_size, &root_hash,
    ) {
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

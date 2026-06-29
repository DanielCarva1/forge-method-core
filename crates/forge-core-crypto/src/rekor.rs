//! Rekor log entry parsing and inclusion proof verification.
//!
//! Extraído de `lib.rs` como parte da recomendação R1 (decompor god-file).
//! Mantém a mesma API interna; funções são `pub(crate)` para uso pelo
//! `lib.rs` e demais módulos do crate.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::Verifier as _;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use serde_json::Value;

use crate::hashing::{hex_bytes, hex_sha256, normalize_sha256_display};

pub(crate) struct ParsedRekorEntry {
    pub(crate) body: Value,
    pub(crate) log_id: String,
    pub(crate) log_index: i64,
    pub(crate) integrated_time: i64,
    pub(crate) proof: ParsedRekorInclusionProof,
}

pub(crate) struct ParsedRekorInclusionProof {
    pub(crate) hashes: Vec<String>,
    pub(crate) log_index: i64,
    pub(crate) root_hash: String,
    pub(crate) tree_size: u64,
    pub(crate) checkpoint: String,
}

pub(crate) struct ParsedCheckpoint {
    pub(crate) signed_body: String,
    pub(crate) tree_size: u64,
    pub(crate) root_hash: String,
    pub(crate) signatures: Vec<Vec<u8>>,
}

pub(crate) fn parse_rekor_log_entry(text: &str) -> Result<ParsedRekorEntry, String> {
    let value = serde_json::from_str::<Value>(text)
        .map_err(|err| format!("rekor_log_entry_json_invalid:{err}"))?;
    let body_b64 = required_string(&value, "body")?;
    let body_bytes = BASE64
        .decode(body_b64.as_bytes())
        .map_err(|err| format!("rekor_body_base64_invalid:{err}"))?;
    let body = serde_json::from_slice::<Value>(&body_bytes)
        .map_err(|err| format!("rekor_body_json_invalid:{err}"))?;
    let verification = value
        .get("verification")
        .ok_or_else(|| "rekor_verification_missing".to_string())?;
    let inclusion = verification
        .get("inclusionProof")
        .ok_or_else(|| "rekor_inclusion_proof_missing".to_string())?;
    let hashes = inclusion
        .get("hashes")
        .and_then(Value::as_array)
        .ok_or_else(|| "rekor_inclusion_hashes_missing".to_string())?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| "rekor_inclusion_hash_invalid".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ParsedRekorEntry {
        body,
        log_id: required_string(&value, "logID")?.to_string(),
        log_index: required_i64(&value, "logIndex")?,
        integrated_time: required_i64(&value, "integratedTime")?,
        proof: ParsedRekorInclusionProof {
            hashes,
            log_index: required_i64(inclusion, "logIndex")?,
            root_hash: required_string(inclusion, "rootHash")?.to_string(),
            tree_size: required_u64(inclusion, "treeSize")?,
            checkpoint: required_string(inclusion, "checkpoint")?.to_string(),
        },
    })
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("rekor_{key}_missing"))
}

fn required_i64(value: &Value, key: &str) -> Result<i64, String> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("rekor_{key}_missing"))
}

fn required_u64(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("rekor_{key}_missing"))
}

pub(crate) fn verify_rekor_entry_inclusion(
    entry: &ParsedRekorEntry,
    rekor_key: &P256VerifyingKey,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    if entry.proof.log_index != entry.log_index {
        reasons.push("rekor_inclusion_log_index_mismatch".to_string());
        return;
    }
    let canonical_body = match serde_json_canonicalizer::to_vec(&entry.body) {
        Ok(bytes) => bytes,
        Err(err) => {
            reasons.push(format!("rekor_body_canonicalization_failed:{err}"));
            return;
        }
    };
    let leaf_hash = rfc6962_leaf_hash(&canonical_body);

    match verify_rekor_checkpoint(&entry.proof, rekor_key) {
        Ok(()) => verified_evidence.push("rekor_signed_checkpoint_valid".to_string()),
        Err(reason) => {
            reasons.push(format!("rekor_inclusion_verification_failed:{reason}"));
            return;
        }
    }

    if entry.proof.log_index < 0 {
        reasons.push("rekor_log_index_negative".to_string());
        return;
    }
    if verify_merkle_inclusion(
        &leaf_hash,
        &entry.proof.hashes,
        entry.proof.log_index as u64,
        entry.proof.tree_size,
        &entry.proof.root_hash,
    ) {
        verified_evidence.push("rekor_inclusion_proof_valid".to_string());
    } else {
        reasons.push("rekor_inclusion_verification_failed:merkle_path_invalid".to_string());
    }
}

pub(crate) fn verify_rekor_checkpoint(
    proof: &ParsedRekorInclusionProof,
    rekor_key: &P256VerifyingKey,
) -> Result<(), String> {
    let checkpoint = parse_signed_checkpoint(&proof.checkpoint)?;
    if checkpoint.tree_size != proof.tree_size {
        return Err("checkpoint_tree_size_mismatch".to_string());
    }
    if checkpoint.root_hash != normalize_sha256_display(&proof.root_hash) {
        return Err("checkpoint_root_hash_mismatch".to_string());
    }
    if checkpoint.signatures.is_empty() {
        return Err("checkpoint_signature_missing".to_string());
    }
    for signature in checkpoint.signatures {
        let Ok(signature) = P256Signature::from_der(&signature) else {
            continue;
        };
        if rekor_key
            .verify(checkpoint.signed_body.as_bytes(), &signature)
            .is_ok()
        {
            return Ok(());
        }
    }
    Err("checkpoint_signature_invalid".to_string())
}

pub(crate) fn parse_signed_checkpoint(checkpoint: &str) -> Result<ParsedCheckpoint, String> {
    let checkpoint = checkpoint.trim_matches('"');
    let (note, signatures) = checkpoint
        .split_once("\n\n")
        .ok_or_else(|| "checkpoint_format_invalid".to_string())?;
    let lines = note.split('\n').collect::<Vec<_>>();
    let [origin, tree_size, root_hash_b64, other @ ..] = lines.as_slice() else {
        return Err("checkpoint_note_invalid".to_string());
    };
    if origin.trim().is_empty() {
        return Err("checkpoint_origin_missing".to_string());
    }
    let tree_size = tree_size
        .parse::<u64>()
        .map_err(|_| "checkpoint_tree_size_invalid".to_string())?;
    let root_hash = BASE64
        .decode(root_hash_b64.as_bytes())
        .map_err(|err| format!("checkpoint_root_hash_base64_invalid:{err}"))
        .map(|bytes| hex_bytes(&bytes))?;
    let mut signed_body = format!("{origin}\n{tree_size}\n{root_hash_b64}\n");
    for item in other.iter().filter(|item| !item.is_empty()) {
        signed_body.push_str(item);
        signed_body.push('\n');
    }
    let signatures = signatures
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(decode_checkpoint_signature)
        .collect::<Vec<_>>();

    Ok(ParsedCheckpoint {
        signed_body,
        tree_size,
        root_hash,
        signatures,
    })
}

fn decode_checkpoint_signature(line: &str) -> Option<Vec<u8>> {
    let line = line
        .trim()
        .strip_prefix('\u{2014}')
        .or_else(|| line.trim().strip_prefix("--"))?
        .trim();
    let mut parts = line.split_whitespace();
    let _name = parts.next()?;
    let signature = parts.next()?;
    let decoded = BASE64.decode(signature.as_bytes()).ok()?;
    (decoded.len() > 4).then(|| decoded[4..].to_vec())
}

fn rfc6962_leaf_hash(entry: &[u8]) -> String {
    let mut content = Vec::with_capacity(entry.len() + 1);
    content.push(0);
    content.extend_from_slice(entry);
    hex_sha256(&content)
}

pub(crate) fn verify_merkle_inclusion(
    leaf_hash: &str,
    hashes: &[String],
    log_index: u64,
    tree_size: u64,
    root_hash: &str,
) -> bool {
    if tree_size == 0 || log_index >= tree_size {
        return false;
    }
    if tree_size == 1 {
        return hashes.is_empty() && leaf_hash == root_hash;
    }

    let mut computed = leaf_hash.to_string();
    let mut index = log_index;
    let mut last = tree_size - 1;
    for proof_hash in hashes {
        if index % 2 == 1 || index == last {
            computed = hash_merkle_node(proof_hash, &computed);
            while index.is_multiple_of(2) && index != 0 {
                index /= 2;
                last /= 2;
            }
        } else {
            computed = hash_merkle_node(&computed, proof_hash);
        }
        index /= 2;
        last /= 2;
    }
    computed == root_hash
}

fn hash_merkle_node(left: &str, right: &str) -> String {
    let Some(left_bytes) = hex_to_bytes(left) else {
        return String::new();
    };
    let Some(right_bytes) = hex_to_bytes(right) else {
        return String::new();
    };
    let mut content = Vec::with_capacity(1 + left_bytes.len() + right_bytes.len());
    content.push(1);
    content.extend_from_slice(&left_bytes);
    content.extend_from_slice(&right_bytes);
    hex_sha256(&content)
}

fn hex_to_bytes(value: &str) -> Option<Vec<u8>> {
    let value = normalize_sha256_display(value);
    if !value.len().is_multiple_of(2) || !value.chars().all(|item| item.is_ascii_hexdigit()) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

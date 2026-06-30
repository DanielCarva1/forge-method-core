//! Rekor log entry parsing and inclusion proof verification.
//!
//! Extraído de `lib.rs` como parte da recomendação R1 (decompor god-file).
//! Mantém a mesma API interna; funções são `pub(crate)` para uso pelo
//! `lib.rs` e demais módulos do crate.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::Verifier as _;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::hashing::{hex_bytes, hex_sha256, normalize_sha256_display};

/// Error raised while parsing rekor log entries or verifying inclusion proofs.
///
/// Mirrors the diagnostic strings previously embedded in `Result<_, String>`
/// signatures. Use [`RekorParseError::display`] to recover the exact message
/// emitted by the legacy implementation at the diagnostic-push boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RekorParseError {
    /// Top-level JSON of the rekor log entry failed to parse.
    LogEntryJsonInvalid { source: String },
    /// A required field was missing or had the wrong type.
    MissingField { field: &'static str },
    /// The base64-encoded body payload failed to decode.
    BodyBase64Invalid { source: String },
    /// The decoded body JSON failed to parse.
    BodyJsonInvalid { source: String },
    /// The `verification` object is missing.
    VerificationMissing,
    /// The `inclusionProof` object is missing.
    InclusionProofMissing,
    /// The `hashes` array of the inclusion proof is missing or not an array.
    InclusionHashesMissing,
    /// An entry of the inclusion proof hashes array was not a string.
    InclusionHashInvalid,
    /// The signed checkpoint format is invalid (no `\n\n` separator).
    CheckpointFormatInvalid,
    /// The checkpoint note header does not expose the expected 3+ lines.
    CheckpointNoteInvalid,
    /// The checkpoint origin line is empty.
    CheckpointOriginMissing,
    /// The checkpoint `treeSize` line failed to parse as u64.
    CheckpointTreeSizeInvalid { source: String },
    /// The checkpoint root hash base64 payload failed to decode.
    CheckpointRootHashBase64Invalid { source: String },
    /// The checkpoint tree size does not match the inclusion proof tree size.
    CheckpointTreeSizeMismatch,
    /// The checkpoint root hash does not match the inclusion proof root hash.
    CheckpointRootHashMismatch,
    /// The checkpoint has no attached signatures.
    CheckpointSignatureMissing,
    /// None of the checkpoint signatures verified against the rekor key.
    CheckpointSignatureInvalid,
}

impl RekorParseError {
    /// Render the error back into the diagnostic string the legacy
    /// `Result<_, String>` API used to emit.
    pub(crate) fn display(&self) -> String {
        match self {
            Self::LogEntryJsonInvalid { source } => {
                format!("rekor_log_entry_json_invalid:{source}")
            }
            Self::MissingField { field } => format!("rekor_{field}_missing"),
            Self::BodyBase64Invalid { source } => {
                format!("rekor_body_base64_invalid:{source}")
            }
            Self::BodyJsonInvalid { source } => format!("rekor_body_json_invalid:{source}"),
            Self::VerificationMissing => "rekor_verification_missing".to_string(),
            Self::InclusionProofMissing => "rekor_inclusion_proof_missing".to_string(),
            Self::InclusionHashesMissing => "rekor_inclusion_hashes_missing".to_string(),
            Self::InclusionHashInvalid => "rekor_inclusion_hash_invalid".to_string(),
            Self::CheckpointFormatInvalid => "checkpoint_format_invalid".to_string(),
            Self::CheckpointNoteInvalid => "checkpoint_note_invalid".to_string(),
            Self::CheckpointOriginMissing => "checkpoint_origin_missing".to_string(),
            Self::CheckpointTreeSizeInvalid { source } => {
                format!("checkpoint_tree_size_invalid:{source}")
            }
            Self::CheckpointRootHashBase64Invalid { source } => {
                format!("checkpoint_root_hash_base64_invalid:{source}")
            }
            Self::CheckpointTreeSizeMismatch => "checkpoint_tree_size_mismatch".to_string(),
            Self::CheckpointRootHashMismatch => "checkpoint_root_hash_mismatch".to_string(),
            Self::CheckpointSignatureMissing => "checkpoint_signature_missing".to_string(),
            Self::CheckpointSignatureInvalid => "checkpoint_signature_invalid".to_string(),
        }
    }
}

pub struct ParsedRekorEntry {
    pub(crate) body: Value,
    pub(crate) log_id: String,
    pub(crate) log_index: i64,
    pub(crate) integrated_time: i64,
    pub(crate) proof: ParsedRekorInclusionProof,
}

pub struct ParsedRekorInclusionProof {
    pub(crate) hashes: Vec<String>,
    pub(crate) log_index: i64,
    pub(crate) root_hash: String,
    pub(crate) tree_size: u64,
    pub(crate) checkpoint: String,
}

pub struct ParsedCheckpoint {
    pub(crate) signed_body: String,
    pub(crate) tree_size: u64,
    pub(crate) root_hash: String,
    pub(crate) signatures: Vec<Zeroizing<Vec<u8>>>,
}

pub fn parse_rekor_log_entry(text: &str) -> Result<ParsedRekorEntry, RekorParseError> {
    let value = serde_json::from_str::<Value>(text).map_err(|err| {
        RekorParseError::LogEntryJsonInvalid {
            source: err.to_string(),
        }
    })?;
    let body_b64 = required_string(&value, "body")?;
    let body_bytes =
        BASE64
            .decode(body_b64.as_bytes())
            .map_err(|err| RekorParseError::BodyBase64Invalid {
                source: err.to_string(),
            })?;
    let body = serde_json::from_slice::<Value>(&body_bytes).map_err(|err| {
        RekorParseError::BodyJsonInvalid {
            source: err.to_string(),
        }
    })?;
    let verification = value
        .get("verification")
        .ok_or(RekorParseError::VerificationMissing)?;
    let inclusion = verification
        .get("inclusionProof")
        .ok_or(RekorParseError::InclusionProofMissing)?;
    let hashes = inclusion
        .get("hashes")
        .and_then(Value::as_array)
        .ok_or(RekorParseError::InclusionHashesMissing)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or(RekorParseError::InclusionHashInvalid)
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

fn required_string<'a>(value: &'a Value, key: &'static str) -> Result<&'a str, RekorParseError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(RekorParseError::MissingField { field: key })
}

fn required_i64(value: &Value, key: &'static str) -> Result<i64, RekorParseError> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or(RekorParseError::MissingField { field: key })
}

fn required_u64(value: &Value, key: &'static str) -> Result<u64, RekorParseError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or(RekorParseError::MissingField { field: key })
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
            reasons.push(format!(
                "rekor_inclusion_verification_failed:{}",
                reason.display()
            ));
            return;
        }
    }

    if entry.proof.log_index < 0 {
        reasons.push("rekor_log_index_negative".to_string());
        return;
    }
    // Checked above: log_index >= 0 here, so the conversion is infallible.
    let log_index = u64::try_from(entry.proof.log_index).expect("log_index checked non-negative");
    if verify_merkle_inclusion(
        &leaf_hash,
        &entry.proof.hashes,
        log_index,
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
) -> Result<(), RekorParseError> {
    let checkpoint = parse_signed_checkpoint(&proof.checkpoint)?;
    if checkpoint.tree_size != proof.tree_size {
        return Err(RekorParseError::CheckpointTreeSizeMismatch);
    }
    if !crate::hashing::constant_time_eq_hex(&checkpoint.root_hash, &proof.root_hash) {
        return Err(RekorParseError::CheckpointRootHashMismatch);
    }
    if checkpoint.signatures.is_empty() {
        return Err(RekorParseError::CheckpointSignatureMissing);
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
    Err(RekorParseError::CheckpointSignatureInvalid)
}

pub fn parse_signed_checkpoint(
    checkpoint: &str,
) -> Result<ParsedCheckpoint, RekorParseError> {
    let checkpoint = checkpoint.trim_matches('"');
    let (note, signatures) = checkpoint
        .split_once("\n\n")
        .ok_or(RekorParseError::CheckpointFormatInvalid)?;
    let lines = note.split('\n').collect::<Vec<_>>();
    let [origin, tree_size, root_hash_b64, other @ ..] = lines.as_slice() else {
        return Err(RekorParseError::CheckpointNoteInvalid);
    };
    if origin.trim().is_empty() {
        return Err(RekorParseError::CheckpointOriginMissing);
    }
    let tree_size =
        tree_size
            .parse::<u64>()
            .map_err(|err| RekorParseError::CheckpointTreeSizeInvalid {
                source: err.to_string(),
            })?;
    let root_hash = BASE64
        .decode(root_hash_b64.as_bytes())
        .map_err(|err| RekorParseError::CheckpointRootHashBase64Invalid {
            source: err.to_string(),
        })
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

fn decode_checkpoint_signature(line: &str) -> Option<Zeroizing<Vec<u8>>> {
    let line = line
        .trim()
        .strip_prefix('\u{2014}')
        .or_else(|| line.trim().strip_prefix("--"))?
        .trim();
    let mut parts = line.split_whitespace();
    let _name = parts.next()?;
    let signature = parts.next()?;
    let decoded = BASE64.decode(signature.as_bytes()).ok()?;
    (decoded.len() > 4).then(|| Zeroizing::new(decoded[4..].to_vec()))
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
        return hashes.is_empty() && crate::hashing::constant_time_eq_hex(leaf_hash, root_hash);
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
    crate::hashing::constant_time_eq_hex(&computed, root_hash)
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

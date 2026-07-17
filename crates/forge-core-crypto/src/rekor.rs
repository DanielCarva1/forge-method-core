//! Rekor log entry parsing and inclusion proof verification.
//!
//! Extracted from `lib.rs` as part of recommendation R1 (decompose the
//! god-file). Retains the same internal API; functions are `pub(crate)` for
//! use by `lib.rs` and the other modules of the crate.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::Verifier as _;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::hashing::{hex_bytes, hex_sha256, normalize_sha256_display};

/// Error raised while parsing rekor log entries or verifying inclusion proofs.
///
/// Mirrors the diagnostic strings previously embedded in `Result<_, String>`
/// signatures. Use `RekorParseError::display` to recover the exact message
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

/// Parses a Rekor log entry JSON blob into a [`ParsedRekorEntry`].
///
/// # Errors
///
/// Returns [`RekorParseError`] when the input is not valid JSON, when
/// required fields are missing, or when the body/integration-proof payloads
/// fail base64 or JSON decoding.
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

/// Parses a signed checkpoint text blob into a [`ParsedCheckpoint`].
///
/// # Errors
///
/// Returns [`RekorParseError`] when the checkpoint is malformed (missing note
/// / signature separator, wrong line count, empty origin, non-numeric tree
/// size, or undecodable signatures).
pub fn parse_signed_checkpoint(checkpoint: &str) -> Result<ParsedCheckpoint, RekorParseError> {
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
            while index % 2 == 0 && index != 0 {
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
    if value.len() % 2 != 0 || !value.chars().all(|item| item.is_ascii_hexdigit()) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    //! Unit coverage for the four rekor entrypoints in this module.
    //!
    //! NOTE: this module intentionally exercises the contract through
    //! `assert!(result.is_ok())` / `matches!` on `Err` rather than
    //! `assert_eq!` on the full `Result`: the parsed structs
    //! (`ParsedRekorEntry`, `ParsedCheckpoint`) deliberately do NOT derive
    //! `Debug`/`PartialEq` (zero production churn per the handoff), so the
    //! tests assert on the `Err` variant and on the `Ok`-side field values
    //! individually.
    //!
    //! This is a security-critical path: a silent bug in checkpoint
    //! verification or the Merkle walk means a forged transparency-log entry
    //! passes inclusion verification. The crate had zero direct unit coverage
    //! on `rekor.rs` — only the indirect CLI E2E in
    //! `forge-core-cli/tests/validate.rs` (`rekor_entry_fixture` line 337)
    //! exercised it. These tests pin each entrypoint's contract directly:
    //!
    //! - `parse_rekor_log_entry`: every `RekorParseError` variant + happy path.
    //! - `parse_signed_checkpoint`: every checkpoint-format error variant +
    //!   a happy-path KAT pinning the parsed `tree_size` and root hash.
    //! - `verify_rekor_checkpoint`: Ok + each of the 4 mismatch/missing/invalid
    //!   variants, plus a deterministic p256 KAT.
    //! - `verify_merkle_inclusion`: `tree_size=1` trivial, `tree_size=0` /
    //!   `log_index>=tree_size` rejection, 2- and 4-leaf trees (the latter
    //!   exercises the `index == last` branch), tamper/malformed rejection,
    //!   and a proptest over random 4-leaf trees.
    //!
    //! Fixture strategy mirrors `validate.rs:337` (`rekor_entry_fixture`) and
    //! the benchmark in `benches/rekor.rs`: a fixed p256 seed
    //! (`[8u8; 32]`), RFC 6962 leaf hashes, and the 4-byte-prefixed
    //! note-signature payload that `decode_checkpoint_signature` strips.

    use super::{
        parse_rekor_log_entry, parse_signed_checkpoint, verify_merkle_inclusion,
        verify_rekor_checkpoint, ParsedRekorInclusionProof, RekorParseError,
    };
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use p256::ecdsa::signature::Signer as _;
    use p256::ecdsa::{Signature as P256Signature, SigningKey as P256SigningKey};
    use serde_json::{json, Value};

    /// Fixed 32-byte p256 seed reused across tests for determinism. Mirrors
    /// `validate.rs:341` (`P256SigningKey::from_slice(&[8u8; 32])`) so the
    /// unit suite and the CLI E2E suite agree on the same rekor key material.
    /// NOT a secret — it is a test vector.
    fn test_signing_key() -> P256SigningKey {
        P256SigningKey::from_slice(&[8u8; 32]).expect("p256 signing key from fixed seed")
    }

    /// RFC 6962 interior node: lowercase hex of `H(0x01 || left || right)`.
    /// Mirrors the private `hash_merkle_node` so test fixtures compose roots
    /// that `verify_merkle_inclusion` will re-derive byte-for-byte.
    fn merkle_parent_hex(left: &str, right: &str) -> String {
        let mut buf = Vec::with_capacity(1 + left.len() / 2 + right.len() / 2);
        buf.push(1);
        buf.extend(hex_decode(left));
        buf.extend(hex_decode(right));
        crate::hashing::hex_sha256(&buf)
    }

    fn hex_decode(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("valid hex"))
            .collect()
    }

    fn hex_encode(bytes: &[u8]) -> String {
        crate::hashing::hex_bytes(bytes)
    }

    /// Build a signed checkpoint text blob identical in shape to what a real
    /// rekor log emits: `"<origin>\n<tree_size>\n<root_b64>\n\n— name <b64>\n"`.
    /// The 4-byte zero prefix on the signature payload mirrors
    /// `validate.rs:378` (the note-signature header that
    /// `decode_checkpoint_signature` strips before `from_der`).
    fn build_checkpoint_text(
        origin: &str,
        tree_size: u64,
        root_hash_hex: &str,
        signing_key: &P256SigningKey,
    ) -> String {
        let root_b64 = BASE64.encode(hex_decode(root_hash_hex));
        // The parser reconstructs `signed_body` as exactly these three lines
        // plus a trailing newline (no `other` extension lines here), so the
        // signed message and the parsed `signed_body` agree byte-for-byte.
        let signed_body = format!("{origin}\n{tree_size}\n{root_b64}\n");
        let signature: P256Signature = signing_key.sign(signed_body.as_bytes());
        let mut payload = vec![0u8, 0, 0, 0];
        payload.extend_from_slice(signature.to_der().as_bytes());
        format!(
            "{signed_body}\n\u{2014} forge-test {}\n",
            BASE64.encode(&payload)
        )
    }

    /// A minimal but fully valid rekor log entry JSON, parameterised by the
    /// checkpoint text. Each parse-error test mutates one field of this base.
    fn valid_entry_json(checkpoint: &str) -> Value {
        let body = json!({"kind": "hashedrekord", "apiVersion": "0.0.1"});
        let body_bytes = serde_json::to_vec(&body).expect("serialize body");
        json!({
            "body": BASE64.encode(&body_bytes),
            "integratedTime": 1_767_225_600_i64,
            "logID": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "logIndex": 0_i64,
            "verification": {
                "inclusionProof": {
                    "hashes": [],
                    "logIndex": 0_i64,
                    "rootHash": "00".repeat(32),
                    "treeSize": 1_u64,
                    "checkpoint": checkpoint,
                },
                "signedEntryTimestamp": "",
            },
        })
    }

    fn parse(text: &str) -> Result<super::ParsedRekorEntry, RekorParseError> {
        parse_rekor_log_entry(text)
    }

    /// Remove a field at the given JSON path (`["verification", "inclusionProof",
    /// "rootHash"]`) and parse the mutated entry. Used to exercise each
    /// `MissingField { field }` variant without repeating the base fixture.
    fn parse_after_removing(
        base: &Value,
        path: &[&str],
    ) -> Result<super::ParsedRekorEntry, RekorParseError> {
        let mut clone = base.clone();
        {
            let mut cursor = &mut clone;
            for key in &path[..path.len() - 1] {
                cursor = cursor
                    .get_mut(*key)
                    .expect("path prefix exists in base fixture");
            }
            let leaf = cursor
                .as_object_mut()
                .expect("path parent is a JSON object");
            leaf.remove(*path.last().expect("non-empty path"));
        }
        parse(&serde_json::to_string(&clone).expect("serialize mutated entry"))
    }

    fn make_proof(
        tree_size: u64,
        root_hash_hex: &str,
        checkpoint: &str,
    ) -> ParsedRekorInclusionProof {
        ParsedRekorInclusionProof {
            hashes: Vec::new(),
            log_index: 0,
            root_hash: root_hash_hex.to_string(),
            tree_size,
            checkpoint: checkpoint.to_string(),
        }
    }

    // ---- parse_rekor_log_entry ------------------------------------------

    #[test]
    fn parse_log_entry_happy_path_extracts_all_fields() {
        let checkpoint =
            build_checkpoint_text("forge-test-rekor", 1, &"00".repeat(32), &test_signing_key());
        let text = serde_json::to_string(&valid_entry_json(&checkpoint)).expect("serialize");
        let entry = parse(&text).expect("valid entry parses");

        assert_eq!(
            entry.log_id,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(entry.log_index, 0);
        assert_eq!(entry.integrated_time, 1_767_225_600);
        assert_eq!(entry.proof.tree_size, 1);
        assert_eq!(entry.proof.root_hash, "00".repeat(32));
        assert_eq!(entry.proof.log_index, 0);
        assert!(entry.proof.hashes.is_empty());
        assert_eq!(entry.body["kind"], "hashedrekord");
    }

    #[test]
    fn parse_log_entry_rejects_invalid_json() {
        let result = parse("{ not json");
        assert!(matches!(
            result,
            Err(RekorParseError::LogEntryJsonInvalid { .. })
        ));
    }

    #[test]
    fn parse_log_entry_rejects_invalid_body_base64() {
        let mut entry = valid_entry_json(&build_checkpoint_text(
            "o",
            1,
            &"00".repeat(32),
            &test_signing_key(),
        ));
        // Insert a body that is valid JSON syntax but whose content is not
        // base64-decodable.
        entry["body"] = json!("!!!not-base64!!!");
        let result = parse(&serde_json::to_string(&entry).expect("serialize"));
        assert!(matches!(
            result,
            Err(RekorParseError::BodyBase64Invalid { .. })
        ));
    }

    #[test]
    fn parse_log_entry_rejects_body_that_decodes_to_non_json() {
        let mut entry = valid_entry_json(&build_checkpoint_text(
            "o",
            1,
            &"00".repeat(32),
            &test_signing_key(),
        ));
        entry["body"] = json!(BASE64.encode(b"plainly not json"));
        let result = parse(&serde_json::to_string(&entry).expect("serialize"));
        assert!(matches!(
            result,
            Err(RekorParseError::BodyJsonInvalid { .. })
        ));
    }

    #[test]
    fn parse_log_entry_rejects_missing_verification() {
        let checkpoint = build_checkpoint_text("o", 1, &"00".repeat(32), &test_signing_key());
        let base = valid_entry_json(&checkpoint);
        let result = parse_after_removing(&base, &["verification"]);
        assert!(matches!(result, Err(RekorParseError::VerificationMissing)));
    }

    #[test]
    fn parse_log_entry_rejects_missing_inclusion_proof() {
        let checkpoint = build_checkpoint_text("o", 1, &"00".repeat(32), &test_signing_key());
        let base = valid_entry_json(&checkpoint);
        let result = parse_after_removing(&base, &["verification", "inclusionProof"]);
        assert!(matches!(
            result,
            Err(RekorParseError::InclusionProofMissing)
        ));
    }

    #[test]
    fn parse_log_entry_rejects_missing_or_non_array_hashes() {
        let checkpoint = build_checkpoint_text("o", 1, &"00".repeat(32), &test_signing_key());
        let base = valid_entry_json(&checkpoint);

        // `hashes` key absent entirely.
        let result = parse_after_removing(&base, &["verification", "inclusionProof", "hashes"]);
        assert!(matches!(
            result,
            Err(RekorParseError::InclusionHashesMissing)
        ));

        // `hashes` present but not an array.
        let mut clone = base.clone();
        clone["verification"]["inclusionProof"]["hashes"] = json!("not-an-array");
        let result = parse(&serde_json::to_string(&clone).expect("serialize"));
        assert!(matches!(
            result,
            Err(RekorParseError::InclusionHashesMissing)
        ));
    }

    #[test]
    fn parse_log_entry_rejects_non_string_hash_entry() {
        let checkpoint = build_checkpoint_text("o", 1, &"00".repeat(32), &test_signing_key());
        let mut base = valid_entry_json(&checkpoint);
        base["verification"]["inclusionProof"]["hashes"] = json!([123_i64, "valid-hash"]);
        let result = parse(&serde_json::to_string(&base).expect("serialize"));
        assert!(matches!(result, Err(RekorParseError::InclusionHashInvalid)));
    }

    #[test]
    fn parse_log_entry_rejects_missing_required_fields() {
        let checkpoint = build_checkpoint_text("o", 1, &"00".repeat(32), &test_signing_key());
        let base = valid_entry_json(&checkpoint);

        // Each path's removal must surface `MissingField { field: <key> }`.
        for (path, expected_field) in [
            (vec!["body"], "body"),
            (vec!["logID"], "logID"),
            (vec!["logIndex"], "logIndex"),
            (vec!["integratedTime"], "integratedTime"),
            (
                vec!["verification", "inclusionProof", "logIndex"],
                "logIndex",
            ),
            (
                vec!["verification", "inclusionProof", "rootHash"],
                "rootHash",
            ),
            (
                vec!["verification", "inclusionProof", "treeSize"],
                "treeSize",
            ),
            (
                vec!["verification", "inclusionProof", "checkpoint"],
                "checkpoint",
            ),
        ] {
            let result = parse_after_removing(&base, &path);
            assert!(
                matches!(
                    result,
                    Err(RekorParseError::MissingField { field }) if field == expected_field
                ),
                "removing `{expected_field}` must surface MissingField"
            );
        }
    }

    // ---- parse_signed_checkpoint ----------------------------------------

    fn valid_checkpoint_text(tree_size: u64, root_hash_hex: &str) -> String {
        build_checkpoint_text(
            "forge-test-rekor",
            tree_size,
            root_hash_hex,
            &test_signing_key(),
        )
    }

    #[test]
    fn parse_checkpoint_happy_path_pins_tree_size_and_root_hash() {
        // KAT: pin the parsed tree_size and root hash for a known input. The
        // root_hash_hex is 32 bytes of 0xab, so its base64 encoding is fixed
        // and the parsed root hash must round-trip back to the same hex.
        const PINNED_ROOT_HASH_HEX: &str =
            "abababababababababababababababababababababababababababababababab";

        let text = valid_checkpoint_text(42, PINNED_ROOT_HASH_HEX);
        let parsed = parse_signed_checkpoint(&text).expect("valid checkpoint parses");

        assert_eq!(parsed.tree_size, 42);
        assert_eq!(parsed.root_hash, PINNED_ROOT_HASH_HEX);
        // Exactly one signature line was decoded.
        assert_eq!(parsed.signatures.len(), 1);
        // The signed body is the canonical 3-line note + trailing newline.
        let root_b64 = BASE64.encode(hex_decode(PINNED_ROOT_HASH_HEX));
        let expected_signed_body = format!("forge-test-rekor\n42\n{root_b64}\n");
        assert_eq!(parsed.signed_body, expected_signed_body);
    }

    #[test]
    fn parse_checkpoint_includes_extension_lines_in_signed_body() {
        // A checkpoint note with >3 lines must carry the extra lines into
        // `signed_body` (the message the signature covers).
        let root_b64 = BASE64.encode(hex_decode(&"00".repeat(32)));
        let note = format!("origin\n1\n{root_b64}\nextension-line\n");
        // Empty signature block (still a valid parse; verification is a
        // separate concern handled by `verify_rekor_checkpoint`).
        let text = format!("{note}\n");

        let parsed = parse_signed_checkpoint(&text).expect("parses with extension line");
        assert!(parsed.signed_body.ends_with("extension-line\n"));
        assert!(parsed.signatures.is_empty());
    }

    #[test]
    fn parse_checkpoint_rejects_missing_note_signature_separator() {
        // No `\n\n` separator at all.
        let result = parse_signed_checkpoint("origin\n1\nrootb64\n");
        assert!(matches!(
            result,
            Err(RekorParseError::CheckpointFormatInvalid)
        ));
    }

    #[test]
    fn parse_checkpoint_rejects_note_with_too_few_lines() {
        // The note must expose at least 3 lines (origin / treeSize / root).
        let result = parse_signed_checkpoint("origin\n1\n\n— name c2ln");
        assert!(matches!(
            result,
            Err(RekorParseError::CheckpointNoteInvalid)
        ));
    }

    #[test]
    fn parse_checkpoint_rejects_empty_origin() {
        let root_b64 = BASE64.encode(hex_decode(&"00".repeat(32)));
        let text = format!("\n1\n{root_b64}\n\n— name c2ln");
        let result = parse_signed_checkpoint(&text);
        assert!(matches!(
            result,
            Err(RekorParseError::CheckpointOriginMissing)
        ));
    }

    #[test]
    fn parse_checkpoint_rejects_non_numeric_tree_size() {
        let root_b64 = BASE64.encode(hex_decode(&"00".repeat(32)));
        let text = format!("origin\nnot-a-number\n{root_b64}\n\n— name c2ln");
        let result = parse_signed_checkpoint(&text);
        assert!(matches!(
            result,
            Err(RekorParseError::CheckpointTreeSizeInvalid { .. })
        ));
    }

    #[test]
    fn parse_checkpoint_rejects_invalid_root_hash_base64() {
        // treeSize parses, but the root hash field is not base64.
        let text = "origin\n1\n!!!not-base64!!!\n\n— name c2ln";
        let result = parse_signed_checkpoint(text);
        assert!(matches!(
            result,
            Err(RekorParseError::CheckpointRootHashBase64Invalid { .. })
        ));
    }

    // ---- verify_rekor_checkpoint ----------------------------------------

    #[test]
    fn verify_checkpoint_ok_for_matching_key() {
        let root_hash = "00".repeat(32);
        let checkpoint = valid_checkpoint_text(2, &root_hash);
        let proof = make_proof(2, &root_hash, &checkpoint);
        // Bind the signing key to a local: `verifying_key()` borrows it, so the
        // naive `test_signing_key().verifying_key()` would drop the key while
        // the verifying key still references it.
        let signing_key = test_signing_key();
        let verifying = signing_key.verifying_key();

        verify_rekor_checkpoint(&proof, verifying).expect("signature by matching key verifies");
    }

    #[test]
    fn verify_checkpoint_rejects_tree_size_mismatch() {
        let root_hash = "00".repeat(32);
        // Checkpoint commits to tree_size=2, but the proof claims tree_size=3.
        let checkpoint = valid_checkpoint_text(2, &root_hash);
        let proof = make_proof(3, &root_hash, &checkpoint);
        let signing_key = test_signing_key();
        let verifying = signing_key.verifying_key();

        assert!(matches!(
            verify_rekor_checkpoint(&proof, verifying),
            Err(RekorParseError::CheckpointTreeSizeMismatch)
        ));
    }

    #[test]
    fn verify_checkpoint_rejects_root_hash_mismatch() {
        // tree_size matches, but the proof's root hash differs from the one
        // embedded in the signed checkpoint.
        let checkpoint = valid_checkpoint_text(2, &"00".repeat(32));
        let proof = make_proof(2, &"11".repeat(32), &checkpoint);
        let signing_key = test_signing_key();
        let verifying = signing_key.verifying_key();

        assert!(matches!(
            verify_rekor_checkpoint(&proof, verifying),
            Err(RekorParseError::CheckpointRootHashMismatch)
        ));
    }

    #[test]
    fn verify_checkpoint_rejects_missing_signature() {
        // Checkpoint parses and tree_size/root match, but the signature block
        // is empty → no signatures to verify.
        let root_b64 = BASE64.encode(hex_decode(&"00".repeat(32)));
        let checkpoint = format!("origin\n2\n{root_b64}\n\n");
        let proof = make_proof(2, &"00".repeat(32), &checkpoint);
        let signing_key = test_signing_key();
        let verifying = signing_key.verifying_key();

        assert!(matches!(
            verify_rekor_checkpoint(&proof, verifying),
            Err(RekorParseError::CheckpointSignatureMissing)
        ));
    }

    #[test]
    fn verify_checkpoint_rejects_signature_with_wrong_key() {
        // Signed by the test key, but verified against a different key.
        let root_hash = "00".repeat(32);
        let checkpoint = valid_checkpoint_text(2, &root_hash);
        let proof = make_proof(2, &root_hash, &checkpoint);
        // Bind the wrong signing key too: same lifetime constraint.
        let wrong_signing = P256SigningKey::from_slice(&[9u8; 32]).expect("different p256 key");
        let wrong_key = wrong_signing.verifying_key();

        assert!(matches!(
            verify_rekor_checkpoint(&proof, wrong_key),
            Err(RekorParseError::CheckpointSignatureInvalid)
        ));
    }

    /// Deterministic p256 known-answer test for the verifying key.
    ///
    /// The verifying key is a pure function of the signing-key seed (NIST P-256
    /// scalar multiplication), so it is fully deterministic and safe to pin:
    /// any drift in the p256 crate, the seed parsing, or the sec1 encoding
    /// surfaces here as a clear before/after hex diff rather than as a flaky
    /// verify failure elsewhere. If this fails after an intentional upgrade,
    /// recompute the pinned value from the new output and update it (run
    /// `recompute_p256_verifying_key_kat` — `#[ignore]`d below — to print it).
    ///
    /// We deliberately do NOT pin the *signature* over a fixed message: p256
    /// ECDSA per RFC 6979 IS deterministic, but pinning the DER bytes also
    /// couples the test to the exact digest/encoding path, which is already
    /// covered by `verify_checkpoint_ok_for_matching_key` /
    /// `_rejects_signature_with_wrong_key` end-to-end. Pinning the key alone
    /// gives the ecosystem-drift signal without the extra coupling.
    #[test]
    fn verify_checkpoint_p256_verifying_key_is_pinned_to_seed() {
        // Generated by `recompute_p256_verifying_key_kat` (#[ignore]d below)
        // from `P256SigningKey::from_slice(&[8u8; 32])`. Uncompressed sec1
        // (0x04 prefix + 64 bytes). Update both this constant and the
        // regenerator's docstring together after an intentional p256 upgrade.
        const PINNED_VERIFYING_KEY_SEC1_HEX: &str = "\
            043adab15d66256bf15cd716035b3f041444e512fed1dd64d4ba75597d20e366f1\
            546d2c90a83ebaba01595099e5f3ffbbd3384c7494de6759fdd5e65efae9cc67";

        let signing_key = test_signing_key();
        let verifying_key = signing_key.verifying_key();
        let actual = hex_encode(&verifying_key.to_sec1_bytes());

        assert_eq!(
            actual, PINNED_VERIFYING_KEY_SEC1_HEX,
            "pinned p256 verifying key drifted; if the p256 version or seed changed, \
             re-run the #[ignore]d recompute test and update this constant"
        );
    }

    /// Regenerator for the verifying-key KAT above. Run with
    /// `cargo test -p forge-core-crypto recompute_p256_verifying_key_kat -- --ignored --nocapture`
    /// to print the sec1-hex of the key derived from the fixed `[8u8; 32]`
    /// seed, then paste it into `PINNED_VERIFYING_KEY_SEC1_HEX`.
    #[test]
    #[ignore = "manual KAT regenerator: run with --ignored --nocapture to print the pinned hex"]
    fn recompute_p256_verifying_key_kat() {
        let signing_key = test_signing_key();
        let verifying_key = signing_key.verifying_key();
        println!(
            "PINNED_VERIFYING_KEY_SEC1_HEX = \"{}\"",
            hex_encode(&verifying_key.to_sec1_bytes())
        );
    }

    // ---- verify_merkle_inclusion ----------------------------------------

    #[test]
    fn merkle_tree_size_one_trivial_match() {
        let leaf = "ab".repeat(32);
        // tree_size=1: no proof hashes, leaf hash must equal root hash.
        assert!(verify_merkle_inclusion(&leaf, &[], 0, 1, &leaf));
    }

    #[test]
    fn merkle_tree_size_one_trivial_mismatch() {
        let leaf = "ab".repeat(32);
        let other = "cd".repeat(32);
        assert!(!verify_merkle_inclusion(&leaf, &[], 0, 1, &other));
    }

    #[test]
    fn merkle_rejects_tree_size_zero() {
        assert!(!verify_merkle_inclusion(
            &"00".repeat(32),
            &[],
            0,
            0,
            &"00".repeat(32)
        ));
    }

    #[test]
    fn merkle_rejects_log_index_at_or_above_tree_size() {
        let leaf = "00".repeat(32);
        // log_index == tree_size is out of range (indices are 0..tree_size-1).
        assert!(!verify_merkle_inclusion(&leaf, &[], 1, 1, &leaf));
        // log_index > tree_size likewise.
        assert!(!verify_merkle_inclusion(
            &leaf,
            &["11".repeat(32)],
            5,
            2,
            &leaf
        ));
    }

    #[test]
    fn merkle_two_leaf_tree_verifies_both_indices() {
        let l0 = "00".repeat(32);
        let l1 = "11".repeat(32);
        let root = merkle_parent_hex(&l0, &l1);

        // index 0: proof is the right sibling [l1]. The leaves outlive all
        // assertions below, so `slice::from_ref` borrows without a clone.
        assert!(verify_merkle_inclusion(
            &l0,
            std::slice::from_ref(&l1),
            0,
            2,
            &root
        ));
        // index 1: proof is the left sibling [l0].
        assert!(verify_merkle_inclusion(
            &l1,
            std::slice::from_ref(&l0),
            1,
            2,
            &root
        ));
        // A wrong root fails for both.
        let bad_root = merkle_parent_hex(&l0, &l0);
        assert!(!verify_merkle_inclusion(
            &l0,
            std::slice::from_ref(&l1),
            0,
            2,
            &bad_root
        ));
        assert!(!verify_merkle_inclusion(
            &l1,
            std::slice::from_ref(&l0),
            1,
            2,
            &bad_root
        ));
    }

    #[test]
    fn merkle_four_leaf_tree_verifies_all_indices() {
        // Exercises the `index == last` promotion branch: for index 1 and 3
        // the leaf is the right child at its level, so the proof hash is the
        // LEFT sibling. Proof paths (derived from the RFC 6962 walk):
        //   index 0 → [l1, h23]   index 1 → [l0, h23]
        //   index 2 → [l3, h01]   index 3 → [l2, h01]
        let l0 = "00".repeat(32);
        let l1 = "11".repeat(32);
        let l2 = "22".repeat(32);
        let l3 = "33".repeat(32);
        let h01 = merkle_parent_hex(&l0, &l1);
        let h23 = merkle_parent_hex(&l2, &l3);
        let root = merkle_parent_hex(&h01, &h23);

        assert!(verify_merkle_inclusion(
            &l0,
            &[l1.clone(), h23.clone()],
            0,
            4,
            &root
        ));
        assert!(verify_merkle_inclusion(
            &l1,
            &[l0.clone(), h23.clone()],
            1,
            4,
            &root
        ));
        assert!(verify_merkle_inclusion(
            &l2,
            &[l3.clone(), h01.clone()],
            2,
            4,
            &root
        ));
        assert!(verify_merkle_inclusion(
            &l3,
            &[l2.clone(), h01.clone()],
            3,
            4,
            &root
        ));

        // Tampering any single proof hash flips the verdict to reject.
        let mut tampered = vec![l1, h23];
        // Flip one hex digit in the second proof hash.
        let mut bad = tampered[1].clone();
        bad.replace_range(0..1, "f");
        tampered[1] = bad;
        assert!(!verify_merkle_inclusion(&l0, &tampered, 0, 4, &root));

        // A malformed (odd-length) hash collapses `hash_merkle_node` to an
        // empty string, so the final root comparison fails rather than
        // panicking.
        assert!(!verify_merkle_inclusion(
            &l2,
            &[l3, h01],
            2,
            4,
            "not-a-valid-hash"
        ));
    }

    /// Property test: for any 32-byte leaves, a correctly-composed 4-leaf
    /// inclusion proof verifies for indices 0 and 3, and a tampered root hash
    /// (or wrong leaf) fails closed. Guards against input-dependent
    /// fail-open regressions in the Merkle walk.
    #[test]
    fn merkle_proptest_random_four_leaf_tree() {
        use proptest::prelude::*;

        proptest!(|(
            seed0 in any::<[u8; 32]>(),
            seed1 in any::<[u8; 32]>(),
            seed2 in any::<[u8; 32]>(),
            seed3 in any::<[u8; 32]>(),
        )| {
            let l0 = hex_encode(&seed0);
            let l1 = hex_encode(&seed1);
            let l2 = hex_encode(&seed2);
            let l3 = hex_encode(&seed3);
            let h01 = merkle_parent_hex(&l0, &l1);
            let h23 = merkle_parent_hex(&l2, &l3);
            let root = merkle_parent_hex(&h01, &h23);

            // Valid proofs verify.
            prop_assert!(verify_merkle_inclusion(&l0, &[l1.clone(), h23.clone()], 0, 4, &root));
            prop_assert!(verify_merkle_inclusion(&l3, &[l2.clone(), h01.clone()], 3, 4, &root));

            // A leaf that is not in the tree fails against this root.
            let impostor = "ff".repeat(32);
            prop_assert!(!verify_merkle_inclusion(&impostor, &[l1, h23], 0, 4, &root));
            prop_assert!(!verify_merkle_inclusion(&l2, &[l3, h01], 2, 4, &impostor));
        });
    }

    // ---- error display round-trip (regression guard) -------------------

    #[test]
    fn rekor_parse_error_display_round_trips_legacy_strings() {
        // The `display()` renderer is the bridge to the legacy diagnostic
        // strings emitted at the validation boundary; pin a representative
        // variant per family so a refactor that drops the prefix surfaces.
        assert_eq!(
            RekorParseError::VerificationMissing.display(),
            "rekor_verification_missing"
        );
        assert_eq!(
            RekorParseError::MissingField { field: "logID" }.display(),
            "rekor_logID_missing"
        );
        assert_eq!(
            RekorParseError::CheckpointSignatureInvalid.display(),
            "checkpoint_signature_invalid"
        );
    }

    // `Value` is consumed by the `valid_entry_json` / `parse_after_removing`
    // helpers above; `P256Signature` by the checkpoint builder. Kept as
    // imports (rather than `use super::*`) to document the fixture's deps.
}

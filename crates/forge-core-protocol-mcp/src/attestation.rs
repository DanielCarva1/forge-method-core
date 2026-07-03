//! Tool-Call Attestation — detached ed25519 signature over the canonicalized
//! tool-call intent (ADR-0006 Decision 4).
//!
//! stdio JSON-RPC carries no HTTP headers, so the proof of *who called* must
//! ride in the request body (in the MCP `_meta` field). A Tool-Call
//! Attestation is a detached ed25519 signature over:
//!
//! ```text
//! canonical = serde_json_canonicalizer::canon({
//!     "tool": <tool_name>,
//!     "arguments": <arguments_object>,
//!     "nonce": <opaque>,
//!     "ts": <unix_seconds>
//! })
//! sig = ed25519.sign(caller_private_key, canonical)
//! ```
//!
//! The verifier re-canonicalizes the intent and checks the signature against a
//! configured authorized public key. The signature proves possession of the
//! private key (origin) and binds to the intent (non-replayable for another
//! tool/args). `ed25519-dalek` is used directly — it is already pinned in the
//! workspace (`Cargo.toml` `[workspace.dependencies]`), reusing the same
//! crypto surface as the rest of Forge (R5).
//!
//! # F08.2 scope
//!
//! Types + canonicalization + verify primitive. The policy enforcement
//! (required-for-mutate, optional-for-readonly default; ADR-0006 Decision 4)
//! is wired into the server's `MutateGate` in F08.3/F08.5.

use std::fmt;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The canonicalized intent that a Tool-Call Attestation signs. Serialized
/// with `serde_json_canonicalizer` so the signature is reproducible across
/// platforms / map orderings (the same canonicalizer used elsewhere in Forge
/// for signed artifacts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalIntent {
    /// The MCP tool name being called.
    pub tool: String,
    /// The MCP arguments object (the exact `arguments` field of `tools/call`).
    pub arguments: Value,
    /// Caller-supplied opaque nonce; the verifier does not interpret it, only
    /// includes it in the signed canonical form (replay binding within a
    /// caller-defined window). Future hardening may track nonces for replay
    /// prevention; F08 leaves that to the caller policy.
    pub nonce: String,
    /// Caller-supplied unix timestamp (seconds). Included in the signed form so
    /// the signature is time-bound; freshness windows are caller policy.
    pub ts: i64,
}

impl CanonicalIntent {
    /// Serialize to canonical JSON bytes (deterministic, JCS-style ordering).
    ///
    /// # Errors
    ///
    /// Returns [`AttestationError::Canonicalize`] if canonicalization fails
    /// (should be impossible for JSON-derived input; surfaces a contract bug).
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AttestationError> {
        let ser = serde_json::to_value(self)
            .map_err(|e| AttestationError::Canonicalize(e.to_string()))?;
        let canon = serde_json_canonicalizer::to_vec(&ser)
            .map_err(|e| AttestationError::Canonicalize(e.to_string()))?;
        Ok(canon)
    }
}

/// The attestation material carried in the MCP `_meta.attestation` field. The
/// signature is over `CanonicalIntent::canonical_bytes()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationInput {
    /// The caller's nonce (must match the signed canonical form).
    pub nonce: String,
    /// The caller's unix timestamp (must match the signed canonical form).
    pub ts: i64,
    /// hex-encoded detached ed25519 signature.
    pub signature: String,
    /// hex-encoded caller public key (ed25519 `VerifyingKey`, 32 bytes).
    /// Alternatively the authorized key could be looked up by a caller id;
    /// F08 keeps it inline for a self-contained proof.
    pub public_key_hex: String,
}

/// Policy for when Tool-Call Attestation is required (ADR-0006 Decision 4):
/// required for mutate, optional for read-only under the default policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttestationPolicy {
    /// Default: attestation required for mutate tools, optional for read-only.
    /// This is the policy the server uses unless an operator hardens it.
    Default,
    /// Hardened: attestation required for ALL tools (read-only included).
    RequireAll,
    /// Permissive: attestation never required (verify-only-when-present). Use
    /// only in trusted local setups; never the default.
    NeverRequired,
}

impl AttestationPolicy {
    /// Whether an attestation is required for a tool of the given mutate-ness.
    #[must_use]
    pub fn requires_for(self, is_mutate: bool) -> bool {
        match self {
            Self::Default => is_mutate,
            Self::RequireAll => true,
            Self::NeverRequired => false,
        }
    }
}

/// Failures verifying a Tool-Call Attestation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationError {
    /// The signature hex was malformed (not 128 hex chars).
    SignatureDecode(String),
    /// The public key hex was malformed (not 64 hex chars / 32 bytes).
    KeyDecode(String),
    /// Canonicalization of the intent failed.
    Canonicalize(String),
    /// The signature did not verify against the public key for this intent.
    Invalid,
}

impl fmt::Display for AttestationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignatureDecode(m) => write!(f, "signature decode failed: {m}"),
            Self::KeyDecode(m) => write!(f, "public key decode failed: {m}"),
            Self::Canonicalize(m) => write!(f, "canonicalization failed: {m}"),
            Self::Invalid => f.write_str("attestation signature invalid"),
        }
    }
}

impl std::error::Error for AttestationError {}

/// The outcome of the attestation gate at the MCP boundary (ADR-0006 Decision
/// 4). `None` means the gate passed (attestation present+valid, OR not
/// required for this tool class). `Some` means rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationGateOutcome {
    /// The policy required an attestation for this tool but none was present.
    RequiredMissing,
    /// An attestation was present (or required) but failed verification. The
    /// carried string is the lossy reason.
    Invalid(String),
}

/// Verifier for Tool-Call Attestations. Holds no mutable state; constructed
/// once per server from the policy.
#[derive(Debug, Clone)]
pub struct AttestationVerifier {
    policy: AttestationPolicy,
}

impl AttestationVerifier {
    #[must_use]
    pub fn new(policy: AttestationPolicy) -> Self {
        Self { policy }
    }

    /// The configured policy.
    #[must_use]
    pub fn policy(&self) -> AttestationPolicy {
        self.policy
    }

    /// Whether an attestation is required for a tool of the given
    /// mutate-ness, under this verifier's policy.
    #[must_use]
    pub fn requires(&self, is_mutate: bool) -> bool {
        self.policy.requires_for(is_mutate)
    }

    /// Verify a Tool-Call Attestation against the caller-provided public key.
    ///
    /// This is a **signature check only** (origin proof). Whether the public
    /// key is *authorized* is a separate concern (the server configures the set
    /// of authorized keys); this function does not consult that set. Callers
    /// must additionally check that `attestation.public_key_hex` belongs to an
    /// authorized principal before treating the verification as proof of an
    /// authorized caller.
    ///
    /// # Errors
    ///
    /// Returns [`AttestationError`] on decode/canonicalization failure, or
    /// [`AttestationError::Invalid`] if the signature does not verify.
    pub fn verify(
        &self,
        intent: &CanonicalIntent,
        attestation: &AttestationInput,
    ) -> Result<(), AttestationError> {
        // Cross-check: the attestation's nonce/ts must match the signed intent.
        if intent.nonce != attestation.nonce || intent.ts != attestation.ts {
            return Err(AttestationError::Invalid);
        }

        let canon = intent.canonical_bytes()?;

        let mut pk_bytes = [0u8; 32];
        let pk_decoded = hex_decode(&attestation.public_key_hex)
            .map_err(|e| AttestationError::KeyDecode(e.clone()))?;
        if pk_decoded.len() != 32 {
            return Err(AttestationError::KeyDecode(format!(
                "expected 32 bytes, got {}",
                pk_decoded.len()
            )));
        }
        pk_bytes.copy_from_slice(&pk_decoded);
        let verifying_key = VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|e| AttestationError::KeyDecode(e.to_string()))?;

        let mut sig_bytes = [0u8; 64];
        let sig_decoded = hex_decode(&attestation.signature)
            .map_err(|e| AttestationError::SignatureDecode(e.clone()))?;
        if sig_decoded.len() != 64 {
            return Err(AttestationError::SignatureDecode(format!(
                "expected 64 bytes, got {}",
                sig_decoded.len()
            )));
        }
        sig_bytes.copy_from_slice(&sig_decoded);
        let signature = Signature::from_bytes(&sig_bytes);

        verifying_key
            .verify(&canon, &signature)
            .map_err(|_| AttestationError::Invalid)
    }
}

/// Minimal hex decoder (no extra dep). `hex` is not in the workspace; this
/// keeps the dep surface minimal. Mirrors the inline hex helpers used in
/// `forge-core-crypto` test paths.
fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err(format!("odd-length hex ({})", s.len()));
    }
    let bytes = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(bytes)
}

/// Hex-encode a byte slice (used in tests / signing helpers).
#[allow(dead_code)] // used only under #[cfg(test)] (signing helpers)
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        // write! into a String never fails.
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::RngCore;

    fn fresh_signing_key() -> SigningKey {
        // ed25519-dalek 2.x removed SigningKey::generate; construct from 32
        // random bytes (rand::rngs::OsRng via RngCore). We use the dev-only
        // `rand` crate rather than enabling the `rand_core` feature on the
        // workspace ed25519-dalek pin (other crates depend on it unchanged).
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        SigningKey::from_bytes(&bytes)
    }

    fn sign_intent(intent: &CanonicalIntent, signing_key: &SigningKey) -> AttestationInput {
        // Test-only signing helper: the intent was just built from JSON above,
        // so canonical_bytes() is infallible here. This is NOT the verification
        // path — the security boundary lives in `AttestationVerifier::verify`
        // (line ~202), which uses `?` and returns AttestationError::Canonicalize
        // on failure → fail-closed rejection at the server's attestation gate.
        let canon = intent.canonical_bytes().expect("canonicalize test intent");
        let sig = signing_key.sign(&canon);
        let pk = signing_key.verifying_key();
        AttestationInput {
            nonce: intent.nonce.clone(),
            ts: intent.ts,
            signature: hex_encode(&sig.to_bytes()),
            public_key_hex: hex_encode(&pk.to_bytes()),
        }
    }

    #[test]
    fn valid_signature_verifies() {
        let sk = fresh_signing_key();
        let intent = CanonicalIntent {
            tool: "preview".into(),
            arguments: serde_json::json!({"--root": "/tmp/x"}),
            nonce: "n-1".into(),
            ts: 1_700_000_000,
        };
        let att = sign_intent(&intent, &sk);
        let verifier = AttestationVerifier::new(AttestationPolicy::Default);
        assert!(verifier.verify(&intent, &att).is_ok());
    }

    #[test]
    fn tampered_arguments_fail() {
        let sk = fresh_signing_key();
        let intent = CanonicalIntent {
            tool: "preview".into(),
            arguments: serde_json::json!({"--root": "/tmp/x"}),
            nonce: "n-1".into(),
            ts: 1_700_000_000,
        };
        let att = sign_intent(&intent, &sk);
        // Tamper: change arguments after signing.
        let tampered = CanonicalIntent {
            arguments: serde_json::json!({"--root": "/tmp/other"}),
            ..intent.clone()
        };
        let verifier = AttestationVerifier::new(AttestationPolicy::Default);
        assert_eq!(
            verifier.verify(&tampered, &att),
            Err(AttestationError::Invalid)
        );
    }

    #[test]
    fn mismatched_nonce_ts_rejected() {
        let sk = fresh_signing_key();
        let intent = CanonicalIntent {
            tool: "preview".into(),
            arguments: serde_json::json!({}),
            nonce: "n-1".into(),
            ts: 1_700_000_000,
        };
        let att = sign_intent(&intent, &sk);
        // Cross-check failure: attestation claims a different nonce.
        let bad_att = AttestationInput {
            nonce: "different".into(),
            ..att
        };
        let verifier = AttestationVerifier::new(AttestationPolicy::Default);
        assert_eq!(
            verifier.verify(&intent, &bad_att),
            Err(AttestationError::Invalid)
        );
    }

    #[test]
    fn default_policy_requires_only_for_mutate() {
        assert!(!AttestationPolicy::Default.requires_for(false));
        assert!(AttestationPolicy::Default.requires_for(true));
        assert!(AttestationPolicy::RequireAll.requires_for(false));
        assert!(!AttestationPolicy::NeverRequired.requires_for(true));
    }

    #[test]
    fn malformed_signature_rejected() {
        let intent = CanonicalIntent {
            tool: "preview".into(),
            arguments: serde_json::json!({}),
            nonce: "n".into(),
            ts: 1,
        };
        let att = AttestationInput {
            nonce: "n".into(),
            ts: 1,
            signature: "zz".into(), // not hex
            public_key_hex: "00".repeat(32),
        };
        let verifier = AttestationVerifier::new(AttestationPolicy::Default);
        assert!(matches!(
            verifier.verify(&intent, &att),
            Err(AttestationError::SignatureDecode(_))
        ));
    }
}

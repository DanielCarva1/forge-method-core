//! Shared SHA-256 / hex helpers.
//!
//! Centralises the small set of hashing/display primitives that are used by
//! multiple crypto domains in this crate (rekor, slsa, x509, payloads). The
//! helpers live here so that [`rekor`](crate::rekor),
//! [`sigstore`](crate::sigstore), and the host-adapter verification
//! entrypoints in [`host_adapter_verification`](crate::host_adapter_verification)
//! all see the same canonical implementations.
//!
//! All items are `pub`: they are consumed across modules inside this crate
//! and the CLI crate re-exports a subset of them.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Lowercase hex SHA-256 of `content`.
#[must_use]
pub fn hex_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

/// Lowercase hex encoding of an arbitrary byte slice.
#[must_use]
pub fn hex_bytes(content: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(content.len() * 2);
    for byte in content {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// `true` if `value` looks like a canonical `sha256:<64 hex>` or bare
/// 64-hex digest.
#[must_use]
pub fn valid_sha256_digest(value: &str) -> bool {
    normalize_sha256_digest(value).is_some()
}

/// Normalise a `sha256:<hex>` / bare-`<hex>` digest to a canonical 64-char
/// lowercase hex form (without prefix), returning `None` if the input is
/// malformed.
#[must_use]
pub fn normalize_sha256_digest(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let digest = trimmed.strip_prefix("sha256:").unwrap_or(trimmed);
    (digest.len() == 64 && digest.chars().all(|item| item.is_ascii_hexdigit()))
        .then(|| digest.to_ascii_lowercase())
}

/// Best-effort lowercase display form for a digest. Strips an optional
/// `sha256:` prefix but, unlike [`normalize_sha256_digest`], does not
/// validate the hex content.
#[must_use]
pub fn normalize_sha256_display(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("sha256:")
        .unwrap_or(trimmed)
        .to_ascii_lowercase()
}

/// Compare two lowercase hex strings in constant time.
///
/// Used for SHA-256 root hashes, OCSP nonces and other secret- or
/// verification-sensitive digests where a naive `==` on `String`/`&str`
/// would short-circuit on the first differing character and leak partial
/// matches through timing. Both inputs are normalised through
/// [`normalize_sha256_display`] before decoding so `sha256:` prefixes and
/// uppercase variants are accepted; any malformed input collapses to a
/// plain `false`.
///
/// Length is leaked (it must, since [`ConstantTimeEq`] refuses mismatched
/// lengths); this is acceptable because the protocol fixes the length of
/// SHA-256 digests (64 hex chars / 32 bytes) and OCSP nonces are typically
/// 16 or 32 bytes. The byte-for-byte comparison itself is constant time.
pub(crate) fn constant_time_eq_hex(left: &str, right: &str) -> bool {
    match (
        decode_hex(&normalize_sha256_display(left)),
        decode_hex(&normalize_sha256_display(right)),
    ) {
        (Some(a), Some(b)) => bool::from(a.ct_eq(&b)),
        _ => false,
    }
}

/// Decode a lowercase/uppercase hex string into bytes. Returns `None` on
/// odd length or any non-hex character. The symmetric inverse of
/// [`hex_bytes`].
fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

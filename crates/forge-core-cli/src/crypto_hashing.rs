//! Shared SHA-256 / hex helpers.
//!
//! Centralises the small set of hashing/display primitives that are used by
//! multiple crypto domains in this crate (rekor, slsa, x509, payloads). The
//! helpers live here so that [`crypto_rekor`](crate::crypto_rekor),
//! [`execute_operation`](crate::execute_operation), and the still-inlined
//! verification routines in `lib.rs` all see the same canonical
//! implementations.
//!
//! All items are `pub(crate)`: this is an internal utility module.

use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of `content`.
pub(crate) fn hex_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

/// Lowercase hex encoding of an arbitrary byte slice.
pub(crate) fn hex_bytes(content: &[u8]) -> String {
    content.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `true` if `value` looks like a canonical `sha256:<64 hex>` or bare
/// 64-hex digest.
pub(crate) fn valid_sha256_digest(value: &str) -> bool {
    normalize_sha256_digest(value).is_some()
}

/// Normalise a `sha256:<hex>` / bare-`<hex>` digest to a canonical 64-char
/// lowercase hex form (without prefix), returning `None` if the input is
/// malformed.
pub(crate) fn normalize_sha256_digest(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let digest = trimmed.strip_prefix("sha256:").unwrap_or(trimmed);
    (digest.len() == 64 && digest.chars().all(|item| item.is_ascii_hexdigit()))
        .then(|| digest.to_ascii_lowercase())
}

/// Best-effort lowercase display form for a digest. Strips an optional
/// `sha256:` prefix but, unlike [`normalize_sha256_digest`], does not
/// validate the hex content.
pub(crate) fn normalize_sha256_display(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("sha256:")
        .unwrap_or(trimmed)
        .to_ascii_lowercase()
}

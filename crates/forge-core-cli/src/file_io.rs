//! File I/O helpers for verification flows.
//!
//! Centralizes the small set of filesystem + decode helpers shared across
//! the host-adapter verification functions in `lib.rs` and the crypto
//! submodules:
//!
//! - `read_required_file` — read raw bytes from disk, pushing a labeled
//!   `{label}_read_failed:{kind}` diagnostic on error (never panics);
//! - `read_signature_file` / `read_public_key_file` — thin wrappers that
//!   delegate to `read_required_file` and then decode the payload as
//!   either raw bytes (already 64 / 32 bytes long) or standard base64;
//! - `decode_base64_or_raw` — shared base64/raw decoder used by the two
//!   wrappers above.
//!
//! All three public helpers are `pub(crate)` and re-exported at the crate
//! root so existing call sites (`crate::read_required_file`,
//! `crate::read_signature_file`, `crate::read_public_key_file`) keep
//! resolving unchanged after the extraction. `decode_base64_or_raw` stays
//! private to this module.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::fs;
use std::path::Path;

pub(crate) fn read_required_file(
    path: &Path,
    label: &str,
    reasons: &mut Vec<String>,
) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            reasons.push(format!("{label}_read_failed:{:?}", err.kind()));
            None
        }
    }
}

pub(crate) fn read_signature_file(path: &Path, reasons: &mut Vec<String>) -> Option<Vec<u8>> {
    read_required_file(path, "signature", reasons)
        .and_then(|bytes| decode_base64_or_raw(bytes, 64, "signature", reasons))
}

pub(crate) fn read_public_key_file(path: &Path, reasons: &mut Vec<String>) -> Option<Vec<u8>> {
    read_required_file(path, "public_key", reasons)
        .and_then(|bytes| decode_base64_or_raw(bytes, 32, "public_key", reasons))
}

fn decode_base64_or_raw(
    bytes: Vec<u8>,
    raw_len: usize,
    label: &str,
    reasons: &mut Vec<String>,
) -> Option<Vec<u8>> {
    if bytes.len() == raw_len {
        return Some(bytes);
    }
    let text = String::from_utf8_lossy(&bytes);
    let compact = text.split_whitespace().collect::<String>();
    match BASE64.decode(compact.as_bytes()) {
        Ok(decoded) if decoded.len() == raw_len => Some(decoded),
        Ok(decoded) => {
            reasons.push(format!("{label}_length_invalid:{}", decoded.len()));
            None
        }
        Err(_) => {
            reasons.push(format!("{label}_base64_invalid"));
            None
        }
    }
}

//! Smoke tests for the zeroize wrappers introduced in R5.x.
//!
//! These tests do not peer into memory (zeroize works by writing zeros
//! before deallocation; observing that without `unsafe` is brittle).
//! Instead they assert the type-level invariants that the zeroize
//! derive macros guarantee: the wrappers implement `Zeroize` and
//! `ZeroizeOnDrop`, they deref to the inner value, and the public
//! newtype serializes transparently so existing JSON consumers are
//! unaffected.

use forge_core_crypto::OcspNonceHex;
use serde_json::json;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Compile-time assertion: `OcspNonceHex` is zeroized on drop.
/// If someone removes the `ZeroizeOnDrop` derive this stops compiling,
/// which is exactly the regression signal we want.
#[test]
fn ocsp_nonce_hex_implements_zeroize_on_drop() {
    fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}
    assert_zeroize_on_drop::<OcspNonceHex>();
}

/// Compile-time assertion: `OcspNonceHex` is `Zeroize`.
#[test]
fn ocsp_nonce_hex_implements_zeroize() {
    fn assert_zeroize<T: Zeroize>() {}
    assert_zeroize::<OcspNonceHex>();
}

/// Construction helpers behave as documented.
#[test]
fn ocsp_nonce_hex_constructors_and_accessors() {
    let from_new = OcspNonceHex::new("aabbcc");
    assert_eq!(from_new.as_str(), "aabbcc");
    assert_eq!(from_new.as_ref(), "aabbcc");

    let from_into: OcspNonceHex = "deadbeef".to_string().into();
    assert_eq!(from_into.as_str(), "deadbeef");

    let direct = OcspNonceHex("cafe".to_string());
    assert_eq!(direct.as_str(), "cafe");
}

/// `OcspNonceHex` serializes as a bare JSON string, preserving the
/// wire format that every JSON consumer of
/// `HostAdapterCertificateOcspStatusVerification` already expects.
#[test]
fn ocsp_nonce_hex_serializes_transparently() {
    let nonce = OcspNonceHex::new("deadbeef");
    let serialized = serde_json::to_value(&nonce).expect("serialize");
    assert_eq!(serialized, json!("deadbeef"));

    let wrapped = serde_json::to_value(Some(nonce)).expect("serialize option");
    assert_eq!(wrapped, json!("deadbeef"));

    let absent: Option<OcspNonceHex> = None;
    let serialized_none = serde_json::to_value(&absent).expect("serialize none");
    assert_eq!(serialized_none, json!(null));
}

/// `Option<OcspNonceHex>` mirrors the deref-based ergonomics of
/// `Option<String>` for the verification call sites that need to read
/// the inner hex as `&str`.
#[test]
fn option_ocsp_nonce_hex_maps_to_str() {
    let nonce = Some(OcspNonceHex::new("deadbeef"));
    let nonce_str: Option<&str> = nonce.as_ref().map(OcspNonceHex::as_str);
    assert_eq!(nonce_str, Some("deadbeef"));

    let missing: Option<OcspNonceHex> = None;
    let missing_str: Option<&str> = missing.as_ref().map(OcspNonceHex::as_str);
    assert_eq!(missing_str, None);
}

/// `Zeroizing<Vec<u8>>` (the wrapper used by `read_required_file` and
/// friends) drops without panicking when the inner value is dropped.
/// This is the absolute minimum sanity check: if the `zeroize` crate
/// feature setup is broken at the workspace level, this test fails.
#[test]
fn zeroizing_vec_drops_cleanly() {
    use zeroize::Zeroizing;
    let bytes = Zeroizing::new(vec![0xFFu8; 32]);
    assert_eq!(bytes.len(), 32);
    drop(bytes);

    let bytes = Zeroizing::new(vec![0u8; 0]);
    assert!(bytes.is_empty());
    drop(bytes);
}

/// `Zeroizing<T>` implements `Deref<Target = T>`, which is what every
/// caller in `host_adapter_verification.rs` relies on after R5.7 made
/// the file readers return `Option<Zeroizing<Vec<u8>>>`.
#[test]
fn zeroizing_vec_derefs_to_vec() {
    use zeroize::Zeroizing;
    let bytes = Zeroizing::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let slice: &[u8] = bytes.as_slice();
    assert_eq!(slice, &[0xDE, 0xAD, 0xBE, 0xEF]);
}

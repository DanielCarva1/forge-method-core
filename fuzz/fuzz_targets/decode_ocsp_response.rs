//! Fuzz target for `forge_core_crypto::ocsp::decode_ocsp_response`.
//!
//! Drives the DER/ASN.1 OCSP response decoder with attacker-controlled
//! bytes. The parser goes through `rasn::der::decode`, which is young and
//! has historically been a source of panics on malformed TLVs. Any panic
//! observed here is a bug in `rasn` or in our handling of its output.

#![no_main]

use forge_core_crypto::ocsp::decode_ocsp_response;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Reuse the parser's own evidence/reason sinks so any panic in the
    // post-decode inspection path is exercised, not just the rasn decode.
    let _ = decode_ocsp_response(data, &mut Vec::new(), &mut Vec::new());
});

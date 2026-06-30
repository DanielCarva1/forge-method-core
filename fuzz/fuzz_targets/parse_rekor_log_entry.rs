//! Fuzz target for `forge_core_crypto::rekor::parse_rekor_log_entry`.
//!
//! Drives the JSON+base64 rekor log entry parser with attacker-controlled
//! text. The parser walks nested JSON, decodes the base64 body, then walks
//! the inner body JSON, and finally reconstructs the inclusion proof. Any
//! panic observed here is a bug in the parser.

#![no_main]

use forge_core_crypto::rekor::parse_rekor_log_entry;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = parse_rekor_log_entry(data);
});

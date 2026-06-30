//! Fuzz target for `forge_core_crypto::rekor::parse_signed_checkpoint`.
//!
//! Drives the note-format parser with attacker-controlled text. The parser
//! splits on `\n\n`, then on `\n`, then parses a tree size and a base64 root
//! hash. Any panic observed here is a bug in the parser.

#![no_main]

use forge_core_crypto::rekor::parse_signed_checkpoint;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = parse_signed_checkpoint(data);
});

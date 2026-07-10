//! Compatibility re-exports for Tool-Call Attestation primitives.
//!
//! Authority construction lives in the host-neutral `forge-core-authority`
//! crate. The MCP adapter keeps this module path so existing Rust consumers do
//! not have to change imports during P4b.2a.

pub use forge_core_authority::attestation::*;

#[cfg(test)]
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

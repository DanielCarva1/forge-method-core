//! Fuzz target for `forge_core_store::claim_wal::recover_claim_wal_from_bytes`.
//!
//! Drives the binary claim-WAL prefix decoder with attacker-controlled
//! bytes. The parser walks a frame-based format with magic, length, CRC,
//! and sequence fields, all read via `from_le_bytes`. Any panic observed
//! here is a bug in the WAL recovery logic.

#![no_main]

use forge_core_store::claim_wal::recover_claim_wal_from_bytes;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = recover_claim_wal_from_bytes(data);
});

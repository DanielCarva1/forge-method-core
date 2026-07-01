//! Benchmarks for the rekor verification hot paths.
//!
//! Measures the three operations an agent triggers when verifying artifact
//! provenance against a rekor inclusion proof:
//!
//! - `parse_signed_checkpoint` — note format parsing (origin / treeSize /
//!   base64 root hash / signature lines). Cheap, hot, called once per proof.
//! - `parse_rekor_log_entry` — JSON + base64 + nested JSON parsing of the
//!   log entry body. Cheap, hot, called once per verification.
//! - `verify_rekor_full_path` — the public entrypoint
//!   `run_host_adapter_rekor_verification`, which combines parse + verify
//!   of the signed checkpoint against the p256 rekor key + Merkle inclusion
//!   proof walk. Parametrized by inclusion proof depth so we see how the
//!   Merkle walk scales (1 / 10 / 100 auxiliary hashes).
//!
//! Only public API is exercised. The internal `verify_rekor_checkpoint` /
//! `verify_merkle_inclusion` helpers are `pub(crate)` on purpose; they are
//! measured indirectly via the public entrypoint so the benchmark reflects
//! real-world call shapes, not synthetic isolation.
//!
//! ## Fixture strategy
//!
//! `cargo bench` calls the benchmark closure many times for calibration.
//! Building a valid p256-signed checkpoint is expensive (~hundreds of µs).
//! We cache each fixture by `(hashes_len)` key in a process-wide `OnceLock`
//! so the signing happens exactly once per depth, and the cached bytes are
//! reused across calibration iterations.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use forge_core_crypto::host_adapter_types::HostAdapterRekorVerificationInput;
use forge_core_crypto::host_adapter_verification::run_host_adapter_rekor_verification;
use forge_core_crypto::rekor::{parse_rekor_log_entry, parse_signed_checkpoint};
use p256::ecdsa::{Signature as P256Signature, SigningKey as P256SigningKey};
use p256::pkcs8::{EncodePublicKey, LineEnding};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

// `Signer` trait brings `sign(&self, msg) -> Signature` into scope for
// `p256::ecdsa::SigningKey`. Without it, the inherent-method lookup fails.
use p256::ecdsa::signature::Signer as _;

/// Fixture on disk: `(rekor-log-entry.json, rekor.pub)` paths.
struct RekorFixture {
    log_entry_path: PathBuf,
    public_key_path: PathBuf,
    /// Pre-serialized log entry text (so parse benchmarks skip the file read).
    log_entry_text: String,
    /// Pre-extracted checkpoint text (so checkpoint parse benchmarks are
    /// isolated from JSON extraction).
    checkpoint_text: String,
}

/// Cache of `(hashes_len) -> RekorFixture`. Population signs one p256
/// signature per depth, which we want to amortize across calibration.
static FIXTURE_CACHE: OnceLock<Mutex<HashMap<usize, RekorFixture>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<usize, RekorFixture>> {
    FIXTURE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Synthetic rekor leaf hash: 32 bytes of pseudo-random, hex-encoded.
fn synthetic_leaf_hash(seed: u8) -> String {
    let bytes: [u8; 32] = std::array::from_fn(|i| seed.wrapping_add(i as u8));
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

/// Replicates the public `hash_merkle_node` algorithm so the fixture root
/// matches what `verify_merkle_inclusion` will recompute.
fn merkle_parent(left: &str, right: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut content = Vec::with_capacity(1 + 32 + 32);
    content.push(1);
    content.extend_from_slice(&hex_to_bytes(left));
    content.extend_from_slice(&hex_to_bytes(right));
    let digest = Sha256::digest(&content);
    hex_encode(&digest)
}

/// Rekor leaf hash per RFC6962: H(0x00 || entry).
fn rekor_leaf_hash(entry: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut content = Vec::with_capacity(entry.len() + 1);
    content.push(0);
    content.extend_from_slice(entry);
    let digest = Sha256::digest(&content);
    hex_encode(&digest)
}

fn build_fixture(aux_hashes: usize) -> RekorFixture {
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("forge-crypto-bench-{pid}-{aux_hashes}"));
    fs::create_dir_all(&dir).expect("create bench temp dir");

    let signing_key = P256SigningKey::from_slice(&[8u8; 32]).expect("p256 signing key");
    let public_key_pem = signing_key
        .verifying_key()
        .to_public_key_pem(LineEnding::LF)
        .expect("public key pem");
    let public_key_path = dir.join("rekor.pub");
    fs::write(&public_key_path, public_key_pem.as_bytes()).expect("write rekor pub");

    let body: Value = json!({
        "kind": "hashedrekord",
        "apiVersion": "0.0.1",
        "spec": {
            "data": {
                "hash": {
                    "algorithm": "sha256",
                    "value": "0000000000000000000000000000000000000000000000000000000000000000"
                }
            },
            "signature": {
                "content": BASE64.encode(b"signature bytes"),
                "publicKey": {
                    "content": BASE64.encode(public_key_pem.as_bytes())
                }
            }
        }
    });
    let body_bytes = serde_json::to_vec(&body).expect("serialize body");
    let canonical_body = serde_json_canonicalizer::to_vec(&body).expect("canonical body");
    let leaf_hash = rekor_leaf_hash(&canonical_body);

    // Build an inclusion proof that ends at a synthetic root. The leaf
    // participates as the first left node; aux hashes compose the right
    // siblings at each level. The verifier's Merkle walk reproduces the
    // same root the signed checkpoint commits to.
    let (hashes, root_hash) = if aux_hashes == 0 {
        (Vec::new(), leaf_hash.clone())
    } else {
        let mut current = leaf_hash.clone();
        let mut composed = Vec::with_capacity(aux_hashes);
        for i in 0..aux_hashes {
            let aux = synthetic_leaf_hash(i as u8);
            current = merkle_parent(&current, &aux);
            composed.push(aux);
        }
        (composed, current)
    };

    let tree_size: u64 = (aux_hashes + 1) as u64;
    let log_index: u64 = 0;

    let root_bytes = hex_to_bytes(&root_hash);
    let checkpoint_body = format!(
        "forge-bench-rekor\n{tree_size}\n{}\n",
        BASE64.encode(&root_bytes)
    );
    let checkpoint_signature: P256Signature = signing_key.sign(checkpoint_body.as_bytes());
    let mut signed_note_payload = vec![0u8, 0, 0, 0];
    signed_note_payload.extend_from_slice(checkpoint_signature.to_der().as_bytes());
    let checkpoint = format!(
        "{}\n\u{2014} forge-bench {}\n",
        checkpoint_body,
        BASE64.encode(&signed_note_payload)
    );

    let expected_log_id =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();

    let log_entry = json!({
        "body": BASE64.encode(&body_bytes),
        "integratedTime": 1_767_225_600_i64,
        "logID": expected_log_id,
        "logIndex": log_index,
        "verification": {
            "inclusionProof": {
                "hashes": hashes,
                "logIndex": log_index,
                "rootHash": root_hash,
                "treeSize": tree_size,
                "checkpoint": checkpoint
            },
            "signedEntryTimestamp": ""
        }
    });
    let log_entry_text = serde_json::to_string_pretty(&log_entry).expect("serialize log entry");
    let log_entry_path = dir.join("rekor-log-entry.json");
    fs::write(&log_entry_path, &log_entry_text).expect("write log entry");

    RekorFixture {
        log_entry_path,
        public_key_path,
        log_entry_text,
        checkpoint_text: checkpoint,
    }
}

fn fixture_for_guard(aux_hashes: usize) -> MutexGuard<'static, HashMap<usize, RekorFixture>> {
    let mut guard = cache().lock().expect("fixture cache poisoned");
    if !guard.contains_key(&aux_hashes) {
        let fixture = build_fixture(aux_hashes);
        guard.insert(aux_hashes, fixture);
    }
    guard
}

fn bench_parse_signed_checkpoint(c: &mut Criterion) {
    let guard = fixture_for_guard(0);
    let fixture = guard.get(&0).expect("fixture");
    let text = fixture.checkpoint_text.clone();
    drop(guard);

    c.bench_function("parse_signed_checkpoint/single", |b| {
        b.iter(|| {
            let _ = parse_signed_checkpoint(&text);
        });
    });
}

fn bench_parse_rekor_log_entry(c: &mut Criterion) {
    let guard = fixture_for_guard(0);
    let fixture = guard.get(&0).expect("fixture");
    let text = fixture.log_entry_text.clone();
    drop(guard);

    c.bench_function("parse_rekor_log_entry/single", |b| {
        b.iter(|| {
            let _ = parse_rekor_log_entry(&text);
        });
    });
}

fn bench_verify_rekor_full_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("verify_rekor_full_path");
    for &aux_hashes in &[0usize, 10, 100] {
        let guard = fixture_for_guard(aux_hashes);
        let fixture = guard.get(&aux_hashes).expect("fixture");
        let log_entry_path = fixture.log_entry_path.clone();
        let public_key_path = fixture.public_key_path.clone();
        drop(guard);

        group.throughput(Throughput::Elements(1));
        let input_template: (PathBuf, PathBuf) = (log_entry_path.clone(), public_key_path.clone());
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("aux_{aux_hashes}")),
            &input_template,
            |b, (log_entry_path, public_key_path): &(PathBuf, PathBuf)| {
                b.iter(|| {
                    let input = HostAdapterRekorVerificationInput {
                        log_entry_path: log_entry_path.clone(),
                        public_key_path: public_key_path.clone(),
                        expected_log_id:
                            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                                .to_string(),
                    };
                    let _ = run_host_adapter_rekor_verification(input);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse_signed_checkpoint,
    bench_parse_rekor_log_entry,
    bench_verify_rekor_full_path,
);
criterion_main!(benches);

//! Benchmarks for the claim WAL hot paths.
//!
//! Measures the operations an agent hits on every state-bearing turn:
//! - `append_claim_wal_record` — one record write (acquires lock, recovers,
//!   encodes, writes, optionally rotates)
//! - `replay_claim_wal` — recover + project to materialized state
//!
//! Parametrized by WAL size (1 / 100 / 1000 records) so we can see how the
//! per-op cost grows. Lock contention is NOT measured here (single-threaded);
//! see `claim_wal_stress.rs` integration test for the parallel path.
//!
//! ## Why cached state roots
//!
//! `cargo bench` runs the benchmark function multiple times for calibration
//! and outlier detection. Without caching, every call to the closure would
//! re-populate the WAL from scratch (1/100/1000 appends per call). We cache
//! each `(kind, size)` pair's state root in a process-wide `OnceLock` so the
//! expensive population happens exactly once per size.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use forge_core_contracts::claim::{
    ActorRole, ClaimContract, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind,
    ClaimStatus, ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
};
use forge_core_contracts::{ClaimId, RepoPath, ScopeId, StableId};
use forge_core_store::claim_wal::{append_claim_wal_record, replay_claim_wal, ClaimWalOperation};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Cache of `(kind, size) -> populated state root path`. The path is stable
/// per process: `<temp>/forge-bench-<kind>-<size>-<pid>`. If it already
/// contains the expected number of records, we skip re-population.
static STATE_CACHE: OnceLock<Mutex<HashMap<(&'static str, u64), PathBuf>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<(&'static str, u64), PathBuf>> {
    STATE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sample_claim(seq: u64) -> ClaimContract {
    ClaimContract {
        id: ClaimId(format!("claim.story.bench-{seq}")),
        contract_ref: RepoPath(format!("claims-active/claim-story-bench-{seq}.yaml")),
        claim: ClaimIdentity {
            claimant_principal_id: None,
            kind: ClaimKind::Story,
            claimant_agent_id: StableId("bench-agent".to_string()),
            claimant_role: ActorRole::Worker,
            registry_ref: None,
        },
        scope: ClaimScope {
            kind: ClaimScopeKind::Story,
            id: ScopeId(format!("bench-{seq}")),
            product_area: None,
            paths: vec![RepoPath(format!("crates/bench/src/mod-{seq}.rs"))],
        },
        lease: ClaimLease {
            acquired_at: "2027-01-15T08:00:00Z".to_string(),
            last_heartbeat_at: "2027-01-15T08:00:00Z".to_string(),
            expires_at: "2027-01-15T08:10:00Z".to_string(),
            ttl_seconds: 600,
            heartbeat_interval_seconds: 120,
            expected_state_version: 0,
        },
        status: ClaimStatusRecord {
            value: ClaimStatus::Active,
            evaluated_at: "2027-01-15T08:00:00Z".to_string(),
            reason_code: None,
        },
        expiry_policy: ExpiryPolicy {
            on_expiry: ExpiryAction::RecordHandoffRequest,
            handoff_required: true,
            release_without_handoff_allowed: false,
            reclaim_policy: ReclaimPolicy::DriverReview,
            handoff_request_ref: Some(RepoPath(
                "contracts/requests/claim-expiry-handoff-request.yaml".to_string(),
            )),
        },
        evidence_refs: Vec::new(),
    }
}

/// Return a state root pre-populated with `n` Acquire records. The path is
/// stable per process so calling this multiple times with the same `(kind, n)`
/// returns the same directory without re-populating.
fn populated_state(kind: &'static str, n: u64) -> PathBuf {
    // Fast path: already populated during a previous calibration call.
    if let Ok(map) = cache().lock() {
        if let Some(existing) = map.get(&(kind, n)) {
            return existing.clone();
        }
    }
    let state = std::env::temp_dir().join(format!("forge-bench-{kind}-{n}-{}", std::process::id()));
    // Wipe any stale state from a previous run, then re-populate from scratch.
    if state.exists() {
        fs::remove_dir_all(&state).expect("clear stale bench state root");
    }
    fs::create_dir_all(&state).expect("create bench state root");
    for seq in 1..=n {
        let claim = sample_claim(seq);
        append_claim_wal_record(
            &state,
            ClaimWalOperation::Acquire,
            &claim,
            "2027-01-15T08:00:00Z",
        )
        .expect("populate wal");
    }
    if let Ok(mut map) = cache().lock() {
        map.insert((kind, n), state.clone());
    }
    state
}

fn bench_append(c: &mut Criterion) {
    // Append cost includes an internal recovery scan, so it grows with WAL
    // length. We measure the per-op cost at three sizes.
    //
    // Each WAL size gets its own pre-populated state root, created ONCE per
    // process via `populated_state` (cached). Inside `iter`, we append one
    // record. The WAL grows by a handful of records over the benchmark run,
    // which is a small perturbation that reflects realistic steady-state.
    let mut group = c.benchmark_group("claim_wal/append");
    for size in [1_u64, 100, 1000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let state: &'static Path = Box::leak(populated_state("append", size).into_boxed_path());
            let mut next_seq = size + 1;
            b.iter(|| {
                let claim = sample_claim(next_seq);
                let result = append_claim_wal_record(
                    state,
                    ClaimWalOperation::Heartbeat,
                    &claim,
                    "2027-01-15T08:00:00Z",
                )
                .expect("append bench");
                next_seq += 1;
                result
            });
        });
    }
    group.finish();
}

fn bench_replay(c: &mut Criterion) {
    // Replay reads the WAL without mutating it, so we can populate ONCE per
    // size and reuse across all samples of that benchmark.
    let mut group = c.benchmark_group("claim_wal/replay");
    for size in [1_u64, 100, 1000] {
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let state: &'static Path = Box::leak(populated_state("replay", size).into_boxed_path());
            b.iter(|| {
                replay_claim_wal(state, false).expect("replay bench");
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_append, bench_replay);
criterion_main!(benches);

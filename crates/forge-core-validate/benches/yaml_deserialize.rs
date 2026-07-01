//! R6.3 — Benchmarks comparing YAML deserialize hot paths across crates.
//!
//! ## Why this benchmark exists
//!
//! Forge migrated from `serde_yaml` to `yaml_serde 0.10.4` in R7. This
//! benchmark validates that decision by measuring `from_str::<T>()` on a
//! representative Operation contract fixture across three candidate crates:
//!
//! - `serde_yaml` — the legacy de-facto standard (now in maintenance mode).
//! - `serde_yml`  — the modern community fork of `serde_yaml`.
//! - `yaml_serde` — the crate Forge actually uses in production.
//!
//! The fixture is `docs/fixtures/operation-contract-v0/facilitate-first-product-idea.yaml`,
//! a real-shaped Operation contract with nested structs, optional fields,
//! `deny_unknown_fields`, enums, and arrays. This is the exact kind of payload
//! Forge parses on every `validate`, `execute-operation`, and `claim` call.
//!
//! ## What we are measuring
//!
//! Only `from_str` for the production type `OperationContractDocument`. We do
//! not measure `to_string`, file I/O, or untyped `Value` deserialization; those
//! are separate hot paths tracked in R6.1/R6.2 store/crypto benchmarks.
//!
//! ## Reading the result
//!
//! If `yaml_serde` is at parity or faster, R7 is confirmed. If it is slower,
//! the trade-off is documented in `progress/r6_benchmarks.md` (R6.3 section)
//! and the migration is not reverted — the safety/maintenance wins of
//! `yaml_serde 0.10.4` are load-bearing per ADR-0007-class rationale.
//!
//! `serde_yaml` and `serde_yml` are dev-only dependencies of this bench; they
//! are NOT part of any production code path.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use forge_core_contracts::OperationContractDocument;
use std::path::PathBuf;

/// Resolves the fixture path relative to the workspace root, independent of
/// where `cargo bench` is invoked from. `CARGO_MANIFEST_DIR` points at the
/// crate root (`crates/forge-core-validate/`), so the workspace root is two
/// levels up: validate -> crates -> workspace root.
fn fixture_path() -> PathBuf {
    let dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(dir)
        .join("..")
        .join("..")
        .join("docs")
        .join("fixtures")
        .join("operation-contract-v0")
        .join("facilitate-first-product-idea.yaml")
}

/// Reads the fixture once and caches the resulting `String`. `cargo bench`
/// calls the benchmark closure many times for calibration; reading the file
/// per-iteration would dominate the measurement with syscall cost and not
/// reflect the real deserialize cost we want to compare.
fn fixture_text() -> &'static str {
    use std::sync::OnceLock;
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| {
        let path = fixture_path();
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()))
    })
}

/// Confirms all three deserializers accept the fixture before any group runs,
/// so a "fast but silently erroring" result can't sneak through. This is the
/// single source of truth for "the bench is measuring the real cost, not the
/// error path."
fn assert_all_accept(text: &str) {
    serde_yaml::from_str::<OperationContractDocument>(text)
        .expect("serde_yaml must accept fixture");
    serde_yml::from_str::<OperationContractDocument>(text).expect("serde_yml must accept fixture");
    yaml_serde::from_str::<OperationContractDocument>(text)
        .expect("yaml_serde must accept fixture");
}

fn bench_deserialize(c: &mut Criterion) {
    let text = fixture_text();
    assert_all_accept(text);

    let mut group = c.benchmark_group("yaml_deserialize/operation_contract");
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.sample_size(150);

    group.bench_with_input(BenchmarkId::from_parameter("serde_yaml"), &(), |b, ()| {
        b.iter(|| {
            let _ = serde_yaml::from_str::<OperationContractDocument>(text);
        });
    });

    group.bench_with_input(BenchmarkId::from_parameter("serde_yml"), &(), |b, ()| {
        b.iter(|| {
            let _ = serde_yml::from_str::<OperationContractDocument>(text);
        });
    });

    group.bench_with_input(BenchmarkId::from_parameter("yaml_serde"), &(), |b, ()| {
        b.iter(|| {
            let _ = yaml_serde::from_str::<OperationContractDocument>(text);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_deserialize);
criterion_main!(benches);

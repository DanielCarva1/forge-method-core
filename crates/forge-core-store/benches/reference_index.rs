//! Benchmark for `build_reference_index`, the hottest startup path.
//!
//! `forge validate` and `forge guide` call this once per invocation to map
//! the workspace's contract/policy/fixture graph. The cost is dominated by:
//! - filesystem walk of `contracts/` and `docs/fixtures/operation-contract-v0/`
//! - yaml_serde parsing of every YAML file found
//! - building the reference index (`ReferenceIndex`)
//!
//! We measure against the workspace itself (the realistic upper bound for a
//! forge-managed repo) and a synthetic minimal root (lower bound).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use forge_core_store::build_reference_index;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Find the workspace root by walking up from this source file.
/// In `cargo bench` the CWD is the crate dir, so `../../..` reaches the root.
fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// A minimal root with just a `contracts/` containing a single empty file.
/// Establishes the lower bound (no files to parse).
fn minimal_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("forge-bench-idx-min-{nanos}"));
    let contracts = root.join("contracts");
    fs::create_dir_all(&contracts).expect("create minimal root");
    // Single tiny contract so the walk has something to do.
    fs::write(
        contracts.join("minimal.yaml"),
        "schema_version: \"0.1\"\nkind: contract\ndefinition_id: bench/minimal\n",
    )
    .expect("write minimal contract");
    root
}

fn count_yaml_files(root: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(root.join("contracts")) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("yaml") {
                count += 1;
            }
        }
    }
    count.max(1)
}

fn bench_build_reference_index(c: &mut Criterion) {
    let real_root = workspace_root();
    let real_files = count_yaml_files(&real_root);
    let minimal = minimal_root();
    let minimal_files = count_yaml_files(&minimal);

    let mut group = c.benchmark_group("reference_index/build");
    group.throughput(Throughput::Elements(real_files.max(minimal_files) as u64));

    group.bench_with_input(
        BenchmarkId::new("workspace", real_files),
        &real_root,
        |b, root| {
            b.iter(|| {
                build_reference_index(root).expect("build reference index");
            });
        },
    );
    group.bench_with_input(
        BenchmarkId::new("minimal", minimal_files),
        &minimal,
        |b, root| {
            b.iter(|| {
                build_reference_index(root).expect("build reference index");
            });
        },
    );

    group.finish();
}

criterion_group!(benches, bench_build_reference_index);
criterion_main!(benches);

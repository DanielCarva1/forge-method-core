# Forge Method Core — Performance Baseline

**Purpose**: a single, agent-readable reference for the measured hot-path
performance of the Forge Method Core runtime. Agents consult this file to
answer "is Forge fast enough for X?" without running `cargo bench` (slow) or
opening CI artifacts.

**Audience**: agents and maintainers. Humans usually get this via `forge-core
guide describe` plus whatever the agent narrates; this file is the canonical
source for raw numbers.

**Last measurement run**: 2026-06-30 (Windows 11 / WSL, dev profile, criterion
0.5 with default sampling unless noted). Linux numbers are expected to be
faster on the `fsync`-bound paths (see *Notes on durability*).

---

## How to reproduce

```bash
# Store hot paths (claim WAL + reference index)
cargo bench -p forge-core-store

# Crypto hot paths (rekor parse + verify)
cargo bench -p forge-core-crypto --bench rekor

# YAML deserialization (cross-crate comparison)
cargo bench -p forge-core-validate --bench yaml_deserialize

# Focused
cargo bench -p forge-core-store -- claim_wal/append
```

The CI workflow `.github/workflows/perf.yml` runs these on every PR with the
`perf` label and on a daily cron. It caches the criterion baseline and fails
the PR on regressions > 15% (see R6.4).

---

## R6.1 — Store hot paths

### `claim_wal/append` (WAL write + fsync)

| Entries | Latency (typical) |
|---|---|
| 1     | 32 ms |
| 100   | 37 ms |
| 1 000 | 41 ms |

### `claim_wal/replay` (recovery scan, no fsync)

| Entries | Latency |
|---|---|
| 1     | 157 µs |
| 100   | 719 µs |
| 1 000 | 7.2 ms (~138 K entries/s) |

### `reference_index/build`

| Workload | Latency |
|---|---|
| Workspace (this repo's real tree) | ~1.5 ms |
| Minimal (empty workspace)         | ~205 µs |

---

## R6.2 — Crypto hot paths

Location: `crates/forge-core-crypto/benches/rekor.rs`

| Benchmark | Latency |
|---|---|
| `parse_signed_checkpoint` (pure parse)         | ~2-3 µs |
| `parse_rekor_log_entry` (JSON + base64 double) | ~6-7 µs |
| `verify_rekor_full_path/aux_0`   (depth 0)     | ~420 µs |
| `verify_rekor_full_path/aux_10`  (depth 10)    | ~450 µs |
| `verify_rekor_full_path/aux_100` (depth 100)   | ~655 µs |

**Dominant cost**: the p256 ECDSA verification on the signed checkpoint
(~400 µs floor). The Merkle walk is O(log n); each auxiliary hash adds ~2 µs.
Parsing is negligible (~6 µs).

**Design note** (applied via `improve-codebase-architecture` deletion test):
the internal helpers `verify_rekor_checkpoint` and `verify_merkle_inclusion`
stay `pub(crate)`. They are measured indirectly through the public
`run_host_adapter_rekor_verification` entrypoint, which is what real callers
use. Exposing them as `pub` only for the benchmark would be a shallow seam.

---

## R6.3 — YAML deserialization (contract parse)

Fixture: `docs/fixtures/operation-contract-v0/facilitate-first-product-idea.yaml`
(3 025 bytes, 94 lines, nested structs + optionals + `deny_unknown_fields` +
enums + arrays — the payload Forge parses on every `validate` /
`execute-operation` / `claim`).

| Crate                          | Median     | Throughput |
|--------------------------------|------------|------------|
| `serde_yaml` 0.9 (legacy)      | 92.9 µs    | 23.3 MiB/s |
| `serde_yml` 0.0.12 (fork)      | 93.4 µs    | 23.2 MiB/s |
| `yaml_serde` 0.10.4 (Forge)    | 99.7 µs    | 21.7 MiB/s |

**Decision**: the R7 migration to `yaml_serde` is **not reverted**. The ~7%
gap is within operational noise for a non-hot path (parse cost is dominated
by I/O and crypto, see R6.1/R6.2). Maintenance and security gains from R7
outweigh ~7 µs per contract. Reopen R6.3 only if `yaml_serde` regresses
> 30% or batch validation (>100 contracts/command) becomes a primary
workload. See `progress/r7_yaml_serde.md`.

---

## Notes on durability (not a bug)

`fsync` on Windows costs 25-50 ms typically, with 300 ms spikes. The
`claim_wal/append` cost (~32 ms) is almost entirely `fsync`. The WAL needs
`fsync` to guarantee durability after a crash; without it, claims are lost on
power loss. On Linux this is expected to be 5-15 ms typical on SSD.

### Real optimizations available (each is a system design change)

1. **Tiered durability** — `--no-sync` flag for benchmarks/tests/dev
   (opt-in). **Shipped** in F15.7b across `claim`, `execute-operation`,
   `rebuild-effect-index` (read paths exclude it by design).
2. **Batch appends** — group N appends into one `fsync` (changes semantics).
3. **Async fsync** in background thread (complicates recovery; threatens
   durability). Not recommended.

---

## Regression gate (R6.4)

`.github/workflows/perf.yml`:

- Daily cron 06:00 UTC + `workflow_dispatch` + opt-in via the `perf` label on
  PRs.
- Caches `target/criterion` between runs (keyed by OS + branch, with fallback
  to `main`).
- Parses criterion's `change: [a% b% c%]` lines via `awk`; the middle value
  (median) is compared against a 15% threshold. PRs fail on alert.
- First run (no cached baseline) trivially passes and establishes the
  baseline for the next run.
- Bench output uploaded as 30-day artifacts for manual inspection.

Threshold rationale: 15% is the middle ground — low enough to catch real
regressions, high enough to not flag CI runner noise.

---

## Source documents

- `docs/dev-docs/forge-method-core-dev-docs-v2/progress/r6_benchmarks.md`
  (raw measurements and pitfalls encountered)
- `docs/dev-docs/forge-method-core-dev-docs-v2/progress/r7_yaml_serde.md`
  (YAML crate decision)
- `.github/workflows/perf.yml` (regression gate implementation)

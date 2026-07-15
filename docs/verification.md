# Verification guide

Verification is proportional while editing and cumulative at a checkpoint. The
executable ownership/trigger source is [`.github/workflows/ci.yml`](../.github/workflows/ci.yml);
this guide explains its P7G topology without claiming that source configuration
alone met a time budget.

## Tier topology

| Tier | Trigger | Per-step budget | Owned evidence |
|---|---|---:|---|
| **Tier 0: static/docs** | Every pull request and push to `main`/`master` | 120 seconds | Generated workspace layout, local Markdown links, public-promise audit, evidence-tool tests, Rust formatting |
| **Focused package/integration** | Every pull request and push to `main`/`master`, after Tier 0 | 900 seconds | Generated command/release subjects, retirement runtime, test inventories, all-feature pedantic clippy, aggregate validation and regression anchors |
| **Platform** | Every pull request and push to `main`/`master`, after Tier 0; native Linux, Windows, Intel macOS, Apple Silicon macOS matrix | 1,800 seconds | Workspace all-target compilation and default workspace tests on every runner; each non-Linux runner also compiles the expensive P6d target |
| **Expensive cumulative journey** | Push to `main`/`master` only, after Tier 0 + focused + platform succeed | 1,800 seconds | Exact Linux P6d reference-pack real-process journey once |

Job timeouts (10, 45, 40, and 35 minutes respectively) are outer safety bounds.
Each wrapped step also enforces its own hard wall-clock timeout, terminates its
complete child process tree, and persists timing evidence. Pull requests do not
execute the expensive journey, but focused all-feature clippy compiles its code
on Linux and every non-Linux platform has a dedicated feature-gated compile.

## Timing and failure evidence

Every owned command runs through:

```bash
python scripts/run-ci-tier.py \
  --tier <stable-id> \
  --budget-seconds <budget> \
  --report target/ci-timing/<report>.json \
  -- <command> [args...]
```

The report records exact argv/display, runner OS/architecture/name/environment,
cache context, elapsed/budget seconds, underlying command exit, budget status,
outcome, and wrapper exit. It is uploaded even after failure:

- `ci-timing-static-docs`
- `ci-timing-focused`
- `ci-timing-platform-${{ matrix.id }}`
- `ci-timing-expensive-journey`

The wrapper also appends the row to `GITHUB_STEP_SUMMARY`. A timeout terminates
the child process tree, records the timeout, and exits `124`; a command that fails
before its deadline retains its normalized exit code. Reports are still written
for timeout and launch failure. Timing evidence therefore cannot hide a test
failure. P7G budget compliance requires corresponding run artifacts (including
the story's consecutive-run evidence); workflow text alone is insufficient.

## Local proportional commands

Documentation-only changes:

```bash
python scripts/check-doc-links.py
git diff --check
```

Tier 0 parity when static/generated surfaces are affected:

```bash
python scripts/generate-workspace-layout.py --check
python scripts/check-doc-links.py
cargo fmt --all -- --check
```

Focused development uses the smallest owning package/integration commands:

```bash
cargo test -p <package>
cargo test -p <package> --test <integration-test>
cargo clippy -p <package> --all-targets -- -D clippy::pedantic
```

Contract changes normally need the contract crate, semantic decision crate,
consumer/kernel and CLI-adapter tests, aggregate validator, plus tamper,
stale-CAS/replay, recovery, and zero-write failures where applicable. Embedded
release material requires its generator's `--check`; use the focused job as the
exact list.

Before integration, reproduce the default coherent gate as needed:

```bash
cargo clippy --workspace --all-targets --all-features -- -D clippy::pedantic
cargo run -p forge-core-cli -- validate --root .
cargo test --workspace
```

The exact cumulative journey, run only once per cumulative push in CI, is:

```bash
cargo test -p forge-core-cli --test domain_pack_cli_e2e \
  --features expensive-p6d-e2e \
  p6d_workflow_journey::p6d_reference_pack_real_journey -- --exact --nocapture
```

The journey emits phase heartbeats and bounds every spawned CLI process to five
minutes; the outer tier wrapper remains the authoritative 1,800-second tree timeout.

Do not describe default `cargo test --workspace` as covering that feature-gated
journey. Do not silently remove a slow test: move it only with an explicit owner,
trigger, compile path, and before/after inventory rationale. CI normalizes exact
test identities with `scripts/check-test-inventory.py` and compares them to
`contracts/test-inventory/workspace.json` and
`contracts/test-inventory/expensive-p6d.json`; additions and removals require an
explicit baseline review rather than disappearing in job topology changes.

## Release evidence

A release additionally requires exact tag/commit/workspace/CLI agreement,
manifested payload verification, packaged native Linux, Windows, Intel macOS,
and Apple Silicon macOS install smoke,
per-asset checksum and Sigstore verification, schema-validated release-level
CycloneDX SBOM, release-note/docs agreement, and residual limitations. Those are
P7E release gates; publication is evidenced only by their successful matching
tag run and independently verified assets.

## Evidence report

```text
Scope and changed trust boundary:
Owned tier(s), trigger, and budgets:
Focused commands/results:
Timing report artifact(s):
Native Linux/Windows/macOS evidence:
Cumulative journey (run / compiled-only / not applicable):
Release package smoke (run / not applicable):
Inventory or failure-injection evidence:
Residual risks:
```

A green command proves only its owned surface. Final claims must match the union
of actual command, timing, platform, package, and review evidence.

# Verification guide

Verification is proportional while editing and cumulative at a checkpoint. The
normal evidence topology is [`.github/workflows/ci.yml`](../.github/workflows/ci.yml);
the independent pull-request enforcement boundaries are
[`.github/workflows/msrv-policy.yml`](../.github/workflows/msrv-policy.yml) and
[`.github/workflows/release-policy.yml`](../.github/workflows/release-policy.yml).
This guide describes their topology without claiming that source configuration
alone met a time budget or changed repository settings.

## Tier topology

| Tier | Trigger | Per-step budget | Owned evidence |
|---|---|---:|---|
| **Tier 0: static/docs** | Every pull request and push to `main`/`master` | 120 seconds | Generated workspace layout, local Markdown links, public-promise audit, evidence-tool tests, Rust formatting |
| **MSRV** | Every pull request and push to `main`/`master`, after Tier 0 completes (even if Tier 0 failed or was skipped) | 300 seconds contract test; 1,800 seconds compile | Exact PyYAML 6.0.3 provisioning and Rust 1.85.1, locked workspace, every Cargo target and feature, adversarial lane tests |
| **Focused package/integration** | Every pull request and push to `main`/`master`, after Tier 0 | 900 seconds | Generated command/release subjects, retirement runtime, test inventories, all-feature pedantic clippy, aggregate validation and regression anchors |
| **Platform** | Every pull request and push to `main`/`master`, after Tier 0; native Linux, Windows, Intel macOS, Apple Silicon macOS matrix | 1,800 seconds | Workspace all-target compilation and default workspace tests on every runner; each non-Linux runner also compiles the expensive P6d target |
| **Expensive cumulative journey** | Push to `main`/`master` only, after Tier 0 + focused + platform succeed | 1,800 seconds | Exact Linux P6d reference-pack real-process journey once |

Job timeouts (10, 35, 45, 40, and 35 minutes respectively) are outer safety bounds.
Each wrapped step also enforces its own hard wall-clock timeout, terminates its
complete child process tree, and persists timing evidence. Pull requests do not
execute the expensive journey, but focused all-feature clippy compiles its code
on Linux and every non-Linux platform has a dedicated feature-gated compile.

The workspace declares `rust-version = "1.85"` because Cargo's manifest field
expresses a stable release line as major.minor. CI pins patch release **1.85.1**
to make compiler behavior reproducible while staying within that declared 1.85
line; the patch pin does not raise the source MSRV to a different Rust release.
The MSRV job deliberately has no Cargo cache, so a newer compiler cannot seed it
and the lane cannot save artifacts for other jobs. Its job-level `if: always()`
is required by the checker: `static_docs` finishing as failed or skipped cannot
suppress normal MSRV evidence. The `Verify MSRV lane contract` step first
installs exact `PyYAML==6.0.3` with dependency installation disabled
(`--no-deps`), rather than trusting runner-preinstalled Python packages, and only
then starts the structured checker tests. `scripts/check-msrv.py` requires that
exact provisioning command and order, discovers every `crates/*/Cargo.toml`,
reconciles it with explicit workspace members, and parses each member's features
and package policy. Its duplicate-key-safe structured YAML check rejects aliases,
merges, unknown protected jobs/steps/keys, unapproved inherited or job/step
environment, mutable protected action references, credential persistence, and any
MSRV step outside the exact named sequence and closed per-step fields. The
accepted compile remains the wrapped exact `cargo +1.85.1 check --locked
--workspace --all-targets --all-features`; the always-run timing upload cannot
change that step's result.

## Independent pull-request enforcement

Repository rules must require the exact check contexts **`MSRV Policy / Enforce
trusted MSRV policy`** and **`Release Policy / Enforce trusted release policy`**
for pull requests to `main`. These repository files define those checks; they do
**not** claim the external repository rules were configured.

Both policy workflows use `pull_request_target` restricted to pull requests whose
base branch is `main`, so GitHub selects workflow authority from the protected
base rather than from the candidate. Each grants only `contents: read`, pins both
checkout actions by commit, disables credential persistence, and checks out the
immutable event-provided base and head SHAs into separate `trusted/` and
`candidate/` directories. After exact pinned/no-dependency PyYAML provisioning,
each job executes only its checker under `trusted/scripts/`. A fork candidate
must be readable with the read-only token; checkout or required-file failure
rejects the policy check closed. No environment with secrets or write authority
may be attached to either job.

The MSRV checker parses candidate CI, policy, manifests, and its future checker as
data. It requires exact triggers, permissions, immutable checkout references,
protected job and step topology, pinned actions, no persisted credentials, and
the trusted-only checker command. It also rejects policy deletion/rename,
symbolic-link substitution, checker drift, `if`/`continue-on-error` bypasses,
compiler/cache overrides, and both direct skip attacks (`static_docs.if: false`
and `msrv.if: false`). Candidate scripts, actions, build files, and Rust code are
never executed by the MSRV policy job.

The release checker likewise never executes candidate code. It requires the
candidate release-policy topology, release-policy checker, and release-lock
checker to remain bound to the protected-base trust root, then authenticates the
exact reviewed release workflow bytes, semantic graph, governed local executable
closure, release payload manifest, locked Cargo invocations, and publication
permissions. Candidate-owned tests can report regressions but cannot authorize a
release workflow or checker change.

A skipped result from a required in-repository PR job is not equivalent to a
successful enforcement decision, and a checker inside the mutable guarded job
cannot enforce its own execution. The required base-workflow contexts are
therefore the security boundaries. There is a bootstrap limitation: a pull
request that first introduces either policy, or intentionally changes its
protected checker trust root, cannot authorize that change using the old base
policy. Review and integrate such a bootstrap through an existing trusted
administrative path, then configure both repository rules before relying on the
contexts for subsequent pull requests.

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

- `ci-timing-msrv` (14-day retention)
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
```
cargo fmt --all -- --check

MSRV topology and exact-toolchain parity:

```bash
python scripts/check-msrv.py
python scripts/test-msrv.py
cargo +1.85.1 check --locked --workspace --all-targets --all-features
```

The adversarial suite includes a standalone, parse-valid let-chain fixture that
a post-1.85 compiler accepts but Rust 1.85.1 rejects with the intended `E0658`
language-stability diagnostic. A successful local run is source-topology
evidence only. Hosted evidence exists only after the `msrv` job runs on GitHub
and retains its `ci-timing-msrv` artifact; that native hosted run is pending for
an unpushed change.

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

## Solo objective grounding checks

The focused CLI integration checks `cooperative_objective_cli_commits_once_and_fresh_next_reads_the_ledger`, `cooperative_objective_grounding_survives_restart_and_local_writes`, and `cooperative_objective_cli_supersedes_then_clarifies_with_replacement_readback` prove exact anchor completion, continued domain-scan after a governed repository edit, restart and `.local` continuity, material supersession reopening, and non-material clarification preserving the current material anchor. The kernel check `solo_objective_grounding_is_material_window_bound_and_not_evaluator_aged` proves that grounding is durable state rather than age-limited evaluator evidence, while still disappearing after `invalidate_all` or anchor revocation. The decisions check `typed_same_owner_grounding_satisfies_a_claim_without_impersonating_its_evaluator` proves grounding and evidence remain distinct in results.

`signed_cli_flow_completes_first_policy_and_resumes_capability_gap` is the paired guard that keeps `strict_external` broker-backed. `internal_fixture_reaches_investigation_then_public_solo_source_command_supersedes_assessments` guards the existing cooperative source-evidence path. These focused checks do not replace the packaged release journey or full release evidence.

## Release evidence

A release additionally requires exact tag/commit/workspace/CLI agreement,
manifested payload verification, the complete packaged Solo Dogfood journey on
native Linux, Windows, Intel macOS, and Apple Silicon macOS, three consecutive
fresh reference runs on x86_64 Linux, retained result evidence, per-asset
checksum and Sigstore verification, a schema-validated release-level CycloneDX
SBOM, release-note/docs agreement, and residual limitations. The packaged
journey uses synthetic cooperative objective input and proves post-commit
`recover`/`apply` idempotence plus final isolation cleanup; it does not prove a
real conversation admission or an injected crash-recovery window. It proves
runtime behavior, not authenticity of any named chat host. Those
are P7E release gates; publication is evidenced only by their successful
matching tag run and independently verified assets.

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

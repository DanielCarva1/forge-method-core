# Forge Core 10/10 Product Hardening Plan

Status: active technical plan  
Owner: Forge core maintainers and agents  
Last reviewed: 2026-07-05

## Purpose

This document turns the current audit findings into an evidence-backed,
incremental hardening plan. The goal is to make Forge Method Core excellent as
both a protocol runtime and a usable product without weakening its essence:

- typed contracts over prose,
- fail-closed mutation,
- sidecar-owned runtime state,
- append-only provenance,
- machine-readable CLI envelopes,
- and agent-first interoperability.

The plan is intentionally staged. It avoids a big-bang rewrite and keeps every
step independently verifiable.

## Audit baseline findings

The initial audit established these findings. The live repository state, not
stale documentation, remains authoritative for any future update:

- `cargo metadata --format-version=1 --no-deps` reports 18 workspace members.
- `cargo check --workspace` passes.
- `cargo fmt --all -- --check` passes.
- `cargo run -p forge-core-cli -- validate --root . --json` passes with zero
  diagnostics.
- `cargo test --workspace --no-fail-fast` and
  `cargo clippy --workspace --all-targets -- -D clippy::pedantic` fail on
  Windows before the test suite can run because
  `crates/forge-core-protocol-mcp/src/server.rs` uses `File::write_all`
  without importing `std::io::Write` in a Windows-only test helper.
- `AGENTS.md` still says the workspace has 10 crates; the live workspace has
  18 members.
- The README repository layout lists only a subset of the current crates.
- The global usage table says `forge-core start [--agent-id <id>]`, while the
  `start` parser rejects `--agent-id`.
- `project resolve --allow-bootstrap-core` recognizes the core repository's
  Bootstrap Core Exception, but `start --root .` without that flag reports
  `no_link`. The command behavior is defensible for ordinary consumer repos,
  but the product surface needs to make the Bootstrap Core Exception explicit
  and consistent.

## First hardening changeset evidence

This first implementation slice resolves the immediate red gate and the
highest-signal drift surfaces:

- The Windows-only MCP test helper imports `std::io::Write`.
- CI now has a Linux quality gate plus an explicit
  `ubuntu-latest` / `windows-latest` platform matrix for
  `cargo check --workspace --all-targets` and `cargo test --workspace`.
- Workspace layout is generated from Cargo metadata in
  `docs/generated/workspace-layout.md` and checked by CI.
- README and AGENTS no longer hand-maintain crate counts.
- `start` accepts optional `--agent-id`, advertises `--allow-bootstrap-core`,
  and explicitly diagnoses the Forge core Bootstrap Core Exception without
  treating arbitrary consumer-local `.forge-method/` state as safe.
- Local verification after this slice:
  - `python scripts/generate-workspace-layout.py --check`
  - `cargo fmt --all -- --check`
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace --no-fail-fast`
  - `cargo clippy --workspace --all-targets -- -D clippy::pedantic`
  - `cargo run -p forge-core-cli -- validate --root . --json`

## Research base

### Rust and CLI codebases

- Cargo's CLI builds a structured `clap::Command` and carries a `verify_cli`
  test with `debug_assert()`, a useful pattern for making parser/help drift a
  test failure:
  <https://github.com/rust-lang/cargo/blob/master/src/bin/cargo/cli.rs>.
- ripgrep keeps a deep `Args` module that converts low-level argument matches
  into a high-level, cloneable configuration object. The lesson for Forge is
  that parsing should collapse into a small, typed interface before runtime
  behavior starts:
  <https://github.com/BurntSushi/ripgrep/blob/041544853c86dde91c49983e5ddd0aa799bd2831/crates/core/args.rs>.
- clap's own documentation shows `CommandFactory::command().debug_assert()` as
  the canonical test hook for validating command definitions:
  <https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html>.
- Cargo documents `cargo metadata --format-version=1` as the machine-readable
  workspace/package interface. Forge docs that describe the workspace shape
  must use this instead of hand-maintained crate counts:
  <https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>.
- The Cargo Book's CI guidance starts from build-and-test workflows. Forge must
  extend that baseline with explicit platform coverage because this failure is
  Windows-only:
  <https://doc.rust-lang.org/cargo/guide/continuous-integration.html>.
- GitHub Actions documents matrix jobs across operating systems. Forge should
  use an OS matrix so `cfg(windows)` code is compiled before release:
  <https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/run-job-variations>.

### Agent systems and provenance research

Western and Eastern research converge on the same architectural direction:
agent systems become trustworthy through typed process evidence, command/data
separation, and auditable provenance rather than by relying on model intent.

- The 2026 evidence-tracing survey argues that final-answer accuracy is not
  enough; agent behavior needs typed execution provenance spanning evidence,
  tool calls, memory, observations, actions, and recovery:
  <https://arxiv.org/html/2606.04990v2>.
- PROV-AGENT extends W3C PROV and incorporates MCP concepts for near-real-time
  agentic workflow provenance. This supports Forge's choice to keep MCP as a
  thin adapter over a canonical command/runtime surface:
  <https://arxiv.org/html/2508.02866v1>.
- "Trustworthy Agentic AI Requires Deterministic Architectural Boundaries"
  argues that high-stakes agents need deterministic mediation,
  command-data separation, and unforgeable provenance. This reinforces Forge's
  fail-closed adapter and kernel design:
  <https://doi.org/10.48550/arxiv.2602.09947>.
- MetaGPT shows that structured workflows/SOPs improve multi-agent software
  engineering coherence over naive agent chaining:
  <https://arxiv.org/html/2308.00352v7>.
- A-MEM and G-Memory show the Eastern research trend toward structured,
  evolving, provenance-aware memory for agents and multi-agent systems:
  <https://arxiv.org/abs/2502.12110> and
  <https://arxiv.org/pdf/2506.07398>.

## Design principles for the remediation

1. **One source of truth per interface.** If humans, agents, MCP, help text,
   and docs need the same command fact, that fact must live in one registry and
   be projected outward.
2. **No big-bang parser rewrite.** The current hand-rolled parsers encode many
   command-specific invariants. Replace drift first, then deepen parsers
   command-by-command.
3. **Platform behavior is product behavior.** Windows-only code must be
   compiled and tested in CI, not treated as a local afterthought.
4. **Documentation is generated where counts drift.** Crate lists and workspace
   topology must be produced from `cargo metadata --format-version=1`.
5. **Adapters stay thin.** MCP and future host adapters must project the
   canonical command surface. They must not own command semantics.
6. **Bootstrap exceptions must be explicit.** The core repo's local
   `.forge-method/` state is a documented exception. CLI output must make that
   exception visible when applicable instead of looking like a consumer repo
   failure.
7. **Every stage has an acceptance gate.** A stage is done only when the
   relevant command, test, generated file, or CI workflow proves it.

## Stage 1 — Repair the red Windows test gate

### Problem

The MCP server test module uses `File::write_all` in a Windows-only helper, but
the `Write` trait is not in scope under `#[cfg(windows)]`. Ubuntu CI misses the
failure because that helper is not compiled there.

### Safe fix

Add a scoped import inside the Windows helper:

```rust
#[cfg(windows)]
fn make_fake_forge_core(success: bool, envelope: &str) -> PathBuf {
    use std::io::Write;
    // ...
}
```

### Acceptance

- `cargo test -p forge-core-protocol-mcp --all-targets`
- `cargo test --workspace --no-fail-fast`
- `cargo clippy --workspace --all-targets -- -D clippy::pedantic`

## Stage 2 — Add Windows CI coverage

### Problem

The workflow currently runs on Ubuntu only. That lets Windows-only code fail in
local use even when CI is green.

### Safe fix

Split CI into:

- a Linux quality job for format, clippy, contract validation, and regression
  anchor;
- a platform test matrix for `ubuntu-latest` and `windows-latest` that runs:
  - `cargo check --workspace --all-targets`,
  - `cargo test --workspace`.

Keep `fail-fast: false` so platform-specific failures are all visible.

### Acceptance

- The workflow YAML contains a matrix with `ubuntu-latest` and
  `windows-latest`.
- Windows matrix job runs both required Cargo commands.
- Linux quality job preserves the current strict clippy and validation gates.

## Stage 3 — Generate workspace layout from Cargo metadata

### Problem

Hand-maintained crate counts and crate lists already drifted. This damages user
trust and agent navigation.

### Safe fix

Add a small script that reads:

```bash
cargo metadata --format-version=1 --no-deps
```

and writes a generated Markdown fragment, for example:

```text
docs/generated/workspace-layout.md
```

The fragment should include:

- workspace member count,
- package name,
- relative crate path,
- target kinds,
- direct workspace dependencies.

Then update README and AGENTS to point to the generated fragment instead of
embedding stale counts. If a short inline summary is needed, the generator
should rewrite that bounded region.

### Acceptance

- Running the generator on a clean tree is idempotent.
- The generated member count equals `cargo metadata`'s workspace member count.
- CI runs the generator in check mode and fails if generated docs are stale.

## Stage 4 — Deepen the command surface seam

### Problem

`command_registry::COMMANDS` is already a useful module, but it does not yet
fully prevent drift. Usage lines, command parser behavior, subcommand help,
MCP allowlists, and future docs can still disagree.

### Target module

Introduce or evolve a deep **Command Surface** module:

- **Interface**: one canonical registry of command paths, usage metadata,
  authority class, adapter exposure, JSON/text support, and a parser/handler
  adapter.
- **Implementation**: hand-rolled parsers can remain behind the interface
  initially; the interface should not force an immediate clap rewrite.
- **Adapters**:
  - CLI dispatch,
  - global help rendering,
  - MCP tool projection,
  - docs generation,
  - command-surface tests.

This follows Cargo's structured command builder and ripgrep's "parse once into
a high-level object" pattern while preserving Forge's existing error-envelope
discipline and hand-rolled error enums.

### Incremental path

1. Add command metadata fields to `CommandSpec` without changing handlers:
   `authority`, `json_mode`, `adapter_visibility`, and `canonical_usage`.
2. Add tests that compare:
   - global usage vs per-command help,
   - MCP default tools vs command registry,
   - documented command paths vs registry.
3. Move one small command (`start`) to a typed parser adapter as a pilot.
4. Repeat for high-value shallow parsers only after the pilot is green.

### Acceptance

- No command usage line is hand-written in more than one authoritative place.
- `forge-core --help`, MCP projection, and docs are all projections.
- A command rename breaks tests before it reaches users.
- The CLI remains backward compatible unless an intentional breaking change is
  documented.

## Stage 5 — Reconcile `start` with `project resolve --allow-bootstrap-core`

### Problem

The core repository is a Bootstrap Core Exception. `project resolve --root .
--allow-bootstrap-core --json` reports that correctly, but `start --root .`
without the flag reports `no_link`. That behavior protects consumer repos, but
the product experience is confusing inside the core repo.

### Safe fix

Make `start` internally attempt a diagnostic-only bootstrap-core resolution
after a missing-link result, but only when all of these are true:

- `<root>/.forge-method/` exists,
- `<root>/Cargo.toml` identifies the package/workspace as Forge core,
- normal resolution failed only because the Project Link is missing.

If those conditions hold, return an explicit Bootstrap Core Exception payload
instead of a consumer `no_link` payload. The next step should explain that
ordinary consumer repos should still run `project init`, while the core repo
can continue with `--allow-bootstrap-core`.

This preserves consumer safety: `start` must not silently create or accept
local `.forge-method/` state for arbitrary repos.

### Acceptance

- `forge-core start --root . --json` in this repository returns a payload that
  names the Bootstrap Core Exception.
- A fresh consumer repo with no link still returns `no_link` and recommends
  `project init`.
- A consumer repo with unsafe local `.forge-method/` state still fails closed
  or points at explicit repair; it must not be normalized as safe.
- `START_USAGE_LINE` and `command_registry` agree on
  `[--allow-bootstrap-core]` and do not advertise unsupported flags.

## Stage 6 — Preserve product essence while raising the score

The path to 10/10 is not "more features"; it is fewer inconsistent surfaces.

### Must preserve

- CLI envelope wire compatibility.
- No `anyhow` / no `thiserror`.
- Accumulating validation diagnostics.
- Sidecar-owned state for consumer repos.
- Thin adapters and kernel-owned mutation.
- Existing contracts and workflow catalog.

### Must improve

- Cross-platform gates.
- Generated docs for live workspace shape.
- Command surface locality and leverage.
- Bootstrap diagnostics.
- Host adapter conformance proof before claiming product readiness.

## Completion definition

This plan is complete only when:

1. All current green-loop commands pass locally.
2. CI covers both Linux and Windows for all targets/tests.
3. README/AGENTS workspace layout cannot drift without a generator check
   failing.
4. Command surface metadata drives CLI help, MCP projection, and generated
   command docs.
5. `start` gives safe, explicit bootstrap diagnostics for both consumer repos
   and the Forge core Bootstrap Core Exception.
6. The release/product docs describe only capabilities proven by tests or
   command output.

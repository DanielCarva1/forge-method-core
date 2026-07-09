# Changelog

All notable changes to **Forge Method Core** (the Rust-only typed-contract
governance runtime) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> Note on version history: the Rust core is a clean rebuild of an earlier
> Python runtime that lived in a sibling archive repo. The legacy `v1.x`–`v2.x`
> git tags belong to that abandoned Python Forge and are **not** ancestors of
> this repo's history. Only `v0.1.0` and `v0.2.0` are reachable from `master`
> here. This CHANGELOG tracks the Rust core only.

---

## [Unreleased]

### Added
- **Agent-native Assurance Case v0.** A deny-unknown-fields typed contract family now represents intent, project snapshots, obligations, evidence-backed claims, Decision Requests, Capability Gaps, ranked next actions, and target-specific readiness.
- **Read-only Obligation Engine vertical slice.** `forge-core-decisions::obligation_engine` deterministically derives a semantically validated Assurance Case from host-proposed intent, observations, epistemic-risk signals, capabilities, and irreducible human decisions without IO, model calls, or mutation authority.
- Four Obligation Engine fixtures cover novel-domain/method/capability gaps, artifact-only progress, explicit waiver, and verified release readiness.

### Changed
- Repository contract validation now includes the Assurance Case family and uses a 125-clean-check regression anchor.
- Agent-native product doctrine, architecture direction, and the phased delivery plan are carried as typed YAML under `contracts/`.

---

## [0.3.0] — 2026-07-07

### Added
- **Wave 2 — funnel-of-autonomy machine-enforcement (D4 / FRUST-031).** `EnforcementPolicy` now carries a binding `contact_density` field derived from `Phase::rank()`: discovery/specification = `high`, plan/evolve = `medium`, build-verify/ready-operate = `low`. The host agent reads this from `guide decide` output and modulates interaction mode: asking is expected at `high`, conditional at `medium` (after the human-approved gate passes, execute without procedural confirmation), and a funnel violation at `low` (escalate only on gate failure or new ambiguity). Closes the gap where agents over-asked during mechanical plan/build execution. Backed by 5 new tests in `guide.rs`.
- **`forge` wrapper shipped in release archives.** Each release asset now contains both `forge-core` (the binary) and `forge` (POSIX sh) / `forge.cmd` (Windows) wrappers that delegate to the binary in the same directory. The `start-forge` skill and other tooling that look up `forge` on PATH now find it without any manual aliasing. Either name works interchangeably.
- Wave 1 status reconciliation (2026-07-07):
  - `CHANGELOG.md` — this file (canonical release-history authority).
  - `contracts/spec/wave-1-status-reconciliation-spec.yaml` — spec kernel for the doc-only Wave 1 batch.
  - Sidecar artifacts `D1`–`D4` (discovery, grill, system-design gate, funnel-autonomy gap evidence) from the dogfood session.

### Changed
- Workspace version bumped `0.2.0` → `0.3.0` (minor: `contact_density` is a new field on a serialized struct; additive for dynamic JSON consumers, breaking for static typed deserializers — minor bump is the honest SemVer call).
- `crates/forge-core-cli/src/guide.rs`: `EnforcementPolicy` struct gains `contact_density: String` field; `resolve_enforcement_policy` no longer lumps discovery/spec/plan as one "human-heavy" block — plan is now correctly `medium` (distinct from discovery/spec `high`).
- `.github/workflows/release.yml`: POSIX and Windows packaging steps now copy the `forge` / `forge.cmd` wrappers from `distribution/` into the archives alongside the binary.
- `contracts/spec/engine-architecture.yaml`: `funnel_of_autonomy` block marked `MACHINE-ENFORCED` with code evidence; `3-plan` human_role clarified ("approves sprint slicing, then the agent executes mechanical work without procedural confirmation").
- `contracts/spec/catalog-audit.yaml`:
  - `system_design_gate.status` → `CLOSED — enforced as mandatory spec->plan gate (DC6)` with code evidence (`phase_transition.rs:113`, test `:244`).
  - `fixes_required` FX1/FX2/FX3/FX5/FX6 marked `CLOSED` with evidence; FX4 marked `PARTIALLY CLOSED`; FX7 remains `OPEN`.
  - `orphan_workflows_no_module` → `[]` (resolved); added `orphan_resolution` block assigning 8 cross-cutting workflows to `core-runtime`, 4 retired/renamed.
- `contracts/spec/slice-4-governance-spec.yaml`: `S4.3` status `pending` → `done` with implementation evidence.
- `README.md`: P3.3 "remains a later perf layer" → **shipped** (snapshot + checkpoint_ref + rotation, thresholds 64MiB/100k/250ms); Option 1 install documents the `forge` wrapper shipped alongside `forge-core`.

### Documentation
- Funnel-of-autonomy (principle #6) is now machine-enforced for contact density (was previously documented-only; gap D4 closed in Wave 2).

---

## [0.2.0] — 2026-07-03

### Added
- **Dual-lane risk router** — `forge-core autonomy route` returns fast vs rigorous lane decisions from `autonomy_policy` + optional `verification_goal`.
- **Seven evolve-phase governance contracts** — `autonomy_policy`, `verification_goal`, `agent_run`, `memory`, `checkpoint`, `eval_run`, `telemetry`.
- **Multi-agent ops visibility** — starts with the `agent_run` run-graph contract.
- **Self-evolve memory** — typed provenance, freshness, promotion, and supersession fields.
- **Outcome observability** — represented by `eval_run` and `telemetry` contracts.
- **Durability hardening** — WAL fsync, path-safety, symlink escape checks, TOCTOU revalidation.
- **GitHub Actions CI** — fmt / clippy / tests / contract validation.

### Changed
- Consumer/sidecar boundary tightened: `state_root` must resolve under `sidecar_root` and end in `.forge-method`; runtime fails closed rather than silently creating consumer-local Forge state.
- Core is now a normal consumer (bootstrap exception removed).

---

## [0.1.0] — 2026-07-01

### Added
- **Initial typed-contract runtime** — contracts, engine, store, validator, CLI.
- **110-workflow catalog** — migrated and eligible for routing.
- **Guide** — `describe` / `decide` / `status` with router eval corpus.
- **Claim engine + conflict detection + worktree isolation + coordination eval.**
- **Integrity spine** — non-malleable, origin-bound authority; write-time rejection of unauthorized mutations.

---

## Versioning & release artifact policy

- Workspace version lives in `Cargo.toml`; git tags `v0.x.y` mark releases.
- Each release publishes provenance artifacts: `.sha256`, `.sigstore`, `.cdx.json` (SBOM) per binary asset, verifiable through the host-adapter supply-chain surface (sigstore / Rekor / Fulcio / SCT / OCSP / CRL / TUF).
- Legacy `v1.x`–`v2.x` tags are **out of scope** — they belong to the predecessor Python runtime in the sibling archive repo and are not ancestors of this history.

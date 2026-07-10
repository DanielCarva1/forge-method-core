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
- **P4a Execution Admission policy decision point.** `forge-core-decisions::execution_admission` now evaluates a pure, deterministic, fail-closed commit-time snapshot spanning the Assurance Case, content-addressed Operation/Command/Effect contracts, trusted principal observations, replay reservation, claim and gate revisions, and commit guarantees.
- A typed P4a specification and executable scenario matrix cover the narrow admitted single-effect WAL path plus untrusted principals, replay, stale snapshots, missing gate evidence, contract tampering, duplicate bindings, unsafe commands, and insufficient commit scope.
- **P4b.1a trusted-principal substrate.** Mutating MCP attestations can now be resolved through a strict operator-owned YAML registry that binds credential, principal, agent, role, audience, exact tools, authority grants, revocation status, and the authoritative ed25519 key. Freshness, canonical execution-intent digest, `operation.execute`, and registry-key verification fail closed; deterministic authority-field KATs and adversarial caller-selected-key tests pin the boundary.
- A deliberately revoked principal-registry example documents safe operator setup without publishing a usable credential.
- **P4b.1b durable replay substrate.** `forge_core_store::replay_wal` now provides an explicitly initialized, CRC32C-framed reserve/consume WAL bound to pseudonymous principal/audience/nonce identity, canonical intent and commit-descriptor digests, and compare-and-swap revisions. Recovery and appends are lock-scoped and fsynced; manifest/WAL half-pairs, corrupt transitions, path escape, and 8 MiB / 10,000-record capacity exhaustion fail closed.
- An effect-lock-first `ReplayCommitGuard` retains both authority locks for the future kernel boundary. The typed replay and execution-trust-boundary specs explicitly keep live MCP/CLI mutation disabled and record the deferred whole-pair anti-rollback, compaction/rotation, and cross-WAL crash-reconciliation work.
- **P4b.2a opaque authority handoff.** New host-neutral crate `forge-core-authority` owns detached attestation verification, the operator principal registry, and non-cloneable/non-deserializable `VerifiedExecutionAuthorization`. Private fields, a single registry-backed execution factory, redacted `Debug`, and audit DTOs prevent callers from minting or serializing verified authority.
- `forge-core-authority` now exposes the adapter-neutral `VerifiedExecutionCall` and injected `ExecutionExecutor` seam; MCP re-exports compatibility names for its projection. The structured request rejects caller-selected root, durability, payload scope/limits, transaction identity, commit time, output flags, and unknown arguments; a private adapter test proves valid authority reaches the executor once without spawning a CLI child.
- **P4b.2b prepared late-admission boundary.** `forge-core-kernel` now consumes verified calls into an opaque single-effect transaction that derives canonical project/audience/contract/payload/lock/WAL/transaction/durability authority, runs file-effect preflight under the exact effect lock, durably reserves replay, and retains an owned effect-lock-first replay guard.
- Late evaluation repeats preflight, accepts only fresh Assurance Case/claim/gate/state/time observations from a trusted snapshot source, rebuilds principal/replay/contract/freshness/commit facts inside the kernel, and evaluates P4a into a non-cloneable admitted typestate. Six boundary tests prove zero project/effect-WAL writes across admission, tampering, cross-audience, stale claim, filesystem drift, and snapshot failure paths.
- **P4b.2c provenance-bound one-effect commit.** `LateAdmittedExecutionTransaction::commit` now consumes the opaque typestate, repeats preflight and Execution Admission from a new bounded snapshot at the immediate commit call, canonicalizes complete redacted authorization/Admission/preflight/descriptor/replay provenance, and fsyncs it in the effect-WAL `begin` before any project write. Raw nonce values are replaced with the verified fingerprint.
- The committed path retains effect authority while replay is consumed, then appends typed `replay_consumed` evidence. Typed receipts distinguish complete success from effect-committed/replay-pending and completion-marker-pending states so callers cannot safely retry an already committed effect as a new request.
- `reconcile_prepared_execution_commits` now recovers incomplete effects, strictly verifies provenance and trusted root/audience bindings, idempotently consumes a pseudonymous replay key after an effect Commit, and appends missing completion evidence. Focused tests cover valid commit, immediate claim/filesystem drift, tampered provenance, provenance-retaining compaction, and the effect-Commit/replay-Consume crash window.
- **P4b.3a typed MCP deployment policy.** A strict YAML contract now distinguishes active read-only deployment from a validated-but-dormant trusted single-effect posture. Closed enums and cross-field checks pin the exact audience, sole `execute-operation` tool, canonical root, bounded loaders, pre-listen reconciliation, one-effect scope, explicit opt-in, provenance commit protocol, and same-user boundary acknowledgement without exposing an activation capability.

### Changed
- The P3 conversational resume token now uses the shared canonical Assurance Case token implementation consumed by execution admission.
- MCP stdio remains read-only by policy: any allowlist containing a mutating tool fails startup even when both the registry and in-process executor are present. P4b.2a removes the forgeable subprocess handoff and P4b.2b/P4b.2c prove the dormant replay-bound Admission, one-effect commit, and recovery path. P4b.3a validates deployment posture but cannot activate it; trusted adapter loading, startup reconciliation, and operation-wide semantics remain subsequent checkpoints.
- Read-only MCP subprocesses now pin the current executable instead of resolving `forge-core` through `PATH`, run in the canonical repo root, clear the inherited environment before copying a minimal OS/runtime allowlist, and receive null stdin so the JSON-RPC stream cannot leak into child commands.
- Replay authority-bearing runtime operations now require a pre-existing manifest/WAL pair; only the explicit operator initializer may create it. The initializer cannot distinguish first bootstrap from wholesale pair deletion or rollback, so an operator-protected root and external epoch/head or initialization policy remain required for enforced deployment. Without compaction, the 10,000-record cap alone permits at most 5,000 completed two-record reserve/consume lifecycles, and the byte cap may permit fewer.
- **Source migration:** P4b.1a/P4b.2a added fields to the public Rust `McpServerConfig`; downstream struct literals must now also supply `mutation_executor` or use `McpServerConfig::default_read_only()`. `AuthorizedPrincipal` fields are private and no longer deserialize; use its getters or `audit()`. The old MCP `attestation` and `principal_registry` module paths remain compatibility re-exports from `forge-core-authority`. Legacy read-only wire payloads remain compatible. Existing custom MCP allowlists must remove `policy: mutate`; a principal registry and executor do not enable mutation.
- **P4b.2b Rust migration:** `VerifiedExecutionAuthorization` audit/getter output now retains the exact freshness windows used at verification. New preparation APIs require a canonical `TrustedExecutionEnvironment` with explicit audience. The borrowed `ReplayCommitGuard` remains compatible; `OwnedReplayCommitGuard` is additive. No CLI or MCP wire shape changed.
- **P4b.2c Rust migration:** `EffectWalRecord` adds optional provenance, replay-binding, and replay-completion fields plus the `replay_consumed` stage; downstream Rust struct literals must initialize the new fields. Legacy serialized WAL lines remain readable through serde defaults. New commit/reconciliation APIs are additive and no CLI or MCP wire shape changed.

---

## [0.4.0] — 2026-07-09

### Added
- **Agent-native Assurance Case v0.** A deny-unknown-fields typed contract family now represents intent, project snapshots, obligations, evidence-backed claims, Decision Requests, Capability Gaps, ranked next actions, and target-specific readiness.
- **Read-only Obligation Engine vertical slice.** `forge-core-decisions::obligation_engine` deterministically derives a semantically validated Assurance Case from host-proposed intent, observations, epistemic-risk signals, capabilities, and irreducible human decisions without IO, model calls, or mutation authority.
- Four Obligation Engine fixtures cover novel-domain/method/capability gaps, artifact-only progress, explicit waiver, and verified release readiness.
- **Conversational assurance Adapter.** `forge-core assurance derive` projects a host-authored Obligation Engine input into an agent-facing envelope containing the complete Assurance Case, compact guidance, human-attention status, and a content-addressed resume token; `assurance resume` validates persisted state and reproduces the same guidance.
- The read-only MCP allowlist now projects the `assurance` tool, including an order-independent flag-only invocation for generic pass-through adapters.
- A typed conversational golden-path fixture proves chat-only human input, explicit future gaps, ranked next action, and replacement-agent continuity without exposing YAML or workflow selection to the human.

### Changed
- Workspace version bumped `0.3.0` → `0.4.0` for the additive Assurance Case, Obligation Engine, CLI command, and default read-only MCP tool surface.
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

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
- **P4b.3b trusted MCP loaders.** Canonical project-confined reads now load byte-bounded operation, command, effect, risk-audit, payload, and complete local snapshot material. Payload bytes require a SHA-256 digest carried in the signed tool intent; authority, content tokens, and exact claim/gate revisions are cross-checked before a redacted audit projection is produced.
- `DormantTrustedMcpExecutor` exercises the complete trusted loader seam in process and then always rejects activation. It has no network, model, subprocess, write, replay-reservation, WAL, or kernel-preparation capability; public MCP mutation remains blocked pending P4b.3c startup reconciliation and explicit opt-in.
- **P4b.3c explicit reconciled MCP activation.** `forge-core mcp serve` now accepts trusted deployment policy, exact allowlist, principal registry, state-relative snapshot, and a separate `--enable-trusted-single-effect` flag. Startup resolves the Project Link sidecar state root, verifies replay authority, runs P4b.2c reconciliation, verifies replay again, and constructs a non-serializable activation proof before listening.
- `TrustedSingleEffectMcpExecutor` carries loaded risk-audit and citation requirements into kernel evaluation before replay reservation, then executes late Admission, immediate-commit Admission, provenance-bound one-effect commit, replay consume, and completion evidence. MCP returns typed `applied`, `blocked`, or `recovery_required`; mutation never falls back to a subprocess.
- Project initialization now provisions the replay manifest/WAL pair as part of the explicit sidecar bootstrap. Runtime activation and request processing never recreate missing replay authority.
- **P4b.4a signed mutable-authority snapshot binding.** Execution Admission requests now carry a canonical SHA-256 token over the complete claim snapshot, gate snapshot, current state version, and trusted observation time. The MCP loader checks this token before replay reservation, while late and immediate-commit Admission recompute it from freshly captured material under retained locks. Snapshot edits and TOCTOU drift therefore fail closed before any effect-WAL record or project write.
- **P4b.4b agent-operated snapshot generation.** `forge-core mcp snapshot` now derives a complete content-bound Admission snapshot from project contracts, the Project Link sidecar, the authoritative claim WAL, required gate documents, and an active registry credential. It writes atomically to a state-root-confined path and returns the exact execution-intent digest for attestation; duplicate refs, state drift, missing claims, corrupt authority, revoked credentials, and output escapes fail closed.
- **P4b.4c secure credential lifecycle.** `forge-core mcp credential` provisions OS-random ed25519 credentials, atomically updates an operator-owned registry, rotates by revoking the old authority before deleting its secret, revokes before secret deletion, and signs exact MCP arguments plus the generated Admission digest in process. Private key bytes are never emitted and registry/secret locations under project or Forge state roots are rejected.
- **P4b.4d real-client readiness proof.** `forge-core mcp readiness` now validates the exact allowlist, active credential and private/public key match, policy audience, snapshot content binding, and startup reconciliation before generating a standard stdio client configuration. An official `rmcp` client test initializes the generated server, lists the exact mutation tool, carries the signed `_meta.attestation` over the wire, applies one governed effect, and verifies effect/replay WALs remain in the Project Link sidecar with no consumer-local `.forge-method`.
- **P4b.5a external replay anti-rollback.** `forge_core_store::replay_anchor` adds a strict bounded operator-protected head with random epoch, monotonic generation, deployment-policy identity, manifest digest, and exact WAL-prefix digest/length/sequence binding. `mcp replay-anchor provision|verify|advance` manages the lifecycle without placing authority in project or Forge state.
- Trusted MCP policy now requires `external_monotonic_head`; readiness-generated clients pin the canonical anchor path, startup verifies it around reconciliation, and every execution verifies before work and advances after durable replay transitions. Focused rollback tests restore a valid older complete WAL, while the official-client proof confirms an applied mutation advances the external head.

### Changed
- The P3 conversational resume token now uses the shared canonical Assurance Case token implementation consumed by execution admission.
- MCP stdio remains read-only by default. Mutation is admitted only for the sole `execute-operation` tool when explicit P4b.3c policy, registry, loader, Project Link sidecar root, startup reconciliation, and enable flag all agree. Incomplete or broader configurations fail closed.
- Read-only MCP subprocesses now pin the current executable instead of resolving `forge-core` through `PATH`, run in the canonical repo root, clear the inherited environment before copying a minimal OS/runtime allowlist, and receive null stdin so the JSON-RPC stream cannot leak into child commands.
- Replay authority-bearing runtime operations require a pre-existing manifest/WAL pair; only the explicit operator initializer may create it. P4b.5a trusted MCP additionally requires a surviving external head to detect wholesale pair rollback. Coordinated rollback of replay state and anchor is still outside the cooperative same-user guarantee. Without compaction, the 10,000-record cap alone permits at most 5,000 completed two-record reserve/consume lifecycles, and the byte cap may permit fewer.
- **Source migration:** P4b.1a/P4b.2a/P4b.3c added fields to the public Rust `McpServerConfig`; downstream struct literals must supply `mutation_executor` and `trusted_deployment` or use `McpServerConfig::default_read_only()`. `AuthorizedPrincipal` fields are private and no longer deserialize; use its getters or `audit()`. The old MCP `attestation` and `principal_registry` module paths remain compatibility re-exports from `forge-core-authority`. Legacy read-only wire payloads remain compatible.
- **P4b.2b Rust migration:** `VerifiedExecutionAuthorization` audit/getter output now retains the exact freshness windows used at verification. New preparation APIs require a canonical `TrustedExecutionEnvironment` with explicit audience. The borrowed `ReplayCommitGuard` remains compatible; `OwnedReplayCommitGuard` is additive. No CLI or MCP wire shape changed.
- **P4b.2c Rust migration:** `EffectWalRecord` adds optional provenance, replay-binding, and replay-completion fields plus the `replay_consumed` stage; downstream Rust struct literals must initialize the new fields. Legacy serialized WAL lines remain readable through serde defaults. New commit/reconciliation APIs are additive and no CLI or MCP wire shape changed.
- **P4b.3b Rust migration:** `ExecutionPayloadBinding::new` remains compatible but carries no trusted digest. Trusted MCP loading requires `new_verified` or wire syntax `target=path#sha256:<64 lowercase hex>`; unsigned bindings continue to parse and then fail closed at the dormant trusted loader. Risk-audit rule sets, rules, and detector variants now reject unknown YAML fields.
- **P4b.3c Rust migration:** trusted policy documents add required `state_root_binding`; read-only uses `disabled` and trusted mode uses `project_link_resolved`. `TrustedExecutionEnvironment::from_project_and_state_roots` supports canonical external sidecars. Field-evidence registry structures now reject unknown YAML fields.
- **P4b.4a source migration:** `ExecutionAdmissionRequest` adds `authority_snapshot_token`. Older serialized requests deserialize the field as empty for diagnostics but cannot enter trusted execution; hosts must regenerate and re-attest the request. Rust struct literals must initialize the new field, normally through `authority_snapshot_token`.
- **P4b.4b CLI migration:** trusted snapshot YAML no longer needs to be authored by hand. Hosts should call `forge-core mcp snapshot` and use its state-relative `snapshot_ref` plus returned `execution_intent_digest` for the attestation step.
- **P4b.4c operator migration:** replace hand-built registry keys and external signing scripts with `mcp credential provision|rotate|revoke|sign`. Existing registry files remain readable; private keys are deliberately not imported from project content.
- **P4b.4d host migration:** run `mcp readiness --client-config-output <json>` after snapshot/signing setup and configure the host from that generated file. Re-running readiness is the supported replacement-agent resume check.
- **P4b.4d Rust migration:** `TrustedExecutionEnvironment` now derives an internal effect-store root from the resolved `.forge-method` state root so embedded and Project Link sidecar deployments retain the same effect-lock/WAL invariants. Non-`.forge-method` state roots fail early. MCP adapters using `rmcp` must preserve protocol `_meta`; Forge restores the library-extracted `RequestContext.meta` before attestation verification.
- **P4b.5a operator/Rust migration:** trusted deployment policies add required `replay_rollback_protection` (`disabled` for read-only, `external_monotonic_head` for trusted mode). Trusted `mcp serve` and `mcp readiness` require `--replay-anchor`; `ReconciledTrustedMcpDeployment::reconcile` adds the external anchor path. Provision with a `deployment_id` exactly equal to the trusted policy id.

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

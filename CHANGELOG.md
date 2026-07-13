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
- **P6b governed Domain Pack resolution, trust, and lifecycle (`0.7.0`).**
  Closed schema-0.2 contracts and a bounded deterministic resolver preserve
  compatible locks, reject dependency confusion/equivocation/revocation, and
  keep pure read-only projections explicitly untrusted. Operator-selected
  Ed25519 registry and publisher verification plus an operator-protected
  monotonic no-fork anchor produces an opaque admission snapshot; only the
  Domain Pack TCB can promote its exact records. Fresh
  recomputation binds raw package sidecars, composition, runtime capability
  demands, default-deny sandbox policy, compatibility, and a bounded project
  snapshot before an opaque commit capability can activate a generation.
  Intent-exact install, upgrade, rollback, and removal persist every staged raw
  artifact in a content-addressed object store and publish complete
  record-addressed immutable generations, a hash-linked ledger, reachable
  receipts, retained OS locking, CAS, and crash-safe active-pointer replacement.
  Historical rollback selects an exact reachable receipt and byte-identical
  lock without colliding with its earlier generation. Removal may persist explicit domain/capability
  gaps as degraded state; external providers remain denied. The mixed CLI
  exposes `resolve|status|recover|preflight|apply`, with mutation excluded from
  the default MCP projection. Fourteen registered contract families, signed
  and adversarial corpora, and a new aggregate validator check cover the slice.
  First trust requires an explicit operator provisioning ceremony that pins
  the exact trust-policy digest; lifecycle preflight/apply never silently
  perform TOFU. Static filesystem escapes and
  non-concurrent tamper fail closed. Same-OS-principal race replacement after
  validation and project mutation after the final snapshot remain outside the
  cooperative local P6b threat boundary; hostile deployments require OS
  principal isolation and remote CAS for immutable artifacts.
- **P6a governed Domain Pack contracts and deterministic composition (`0.6.0`).**
  Five closed schema-0.1 families model candidate manifests, typed content,
  persistent project requirements, exact composition requests, and auditable
  candidate projections. The pure bounded composer verifies raw/JCS manifest
  and content identities plus license artifacts, SemVer compatibility, exact dependencies,
  declared conflicts, cycles, namespace ownership, typed references, capability
  gaps, and bilateral whole-policy replacements before deterministically composing a sealed
  core with multiple packs. A neutral two-pack/removal corpus proves stable
  order and explicit missing-domain/capability gaps. Read-only `domain-pack
  validate|compose` commands provide the agent surface with path confinement and
  zero-write E2E evidence. Domain Pack YAML remains unable to enter or mutate
  the kernel's exact 42-policy registry; lifecycle resolution, trust,
  install/upgrade/rollback/remove, and activation remain P6b work.
- **P5d.5 signed legacy retirement and the pullable `0.5.0` checkpoint.** Forge
  preserves a byte-identical 110-workflow historical audit subject, routes only
  the 68-workflow operational catalog, and retires exactly 42 admitted legacy
  authorities through policy-derived five-surface deletion evidence, typed
  consumer diagnostics, independent Ed25519 roles, and opaque kernel admission.
  Runtime evidence re-executes 189 frozen P5d.4 scenarios for the 27 additional
  policies and requires exact report, outcome, and digest equality in the
  promoted admitted runtime. The separate golden-path suite exercises the 15
  base policies with signed authority, readiness, completion receipts, and
  replacement-agent continuation; together the two gates cover 42 policies.
  Guide payload schema `0.2` and minimum consumer/package version `0.5.0` are
  additive distribution boundaries; the frozen workflow governance release
  identity remains `0.4.0`.
- **P5a workflow migration foundation.** A closed typed plan and pure
  `forge-core-decisions` evaluator now inventory all 110 workflow documents,
  classify 15 golden-path workflows, 18 domain-pack candidates, and 77
  compatibility playbooks, and emit a deterministic content-addressed
  migration manifest with future policy, obligation, claim, playbook, and
  evaluator links.
- `forge-core guide migration-audit` exposes the complete read-only audit to a
  host agent from embedded defaults or explicit catalog/plan paths. Exact
  110/110 legacy projection parity, schema/count/digest drift, overlapping or
  unknown classification, malformed plans, and unsafe retirement policy are
  covered by unit, adversarial, and real-binary tests.
- A full-catalog SHA-256 deletion baseline binds every typed workflow field,
  including procedural steps that are absent from the legacy routing
  projection, so silent content loss blocks P5b readiness.
- **P5b workflow governance authority boundary.** Closed bundle/evaluation
  contracts and a pure deterministic Module derive candidate phase/dependency
  eligibility, progression, claims/obligations, completion, Capability Gaps,
  Decision Requests, and ranked next actions for simulation.
- Evidence rules bind accepted kinds, strength floors, freshness, passing
  thresholds, and disproof behavior. Missing, weak, stale, inconclusive, or
  contradictory evidence cannot fabricate even a candidate verified claim;
  caller-asserted completion remains incomplete until simulated obligations
  are satisfied, and the raw result remains non-authoritative either way.
- `forge-core guide govern-simulate` exposes explicitly `simulation_only`
  active/blocked/complete candidates and an optional simulation-only legacy
  compatibility projection. These raw YAML results cannot unlock progression,
  completion, mutation, or Execution Admission. A representative
  `write-spec`/`build-story` corpus proves cyclic/dangling policy rejection,
  input-order determinism, playbook deletion independence, explicit ignorance,
  and legacy workflow-id mismatch fail-closed behavior.
- The live-authority lane is an opaque, non-deserializable kernel typestate
  that requires a trusted snapshot rather than caller-authored observations.
  The admitted policy bundle is encapsulated in that snapshot and receives an
  internally computed canonical digest; verified completion remains bound to
  project, state version, phase, and readiness target. Serializable audits
  cannot reconstruct authority.
- **P5c executable golden-path governance.** A repository-owned admitted bundle
  now migrates the 15 selected discovery, planning, build, verification,
  correction, continuity, readiness, and release behaviors into closed typed
  policies with deterministic priority, activation, prerequisites, due targets,
  capabilities, decisions, evaluators, evidence rules, and waiver limits.
- The trusted Project Snapshot Adapter derives policy selection, phase, state
  version, applicability, prerequisite completion, active signals, capabilities,
  human decisions, evidence freshness, waivers, registry revocation, and continuity from
  the admitted bundle plus durable receipts. Caller-authored workflow, phase,
  bundle, target, availability, evidence freshness, and completion are not
  authority.
- The kernel-owned workflow-governance ledger is confined to the Forge state
  root, exclusively locked, fsynced, capacity-bounded, and hash-chained over
  project/bundle/state identity. Recovery rejects corruption, malformed or torn-tail truncation,
  sequence gaps, previous-head mismatch, duplicate records, and illegal state
  regression rather than repairing authority-bearing history. Without an
  external monotonic anchor it cannot detect clean truncation to a valid prefix
  or a same-user rollback of the complete internally consistent ledger.
- `forge-core workflow init|next|resume|shadow` exposes the chat-host
  path without workflow selection. Applicability, capability, evidence, human
  signal, decision, and waiver receipts require exact registry-authorized signed intents
  bound to the current project/snapshot/ledger context; unsafe caller-authored
  direct observation commands are not part of the authority surface.
- Completion is a one-use opaque transition: consumption re-locks and rechecks
  the project snapshot, admitted bundle digest, ledger head, state version,
  phase, selected policy, readiness target, and current evidence before appending
  policy-completion and replacement-agent continuity receipts. Drift fails
  closed without a completion append.
- Fixed-registry consumption now compares the complete signed principal audience,
  tool, role, key, and grant set, so a caller-owned registry cannot escalate an
  otherwise valid fixed credential. Signal episodes are monotonic across
  freshness/snapshot drift, semantic evidence identities deduplicate repeated
  scenarios, and Release completions reopen after project drift.
- Raw semantic ledger mutation moved into the dedicated
  `forge-core-workflow-governance-tcb` crate, which is a direct dependency only
  of the kernel Adapter. Store, CLI, and MCP no longer receive an unchecked
  workflow mutation API through Cargo feature unification. Multi-record
  governance operations prepare under one lock and replace the WAL only once,
  so late preparation failures cannot durably apply a prefix.
- Policy-scoped applicability, capability, decision, evidence, and waiver
  receipts are accepted only for the currently selected policy or an explicit
  live boundary recheck. Snapshot drift invalidates applicability and every
  completion target; future-policy evidence and pre-authorized waivers fail
  closed. Direct same-user state-root writes remain outside the isolation claim.
- Read-only shadow output compares migrated and legacy projections for the same
  live snapshot while denying mutation and retirement. Kernel and real-binary
  consumer tests exercise automatic 15-policy routing,
  artifact-only rejection, representative execution, late completion
  consumption, deterministic replacement-agent resume, stale/contradictory
  evidence, scope/time-bounded authorized waivers, mid-flight replacement-agent
  resume, and junction-backed workspaces. All P5c publication gates pass.
- **P5d.1 versioned workflow-release foundation.** Closed Rust contracts now
  model stable release lineage, ordered content-addressed candidate batches,
  exhaustive per-workflow disposition intent, explicit quarantine, versioned
  compatibility/deprecation diagnostics, domain-pack deferral, and signed
  retirement proposals. Authored documents have no `executable` or `retired`
  state, and every raw batch is fixed to `candidate_only`.
- A pure release evaluator binds the exact 110-workflow P5a inventory, canonical
  legacy and batch digests, zero-based batch order, embedded evidence byte
  integrity (explicitly not behavioral sufficiency),
  workflow/policy bindings, and the globally composed policy graph. It emits a
  deterministic candidate-only structural scorecard and fails closed on missing/duplicate
  entries, domain leakage, evidence drift, cross-batch conflicts, compatibility
  shrinkage, and unverified retirement.
- `forge-core guide rollout-audit` exposes the P5d.1 evaluator as a read-only
  agent surface with repeatable batch inputs and typed blocked results. A real
  binary E2E builds an explicit 110-entry manifest over the actual 15-policy
  golden bundle and proves no assessment can serialize as executable or
  retired.
- Guided start now hands every healthy sidecar to idempotent `workflow init`
  and then `workflow next` with an explicit structured root argv. Legacy guide,
  operation-contract, and preview material remain compatibility diagnostics,
  while the shipped `start-forge` skill fails closed on unexpected routing or
  ledger errors instead of asking a human/agent to select workflow or phase.
- **P5d.2 opaque release admission and project pinning.** A closed embedded
  registry now separates stable lineage, release identity, runtime-bundle
  identity, policy-set identity, and registry provenance. Its raw evaluation is
  explicitly non-authoritative; only a non-cloneable, non-serializable kernel
  loader can admit the exact legacy P5c bundle and its adjacent foundation
  successor. The successor has a new bundle identity but the same 15 policy
  objects, so none of the other 95 catalog workflows gains runtime authority.
- **P5d.3 reviewed core-assurance candidate and behavioral shadow corpus.** A
  typed non-authoritative overlay composes five non-golden policies with the
  frozen fifteen-policy predecessor, while three ambiguous/unsafe workflows
  remain explicit quarantines. Exact raw-byte and canonical semantic digests
  bind an acyclic review subject, coverage policy, two corpus partitions, five
  ablated bundles, and a derived 35-scenario shadow report.
- Behavioral evaluation now recomputes governed outcomes rather than trusting
  authored pass counts: every candidate has positive, negative, ambiguity,
  false-completion, stale-evidence, replacement-agent resume, and isolated
  ablation evidence. The result is candidate-only with zero mismatches/errors;
  it cannot admit a release or satisfy runtime governance.
- Release-registry evolution checks now preserve the exact historical prefix,
  compare arbitrary-length SemVer without panic, authenticate appended
  manifest/runtime bytes, and reject fork, reorder, drift, or missing history.
  A frozen real P5d.2 upgraded WAL proves predecessor pins and record digests
  survive future repository evolution.
- **P5d.4a independent review and first new-policy admission.** A closed Review
  Index binds raw and canonical identities for the complete P5d.3 evidence
  graph, candidate and promoted bundles, predecessor/expanded registries,
  evaluator source, and frozen WAL. The pure evaluator recomputes the 35
  scenarios and derives only `ready_for_independent_authorization` with
  `non_authoritative` authority.
- A fixed reviewer registry and domain-separated Ed25519 payload require
  distinct semantic-reviewer and release-authorizer principals, credentials,
  public keys, and signatures. Revoked/out-of-window credentials, wrong
  audience/domain/key, duplicate identity, blocking findings, and artifact or
  promotion transplant fail closed. Private keys are absent from the repository.
- The kernel consumes a non-cloneable/non-deserializable verified capability to
  admit only the exact append-only third release. Its final bundle contains 20
  policies, preserves all P5d.2 history, excludes all three quarantines, and
  requires the explicit foundation-to-core-assurance CAS-bound upgrade with
  `invalidate_all` receipt semantics. Kernel and real CLI E2E tests prove
  replacement-agent resume, prepared-authority invalidation, receipt-window
  reset, idempotency, and no direct genesis skip.
- Existing P5c ledgers remain byte-compatible and resolve to an implicit legacy
  release even when a newer binary is installed. A typed `release_upgraded`
  event remains source-enveloped, binds exact source/target release and policy
  identities, then activates the target only for following records. Direct
  bundle switches, stale heads, self/reverse/skipped transitions, policy drift,
  tampered proofs, and generic event injection fail closed.
- `forge-core workflow release-status` returns the durable active release, pin
  origin, exact head/snapshot CAS values, admitted adjacent successor, and the
  structured argv an agent can execute. `workflow release-upgrade` accepts only
  a target release id plus those CAS digests—never registry, manifest, batch,
  bundle, or release paths—and is idempotent after a committed transition.
  `init`, `next`, and `resume` expose the same release audit to replacement
  agents.
- Workflow WAL replacement now uses a bounded, digest-bound next/previous/
  transaction protocol on Windows and reconciles it under the ledger lock
  before recovery. Fault injection proves interrupted upgrades recover exactly
  the prior WAL or the committed target and never silently reinitialize an
  empty ledger; corrupt, ambiguous, symlinked, or non-regular protocol states
  fail closed. This is logical crash recovery, not an external monotonic
  rollback anchor, and Windows directory flushing remains best-effort.
- Receipt carryover is allowed only for an exact policy-set match. Incompatible
  future releases establish a post-transition receipt window, while prepared
  completion authority captured before any upgrade fails its late head/bundle
  recheck. Historical registry provenance remains auditable without requiring a
  future binary's whole registry digest to stay frozen.
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
- **P4b.5b Execution Principal propagation.** A portable `ExecutionPrincipal` evidence DTO now preserves the registry-verified principal id, agent id, and role through claims, conflict attribution, Execution Admission, effect-WAL provenance, recovery, durable pre-effect traces, kernel receipts, and trusted MCP results. Serialized identity remains evidence only and cannot replace opaque verified authority.
- Claim acquisition accepts `--principal-id`; the claim WAL and projections preserve it, and trusted execution requires an exact principal/agent/role claim match. The kernel durably stages a principal trace before effect-WAL Begin and returns its event id; recovery rejects provenance whose explicit principal diverges from the authorization audit. Official-client tests prove the registry principal reaches both receipt and trace.
- **P4b.6a operation-wide WAL substrate.** `forge-core-kernel::compose_operation_effect_bundle` validates an operation's complete ordered effect set, preserves constituent identity, resolves logical and physical targets through the store's canonical mapping, rejects overlapping aliases, and derives one internal `operation_transaction` envelope for the complete disjoint file-backed write set.
- Operation-wide failure and crash tests prove that one later constituent failure or a missing commit marker restores every earlier write. Execution Admission now accepts only a verified multi-effect `whole_operation` WAL scope, while saga remains fail-closed. The legacy executor rejects multi-effect operations before command evidence or project writes instead of committing effects independently.
- **P4b.6b prepared operation-wide commit.** The host-neutral opaque `ExecutionRequest` and `PreparedExecutionMaterial` now carry a complete ordered effect set while preserving the legacy single-effect constructor/accessor. The kernel binds every source ref, content token, operation declaration, loaded document, and payload target before lock or replay reservation, then derives one operation-wide transaction envelope.
- Late and immediate-commit preflights and Execution Admission now cover the complete envelope. Commit provenance binds both the envelope and ordered constituent effects; principal traces and receipts preserve every source identity; recovery validates the stored strategy/effect scope and reconciles replay without reapplying committed writes. Adversarial tests cover reordered refs, second-effect drift, one Begin/Commit pair, and the effect-Commit/replay-Consume crash window.
- **P4b.6c trusted MCP operation-wide activation.** Deployment policy now has a closed `trusted_operation_wide`/`operation_wide` posture and `mcp serve` requires its dedicated `--enable-trusted-operation-wide` flag; it cannot be combined with or substituted by the single-effect opt-in. Startup reconciliation, Project Link roots, external replay anchor, exact allowlist, registry, audience, and same-user acknowledgement remain mandatory.
- Trusted loading and snapshot generation now bind the operation's complete ordered 2..64 unique effect set, every content token, and the payload union. MCP accepts ordered `--effect`/`--payload` arrays, detached signing covers that exact JSON object, readiness derives the matching enable flag, and scalar single-effect syntax remains compatible.
- The official `rmcp` client proof now initializes the generated operation-wide server, lists and invokes `execute-operation`, commits two sidecar outputs in one WAL transaction, preserves both effect ids and the verified principal in evidence, consumes replay, advances the external anchor, and creates no consumer-local `.forge-method`.

### Changed
- Contract-family inventory and generated schema views now include workflow
  governance releases, migration batches, and retirement authorization
  proposals. `FamilyKind` gains the corresponding variants; exhaustive Rust
  matches must handle them. These schema surfaces remain candidate/support
  only and do not activate a runtime release or trusted retirement verifier.
- The contract-family inventory and generated schema views now register the
  workflow migration plan as a non-authoritative migration manifest. P5 remains
  in progress; no workflow execution or legacy retirement authority moved in
  P5a.
- The repository validator now semantically validates the canonical Workflow
  Governance Policy bundles. P5b advanced the clean-check regression anchor to
  126; the admitted P5c golden-path bundle advances this checkpoint to 127. P5
  remains in progress: P5c completed executable governance for the selected
  15-policy golden path and passed its publication gates; P5d remains responsible
  for governed rollout across the remaining catalog and evidence-backed legacy
  retirement.
- The workflow governance ledger, not `state.yaml`, is authoritative for the
  P5c path. `state.yaml` remains a compatibility projection. The internal hash
  chain detects record-level tampering and malformed/torn tails but has no
  external monotonic anchor; clean truncation to a valid prefix and malicious
  same-user rollback of an entire internally consistent ledger are not detected.
  P5c targets the local CLI path and does not claim an end-to-end MCP workflow
  Adapter or hostile-user isolation.
- **P5c Rust migration:** `CallerRole` adds `Human`; exhaustive downstream Rust
  matches must handle the new variant. Workflow-governance bundle/evaluation
  contracts add typed routing, prerequisites, due targets, evaluator providers,
  waiver policy, receipt subjects/provenance, freshness, principal diversity,
  and durable ledger events. Raw P5b simulation remains non-authoritative.
- The P3 conversational resume token now uses the shared canonical Assurance Case token implementation consumed by execution admission.
- MCP stdio remains read-only by default. Mutation is admitted only for the sole `execute-operation` tool when exact policy scope, registry, loader, Project Link sidecar root, startup reconciliation, and its dedicated enable flag all agree. Incomplete, cross-scope, or broader configurations fail closed.
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
- **P4b.5b claim/Rust migration:** `ClaimIdentity` adds optional `claimant_principal_id` so legacy serialized claims remain readable; new trusted flows must populate it. `AcquireRequest`, active/reconcile summaries, commit provenance, commit receipts, and not-committed outcomes add principal evidence fields. Consumers using Rust struct literals must initialize the new fields; use the verified registry principal rather than a caller-invented identity.
- **P4b.6a Rust migration:** `EffectKind` adds `OperationTransaction`; exhaustive downstream matches must handle the new variant. The bundle constructor is additive and public MCP remains single-effect. Legacy callers that attempted multiple per-effect commits now receive `OperationWideCommitRequired` before side effects and must migrate to the later prepared operation-wide path.
- **P4b.6b Rust migration:** `ExecutionRequest::new` remains source-compatible and maps its optional effect to an internal ordered set; use `new_operation_wide` and `effect_contract_refs` for multiple effects. `PreparedExecutionMaterial::new` remains single-effect; use `new_operation_wide` for the complete set. `PreparedCommitDescriptor` and `ExecutionCommitReceipt` add constituent effect evidence and commit strategy fields. Recovery treats their absence as a legacy single-effect descriptor while rejecting malformed new scope.
- **P4b.6c MCP migration:** operation-wide hosts use `trusted_operation_wide`, `effect_scope: operation_wide`, ordered JSON arrays for `--effect` and `--payload`, and `--enable-trusted-operation-wide`. Existing `trusted_single_effect`, scalar arguments, `ExplicitTrustedSingleEffectOptIn`, and `TrustedSingleEffectMcpExecutor` remain compatible aliases/paths. Readiness chooses the enable flag from policy and rejects scope mismatch.

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

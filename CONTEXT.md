# Forge Method Core Domain Context

This file is a navigation aid for agents working on the Forge codebase. It is
not runtime authority. Canonical product decisions remain typed under
`contracts/`.

## Product thesis

Forge is an agent-native governance control plane. A human communicates through
chat with a host agent. The host agent owns research, artifacts, implementation,
tests, and explanation. Forge owns project truth, obligations, authority,
evidence, continuity, and next-best-action guidance.

Forge governs what must be true and what must be proven. It does not script the
agent's words, persona, reasoning, or implementation strategy.

Canonical policy:
`contracts/policies/agent-native-product-constitution.yaml`.

Active architecture direction:
`contracts/spec/agent-native-assurance-architecture.yaml`.

Active implementation plan:
`contracts/plan/agent-native-guidance-plan.yaml`.

Pullable compatibility checkpoint: the Rust workspace package version is
`0.7.0`. Guide `describe`/`status` retirement diagnostics use payload schema
`0.2`, and consumers of that extended surface must be at least `0.5.0`.
Workflow governance release identity remains the frozen `0.4.0` five-release
chain; package SemVer and governed release identity are separate axes.

## Domain language

- **Human**: the source of intent, preferences, value judgments, trade-offs,
  acceptance, and exceptional authority. The human is not expected to edit
  Forge artifacts or operate the development toolchain.
- **Host Agent**: the model-driven worker that converses with the human and
  performs research, planning, artifact creation, implementation, verification,
  and explanation.
- **Forge**: the model-agnostic protocol and runtime that governs state,
  obligations, authority, evidence, and continuity.
- **Intent Proposal**: the host agent's typed interpretation of the human's
  desired outcome, constraints, preferences, unacceptable outcomes, and open
  uncertainty.
- **Project Snapshot**: a derived, evidence-backed view of the project's current
  state. It is not a hand-edited status document.
- **Obligation**: a condition that must become true or be explicitly waived by
  authorized judgment. It defines required outcomes, not procedural dialogue.
- **Assurance Claim**: a proposition about the product or process whose status
  is unknown, hypothesized, supported, verified, disproven, or waived.
- **Evidence**: provenance-bearing observation that supports or disproves an
  Assurance Claim. Representative execution is stronger than artifact presence.
- **Playbook**: a non-authoritative strategy an agent may use to satisfy one or
  more obligations.
- **Evaluator**: a deterministic or governed method for assessing evidence
  against an Assurance Claim.
- **Decision Request**: a question sent to the human only when value, preference,
  material trade-off, irreversible risk, cost, or external authority cannot be
  resolved from project evidence.
- **Capability Gap**: an explicit statement that the current agent, tools,
  environment, knowledge, or evaluators cannot reliably complete or verify an
  obligation.
- **Domain Pack**: a content-addressed, candidate-only extension that contributes
  namespaced policies, obligations, hazards, lifecycle models, playbooks,
  evaluators, fixtures, capability requirements, Adapter declarations, and
  provided domains. P6a composes exact caller-supplied packs deterministically;
  it does not install, trust, activate, execute, or grant mutation authority.
- **Domain Pack Project Requirements**: durable desired domains and capabilities
  stored independently of pack presence. Removing or rejecting a pack therefore
  creates explicit missing-domain/capability gaps instead of erasing the need.
- **Domain Pack Composition Projection**: a pure candidate-only projection over
  one sealed core binding, persistent requirements, and exact raw/JCS-bound pack
  inputs. Even `composable` is structural evidence, never runtime admission.
- **Workflow Migration Manifest**: a deterministic, content-bound inventory
  that classifies every legacy workflow and links it to candidate governance
  targets. In P5a it is compatibility evidence only, never execution or
  retirement authority.
- **Workflow Governance Release Manifest**: versioned, content-addressed P5d
  rollout intent under a stable lineage. It lists one explicit disposition for
  every catalog workflow and an ordered set of candidate batches. A raw
  manifest cannot claim executable or retired state.
- **Workflow Migration Batch**: a candidate-only set of workflow/legacy-digest/
  policy bindings plus representative, adversarial, and shadow evidence refs.
  Only global release composition may validate a batch; deserialization never
  activates it.
- **Workflow Release Scorecard**: a deterministic read-only projection derived
  from the manifest, exact catalog, P5a audit, batches, composed policy graph,
  and content-integrity-checked embedded evidence refs. P5d.1 does not prove
  behavioral sufficiency; its states are structural candidate/compatibility/
  quarantine/domain/retirement-pending only, not runtime authority.
- **Workflow Governance Release Registry**: a closed embedded list of exact
  release, runtime-bundle, predecessor, policy-set, and provenance bindings.
  Raw parsing and pure validation remain `non_authoritative`; project-local or
  caller-selected registry, manifest, batch, and bundle paths never enter the
  live admission lane.
- **Admitted Workflow Governance Release**: a non-cloneable,
  non-serializable kernel value minted only from the fixed embedded registry.
  Release identity, runtime-bundle identity, policy-set identity, and registry
  provenance are distinct. In P5d.2 only the exact P5c 15-policy set can cross
  this boundary.
- **Workflow Release Pin**: the active admitted release reconstructed from the
  workflow ledger. A P5c ledger maps to its exact implicit genesis release;
  later releases require a hash-chained `release_upgraded` transition. Merely
  installing a newer Forge binary cannot move the pin.
- **Workflow Behavioral Review Subject**: an acyclic, content-addressed P5d.3
  candidate identity over exact overlays, composed policy set, legacy digests,
  quarantines, proposed release lineage, evaluator, and governed projection.
  It excludes evidence and final batch/manifest digests so its corpus can bind
  it without a hash cycle.
- **Workflow Behavioral Shadow Evidence**: a non-authoritative recomputation of
  normalized governed outcomes across positive, negative, ambiguity,
  false-completion, stale, resume, and ablation scenarios. A consistent report
  is only a review candidate; co-authored policy and fixtures cannot admit a
  release or prove independent semantic truth.
- **Sequential Workflow Release Admission V2**: a release-specific schema-0.2
  review, evaluation, and signed authorization for one adjacent append only.
  The kernel starts from the frozen P5d.4a V1 result and folds each compiled V2
  descriptor in order. Authorization for one release cannot admit another, and
  a blocked tail cannot expose a partially advanced registry.
- **Workflow Retirement Authorization**: a closed signed proposal binding the
  exact legacy workflow, replacement policy, release, compatibility window,
  shadow evidence, deletion evidence, reviewer registry, audience, and time.
  Trusted signature and semantic verification are required before retirement.
- **Workflow Policy**: the closed, kernel-evaluated definition of workflow
  eligibility, prerequisite policies, obligations, claims, evaluator rules,
  capability requirements, and irreducible Decision Requests. It defines what
  must be true, never the words or implementation strategy an agent must use.
- **Workflow Governance Simulation**: a pure deterministic candidate result
  derived from one Workflow Policy and caller-authored observations. It is
  always `simulation_only`; even candidate completion cannot unlock progression,
  completion, mutation, or Execution Admission.
- **Verified Workflow Governance Decision**: an opaque, non-deserializable
  kernel typestate derived only from a trusted Project Snapshot. It, not raw
  YAML or playbook position, is the intended progression/completion authority
  for the admitted P5c golden path when the live Adapter derives it from
  authoritative receipts. P5c integration is complete for that admitted set.
- **Admitted Workflow Governance Bundle**: the repository-owned, embedded
  15-policy golden-path bundle selected inside the kernel and bound by a
  canonical digest. A caller cannot replace it with a preferred workflow,
  policy, phase, or readiness target.
- **Workflow Governance Ledger**: the state-root-confined, exclusively locked,
  fsynced, hash-chained receipt history from which the P5c Adapter derives
  workflow state. `state.yaml` is a compatibility projection, not authority for
  this path.
- **Workflow Governance Receipt**: a typed, content-bound observation of project
  import, applicability, signal, capability probe, human decision, evaluator
  evidence, waiver, completion, revocation, phase advancement, or replacement-
  agent continuity. Freshness and revocation are computed from receipt data and
  trusted observation time rather than caller labels.
- **Phase Projection**: a human- and agent-friendly summary of maturity derived
  from satisfied obligations. It is not the primary source of authority.
- **Execution Principal**: the authenticated and authorized caller identity and
  role derived by a trusted Adapter. A caller-selected key or a valid signature
  by itself is not an Execution Principal with mutation authority.
- **Execution Admission**: a deterministic commit-time decision that binds the
  ready Assurance Case, exact Operation/Command/Effect contracts, principal,
  replay reservation, claim/gate revisions, and commit guarantees. P4a is the
  pure policy decision point; it is not yet the runtime enforcement point.
- **Replay Reservation**: a durable, single-use binding between a fresh nonce,
  a revision, and the canonical execution-intent digest.
- **Commit Assurance**: verified guarantees that the chosen WAL or saga scope
  can recover, roll back, or compensate the complete authorized mutation.
- **Operation Effect Bundle**: a kernel-derived internal transaction envelope
  that preserves the declared effect identities while placing the complete
  disjoint file-backed write set under one effect lock, WAL, and recovery
  outcome. It is implementation, not caller-selected authority.
- **Constituent Effect**: one original content-addressed effect contract bound
  to an operation-wide transaction. Its ref/id/token remain durable provenance
  even though the store commits the derived envelope as one effect id.

## Architectural direction

- The **Project Snapshot Module** concentrates state derivation.
- The **Obligation Engine Module** derives required claims, gaps, decisions, and
  next-best actions from Intent Proposal plus Project Snapshot.
- The **Execution Assurance Kernel** governs authority and durable mutation.
  The P4a decision module defines its fail-closed admission contract; the P4b
  Adapter/kernel integration is required before Forge claims runtime enforcement.
- The **Operation Effect Bundle Module** deepens the existing local effect-store
  transaction for multi-effect operations. The prepared kernel now binds and
  commits that bundle under opaque authority; the MCP Adapter activates it only
  through exact bounded ordered loading, scope-specific policy/opt-in, startup
  reconciliation, and signed intent. Sagas remain future work for external or
  irreversible commit domains.
- The **Workflow Migration Foundation Module** reads the complete typed catalog
  and a repository-owned plan, then returns one deterministic classification,
  target-link, shadow-parity, and deletion-baseline manifest. Its Interface is
  read-only; P5a deliberately cannot execute, mutate, or retire workflows.
- The **Workflow Governance Release Module** validates closed P5d release and
  batch candidates against the complete P5a inventory, canonical digests,
  embedded evidence, and the globally composed policy graph. `guide
  rollout-audit` exposes only `candidate_only` results. P5d.2 adds a separate
  opaque runtime loader for the fixed embedded registry, exact P5c-compatible
  release pinning, and adjacent CAS-bound upgrades; the audit result itself
  still cannot activate anything. P5d.3 adds a five-policy typed overlay,
  three quarantines, exact raw/canonical digest separation, and 35 recomputed
  behavioral scenarios while keeping the registry unchanged. P5d.4a binds that
  complete graph into distinct semantic-reviewer and release-authorizer
  signatures, then lets only a fixed kernel loader consume the opaque verified
  capability. The append-only third release contains 20 policies and uses
  `invalidate_all`. P5d.4b.1 freezes that V1 path and adds a generic V2 contract
  for exactly one release-specific review and authorization at a time. The
  loader folds the 13-policy assurance-operations release sequentially into a
  four-release, 33-policy registry only after 91 scenarios and independent
  authorization pass. P5d.4b.2 then folds nine agent-native continuity and
  lifecycle policies after 63 scenarios into a five-release, 42-policy
  registry, preserving the exact predecessor prefix. P5d.4b is complete with
  42 migration, 47 compatibility-only, three quarantined, and 18 domain-pack
  candidates. P5d.5 then freezes the full 110-workflow historical subject,
  removes exactly the 42 admitted legacy documents from the 68-workflow
  operational catalog, and admits their retirement through policy-derived
  five-surface deletion evidence, repository consumer fixtures, two
  independent Ed25519 roles, and an opaque kernel capability. Runtime and
  legacy lifecycle remain separate scorecard axes: 42/47/3/18 and 42/68.
  Domain authority stays in P6.
- The **Workflow Governance Kernel Module** validates a closed policy bundle
  and separates two lanes. `guide govern-simulate` derives candidate guidance
  from raw YAML and is never authority. The opaque verified lane receives a
  trusted Project Snapshot whose phase/state, prerequisite completion,
  applicability, signals, capabilities, human decisions, evaluator results,
  waivers, revocations, and freshness come from the ledger-pinned admitted
  release and durable receipts. `workflow init|next|resume|release-status`
  exposes its guidance and pin to a host agent; `release-upgrade` accepts only
  an admitted target id plus exact release/head/snapshot CAS digests. Completion
  is consumed only after a lock-scoped late recheck, and authority prepared
  before an upgrade fails on the new head/bundle. Advisory playbooks and legacy
  simulation/shadow projections sit outside authority.
- The **Domain Pack Module** validates five closed schema-0.1
  document families, exact raw and canonical manifest/content identities, SemVer
  compatibility, dependency/conflict graphs, namespace ownership, bilateral
  whole-policy replacements, typed cross-references, and bounded resource use. It then
  topologically composes core plus multiple packs with deterministic priority
  remapping and an auditable projection digest. P6b adds fourteen closed
  schema-0.2 lifecycle families, a bounded resolver whose raw output remains
  explicitly untrusted, operator-selected registry/publisher signature
  verification, a monotonic no-fork registry anchor, exact locks, default-deny
  built-in capability binding, compatibility reports, and a separate TCB. The
  TCB consumes opaque anchored supply-chain and project-snapshot proofs, freshly
  recomputes resolution/raw composition/trust/compatibility/operation intent,
  persists every exact staged raw object, and activates only a cross-linked
  record-addressed immutable generation through one crash-safe CAS pointer.
  Historical rollback uses a reachable receipt and byte-identical lock without
  reusing its transactional generation. Initial registry trust requires an
  explicit operator provisioning ceremony that pins the exact trust-policy
  digest; lifecycle preflight/apply never silently perform trust on first use.
  Static links, junctions, traversal,
  special files, and non-concurrent tamper fail closed. A malicious process
  running as the same OS principal can still race-replace a node after
  validation or mutate the project after the final snapshot check; that hostile
  model requires separate OS-principal permissions and remote CAS for immutable
  artifacts. `domain-pack
  validate|compose|resolve|trust-provision|status|recover|preflight|apply` is the agent surface;
  only `apply` activates, while status/recovery may finish an interrupted
  pointer transaction. The universal kernel registry remains the exact
  five-release 42-policy P5 authority and excludes all 18 deferred domain
  candidates.
- Host-specific integrations are **Adapters** at a host seam; deleting one must
  not change Forge domain behavior.
- Workflows migrate from authoritative step sequences into policies,
  obligations, playbooks, and evaluators.

P5c completed executable governance for the selected 15-policy golden path,
including its signed observation boundary, adversarial golden-path proof, and
full workspace gates. P5d.1 now establishes versioned release/batch contracts,
explicit catalog disposition, content-addressed evidence validation, and a
candidate-only scorecard; it does not activate a new release or retire legacy
authority. P5d.2 now admits only a policy-equivalent foundation successor,
preserves unchanged P5c ledgers, derives the active release from durable
history, and moves it only through a crash-recoverable adjacent transition.
P5d.3 owns candidate-only behavioral comparison and quarantine for the first
reviewed batch. P5d.4a now admits those five policies only after independent
cryptographic review, exact evaluator recomputation, and an explicit adjacent
project upgrade. P5d.4b.1 admits the assurance-operations batch through a
sequential V2 authority chain without rewriting frozen V1 history. P5d.4b.2
completes reviewed core rollout with the nine continuity/lifecycle workflows `checkpoint-preview`,
`collaboration-handoff`, `research-closeout`, `retrospective`, `sprint-status`,
`project-context`, `spec-distillation`, `evolve-project`, and
`product-area-map`. P5d.5 completes P5 with signed deletion-backed retirement,
verified tombstone diagnostics, a byte-identical evidence archive, and the
final two-axis scorecard; 47 compatibility-only workflows remain explicitly
non-executable, three remain quarantined, and 18 remain reserved for P6. The workflow
path targets the local agent-facing CLI; allowlist metadata does not constitute
an end-to-end MCP workflow Adapter.

The workflow ledger's internal hash chain detects record tampering,
malformed/torn tails, sequence gaps, and head mismatch within the history it
receives, but it has no external monotonic anchor. It cannot distinguish clean
truncation to a previously valid prefix from legitimate history. A malicious
same-user rollback of the entire internally consistent ledger is therefore an
explicit residual threat, and hostile-user isolation is not claimed.
Interrupted local WAL replacement is a different failure class: P5d.2 uses
digest-bound next/previous/transaction artifacts and reconciles them under the
ledger lock to the exact prior or committed WAL. Ambiguous or corrupt protocol
state fails closed. This logical crash recovery does not provide the missing
external rollback anchor, and Windows directory flushing remains best-effort.
The shipped CLI confines raw workflow-ledger mutation to
`forge-core-workflow-governance-tcb`, a direct dependency only of
`forge-core-kernel`. `forge-core-store`, CLI, and MCP expose no semantic append
surface, preventing Cargo feature unification from widening the TCB. The
kernel Adapter and the dedicated ledger crate form the shipped workflow TCB;
direct same-user state-root writes remain outside the P5c isolation claim and
this process boundary is not a cryptographic sandbox.

These Modules should earn **Depth** by keeping their **Interface** smaller than
their **Implementation**, increasing **Leverage** for callers and **Locality** for
maintainers. Apply the deletion test before introducing additional crates or
pass-through layers.

## Core epistemic rule

Human ignorance is expected. Agent ignorance is expected. Hidden or unmanaged
ignorance is a governance failure.

Forge cannot guarantee discovery of every unknown unknown. It can require a
repeatable assurance process that makes consequential ignorance increasingly
likely to surface before completion is declared.

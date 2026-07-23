# Product status

## Current checkpoint

The source workspace is package version `0.12.0`. It includes implemented P5,
P6a–P6d, P7a.2, P7b, P7C discovery/governed acquisition, a source-level P7D
joined Core/pack rebase, and controls for P7E release correctness and P7G
evidence-preserving CI topology.
This source document does not infer whether the matching `0.12.0` tag has
published assets; verify the selected GitHub Release and exact sidecars.
Production-host P7F evidence, actor independence, and full P7 completion remain
separate claims.

Use the canonical [four-identity table](../README.md#four-identitiesdo-not-collapse-them):
source checkpoint, selected verified prebuilt (`v0.4.0` was the historical
predecessor to this candidate), project workflow release pin,
and project-local Domain Pack effective epoch are deliberately distinct.
Verify commit/tag, binary version, archive manifest/assets, project status, and
CI evidence independently.

## Implemented

- Typed contracts, semantic validation and generated command/workspace maps.
- Agent-native project bootstrap and durable workflow init/resume/next.
- Append-only reviewed universal-core release chain with a 43-policy P7b
  successor and pinned release upgrades.
- Claims, conflict/gate enforcement, transaction/replay/recovery foundations.
- Domain Pack composition, governed lifecycle, and reviewed-learning promotion.
- Accepted-intent-bound Domain Pack search/explain with exact reviewed-entry
  metadata, deterministic candidate-only projections, and explicit gaps.
- Candidate-only acquisition planning that replays the exact discovery request,
  binds normalized requirements, rejects stale or tampered selection state,
  and derives exact P6 resolver/composer inputs from matching package material.
- Versioned reference-pack manifest, content, evidence, and hostile corpus are
  included in the deterministic release payload. High-level acquisition takes
  an exact current candidate plus explicit operator approval, derives P6
  lifecycle internals, and can activate a clean project's first generation.
  C6.1 source closure also derives initialized-project install, upgrade,
  rollback, remove, and changed-Core rebase operations without human-authored
  lifecycle documents. Targeted compile-only checks are green for
  `forge-core-contracts`, `forge-core-decisions --all-targets`,
  `forge-core-domain-pack-tcb --all-targets`, and
  `forge-core-cli --all-targets`. No runtime or heavy/stabilization tests,
  failure injection, hosted CI, release/publication, independent semantic review,
  real-host, or field evidence ran.
- C6.2 source implements signed catalog mirrors and immutable descriptors,
  publisher and registry signature coverage, monotonic protected catalog anchors
  with cumulative revocation state, candidate-only online/cache/offline planning,
  exact byte and media verification, deterministic reject-on-full cache policy,
  rollback provenance, and a bounded local CLI adapter. The CLI imports the exact
  operator-provisioned object set from a local artifact root; it does not perform
  DNS, TLS, redirects, subprocess downloading, general network transport, trust,
  installation, lifecycle commit, or activation. Targeted compile-only checks are
  green for `forge-core-contracts --all-targets`,
  `forge-core-decisions --all-targets`,
  `forge-core-command-surface --all-targets`, and
  `forge-core-cli --all-targets`; deferred C3.2-C3.4 evidence remains pending.
- C7.1 source implements deterministic generic pack skeleton generation, exact
  sealed-Core validation, candidate/composition/compatibility/learning/review
  diagnostics, and a non-authoritative CLI adapter that writes exact generated
  bytes only beneath a fresh explicit root or renders a report. Passing C7.1 is
  not signing, publication, trust, installation, lifecycle commit, or activation.
  Its contracts, decisions, command surface, CLI, and source-test targets compile;
  runtime, publication, independent-review, real-host, and field evidence remain
  deferred.
- C7.2 source composes C7.1 author evidence and C6.2 immutable descriptors into
  candidate-only package preparation, independent-review readiness, exact
  external-signing requests, unverified detached-signature evidence assessment,
  unsigned catalog evolution, and cumulative revocation proposals. Existing
  authority owners still own signing bytes, cryptographic verification, policy,
  protected-anchor advancement, and catalog admission. C7.2 adds no private-key
  custody, network publication, CLI, trust, lifecycle commit, installation, or
  activation. Contracts, decisions, and source-test targets compile; actual
  signing, publication, independent review, runtime, and field evidence remain
  pending.
- C5.1-C5.3 source now provides candidate-only generation-chained
  post-BuildVerify episodes, guarded phase advancement, and complete durable
  replacement continuity. The episode binds an exact release, rollback baseline,
  deployment and operational observations, feedback, incident/bug intake,
  evolution identity, and the five existing readiness/continuity policy roles.
  C5.2 compare-and-swap admission reuses admitted policy/assurance and hard
  transition gates for `BuildVerify -> ReadyOperate` and
  `ReadyOperate -> Evolve`; rollback assessment and evolution triage preserve
  phase. C5.3 epoch `0.8` records retain each complete episode snapshot plus
  owner-bound request, completion, and reviewed health-recovery projections.
  Fresh-process recovery joins exact stable request and claim IDs, selects the
  latest complete generation, classifies claim liveness, and returns cloned
  authority-free data. Historical summary-only epoch `0.7` records remain
  readable but cannot satisfy complete replacement reconstruction. Generic
  append remains rejected, ordinary automatic phase planning still stops at
  BuildVerify, and deserialization grants no claim, mutation, phase, release,
  signing, trust, private-key, lifecycle, install, activation, protected-anchor,
  or host-selection authority. Targeted compile-only all-target checks are green
  for the workflow-governance TCB and kernel after the C5.3 changes. Rust tests,
  runtime fresh-process/failure-injection evidence, deployment/rollback
  execution, hosted CI, publication, release, real-host, and field evidence
  remain pending.
- FRUST-020 source completes the typed OperationContract planner behavior used by
  C5.2. Already-done and report-only plans return read-only status; expired-claim
  handoff requires review; missing runtime-handoff gates remain gate-required;
  suggestible handoff and correct-course routes wait for explicit human input.
  Non-ready executor calls now return those typed outcomes before durable producer
  admission or mutation-gate evaluation, and focused source tests assert that no
  Forge state root, staging, command evidence, or effect application is created.
  The kernel all-target source and test targets compile; Rust test execution and
  broader runtime/hosted evidence remain deferred.
- FRUST-030 source makes Guide operation-contract-first. The accepted protocol
  composes the existing `GuideDecision` with the exact next
  `OperationContractDocument`; kernel validation binds legal catalog/phase routing,
  guide provenance, workflow, phase, allowed actions, observed state version,
  diagnostics, and one of six closed routes before the CLI returns that exact
  operation. Facilitation, research, visual alignment, correct-course,
  already-done, and mechanical execution fail closed on host substitution or route
  policy drift. `--protocol-file` is canonical; the legacy flag spelling cannot
  admit a decision-only document. All-target source and source-test targets compile
  for contracts, command surface, kernel, and CLI. Protocol acceptance itself
  grants no mutation authority, and Rust test execution plus broader runtime/hosted
  evidence remain deferred.
- FRUST-031 source completes the C5.2 funnel-autonomy boundary with one accepted
  typed policy shared by Guide phase projection, OperationContract planning, and
  the execution gate. Early ambiguous phases restore human guidance and research
  pressure; mechanical plan/execute/repair loops require an eligible phase, exact
  lane claim, passing aggregate gate, authority evidence, and effect references.
  Undeclared destructive effects block at runtime, while publish behavior and
  explicit authority-transition candidates restore exact release and authority
  gate scopes. Typed risk declarations remain descriptive and cannot grant
  mutation, phase, release, signing, private-key, trust, lifecycle, or host-selection
  authority. `selected_host` remains none. Contracts, decisions, kernel, and CLI
  all-target source plus source-test targets compile; runtime tests and broader
  stabilization/hosted evidence remain deferred. C5.2 and its C5.3 durable
  replacement-continuity follow-on are source-complete pending that evidence.
- C2.2 source now owns complete-state `backup create`, `backup verify`,
  `restore preflight`, and `restore apply`. It binds the Project Link, full
  sidecar authority membership, validated public external registries/anchors,
  exact release identity, immutable archive and receipt publication, retained
  restore authority, protected completion selection, rollback checks, and
  fail-closed recovery/cleanup. External broker private keys stay outside Forge
  state and Forge backups. Targeted all-target checks are green for Store,
  EventLog, workflow-governance TCB, kernel, Domain Pack learning-store, Domain
  Pack TCB, and CLI. These were direct compile-only implementation checks with
  the available toolchain, not hermetic Rust 1.85/MSRV, runtime, interruption,
  mixed-version, platform, hosted-CI, release, or field evidence. Durable
  reinitialize-as-new is now source-complete: public plan/apply dispatch captures
  exact predecessor and diagnosis inputs, requires a plan-derived confirmation,
  and transfers the sealed plan into a retained `ProjectLinkCas`. The Store keeps
  the predecessor alive through a private lifetime anchor, reserves an unrelated
  destination, publishes the exact successor through CAS, persists operation-scoped
  plan/WAL/receipt leaves, and verifies retries against the live successor. This
  abandons rather than reinstalls prior authority, selects no host, and accepts no
  private external broker keys. Runtime interruption/retry and failure-injection
  evidence remain deferred.
- C2.3 and FRUST-061 source now expose one explicit `forge-core lifecycle`
  family for idempotent setup, status, doctor, verified install/update,
  rollback, and product-owned uninstall. A closed release document binds
  semantic version/core compatibility, immutable source and provenance
  references, rollback metadata, typed release notes, and exact asset inventory.
  The CLI reuses distribution admission and local artifact verification, then
  rechecks the exact bounded no-follow bytes before publishing an immutable
  content-addressed generation and audit receipt. Exact retry is idempotent;
  downgrade and same-version content drift reject; rollback verifies the prior
  generation; status reports exact host configuration while `selected_host`
  remains none. Uninstall removes only unchanged exact inventory and preserves
  unknown or modified files, consumer projects, Project Links, sidecars,
  operator anchors, backups, registries, and external private keys. Targeted
  locked all-target checks are green for contracts, command surface, and CLI.
  Rust test execution, C2.4 interruption/mixed-version coverage, failure
  injection, MSRV/platform matrices, hosted CI, publication/release,
  downloaded-asset verification, real-host, and field evidence remain deferred.
- Adjacent-Core rebase derives exact CAS from release and lifecycle state,
  revalidates persisted package inputs and external operator roots, commits an
  immutable target-Core generation, and appends one joined Core/effective epoch
  event. Persisted-plan recovery handles the lifecycle-to-workflow crash
  boundary. This remains source-level until cumulative E2E/CI evidence passes.
- Generic effective epochs and a game-development reference proof without
  game-specific Rust.
- Durable accepted human intent with kernel-derived revisions and monotonic
  assurance epochs; callers cannot choose identifiers, epochs, or status.
- Exactly eight explicit universal lenses with five closed epistemic states:
  `unknown`, `supported`, `verified`, `disproven`, and `waived`.
- Reviewer-origin representative-slice definitions and separately originated
  runtime execution using configured broker and separation-domain checks. This
  is protocol-level separation, not proof of physical reviewer independence.
- Native Linux, Windows, Intel macOS, and Apple Silicon macOS default
  workspace/platform gates plus one Linux reference protocol journey; every
  non-Linux runner separately compiles that feature-gated journey.
- Source release tooling binds archive version, exact `release_tag`, and full
  `source_commit`; the workflow re-verifies payload/checksum before publication.
- Source release CI extracts native x86_64 Linux/Windows and Intel/Apple Silicon
  macOS packages and smokes binary/wrapper version plus `start`, `workflow init`,
  `resume`, `release-status`, and `next`; this is not evidence of a published
  asset.
- CI source topology enforces 120-second Tier 0, 900-second focused, and
  1,800-second platform/cumulative hard step timeouts, terminates timed-out
  process trees, and persists JSON timing evidence. Budget targets become
  evidence only when the corresponding hosted CI runs complete.

## Deliberate boundaries

- Forge does not ship a model or hosted agent.
- It cannot guarantee discovery of every unknown unknown.
- It does not silently install into host skill/plugin directories.
- Release-visible reference-pack bytes are not an automatically trusted public
  registry and carry no private signing key or operator approval.
- Discovery and acquisition planning do not download, trust, install, or
  activate a package. Only explicit `domain-pack acquire apply` may activate a
  reviewed local artifact set after every trust and lifecycle check.
- A newer binary does not silently migrate project authority.
- Same-OS-principal hostile isolation is outside the cooperative threat model.

- The P7F bundle checker proves only structure and content integrity; it cannot
  prove production-host use, chat-only interaction, semantic coverage, actor or
  reviewer independence, publication, or P7F passage.
- Forge governs Forge-mediated writes only. Direct editor, shell, or host writes
  remain ungoverned unless covered by an admitted Forge transaction and receipt.

## Adoption gaps that must remain honest

The `0.12.0` checkpoint retains P7a.2 deterministic, state-bound workflow
action packets; minimal closed-input request preparation; an external broker
registry that stores public keys only; host-origin event verification; and
durable reserve/commit replay state. `workflow action apply` is the one-call
high-level lane; the old request-file plus attestation-file commands remain
expert compatibility surfaces. The local credential signer remains a
cooperative same-principal proxy and is not proof of human presence or
independent review.
The local `workflow action authorize` convenience is therefore restricted to
`operator_credential_broker` packets; high-authority packet classes require the
external broker.
C1.2 generic broker source is complete: it composes the public-key registry,
signed origin event, action packet, reserve/commit WAL, provenance, bounded replay
recovery, and separate administrative controls. Forge persists public metadata and
content-free receipts only; external private broker keys never enter Forge state or
backups, and no CLI, MCP, or IPC signing oracle exists. C1.3 also has typed,
candidate-only read-only MCP and setup-gap source. These source closures do not
select, support, install, activate, or release a host.

The C1 protocol source adds broker event `0.2`, signed opaque native-host
event/session/interaction provenance, permanent workflow-ledger wire epoch `0.5`,
and recovery-only handling for frozen broker event `0.1`. These are Rust-core
security primitives, not selected-host evidence. The exact Codex CLI `0.143.0`
spike found protocol-shaped hooks, approvals, and app-server messages but no
unforgeable signer boundary. Pi `0.80.7` exposes unauthenticated in-process input
and confirmation hooks under shared OS authority. OpenCode `1.14.33`, with the
locally loaded `1.14.25` plugin/SDK surface, exposes client-callable prompts,
permission replies, TUI callbacks, and shell-capable plugins rather than a trusted
human-origin path. None is selected for C1 on current evidence, and no reference
host is supported yet.

P7b accepts human outcomes and constraints, not caller-authored methodology or
quality status. The agent drafts scenarios, falsifiers, representative
environment expectations, and failure modes; an independent Reviewer origin
must accept the exact latest definition. A Runtime origin in a different
configured separation domain must match that definition, the exact subject,
current snapshot/effective epoch, and every scenario. Partial execution is
only supported, any current failure is disproven, and research alone never
becomes verified. P7b reuses the existing evidence receipts and action packets;
there is no second evidence store.

Replay safety is a bounded, fail-closed recovery saga across the action replay
WAL and governance ledger. A lock-held replay reservation preflights conflicts
and capacity without appending before the authoritative ledger commit, avoiding
orphan pre-commit tombstones. When the replay WAL remains hash-chain-valid, exact
retries reconcile a missing entry or complete reserve after a post-ledger replay
interruption. Torn/corrupt replay bytes fail closed and are not automatically
truncated. Forge does not claim cross-filesystem atomicity or safety after an
attacker rewrites both the state root and its external trust anchors.

This checkpoint proves a host-neutral broker protocol, not a production host's
identity assurance. A configured broker vouches for the signed origin subject
and separation domain. Physical presence, OS-principal isolation, and a
representative Codex/OpenClaw/other-host journey remain P7f evidence rather
than hidden P7a assumptions.

Release tags and packaging may lag source checkpoints. Source installation is
the only way to run unreleased commits; it does not turn them into a release.
Prebuilt users must use only assets listed on the selected release and inspect
the archive's manifest/verification sidecars. The current source release and CI
hardening remains an implementation claim, not publication or elapsed-time
evidence.

## Roadmap rule

Post-P6 work is selected by promise-audit evidence. The canonical
[product gap register](product-gap-register.md) records accepted gaps, and the
[typed closure plan](../contracts/plan/product-gap-closure-plan.yaml) owns their
sequence and exit evidence. Priority goes to gaps that prevent a normal
chat-only journey, then distribution/fork operability, then ecosystem breadth.
Domain methods belong in reviewed packs, not core.

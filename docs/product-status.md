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
  lifecycle internals, and can activate a clean project's first generation;
  public remote catalog/download productization remains pending.
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
- Independently reviewed representative-slice definitions and separately
  originated runtime execution using the existing evidence ledger and packets.
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
WAL and governance ledger. Exact retries reconcile durable provenance; Forge
does not claim cross-filesystem atomicity or safety after an attacker rewrites
both the state root and its external trust anchors.

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

Post-P6 work is selected by promise-audit evidence. Priority goes to gaps that
prevent a normal chat-only journey, then distribution/fork operability, then
ecosystem breadth. Domain methods belong in reviewed packs, not core.

# Product status

## Current checkpoint

The source workspace is package version `0.11.0`. P5 and P6a through P6d are complete
protocol/architecture checkpoints; P7a.2 is implemented pending its cumulative
gate. The source includes workflow-release, Domain Pack, lifecycle, learning,
effective-epoch, replacement-resume, and external origin-broker authority
surfaces. This is not yet a clean production-host chat-only journey proof.

This does not mean every ecosystem feature is complete or every GitHub Release
contains the current source checkpoint. Verify commit/tag, binary version,
archive contents, and CI independently.

## Implemented

- Typed contracts, semantic validation and generated command/workspace maps.
- Agent-native project bootstrap and durable workflow init/resume/next.
- Reviewed 42-policy universal core with pinned release upgrades.
- Claims, conflict/gate enforcement, transaction/replay/recovery foundations.
- Domain Pack composition, governed lifecycle, and reviewed-learning promotion.
- Generic effective epochs and a game-development reference proof without
  game-specific Rust.
- Explicit evidence/capability/decision gaps and representative-execution
  requirements.
- Linux and Windows default workspace/platform gates plus one Linux reference
  protocol journey; Windows separately compiles that feature-gated journey.

## Deliberate boundaries

- Forge does not ship a model or hosted agent.
- It cannot guarantee discovery of every unknown unknown.
- It does not silently install into host skill/plugin directories.
- A reference Domain Pack is not an automatically trusted public registry.
- A newer binary does not silently migrate project authority.
- Same-OS-principal hostile isolation is outside the cooperative threat model.

## Adoption gaps that must remain honest

The `0.11.0` P7a.2 source checkpoint adds deterministic, state-bound workflow
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
authoritative for unreleased commits; prebuilt users must use only assets
listed on the selected release.

## Roadmap rule

Post-P6 work is selected by promise-audit evidence. Priority goes to gaps that
prevent a normal chat-only journey, then distribution/fork operability, then
ecosystem breadth. Domain methods belong in reviewed packs, not core.

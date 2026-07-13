# Product status

## Current checkpoint

The source workspace is package version `0.10.0`. P5 and P6a–P6d are marked
complete as protocol/architecture checkpoints in the typed product plan and
covered by workflow-release, Domain Pack, lifecycle, learning, effective-epoch,
replacement-resume, and cross-platform workspace evidence. This is not yet a
clean public chat-only journey proof.

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

The source checkpoint now exposes `workflow credential
provision|rotate|revoke|status|sign`. It derives the fixed operator registry and
secret directory from the Project Link, uses closed role/grant profiles, and
signs only a typed request kind without exposing private keys. P7a remains open:
`workflow next` does not yet generate the exact request/action packet, and the
host must still pass the signed output to the existing `*-authorize` command.
The local command is a cooperative signing proxy, not proof of human presence
or independent review; a host/operator approval broker remains required before
those high-authority product claims can close.

Release tags and packaging may lag source checkpoints. Source installation is
authoritative for unreleased commits; prebuilt users must use only assets
listed on the selected release.

## Roadmap rule

Post-P6 work is selected by promise-audit evidence. Priority goes to gaps that
prevent a normal chat-only journey, then distribution/fork operability, then
ecosystem breadth. Domain methods belong in reviewed packs, not core.

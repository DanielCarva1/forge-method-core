# Forge Desktop 0.1.1 alpha — release draft

**Status: local candidate, not published.** This is a Windows x64 desktop
building-block update, not a claim that the whole Forge product is complete.

## Included

- Portuguese Home, Explore and My Projects screens, with eight illustrated
  starting themes and local shortcuts to previously confirmed projects.
- Native Windows folder selection, with the existing Forge resolver validating
  the chosen project. Cancelling preserves the confirmed project; an invalid
  path clears the prior confirmation rather than showing stale project data.
- A readable view of the latest *recorded* Forge activity, next step and open
  decision count. It is not live agent progress.
- Codex CLI conversation, reconnection through a project-scoped bookmark,
  interruption, and light/dark/contrast preferences.

## Requirements and limits

- Requires an existing linked Forge project and an installed `forge-core`.
  The app does not create or link projects yet.
- Requires a separately installed and authenticated Codex CLI for conversation;
  Codex and Forge core are not bundled in this installer.
- Windows x64 only; unsigned NSIS current-user installer. No automatic in-app
  updater: install the new package over the prior alpha.
- The screens do not yet expose every Forge workflow action or full decision
  forms. A recorded status must not be read as live progress or whole-product
  completion. Mobile remote access is not provided.
- Parallel record reads exposed temporary Forge core lock conflicts. The
  desktop now serializes its own record reads and retries only recognized
  transient conflicts within a bounded period. Direct parallel core CLI reads
  can still conflict, and the record remains a snapshot, not a live status
  feed. If consultation ultimately fails, the UI hides the old record and
  offers another attempt.
- Publication and download verification require maintainer approval; a source
  commit or local installer is not a downloadable release.

## Candidate verification

The original local `0.1.1` candidate was superseded after the record-read fix.
The exact replacement candidate has SHA-256
`DC7429FB4FDE6190EDED7323B45F190D980D1E23A3EAB4CDCE040E6F88FA08C8`.
It was silently installed over the prior local `0.1.1` and then tested as a
silent `0.1.0` to `0.1.1` upgrade. Headless installed-app checks passed for
launch, saved project shortcut, theme/contrast preferences and record readback;
paired record consultations passed under the tested contention. Conversation
continuity across this exact upgrade remains **NOT_RUN**. The candidate is not
release-approved or downloadable.

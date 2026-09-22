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
- Native development checks intermittently failed to read the Forge record;
  the cause is not established. The UI keeps the old record hidden and offers
  another consultation. Do not treat this as a reliable live status feed.
- Publication and download verification require maintainer approval; a source
  commit or local installer is not a downloadable release.

## Candidate verification

One local `0.1.1` NSIS candidate was built and silently installed over a local
`0.1.0` alpha. Installer exit, Windows registration, and installed executable
version passed; SHA-256 is recorded in `README.md`. Under the current
headless-only testing requirement, post-upgrade app runtime, preferences and
conversation continuity are **NOT_RUN**. The candidate is not release-approved
or downloadable.

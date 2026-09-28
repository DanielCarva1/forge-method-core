# Forge Desktop 0.1.54 alpha.1

This update makes two common actions more reliable:

- Opening a Codex conversation without sending a message no longer saves an
  empty, potentially non-resumable thread as the project's history. An older
  real conversation remains available until a new Send is acknowledged.
- If a selected file or a file cited by the agent fails project-bound
  validation, the last valid local preview stays visible. The failed file is
  not saved as the active preview. A valid replacement still updates normally.

## Verification and limits

Focused headless browser checks covered empty/reopen, first Send/reopen,
preserving an older conversation bookmark, rejected preview replacements and
valid replacements. A rebuilt hidden Windows app used the real Codex bridge
to open an empty thread and restart without resuming it; a separate bounded
read-only Send received a real reply and resumed after restart without replay.
The hidden Windows preview check used native project-bound validation to reject
an outside file while keeping the earlier result visible. No broad Rust or
browser suite and no manual GitHub CI were run for these UI-only changes.

This is still an unsigned Windows x64 alpha with manual installer updates.
The installer bundles the pinned Forge core 0.13.2, not the source-tree core
0.13.3. Clean-machine setup, fresh ChatGPT login completion, actual external
browser launch and manual screen-reader acceptance remain NOT_RUN. The
occasional Forge record first-read error has not been isolated. Model-specific
token use and BRL-equivalent cost remain UNKNOWN.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.54_x64-setup.exe`, 123,074,217
bytes, SHA-256
`732E915472785745FFAE2D260110760448881E75222F52EBF2F76CD0239E5FFC`.
Its 93-byte `.sha256` sidecar records the same hash. Silent installation over
0.1.53 exited 0 without changing the candidate. The installed executable
reports ProductVersion 0.1.54, SHA-256
`6D09B50E2FDED316885F099F0E2792016B2A6D55F12A1BD0C59B6273630B87F3`.
The installed candidate passed the focused hidden-native empty-thread,
real replied-thread restart and rejected/valid preview checks. This exact
candidate was published as the
[0.1.54 alpha prerelease](https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.54-alpha.1).
An unauthenticated download of the installer and sidecar matched the tested
candidate's 123,074,217 bytes and SHA-256. Silent installation of the
downloaded installer exited 0 and installed the same executable SHA-256. A
focused hidden-native empty-thread/restart check passed again after that
installation. This is same-machine evidence, not clean-machine installation.

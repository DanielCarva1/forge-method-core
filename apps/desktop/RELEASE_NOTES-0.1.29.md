# Forge Desktop 0.1.29 alpha.1

This Windows x64 alpha improves recovery when a Codex conversation does not
finish normally. If Codex disconnects or reports that an update is required,
the app now consults the existing Forge project record again instead of
leaving it hidden until a manual refresh. An interrupted, failed or
disconnected turn also re-reads an already selected local preview. This
matters when a file changed before the turn stopped: the app shows what is
currently on disk rather than assuming that a failed turn made no changes.

The app does not infer a new result, resend a message, or change project
files as part of this recovery. A preview that was already unavailable is
not retried automatically. If the person is reading an enlarged preview,
the file refresh waits until that view is closed. Forge remains the source
of project state, and Codex remains the source of conversation history.

## Verification

- The browser UI suite reproduced the missing record read and stale preview
  before the respective fixes, then passed with regression coverage for
  disconnect, update-required, interrupted and failed events.
- The source-built Windows app passed a hidden native smoke test. Controlled
  Codex events caused real native reads of the existing Forge record and of
  a changed file in a temporary project. No real Codex message was sent.
- Desktop `cargo check`, all 50 non-ignored Desktop crate tests, strict
  Clippy, eight frontend unit tests, the complete browser UI suite, and
  `git diff --check` passed. The full separate core workspace was not run
  for this Desktop-only package.
- The exact candidate was silently installed over public 0.1.28 on Windows.
  The installed app passed the same hidden native smoke, then reopened an
  existing real 33-message Codex conversation, displayed its actual HTML
  result, drafted a change request and survived a full process restart
  without sending or editing a file.

## Limits

- Unexpected real-provider disconnects, update-required incidents, and
  changes made during a genuinely failed Codex turn have not been induced
  against the live provider. The native event tests use controlled events.
- This does not add an automatic updater. Install this NSIS package over
  0.1.28. The installer remains unsigned and includes the pinned Forge core
  0.13.2 and Codex CLI. WebView2 may be downloaded during setup if missing.
- The external-browser action, fresh-account sign-in completion,
  clean-machine installation and manual screen-reader acceptance remain
  unverified end to end. Mobile and self-contained offline distribution
  remain future work.
- A previous intermittent timeout when both Windows file dialogs were used
  in one hidden run has no confirmed cause. This release does not claim to
  fix it.
- Per-task subscription cost and an Astra/Sol/Luna savings comparison
  remain unknown because attributable model usage counters are unavailable.

## Tested candidate

The single NSIS candidate is `Forge_0.1.29_x64-setup.exe`, 123,071,538 bytes,
SHA-256 `299CCC8718241168318E9BCFA98A6AAC187573A01EEF44EA3FB3711F45DD14A1`.
The matching `.sha256` sidecar is adjacent to it. Silent installation of
these exact bytes over public 0.1.28 returned exit code 0 and left the
installer hash unchanged. The installed 0.1.29 executable has SHA-256
`EF66C040BF1B4E86181F121388EE5A817EE2AF59CA356EA5F2EAECF8791A0959`.
Public download continuity will be checked after publication; local
installation alone does not prove that the release asset is available.

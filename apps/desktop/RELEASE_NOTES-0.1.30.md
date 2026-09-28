# Forge Desktop 0.1.30 alpha.1

This Windows x64 alpha keeps an already selected result current when a Codex
turn stops during another UI action. If a local preview read is still in
flight, the app re-reads that same project file once after the first read
finishes. If the person is choosing another file and cancels, the app
re-reads the previously selected file. This closes two timing gaps that
could leave an older result visible after the conversation changed it.

The app does not infer a new result, select a file on the person's behalf,
send a message, or change project files. An unavailable file is not retried
automatically. A queued read is discarded when switching projects.

## Verification

- The browser UI suite reproduced both timing gaps before their fixes and
  passed afterward. It also covers a failed in-flight read, explicit retry,
  and isolation of a queued read across a project switch.
- A source-built Windows app passed a hidden native smoke. Controlled Codex
  events overlapped a real native project-file read and a canceled file-picker
  response; the subsequent reads returned the current file bytes. No real
  Codex message was sent in this smoke.
- The same source build reopened an existing real 33-message Codex chat,
  showed its actual HTML result, prepared a change request, and survived a
  full process restart without Send or file edits.
- Desktop `cargo check`, five focused preview tests (one optional association
  test ignored), all 50 non-ignored Desktop crate tests, strict Clippy, eight
  frontend unit tests, the complete browser UI suite, JavaScript syntax
  checks, and `git diff --check` passed. The separate core workspace was not
  rerun for this Desktop-only change.
- The exact candidate was silently installed over public 0.1.29 on Windows.
  The installed app passed the hidden native smoke and the real 33-message
  read-only restart journey with no Send or file edit.

## Limits

- These overlap tests use controlled Codex terminal events. An unexpected
  live-provider failure during either precise timing window was not induced.
- This does not add an automatic updater. Install the NSIS package over the
  previous alpha. The installer remains unsigned and includes pinned Forge
  core 0.13.2 and Codex CLI. WebView2 may be downloaded during setup if absent.
- A positive launch in the default external browser, fresh-account login
  completion, clean-machine installation, and manual screen-reader acceptance
  remain unverified end to end. Mobile and self-contained offline distribution
  remain future work. Issue #91 and the full Windows journey are not complete.
- A previous intermittent timeout when both Windows file dialogs were used
  in one hidden run has no confirmed cause; this release does not claim to
  fix it.
- Per-task subscription cost and model-routing savings remain unknown because
  attributable model usage counters are unavailable.

## Tested candidate

The single NSIS candidate is `Forge_0.1.30_x64-setup.exe`, 123,098,780 bytes,
SHA-256 `204A103BB2D78E82816DD6A93A6E7DEC0E7FD6048681C79CEBF1E8EAA010B92D`.
The adjacent `.sha256` sidecar matches. Silent installation of these exact
bytes over public 0.1.29 returned exit code 0 without changing the candidate.
The installed 0.1.30 executable has SHA-256
`C1F01E9E3612DF2D8B14EA0C669073E8F26FFA3F51952AF0685CBA0B6CB17F79`.
Public download continuity will be checked after publication; local
installation alone does not prove that the release asset is available.

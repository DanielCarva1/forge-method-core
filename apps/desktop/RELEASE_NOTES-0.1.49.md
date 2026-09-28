# Forge Desktop 0.1.49 alpha.1

Requests prepared from a real project file or Forge's project record now use
the same local draft handling as typed messages. A change request or request
for an explanation stays in its confirmed project, survives closing and
reopening the app, and is never sent automatically. The composer shows that
the draft is saved on this device. **Enviar** is now disabled when the
composer is empty; typing or preparing a request enables it.

This fixes a mismatch in the prior alpha: shortcut text appeared in the
composer, but could be absent from the saved draft after a restart. Existing
project-bound preview reads, explicit Send, and uncertain-delivery protection
are unchanged. Drafts remain local to this Windows user's app profile and are
not app-encrypted; avoid secrets in unsent text.

## Verification and limits

The browser UI suite reproduced the missing saved shortcut text and the
enabled-but-empty Send before the fixes, then passed after them. Hidden native
Windows testing read a real project file, prepared an unsent change request,
restarted the full app process, reopened the project, and recovered the
request without calling Send. The native file-picker response was simulated;
the file content was validated and read through native code. A real Codex
turn was NOT_RUN in this package; earlier releases separately tested one.

This remains an unsigned Windows x64 alpha without an auto-updater. Bundled
Forge core is pinned to 0.13.2. Clean-machine setup, external-browser launch,
real provider login completion, physical mobile access and manual
screen-reader acceptance remain NOT_RUN. Per-model tokens, BRL-equivalent
cost and Pro quota attribution remain UNKNOWN; no savings are claimed. No
manual GitHub CI was triggered for this change.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.49_x64-setup.exe`, 123,160,487
bytes, SHA-256
`537C7D80780EFB52A6957ED54CDA7CC2E32F0ED329D3084B5EA2CB6022AA87D6`.
Its adjacent 93-byte `.sha256` sidecar records the same hash. Silent
installation over public 0.1.48 returned exit 0, preserved the candidate
bytes and installed ProductVersion 0.1.49, executable SHA-256
`59CAC58759BAB28A6994FC8D0DB91E37947B87657E0EE5677F6EFE23132E4CFD`.
The installed candidate passed the hidden native shortcut-restart smoke.
Package commit `b8a878a5` was pushed and tagged
`desktop-v0.1.49-alpha.1`. Unauthenticated public downloads of the installer
and sidecar returned identical bytes and hash to the tested candidate.
Installing those downloaded bytes silently returned exit 0, preserved the
installer hash and installed the same 0.1.49 executable hash. The downloaded
installation passed the hidden native shortcut-restart smoke. This is
same-machine package proof, not clean-machine installation. A source commit
alone does not update the installed app.

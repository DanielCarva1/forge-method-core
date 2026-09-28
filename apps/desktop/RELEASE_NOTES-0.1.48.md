# Forge Desktop 0.1.48 alpha.1

An unsent message in a confirmed project now survives closing and reopening
the Windows app. Each confirmed project has its own draft, stored in the local
app profile on this device. Reopening a project restores its text without
connecting to Codex or sending it. Switching projects does not copy one
project's text into another. A draft typed before confirming a project stays
only in the current app session until the folder is confirmed.

After Codex accepts a Send, the app removes that project's saved draft. If
delivery is uncertain or local storage fails, the app warns the user and does
not automatically resend. The composer also says when a draft is saved on
this device. Draft text is not encrypted by the app; anyone with access to
this Windows user's app profile may be able to read it. Avoid writing secrets
in an unsent message.

## Verification and limits

The browser UI suite first reproduced loss of a draft on reload against the
previous version, then passed project isolation, restart, failed validation,
accepted/uncertain Send and storage-failure checks. The hidden native Windows
WebView passed a full process restart with a real project: the draft returned
and no Send call was made. The same app was visually reviewed in its native
dark theme. Browser cases use simulated native replies; the restart case uses
the real native app but does not send a real Codex turn.

This is an unsigned Windows x64 alpha with no automatic updater. Forge core
remains pinned to 0.13.2. Clean-machine setup, external-browser launch, real
provider login completion, physical mobile access and manual screen-reader
acceptance remain NOT_RUN. Per-model tokens, BRL-equivalent cost and Pro quota
attribution remain UNKNOWN; no savings are claimed. No manual GitHub CI was
triggered for this change.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.48_x64-setup.exe`, 123,052,041
bytes, SHA-256
`24FBBD30420DCB5340E29B9CA743BD4BDC98BE0E86027BC3CE9C056A17E2174F`.
Its adjacent 93-byte `.sha256` sidecar records the same hash. Silent
installation over 0.1.47 returned exit 0, preserved the candidate bytes and
installed ProductVersion 0.1.48, executable SHA-256
`544070E31102467A1F741CC091A46BD25B8504CDF17C216F70BC69596143EDB1`.
The installed candidate passed the hidden native process-restart smoke.
Package commit `7e6f4e4b` was pushed and tagged
`desktop-v0.1.48-alpha.1`. Unauthenticated public downloads of the installer
and sidecar returned identical bytes and hash to the tested candidate.
Installing those downloaded bytes silently returned exit 0, preserved the
installer hash and installed the same 0.1.48 executable hash. The downloaded
installation passed the hidden native process-restart smoke. This is
same-machine package proof, not clean-machine installation. A source commit
alone does not update the installed app.

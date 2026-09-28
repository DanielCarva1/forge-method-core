# Forge Desktop 0.1.50 alpha.1

The project record now gives an **open choice** priority over optional questions
suggested by Forge. When both are present, it offers one primary action to ask
for the choice's original context in the same conversation. Suggested questions
remain available in a clearly optional disclosure, with their caution shown
beside them rather than in the main reading path. A suggestion is never shown
as an accepted decision. The app does not guess the original wording of a
pending choice when Forge cannot provide it.

This is a presentation change. It does not send a message, record a decision,
change Forge's lifecycle, or add a separate project store.

## Verification and limits

The browser UI suite was red on the prior mixed-choice presentation and passed
after the change. Eight JavaScript unit tests, 51 Desktop Rust tests (one
ignored), and strict Desktop Clippy passed. Hidden native Windows WebView runs
passed on the debug build and installed candidate with actual Forge project
readback and a controlled pending-choice response; the action prepared an
unsent draft without recording a choice. Dark browser and light native
captures were inspected. Real Codex creation, preview, follow-up and restart
were verified separately on installed 0.1.49; no real Codex turn was sent for
this copy-and-layout change.

This is an unsigned Windows x64 alpha without an auto-updater. Bundled Forge
core remains pinned to 0.13.2. Clean-machine setup, external-browser launch,
new provider login completion, physical mobile access and manual screen-reader
acceptance remain NOT_RUN. Per-model tokens, BRL-equivalent cost and Pro quota
attribution remain UNKNOWN; no savings are claimed. No manual GitHub CI was
triggered for each UI edit.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.50_x64-setup.exe`, 123,091,836
bytes, SHA-256
`D08F23DC35507E4BDA570B9A67850576DCC1EB4A325212558F0606850E53E614`.
Its adjacent 93-byte `.sha256` sidecar records the same hash. Silent
installation over 0.1.49 returned exit 0, preserved candidate bytes and
installed ProductVersion 0.1.50, executable SHA-256
`E781B535C3F29512B38DFD82C13D9BED72E6A4BA78C66712CF4DF19A610702BD`.
The installed candidate passed the hidden native smoke above. Public download
and installation readback are still pending; the installer is not yet
available as a 0.1.50 release.

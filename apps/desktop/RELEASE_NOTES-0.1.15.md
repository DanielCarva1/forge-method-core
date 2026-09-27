# Forge Desktop 0.1.15 alpha.1

This Windows x64 alpha improves the first project journey. An empty **Meus
projetos** screen now offers a clear choice: explore an idea or choose a folder.
The folder may be new or already contain files. Submitting without a folder
shows an instruction in the app and focuses the field rather than relying on
the browser's validation bubble.

More importantly, a project-bound Codex conversation now receives the same
Forge core executable that the app uses for project commands. This avoids the
agent silently finding an older machine-wide Forge version. The adjustment is
limited to that Codex subprocess; it does not change the computer's global PATH
or the login-only subprocess. The installer still bundles Forge core 0.13.2
and Codex CLI 0.157.1.

## Verified scope

- PASS: browser UI suite for the empty and populated project screens, folder
  validation, conversation and responsive behavior.
- PASS: hidden native Windows folder-dialog cancellation and selection, empty
  folder onboarding, and visible missing-folder feedback.
- PASS: a real Codex conversation on a disposable project used the bundled
  Forge core 0.13.2 for `start` and `workflow resume`, asked a relevant product
  question, and restored its answer after a WebView reload without resending.
- PASS: a separate bounded real Codex conversation restored in order after a
  full native process restart, without resending.
- PASS: Desktop Rust unit tests (48), focused check and strict Clippy.
- PASS: one NSIS candidate installed silently over 0.1.14. The installed
  0.1.15 executable and bundled core were checked, then the installed app
  passed the real Windows folder picker, a project-bound Codex/Forge discovery
  reply, WebView reload and full process restart with no resend. Executable
  overrides were unset for this test.

## Limits

In the activation probe, the agent attempted Forge's `decision_required`
request, but its shell tool rejected the composed command before execution.
That request is **read-only by design**; no durable decision write should occur
before the person chooses. The agent asked the person a question, but a typed
Forge Decision Request through this app remains **unverified**. Completing a
fresh ChatGPT login in a browser, the default-browser
button, and clean-machine availability of the Start Forge skill have not been
verified. The installer is unsigned and has no automatic updater. Manual
accessibility acceptance and mobile remain open. This is an alpha building
block, not a complete-product claim. Model-specific token use and BRL cost are
UNKNOWN.

## Candidate and publication

The single tested NSIS candidate is `Forge_0.1.15_x64-setup.exe`, 123,028,746
bytes, SHA-256
`4491A0B935B190F02A0990A2C276FA1A258FB18D2F892FA07C18C6B5FC8B47DB`.
The public prerelease and its asset were verified after publication. GitHub
reports the same size and SHA-256; an unauthenticated download matched the
candidate exactly. That downloaded installer reinstalled with exit 0, and
the installed executable and bundled core/Codex hashes matched the local
tested installation. A bounded hidden native regression on those downloaded
bytes again passed the real Windows folder picker, real Codex send/reply,
WebView reload and full process restart without resending. The longer
Forge-activation probe ran on the byte-identical candidate before publication;
the public-download regression used a no-tools prompt instead.

A further hidden native run on the installed public bytes completed the full
local-result journey: Codex created an HTML page in a temporary project, the
app opened it in its isolated preview, a follow-up in the same conversation
changed that file and refreshed the preview, and the ordered conversation
survived a full desktop process restart. The Windows folder and preview-file
pickers were both exercised. This proves the tested journey, not arbitrary
projects or clean-machine first use.

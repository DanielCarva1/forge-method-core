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

In the activation probe, the agent attempted to record a pending decision,
but its shell tool rejected that composed command before execution. Therefore
durable decision writing and full Forge governance continuity are **not**
claimed. Completing a fresh ChatGPT login in a browser, the default-browser
button, and clean-machine availability of the Start Forge skill have not been
verified. The installer is unsigned and has no automatic updater. Manual
accessibility acceptance and mobile remain open. This is an alpha building
block, not a complete-product claim. Model-specific token use and BRL cost are
UNKNOWN.

## Candidate and publication

The single tested NSIS candidate is `Forge_0.1.15_x64-setup.exe`, 123,028,746
bytes, SHA-256
`4491A0B935B190F02A0990A2C276FA1A258FB18D2F892FA07C18C6B5FC8B47DB`.
Public download verification is pending. A commit alone does not make this
version available to install.

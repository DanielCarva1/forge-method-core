# Forge Desktop 0.1.13 alpha.1

This Windows x64 alpha adds a first-use ChatGPT access path inside Forge. A
person who has not signed in can keep their draft, request a temporary code,
open the official authorization page, check or cancel the attempt, and then
choose when to send. Forge does not ask for a password and does not send the
draft automatically. Existing project, preview, conversation and restart flows
remain available. The installer continues to bundle Codex CLI 0.157.1 and
Forge core 0.13.2; it does not replace global copies of those tools.

## Verified scope

- PASS: Desktop check, all 47 Rust unit tests, strict Desktop Clippy and browser
  tests, including a simulated first-use sign-in with exactly one later send.
- PASS: hidden native WebView with an isolated signed-out Codex home and real
  bundled Codex executable displayed a device challenge, preserved the draft,
  made no send and canceled the attempt.
- PASS: a separate no-network app-server fixture completed the login protocol
  through native IPC; the UI read the account, closed the sign-in panel and
  enabled a later send without sending the draft itself.
- PASS: one NSIS candidate upgraded the installed 0.1.12 to 0.1.13 without
  replacing the pinned core/Codex resources. On the installed 0.1.13, hidden
  native tests with executable overrides unset repeated signed-out device-code
  start/cancel, controlled completion, and a real authenticated Codex reply
  restored in order after WebView reload and a full app restart.

## Limits

**Real completion of a new ChatGPT login has not been verified.** The native
test intentionally cancels; the completion test is a controlled fixture, not
OpenAI authorization. Opening the browser from the button and a genuinely
fresh Windows account or VM remain untested. Do not treat this as completed
clean-machine sign-in proof. If the access flow does not finish, the previous
Codex CLI sign-in remains a fallback; no conversation or draft is deleted.
Automatic updates, installer signing, manual accessibility acceptance and
mobile also remain open. This is an alpha building block, not a finished
general-release experience. Model-specific tokens and BRL cost are UNKNOWN.

## Candidate and publication

The tested NSIS candidate is `Forge_0.1.13_x64-setup.exe`, 123,011,405 bytes,
SHA-256 `76D758454DFFB2508029E4EF81AFC5AA6EBD82A31015AFD894EEB5E548FF0D33`.
Public readback and download verification remain pending until publication.
Do not treat a commit as a published installer.

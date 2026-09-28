# Forge Desktop 0.1.27 alpha.1

This Windows x64 alpha makes the first project and its local result easier to
understand. An empty Forge record now points to the conversation as the next
action without claiming that work has already been recorded. The empty result
area explains how to open a file cited by the agent or choose one from the
project folder. When a file is open, **Pedir mudança neste arquivo** appears
immediately below the result instead of below the technical preview notes;
it prepares a request in the same conversation and does not send it. The
file's origin and the fact that a local preview does not prove internet
publication are stated in plain language.

No Forge engine, Codex protocol, or project-state ownership changed. The
preview remains protected and noninteractive. Opening a page in the external
browser is a separate action with its own safety note.

## Verification

- The complete browser UI suite passed, including empty-state guidance,
  preview reading order, and change-request drafting without a Send.
- A source-built native Windows WebView on an isolated desktop passed project
  onboarding, authoritative empty-record readback, protected local preview,
  navigation, and frontend-to-Rust checks.
- A separate native read-only test resumed an existing 33-message real Codex
  conversation, opened its actual generated HTML result, found the change
  action in view, drafted the change request, fully restarted the app, and
  verified message count/order and unchanged result bytes. The installed
  0.1.27 app passed this check. It sent no new provider turn. This tests
  continuity and UI wiring, not a new model reply.

## Limits

- The **Usar no navegador** button launching a normal external browser has
  not been verified end-to-end in isolation; tests avoid opening a browser on
  the user's active desktop.
- Real fresh-account login, clean-machine installation, manual screen-reader
  acceptance, mobile access, and self-contained offline distribution remain
  unverified or unfinished.
- There is no automatic updater or code signature. Install the new NSIS
  package over 0.1.26. The installer uses a bundled Codex CLI and pinned
  Forge core; WebView2 may be downloaded by the installer if missing.
- This release does not establish per-task Pro subscription cost or a
  controlled Astra/Sol/Luna savings comparison.
- One hidden native run combining the real folder and file dialogs timed out
  after the Markdown preview step. Each dialog passed separately and the
  combined run passed on retry, including protected HTML/CSS/image rendering.
  The timeout's cause remains undetermined; it is not counted as a product
  failure or silently counted as a pass.

## Candidate

The single locally tested candidate is `Forge_0.1.27_x64-setup.exe`,
123,106,078 bytes, SHA-256
`F25D5B43D28D0FCD7E01B3D1ABE093E21D7AB49A61A59C3B6897AB894D83C724`.
Silent installation over public 0.1.26 returned exit code 0 without changing
candidate bytes. The installed executable reports 0.1.27 with SHA-256
`410C0208EADD831DD3622BE24628F8025C1FAC3BCEDE57F583CF5764805C418A`.
The installed app passed hidden native first-use and real-result/restart
checks on this machine. This is not a clean-machine test. The public installer
and checksum sidecar were downloaded without authentication. The downloaded
installer matched the tested candidate's exact byte count and SHA-256;
installing those downloaded bytes silently returned exit code 0, reported
version 0.1.27, and produced the same installed executable SHA-256. A
read-only real-result/restart check passed after the downloaded install.

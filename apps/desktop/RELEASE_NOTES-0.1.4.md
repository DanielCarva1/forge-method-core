# Forge Desktop 0.1.4 alpha — published

This Windows x64 alpha update addresses first-time project setup. It is not
a claim that every Forge core capability has a dedicated screen.

## Change from 0.1.3

- A cold `forge-core start` can take longer than the ordinary 15-second
  project-read limit while it prepares a new project's sidecar. The app now
  gives explicit `start` and `workflow init` bounded 85-second attempts,
  without slowing read-only project checks. The project screen explains that
  first-time preparation can take a while, instead of reporting a premature
  failure after authority has already begun to be created.

## Alpha limits

- `forge-core` and a compatible authenticated Codex CLI remain separate
  installations. The installer is unsigned and current-user only; there is
  no in-app automatic updater.
- The preview is local, read-only, scriptless and noninteractive; it is not
  proof of publication or production behavior.
- The Forge record can lag behind the chat. Full plain-language history of
  resolved decisions is not yet available because the current core report
  does not provide immutable user-facing question/choice wording.
- A real process crash during an in-flight Codex Send has not been validated
  for exactly-once behavior. The app does not automatically replay an
  unconfirmed Send.
- Full accessibility and visual acceptance across all screens remain open.

## Verification

The previous public 0.1.3 binary completed a real Codex artifact-to-change
journey after a cold-setup failure and retry. An independent new-project CLI
fixture measured `start` at 38.43 seconds, exceeding the 0.1.3 desktop
15-second attempt limit. The local 0.1.4 source then passed a first-action
cold-project native smoke in a hidden debug build. Focused and full desktop
Rust tests, Node tests and the controlled browser suite passed locally.

The final desktop source gate passed offline locked `cargo check`, four
focused project tests, all 44 desktop Rust tests, eight Node tests,
`cargo fmt --check`, strict desktop Clippy, the full controlled browser
suite and `git diff --check`. The release executable passed hidden native
testing with actual Windows folder and file dialogs, first-time Forge
onboarding and a real Codex page-creation, local-preview and follow-up
change journey in the same conversation. The resulting chat was restored
after WebView reload without resending.

One NSIS candidate was generated at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.4_x64-setup.exe`
(4,621,382 bytes; SHA-256
`06EA9E99C22D56FE39069BC73C0FF0C45023A64FF79F064A3A361BB68B82BBDE`).
The exact candidate was silently installed over the previously installed
0.1.3 with exit code 0. Installed 0.1.4 reports SHA-256
`A41160EE51F9F0E91841CDDABE8D05A8EDA79DC8FD878F4CEF4D99EDF8ED1E1C`.
The disposable Codex user/reply pair preserved through earlier upgrades
was restored again with no new Send. A separate first-use native smoke
against installed 0.1.4 passed on a never-used folder and used actual
Windows folder/file dialogs. That installed-binary smoke did not send a
new Codex turn; the real artifact journey ran on the release executable.

The one-time 0.1.2 WebView startup timeout from the earlier upgrade
fixture remains unexplained; it did not recur in this 0.1.4 installed
upgrade. Model-specific usage and BRL cost remain UNKNOWN.

## Publication readback

Release: https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.4-alpha.1

GitHub lists the one tested installer and its SHA-256 sidecar as uploaded
assets. A fresh unauthenticated download returned 4,621,382 bytes with
the exact candidate hash above. That downloaded file was silently
installed over local 0.1.4 with exit code 0; installed version and
executable hash still matched the values above. The disposable real
Codex conversation from the earlier installed version was restored yet
again with exactly one user/reply pair and no new Send. The public
download was not used to repeat a new real artifact-to-change journey
or a real in-flight crash.

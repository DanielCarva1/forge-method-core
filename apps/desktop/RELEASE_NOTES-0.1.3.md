# Forge Desktop 0.1.3 alpha — published

This is an alpha building-block update to the Windows x64 desktop app. It is
not a claim that every Forge core capability has a dedicated screen.

## Changes from 0.1.2

- A saved conversation now has a visible **Continuar conversa anterior** action
  in the conversation heading. When the last Send was not confirmed, the
  action instead says **Conferir envio anterior**. Both reopen the same
  project-scoped Codex conversation for review without sending or replaying.
- Completed Codex replies can open supported local files when the reply names
  a project-relative path in inline code, as well as in a Markdown link. The
  existing native project/path validation still owns access. URLs and code
  blocks are not converted into privileged file actions.
- At desktop window heights up to 760px, the conversation card grows with the
  page instead of clipping its invitation when a draft expands. Taller
  windows keep the bounded sticky chat. The resume action uses the same
  minimum 48px target as other buttons.
- A native development test now exercises a real Codex-created local HTML
  file, its safe local preview, a change request in the same conversation,
  actual file readback, and chat recovery after a WebView reload.

## Alpha limits

- `forge-core` and a compatible authenticated Codex CLI remain separate
  installations. The installer is unsigned and current-user only; there is
  no in-app automatic updater.
- The preview is local, read-only, scriptless, and noninteractive. It is not
  evidence of publication or of a dynamic site's production behavior.
- The Forge record can lag behind the chat. Accepted objective revisions,
  pending decisions and suggested questions are distinct; the full plain-
  language history of resolved decisions is **not** available in this UI.
  The current core report exposes references and audit digests rather than
  immutable user-facing decision text. No wording is reconstructed from IDs.
- A real process crash during an in-flight Codex Send has not been validated
  for exactly-once behavior. The unconfirmed-send guard prevents automatic
  replay but does not prove delivery.
- The real artifact-to-change exercise used a local debug build and a
  disposable project; the installed-binary checks below did not repeat that
  Codex turn. A full accessibility audit and complete visual acceptance are
  not yet claimed for 0.1.3.

## Verification gate

The exact local installer is
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.3_x64-setup.exe`
(4,623,640 bytes; SHA-256
`6F5CDE59A85393BBAB77394739E8D5FA4271E4C88856593683069BDD25464EAB`).
It was built once from this source package. `cargo check`, 10 focused
progress tests, all 43 desktop Rust tests, eight Node tests, formatting,
strict desktop Clippy and the complete controlled browser suite passed.
The release executable passed the hidden native smoke and full-process
controlled unconfirmed-send guard. The installed `0.1.2` created one real
Codex user/reply pair in a disposable project/profile. The exact 0.1.3
installer then silently upgraded that installation with exit code 0; the
installed product reports 0.1.3. The installed 0.1.3 restored exactly the
pre-upgrade pair without Send, and its hidden native smoke passed including
the actual Windows folder and file dialogs. The installed executable SHA-256
is `6F3193CF8B4F11BB29FE4A8355AB1BB1996E1DD14F1B08E96D05222C739A9E77`.

The first pre-upgrade fixture startup failed once waiting for the initial
WebView Home screen; retrying the same installed 0.1.2 with the same
disposable profile passed. No Codex turn was sent by the failed attempt.
The cause of that transient startup remains unknown. The actual Codex
artifact-to-change test passed earlier on the local debug build, not on
the installed 0.1.3 binary. Model-specific usage and BRL cost remain UNKNOWN.

## Publication readback

Release: https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.3-alpha.1

The release contains the one tested installer and its matching SHA-256
sidecar. A fresh public download returned 4,623,640 bytes and the exact
candidate hash above. That downloaded file was installed silently over the
local 0.1.3 installation with exit code 0. The installed version and
executable hash still matched the values above, and the disposable real
Codex conversation from installed 0.1.2 was again restored with exactly
one user/reply pair and no new Send. The installed public download was not
used to repeat the artifact-to-change journey or a real in-flight crash.

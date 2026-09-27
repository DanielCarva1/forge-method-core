# Forge Desktop 0.1.9 alpha.1

This Windows x64 alpha makes a local file cited in a resumed Codex answer
easier to inspect and change after closing and reopening Forge. Project state
still comes from Forge, conversation history from Codex, and file contents
from the existing native, project-bound preview command.

## Change from 0.1.8

- If the latest completed Codex reply cites exactly one distinct supported
  local file, the empty preview offers **Conferir arquivo citado**. It appears
  again after restoring the conversation, without storing another transcript
  or file bookmark. Clicking it checks the actual file in the current project.
- The preview then offers **Pedir mudança neste arquivo** in the same
  conversation. This prepares an editable message; it does not send a turn.
- If the reply is incomplete, a later user message is still unanswered, or
  several different files are cited, the UI does not guess which file is the
  result. The person can still use a file action in the reply or choose a
  file. A citation is not proof that the file exists or was published.

## Verification and limits

The controlled browser suite failed on the missing restored-file action
before implementation and passed afterward. Eight Node tests, JS syntax and
the focused hidden native WebView test passed. All 44 Desktop Rust tests,
desktop-only offline locked check, formatting and strict Clippy passed. The
native test used a
controlled Codex-history response, a real project file, a full process
restart, a prepared change request without Send, and native rejection of an
outside-project path. A separate hidden native journey with authenticated
Codex created and changed actual local HTML in the same conversation and
restored that conversation after a full app-process restart. The new shortcut
was not guaranteed to appear in that real reply: it intentionally requires
one distinct supported file citation. The controlled test proves that exact
condition; it does not prove every model response uses that format.

The app still depends on separate compatible `forge-core` and authenticated
Codex CLI installations. The installer is unsigned and current-user only;
there is no in-app auto-updater. Local preview is read-only, with no claim of
publication or full interactive-site behavior. Complete manual visual,
contrast and screen-reader acceptance remains unfinished. Model-specific
tokens and BRL cost are UNKNOWN. GitHub CI was not manually started.

One NSIS candidate was built at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.9_x64-setup.exe`
(4,621,160 bytes; SHA-256
`E16351D7DAE468968FDF43D0F3811C835DEBB428C407A2D8ED2D5185ADE1DDA9`).
Its `.sha256` sidecar sits beside it. This exact candidate installed silently
over 0.1.8 with exit 0. Installed version is 0.1.9; installed executable
SHA-256 is
`6EA8B9B72E46A3806F93AA316FABC367BFE04095B0A1DCE6E81C7AE960A25DBD`.
The installed binary passed the focused hidden native shortcut/restart test
and restored an older disposable real Codex user/reply pair without sending
a new turn. The candidate was not rebuilt after hashing.

## Publication readback — 2026-09-27

The prerelease is available at
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.9-alpha.1`
from annotated tag `desktop-v0.1.9-alpha.1`, which resolves to source commit
`899d054281ecc247cba2740caf00cff7c0bba68c`. GitHub reported the
installer asset as 4,621,160 bytes with the expected SHA-256. A fresh,
unauthenticated public download to
`C:/ForgeFast/forge-alpha-019-public-20260927` matched the candidate's size,
hash and checksum sidecar. That downloaded installer, not a rebuilt file,
silently reinstalled with exit 0; installed version and executable hash
matched the tested installation. The installed downloaded bytes passed the
hidden native full-process resume, local-file preview, prepared change and
outside-path rejection check. The earlier real Codex journey was run on the
same UI source before the version metadata bump; it was not repeated on the
downloaded installer. No GitHub CI was manually triggered.

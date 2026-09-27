# Forge Desktop 0.1.11 alpha.1

This Windows x64 alpha removes the need to provide a separately installed,
compatible Codex CLI for the ordinary installed-app conversation. The installer
includes the pinned Codex CLI 0.157.1 runtime and uses it before older machine
installations. A deliberate `FORGE_CODEX_EXE` developer override still wins.

The app does not change an existing global Codex installation, download a CLI
when it starts, or send credentials through its webview. Sign-in remains through
the Codex CLI's supported ChatGPT account flow. Forge core is still a separate
prerequisite; this is **not** a clean-machine, self-contained setup claim.

## Verification and limitations

- PASS: pinned npm install, focused Desktop check/agent tests, all 45 Desktop
  Rust tests, strict Desktop Clippy, eight Node tests, browser suite and NSIS
  release build. No separate core workspace or manual GitHub CI run.
- PASS: NSIS archive contains Codex's native binary and its resources, license
  and notice. The exact candidate silently upgraded the local 0.1.10 app;
  installed Codex reports `codex-cli 0.157.1`.
- PASS: installed app, with `FORGE_CODEX_EXE` unset, sent a real Codex message,
  received a response and restored it after WebView reload. A separate hidden
  native full-process restart used controlled history, validated local files
  and rejected an outside-project path.
- NOT_RUN: a fresh real artifact create/change journey and full-process restart
  of that same real Codex conversation on the installed 0.1.11 binary; those
  flows were covered on 0.1.10 source or by controlled native history.
- Still missing: bundled `forge-core`, in-app sign-in, automatic updating,
  signing, manual screen-reader/contrast acceptance, mobile and clean-machine
  install. The pinned Codex CLI may need a future app update when the provider
  changes compatibility. Model-specific tokens and BRL cost remain UNKNOWN.

## Candidate

`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.11_x64-setup.exe`
is 111,295,705 bytes, SHA-256
`00E4E5F1C749C395A5844A50A926B88F6D62833934911F998B19D96CF6F1E6E4`.
The exact candidate was installed with exit 0. Installed executable SHA-256 is
`FCF8AFB036789F070891541BBA3A66CC7CC194D9DC685CAE19C0BF1C14BD3A04`;
installed Codex executable SHA-256 is
`8CB0E69E99FF2A158C54815DB82D0F2E524D8F301BC30184722CFD1AE5973574`.
Do not rebuild or replace this candidate after hashing. At candidate time it
was not public; publication readback is recorded below.

## Publication readback — 2026-09-27

The prerelease is available at
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.11-alpha.1`.
Annotated tag `desktop-v0.1.11-alpha.1` resolves to source commit
`7c5b83d5d5e317d9d9b812cf17bb4915e7decb8e`. A fresh unauthenticated
download of the installer and sidecar matched the candidate's size and SHA-256.
The downloaded file reinstalled with exit 0; installed version and app/Codex
hashes matched the earlier candidate installation. The installed downloaded
bytes again passed a hidden native real Codex send/response/reload without an
override. GitHub CI was not manually started.

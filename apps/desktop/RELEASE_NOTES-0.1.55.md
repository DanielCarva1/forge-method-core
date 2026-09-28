# Forge Desktop 0.1.55 alpha.1

This update improves the path from an idea to a real project and makes manual
updates easier to find.

- After choosing a folder for an Explore idea, the conversation button changes
  to **Preparar projeto**. It validates and prepares that folder instead of
  reopening the picker. The draft stays in place; only a later, explicit
  **Enviar** sends it to Codex. A failed preparation keeps the draft and
  allows retry.
- **Versão e atualizações** explains which Desktop installer to choose. The
  update button opens a Desktop-filtered GitHub release list rather than the
  mixed repository list. A model-version error offers a direct route to these
  instructions and no longer tells ordinary users to update a separate CLI.

## Verification and limits

Focused headless-browser checks covered the folder choice/preparation/Send
boundary, a failed preparation, version guidance, narrow enlarged text,
browser-launch request and fallback. A rebuilt hidden Windows app prepared
one disposable project from an Explore draft using the native Forge backend,
verified that no Codex Send occurred, and opened the in-app update guidance.
Desktop `cargo check -p forge-desktop --locked --offline -j2` passed. The
headless browser-launch request uses a simulated native bridge; actually
opening the external browser was NOT_RUN to avoid interrupting the user's
desktop. No broad suite or manual GitHub CI was run.

This remains an unsigned Windows x64 alpha with manual installer updates.
The installer bundles pinned Forge core 0.13.2, not source-tree core 0.13.3.
Clean-machine installation, fresh ChatGPT login completion, actual external
browser launch and manual screen-reader acceptance remain NOT_RUN. The
occasional Forge record first-read error has not been isolated. Model-specific
token use and BRL-equivalent cost remain UNKNOWN.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.55_x64-setup.exe`, 123,128,502
bytes, SHA-256
`BE0FC873A0286820B62299909E4D99D79926F97155BDEA499BD77FF67370C402`.
Its 93-byte `.sha256` sidecar records the same hash. Silent installation over
0.1.54 exited 0 without changing the candidate. The installed executable
reports ProductVersion 0.1.55, SHA-256
`9B86D0921FD5DACD892851790557FD9D4CE0C2A9C8320B6FCC8390FF219A91AE`.
The installed candidate passed the focused hidden-native project preparation
and update-help check. Public upload, anonymous download and download-install
readback are still pending; this candidate is not yet a public release.

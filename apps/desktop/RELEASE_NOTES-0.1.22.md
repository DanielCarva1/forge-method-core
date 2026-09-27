# Forge Desktop 0.1.22 alpha.1

This Windows x64 alpha makes a local HTML result easier to understand and use.
The protected in-app preview remains visual and noninteractive. When it shows
an HTML page, **Usar no navegador** explicitly asks Windows to open that
project file with the default HTML handler, so the person can try its
interactive behavior outside the Forge preview. The app warns that an ordinary
browser can run the page's code and access the network. Nothing opens
automatically.

The project record also distinguishes a completed work item from the general
project stage: completed activity is labeled “Resultado registrado”, while a
project may still show its broader “Descoberta” stage.

## Verification

- A real Codex conversation created a functional personal ideas page and
  completed its Forge Work Focus. An independent Chromium local-file check
  covered adding, listing, persistence after reload, cancellation of removal,
  and removal. This project is a disposable fixture, not part of the installer.
- Browser UI tests cover the HTML-only action, no automatic external opening,
  failed-open feedback, accessibility text sizing, and the completed-work
  wording. Rust tests cover rejection of outside-project, non-HTML, and
  relative paths. A hidden native debug build re-opened the same real chat,
  showed the action, and rejected an unavailable file without launching it.
- PASS: 11 frontend unit tests, the browser UI suite, Desktop `cargo check`,
  all 50 Desktop crate tests, strict Clippy, `git diff --check`, and one NSIS
  release build. The exact candidate below installed silently over 0.1.21
  with exit 0; installed 0.1.22 passed hidden native smoke and read-only
  real-chat readback of the completed Forge record, actionable local result,
  HTML-only browser action and invalid-file rejection with zero new sends.

## Limits

- A **successful** Windows default-browser launch has not been exercised in
  this automated run: it could create a visible tab on the maintainer's active
  desktop even from a hidden test. The tested native boundary is validation and
  rejection, not positive external-launch readback. Browser behavior may vary
  with the user's default `.html` association.
- The in-app preview still intentionally does not execute JavaScript. Only
  open HTML from projects you trust. Local files are not published by this
  action.
- The installer is unsigned and has no automatic updater. Clean-machine
  setup, fresh-account login, manual accessibility acceptance, mobile, and a
  controlled model-cost comparison remain unverified. Token use cannot be
  converted into a reliable per-task Pro cost from available counters.

## Candidate

The single tested candidate is `Forge_0.1.22_x64-setup.exe`, 123,049,545
bytes, SHA-256
`3C619DBCE8451760C091BF34B754FFE3B7D0E467786348789F3FA91C402669C1`.
The installed executable SHA-256 is
`849B3E2ED14280C287321F169EA5937B157C37D7BAFB3FF2FE6DB729B2DD0CD7`.
The bundled Forge core remains pinned at 0.13.2; its installed SHA-256 is
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`.
A source commit alone is not a published installer; availability requires a
public release and exact-byte download check.

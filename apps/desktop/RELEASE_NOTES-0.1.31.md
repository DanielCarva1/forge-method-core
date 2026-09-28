# Forge Desktop 0.1.31 alpha.1

This Windows x64 alpha makes the first conversation easier to start. After
choosing an idea in Explore or writing one directly, **Escolher pasta para
continuar** opens the existing folder picker. The draft remains in place;
choosing a folder alone neither prepares the project nor sends a message.
After **Continuar nesta pasta** confirms the Forge project, the same action
becomes **Enviar**. The composer now keeps writing and Send together, closer
to the approved conversation design, with the agent-capability notice visible.

The package also fixes a stale local preview: when a Codex turn finishes
while the enlarged preview is open and the person navigates away, the
selected project file is re-read when the preview closes. It does not guess
which file changed or carry a refresh into another project.

## Verification

- The complete browser UI suite passed, including the folder-choice handoff,
  preserved draft, no project start or Send on folder selection, the composer
  layout at desktop widths, narrow reflow, and the preview route regression.
- The source-built Windows app passed hidden native smoke using real Forge
  onboarding and project-file readback. Controlled Codex terminal events
  exercised the preview refresh. No real provider message was sent in that
  smoke.
- Desktop `cargo check`, six focused preview tests (one optional association
  test ignored), all 50 non-ignored crate tests, strict Clippy, eight frontend
  unit tests, JavaScript syntax checks, the browser UI suite, and
  `git diff --check` passed. The separate core workspace was not rerun for
  these Desktop-only changes.
- The exact candidate was silently installed over public 0.1.30. The
  installed app passed hidden native smoke, including a real Windows folder
  picker opened from the drafted idea and canceled without losing that draft,
  real Forge onboarding, and read-only preview checks.

## Limits

- This is an alpha building block, not the finished Forge UI or proof that
  every backend capability has an approachable screen. The full #91 result
  review and #92 decision-history requirements remain incomplete.
- A real Codex turn in the exact new folder-choice and preview-timing flows
  was not induced. Existing real-conversation evidence is separate.
- This release does not add an auto-updater. Install the NSIS package over
  the previous alpha. The installer is unsigned and includes pinned Forge
  core 0.13.2 and Codex CLI; WebView2 may download during setup if absent.
- Fresh-account login completion, clean-machine setup, manual screen-reader
  acceptance, and mobile/offline self-contained distribution remain unverified
  or future work. No per-task Pro-quota or BRL savings can be attributed from
  the available counters.

## Tested candidate

The single NSIS candidate is `Forge_0.1.31_x64-setup.exe`, 123,079,445 bytes,
SHA-256 `A167B06A826B52D9FDCD544D5CA07D41F7356F81ADC2442CF868B2D171EED887`.
Silent installation of these exact bytes over public 0.1.30 returned exit
code 0 and left the candidate hash unchanged. The installed 0.1.31
`forge-desktop.exe` has SHA-256
`49713D34D8EF62382B7A68AEEEF617DEF6160D36FEF18D82CCA3A288218F92CB`.
Public download continuity will be checked after publication; local
installation alone does not prove that the release asset is available.

# Forge Desktop 0.1.63 alpha.1

**A more comfortable conversation view.** In a confirmed project, open
**Opções → Ampliar conversa** to give the chat the full workspace width.
**Mostrar resultado e projeto** returns to the side panels; opening a cited file
also returns to them before the file is checked. The conversation and unsent
draft stay in place. Codex replies now use a wider but bounded reading column
instead of leaving most of the expanded view empty.

## Verification and limits

Focused browser checks cover expanding and leaving the conversation, draft
preservation, cited-file return, readable reply width and no horizontal
overflow at enlarged text. A focused hidden native Windows check covers the
same reading layout with a restored test conversation, plus real project-bound
file checks, app restart and an unsent draft. No full suite, manual GitHub CI
or new real Codex message was run for this UI package.

This remains an unsigned Windows x64 alpha. Updates are manual NSIS installs
over the previous version. It bundles pinned Forge core 0.13.2. Clean-machine
setup, first-time ChatGPT login completion, external-browser launch and manual
screen-reader acceptance remain unverified. A local preview does not prove
internet publication. Model-specific tokens and BRL-equivalent cost remain
unknown.

## Installer candidate

The single Windows x64 NSIS candidate is `Forge_0.1.63_x64-setup.exe`
(123,095,797 bytes, SHA-256
`D0092DE52038BDC7BD55BEFC72FD37334447579539ABDB446D9F87D112D2F687`).
Its 93-byte SHA-256 sidecar matches. Installation over 0.1.62 and public
download verification remain pending; the installed 0.1.62 app was open during
packaging and was not interrupted.

# Forge Desktop 0.1.57 alpha.1

This update makes the first project step clearer after choosing a theme in
Explore. The workspace confirms that the idea is an unsent draft and places
the next action near its heading. From there, the person can choose a folder
and prepare it without sending the draft or reopening the picker. If project
preparation fails, the error appears beside that action and the draft remains
available for a retry. The action disappears when the project is ready.

## Verification and limits

A focused headless-browser check covered the 360px layout, folder selection,
failed preparation and retry, draft preservation, and one explicit Send. A
hidden native Windows build prepared a disposable real Forge project through
this action, reopened it from Home through native inspection, and made no Codex
Send. The additional top-of-page failure message was checked in the focused
browser test, not in the native build. No broad suite or manual GitHub CI ran.

This remains an unsigned Windows x64 alpha updated by installing the new NSIS
package over the previous version. It bundles pinned Forge core 0.13.2, not
source-tree core 0.13.3. Clean-machine setup, first-time ChatGPT login
completion, external browser launch, physical mobile use, and manual screen
reader acceptance remain NOT_RUN. The established real-Codex chat/result,
change-request and restart checks from earlier alpha packages were not rerun
for this UI onboarding update. Model-specific tokens and BRL-equivalent cost
remain UNKNOWN.

## Candidate

One NSIS candidate was built: `Forge_0.1.57_x64-setup.exe`, 123,050,785
bytes, SHA-256
`F8C85C18EBF0EED265C826B8E438B1B500E9E0C505CF1B887B9385C4DF85B43E`.
Its 93-byte `.sha256` sidecar records the same hash. Silent installation over
0.1.56 exited 0 without changing the candidate. The installed ProductVersion
is 0.1.57; its executable SHA-256 is
`0CC10EBB0F8C81DD7863AB9894C412C5339B0C7379D115265C615DB5336BABE3`.
The installed candidate passed the focused hidden-native real Forge project
preparation and Home reopen check without a Codex Send. This is same-machine
installation evidence, not clean-machine setup.

# Forge Desktop 0.1.59 alpha.1

The first-use workspace now starts with your idea. On a narrow window, the
conversation appears above folder setup; on a wide window, it appears to the
left of the folder sidebar. The writing field is visible without a long
scroll, and the oversized empty illustration no longer pushes it away.
Numbered steps were removed because you can write an idea or choose a folder
first. If you try to send before choosing a project, Forge asks for a folder
and keeps your text unsent. Once the project is prepared, the same draft is
ready for an explicit Send.

## Verification and limits

A focused browser check covered 360px and 1180px layout, no horizontal
overflow, draft preservation during folder choice, failed project preparation
and retry, and one explicit Send. Visual screenshots were inspected. A hidden
native Windows build checked the narrow layout, prepared a disposable
real Forge project from Explore, reopened it from Home through native
inspection, and made no Codex Send. The wide layout was browser-checked, not
native-checked. A native folder-dialog
choice initiated by the generic idea-first Send remains NOT_RUN in this
package; earlier dialog evidence belongs to separate packages.

This remains an unsigned Windows x64 alpha updated manually by installing
the NSIS package over the previous version. It bundles pinned Forge core
0.13.2, not source-tree core 0.13.3. Clean-machine setup, first-time ChatGPT
login completion, external browser launch, physical mobile use, and manual
screen reader acceptance remain NOT_RUN. Earlier real-Codex chat/result,
change-request and restart evidence was not rerun for this UI-only update.
Model-specific tokens and BRL-equivalent cost remain UNKNOWN. No broad suite
or manual GitHub CI ran.

## Candidate

One NSIS candidate was built: `Forge_0.1.59_x64-setup.exe`, 123,091,571
bytes, SHA-256
`36EE2AEA005AF68EFA3488149A01DE0651270AA3674D5160D5AA52EA1C71A661`.
Its 93-byte `.sha256` sidecar records the same hash. Silent installation over
0.1.58 exited 0 and left candidate bytes unchanged. The installed
ProductVersion is 0.1.59; the installed executable SHA-256 is
`D5AA8AB443B1262677AB988101F9E608B2884649362A191CC59E1A7823C131E7`.
The installed candidate passed the focused hidden-native 360px first-use,
real project preparation and Home reopen check without a Codex Send. This
is same-machine upgrade evidence, not clean-machine installation proof.

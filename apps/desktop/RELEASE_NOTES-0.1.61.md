# Forge Desktop 0.1.61 alpha.1

**Finding a real result is easier.** The **Choose file** dialog now starts in
the opened project's folder. Forge still validates the chosen file against the
project before showing it; the folder is a navigation hint, not a bypass.
If a deliverable cannot be displayed in the protected preview, the result
card now offers **Copy file path** so it can be found in Windows Explorer or
another application. Copying requires an explicit click and does not open or
execute the file. A missing project folder or unavailable clipboard gets a
clear message without discarding the previous valid result.

## Verification and limits

Focused Desktop `cargo check` and `preview::tests` passed (six tests, one
browser-association check ignored by design). A focused browser check covered
the project-root argument, missing-folder feedback and clipboard success and
failure. The hidden native Windows app selected `result.txt` by filename in
the real file dialog after opening a disposable project, confirming that the
dialog started in that project folder. Native project-bound reading also
showed a nonvisual ZIP-like file and its copy action. The same focused native
run covered rejected outside paths, app restart and an unsent change request
with controlled Codex history. The user's global clipboard was not changed
by the native test. No new real Codex turn, broad suite or manual GitHub CI
ran for this result-access update.

This remains an unsigned Windows x64 alpha. Updates are manual NSIS installs
over the previous version. It bundles pinned Forge core 0.13.2. Clean-machine
setup, first-time ChatGPT login completion, external-browser launch, physical
mobile use and manual screen-reader acceptance remain unverified. A safe
local preview is not proof that a site or file was published. Model-specific
tokens and BRL-equivalent cost remain unknown.

## Tested installer

The single Windows x64 NSIS candidate is `Forge_0.1.61_x64-setup.exe`
(123,165,896 bytes, SHA-256
`D0C3CBC16716BBCF8B7690891BF42002E5CD866C297C3266908EB5154070F3D2`).
Its 93-byte SHA-256 sidecar matches. Silent installation over 0.1.60 exited
successfully without changing the candidate. The installed app reports
ProductVersion 0.1.61 and passed the focused hidden-native result-access test.
Anonymous download readback remains pending until publication.

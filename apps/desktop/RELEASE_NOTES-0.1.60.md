# Forge Desktop 0.1.60 alpha.1

This update keeps your idea at the front while you choose or correct a project
folder. If Forge cannot prepare the selected folder, the idea and its reading
order remain intact. When a new result file cannot be opened, the previous
validated preview stays visible instead of reverting to an empty layout.

The first-use journey was checked in a hidden native Windows app using the
actual folder picker: select a disposable folder, prepare its Forge project,
and reopen it from Home without sending a Codex message. Focused browser
checks also covered the failed-folder and rejected-result states. This release
changes the UI only; it does not replace the backend or alter project files on
its own.

## Limits

This remains an unsigned Windows x64 alpha. Updating is manual: install the
NSIS package over the previous version. It bundles pinned Forge core 0.13.2.
Clean-machine setup, first-time ChatGPT login completion, external-browser
launch, physical mobile use, and manual screen-reader acceptance remain
unverified. Existing real Codex conversation and file-changing-turn tests
were not repeated for these UI-only changes. A focused hidden-native test did
reopen the app and prepare a change request, without sending it. No broad
suite or manual GitHub CI ran. Model-specific tokens and BRL-equivalent cost
remain unknown.

## Candidate

One NSIS candidate was built: `Forge_0.1.60_x64-setup.exe`, 123,044,511
bytes, SHA-256
`62632B5B475C63C236F119B836E7E7A8D088D02298587DE335FAED1A3AD46103`.
Its 93-byte `.sha256` sidecar records the same hash. Silent installation over
0.1.59 exited 0 and left candidate bytes unchanged. The installed executable
reports ProductVersion 0.1.60 and SHA-256
`D745E89BA2B7772655932F8C9164DB819867ECD67BE83CE44D882BB51BA07D51`.
That installed candidate passed two focused hidden-native checks: actual
Windows folder selection, native Forge project preparation and Home reopen at
360px with zero Sends; and real project-bound result preview, rejected outside
file preserving the prior result/layout, full app restart, and a prepared but
unsent change request. The Codex history in the latter check was a controlled
fixture, not a real new Codex turn. Anonymous release download readback is
pending publication.

# Forge Desktop 0.1.47 alpha.1

Switching projects no longer carries an unsent message from the previous
project into the next one. A draft stays available if you return to its
original project during the same app session. Starting a fresh idea without
a suggested text begins with an empty composer; choosing a suggested idea
still places that suggestion in the composer without sending it.

The project/conversation/result flow remains local to the chosen folder.
There is no new project registry or backend protocol. Unsent drafts are kept
only in memory for this app session; closing the app does not save them.

## Verification and limits

The browser UI suite reproduced the cross-project draft leak before the fix
and passed after it, including canceled switching, returning to the original
project, clearing a draft, and a fresh Explore idea. Hidden native Windows
testing confirmed that choosing another project clears its predecessor's
folder, conversation and draft, then restores that draft when the original
project is reopened. The installed 0.1.47 candidate also completed a separate
new-idea journey with one real Codex reply in a newly selected project. A
combined smoke run passed that journey but then timed out because the next
test tried to edit a folder while Codex was still connected; the two journeys
passed when run separately. The signed-out first-use native smoke also passed
without sending early or losing its draft. Real login completion was NOT_RUN.

This remains an unsigned Windows x64 alpha without an auto-updater. The
bundled Forge core remains pinned to 0.13.2. Clean-machine setup,
external-browser launch, real provider login completion, physical mobile
access and manual screen-reader acceptance remain NOT_RUN. Per-model tokens,
BRL-equivalent cost and Pro quota attribution remain UNKNOWN; no savings are
claimed. No manual GitHub CI was triggered for these changes.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.47_x64-setup.exe`, 123,092,052
bytes, SHA-256
`E397A974F81419C409D17F58B75A06323487BD75A90423FF2A2DE3F7704E6DF8`.
Its adjacent 93-byte `.sha256` sidecar records the same hash. Silent
installation over public 0.1.46 returned exit 0 without changing the
candidate bytes. The installed executable reports ProductVersion 0.1.47,
SHA-256
`660DE983355AD29DDC72467E59B7053C6317A0D43A254810077C2C215694DF34`.
The public download verification will be recorded after publication. A
source commit alone does not update the installed app.

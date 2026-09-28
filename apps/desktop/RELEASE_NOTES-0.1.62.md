# Forge Desktop 0.1.62 alpha.1

**More room to read and continue the conversation.** Secondary conversation
controls now live under **Options** instead of crowding the chat header. A
normally resumed conversation uses a shorter notice; interruption and
unconfirmed-send warnings keep their explicit recovery instructions. The
Codex capability warning remains visible beside the composer, with its full
explanation available on expansion.

When a selected project file cannot be rendered in Forge, the desktop layout
now gives the conversation more space instead of reserving the widest column
for a nonvisual result. Renderable results keep their wider preview. The
validated file path, copy action and change-request entry remain available;
the mobile single-column layout is unchanged.

## Verification and limits

Focused browser checks covered empty/reopened conversation controls, keyboard
access, enlarged narrow text, the capability warning, nonvisual-result column
balance and previous-preview preservation. A focused hidden native Windows
check covered a real project-bound file picker/read, rejected outside path,
app restart, restored citations, nonvisual layout and an unsent change draft.
The app was built from current source and inspected on an isolated desktop.
No full suite, manual GitHub CI or new real Codex message was run for this UI
package. The larger browser/native scripts were syntax-checked but not run
end to end.

This remains an unsigned Windows x64 alpha. Updates are manual NSIS installs
over the previous version. It bundles pinned Forge core 0.13.2. Clean-machine
setup, first-time ChatGPT login completion, external-browser launch, physical
mobile use and manual screen-reader acceptance remain unverified. A safe
local preview is not proof that a site or file was published. Model-specific
tokens and BRL-equivalent cost remain unknown.

## Tested installer

The single Windows x64 NSIS candidate is `Forge_0.1.62_x64-setup.exe`
(123,146,772 bytes, SHA-256
`F8693C2A4E05518530F95F26B93920D27BED81B362E5B31E83CED1B4CBE5ACCC`).
Its 93-byte SHA-256 sidecar matches. Silent installation over 0.1.61 exited
successfully without changing the candidate. The installed app reports
ProductVersion 0.1.62 (executable SHA-256
`CC384B7FD2C16CC546DF05C7975AE16569574380ED1F9479F5F0F5E524829FBE`)
and passed the focused hidden-native result and conversation check. The
published installer was downloaded without authentication and matched the
candidate's size and SHA-256; its sidecar matched byte-for-byte. The
downloaded copy was hash-checked but not installed a second time.

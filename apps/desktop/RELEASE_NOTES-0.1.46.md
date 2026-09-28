# Forge Desktop 0.1.46 alpha.1

The first-use **Copiar código** confirmation now appears directly under the
temporary login code, where it stays visible instead of falling below the
other controls. If copying fails, the same spot explains how to select the
code manually. Starting a new challenge or canceling clears that message;
the user's draft remains untouched.

The **Como funciona** section now shows the version reported by the installed
app and offers **Ver versões disponíveis**. That action asks Windows to open
the project's fixed public releases page in the default browser; it does not
check, download, or install an update automatically. If the browser cannot be
opened, the page displays an address the user can copy.

## Verification and limits

The browser UI suite covered successful and failed copy feedback, unavailable
native bridge, successful and failed update-page requests, the real-version
display, and narrow/enlarged layout. The hidden native debug WebView showed
the copy confirmation beside the code and the real installed version in the
help section; native auth, no-early-Send, and general shell checks passed.
The native copy test replaced the clipboard method locally, so it did not
alter the user's system clipboard. Opening the external browser and completing
a real provider login were NOT_RUN to avoid taking over the user's desktop.

This is an unsigned Windows x64 alpha without an auto-updater. The bundled
Forge core remains pinned to 0.13.2. Clean-machine setup, physical mobile
access, and manual screen-reader acceptance remain unverified. Per-model
tokens, BRL-equivalent cost, and Pro quota attribution remain UNKNOWN; no
savings are claimed. No manual GitHub CI was triggered for these changes.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.46_x64-setup.exe`, 123,117,912
bytes, SHA-256
`98CFE23151DD7D93D1A3871612380EC7A4D20AE4B40AEBB3F123BAB102DDF98D`.
Silent installation over public 0.1.45 returned exit 0 without changing the
candidate bytes. The installed executable reports ProductVersion 0.1.46,
SHA-256
`DA5C1E8960BE7E6F9E2070BF048F45AEEB8001A97728B1DAA103206A4C876FF7`.
Hidden native general shell, signed-out first-use, and real-result/restart
smokes passed on the installed candidate. Public-download readback remains
to be recorded after publication; source commit alone does not update the app.

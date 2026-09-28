# Forge Desktop 0.1.42 alpha.1

This Windows alpha makes long conversations easier to navigate. When you scroll
up to read older messages, **Ir para a mensagem mais recente** returns to the
end of the same conversation. The control stays hidden when you are already at
the end or when there are no messages. It does not send, reconnect, or alter a
project. Keyboard focus returns to the conversation reading region after use.

## Verification and limits

The complete browser UI suite passed, including 160 restored messages, a new
reply while reading earlier messages, keyboard traversal, project switching,
and a short 390 × 420 layout probe. The layout probe simulates reduced viewport
height; it is not proof of a physical on-screen keyboard. Hidden native Windows
smoke passed with real Forge project readback and a controlled long-history
fixture. A separate hidden native test reopened an existing 39-message Codex
conversation and used the new control before and after a full app restart;
message count and order were preserved, and no message was sent or project file
changed. This verifies that bounded path, not every possible conversation.

The installer is unsigned, Windows x64 only, and has no auto-updater. The
bundled Forge core remains pinned to 0.13.2. Protected static-page previews
do not execute JavaScript. Clean-machine setup, physical mobile-device use,
manual screen-reader acceptance, per-model token counts, BRL-equivalent cost,
and Pro-quota attribution remain unverified or unavailable. No manual GitHub
CI was triggered for this package.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.42_x64-setup.exe`, 123,059,821 bytes,
SHA-256 `5F8A94E180837CFA3FDD219F772ACE79C225661E9880E83E2690878A5106EAB0`.
Silent installation over public 0.1.41 returned exit 0 without changing the
candidate hash. The installed executable reports Windows ProductVersion
0.1.42 and SHA-256
`08A50AF63DC1BFC6ECD116A465ED30E85BB7BE2BB7DD7455C3C4CA888B55263B`.
Hidden native installed-candidate smoke and the read-only 39-message Codex
history journey passed. Public download verification remains pending until
publication; a commit or tag alone does not make this version available.

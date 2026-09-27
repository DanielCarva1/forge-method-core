# Forge Desktop 0.1.20 alpha.1

This Windows x64 alpha improves continuity between the conversation, a local
result, and the Forge project record. After a later planning-only reply, the
app still offers the last file cited in that conversation, so a person can
reopen the result before requesting a change. It does not guess a result from
chat text: the clicked file is still checked against the selected project by
the native preview command.

When Forge has an accepted product direction but no current Work Focus, the
record now says that the direction is registered and the next work is pending.
It no longer describes that state as having no Forge steps. A reopened chat
also uses a plain resume message without implying that a Forge record exists.

## Verification

- Browser UI checks cover the absent/current record states, a previous file
  after a planning reply, and restoration of a 202-message conversation.
- On a real installed 0.1.19 conversation, the accepted direction and Work
  Focus survived app restart; a source debug build reopened an earlier HTML
  result after a later planning reply without sending another message.
- PASS: eight frontend unit tests, 49 Desktop crate tests, strict Clippy and
  one NSIS release build. The browser UI suite passed.
- PASS: the candidate installed silently over public 0.1.19. The installed
  0.1.20 app resumed the same real Codex conversation and read the exact Forge
  direction and Work Focus without sending a message or changing project
  files. It reopened the earlier local HTML result after the later planning
  reply. Hidden native WebView smoke passed; its first-use view was a
  controlled fixture, not a fresh provider turn.

## Limits

This alpha is not the complete Forge experience. The installer is unsigned,
has no automatic updater, and has not been tested on a clean machine or with a
fresh ChatGPT account. The older real conversation began with legacy agent
instructions, so its continuity does not prove that every future agent follows
the current bundled-skill handoff. No provider turn is needed to test these UI
changes; task-specific Pro allowance and BRL cost remain UNKNOWN.

## Candidate

The single locally tested candidate is `Forge_0.1.20_x64-setup.exe`,
123,075,607 bytes, SHA-256
`282125B2A99E339233A3678602829FC5D6311E2D4A1F62F636C2FCD8FAF9AFE7`.
The installed executable SHA-256 is
`11651B7F81A6BF96FE2948FAF689B5CD918CC37B8439408C659B511DE2E3350B`.
The installer is publicly available only after publication and an
unauthenticated download of those exact bytes. A local installation alone is
not publication.

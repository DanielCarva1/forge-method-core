# Forge Desktop 0.1.33 alpha.1

This Windows x64 alpha makes an opened project easier to use in a narrow
window. Once a project is confirmed, **Conversa**, **Prévia** and **Andamento**
show the existing conversation, local result and Forge record without requiring
the person to scroll through every panel. The real folder controls remain
available with Andamento. Opening a file cited in the conversation moves to its
local preview; preparing a change or explanation returns to the same unsent
conversation draft. Wider desktop windows keep their existing layout.

The empty-record guidance now says to start with the conversation rather than
assuming the conversation is beside the record. A duplicate phase badge was
removed from the narrow confirmed-project heading. Neither change invents
project progress or modifies the Forge backend.

## Verification

- The browser UI suite covers pane switching, keyboard activation, 200% narrow
  text without document overflow, preservation of the actual preview and
  draft, result-open and change/explanation return paths, and desktop resize.
- A hidden native Windows app was exercised with real folder onboarding and
  Forge-record readback. Its narrow conversation, record, empty preview and
  loaded isolated HTML preview were visually reviewed. The loaded local page
  retained the existing script, remote-resource and app-origin isolation.
- Desktop `cargo check`, all 50 non-ignored Desktop tests, strict Desktop
  Clippy, browser UI suite, release build and installed-candidate native smoke
  passed. No separate core-workspace run or manual GitHub CI run was made for
  this frontend package.
- Same-machine installation over 0.1.32 passed. The published installer and
  sidecar were downloaded without authentication, matched the candidate bytes,
  and the downloaded file passed silent installation and hidden native smoke.
- After publication, the installed public bytes passed a hidden native
  390px real-Codex journey in a disposable project: Send, generated local HTML,
  isolated preview, a follow-up change in the same conversation, WebView reload
  and full process restart without replaying a turn.

## Limits

- This is a responsive desktop-shell improvement, not mobile-device
  distribution or secure remote-agent access. Manual screen-reader acceptance
  remains unverified. The real-Codex test is one bounded disposable journey,
  not proof of every account, project or provider response.
- The preview is local and protected; it does not confirm publication or fully
  execute interactive websites. Clean-machine setup and fresh-account login
  completion remain unverified.
- There is no auto-updater. Install this unsigned NSIS package over the previous
  alpha. Setup may download WebView2 if it is not already present. Per-task
  model cost and Pro-quota savings remain unknown.

## Tested candidate

The single tested NSIS candidate is `Forge_0.1.33_x64-setup.exe`, 123,105,178
bytes, SHA-256
`90D94DE8984F92BE810999A1B06F6EC523D60061490B28BF58288AC9959AE80B`.
Silent installation over 0.1.32 returned exit 0. The installed executable
reports 0.1.33 and has SHA-256
`A9B072C9924EE8E55B9596810E69AF19D5BE43384ACD18A22B476390BDFFA188`.
The candidate hash remained unchanged after installation. The published
installer downloaded at 123,105,178 bytes with the same SHA-256, matching its
published sidecar. Installing those downloaded bytes returned exit 0 without
changing their hash; the installed executable retained the SHA-256 above.
Hidden native smoke passed after that installation, including real folder
onboarding, record readback, narrow panel switching, local preview and
controlled conversation events. A later opt-in test of the same installed
public binary exercised actual Codex Send, local result/change and restart.
One earlier real-reply wait timed out; later real-agent runs passed, so that
intermittent provider/transport condition is not claimed resolved.

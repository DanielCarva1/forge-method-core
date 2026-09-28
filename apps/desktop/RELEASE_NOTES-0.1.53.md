# Forge Desktop 0.1.53 alpha.1

Opening a saved Forge project now brings its saved Codex conversation back
without a second click or an automatic message. A real replied conversation
was restored after restarting the Windows app. A previous send whose delivery
is uncertain still requires explicit review before another message can be sent.

The writing area gets more room before there is a result; after a local result
loads, the preview gets more room. For a local HTML page, **Usar no navegador**
now sits immediately after the protected preview as the main next action;
requesting another change remains available. Opening the page in the system
browser still requires a separate confirmation.

## Verification and limits

The controlled browser UI suite passed, including saved-project reopening,
draft isolation, project switching and enlarged text. All 52 non-ignored
Desktop Rust tests, strict Desktop Clippy and eight frontend unit tests passed.
A focused hidden-native
test used the real Forge project and Codex bridge: one read-only message
received a reply, the app process restarted, both messages returned with the
same project, and no second message was sent. A separate probe found that an
empty Codex thread with no sent message did **not** resume after restart; that
case is not claimed as fixed.

This is still an unsigned Windows x64 alpha without automatic updates. It is
installed over the earlier package. Clean-machine setup, fresh provider login
completion, actual external-browser launch and manual screen-reader acceptance
remain NOT_RUN. The occasional Forge record first-read error has not been
isolated. Model-specific token use and BRL cost remain UNKNOWN.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.53_x64-setup.exe`, 123,040,987
bytes, SHA-256
`1ED8B945DF44400CC697E8A6735C5D00D9E57D4D5B3F534A022AA4512FFDD377`.
Its adjacent 93-byte `.sha256` sidecar records the same hash. Silent
installation over 0.1.52 returned exit 0 without changing the candidate.
The installed executable reports ProductVersion 0.1.53, SHA-256
`A841B2C05F78EE1FE1D76452C8CB68165E44625E81E1084BF8D198941D17F6A6`,
and passed the hidden native smoke with real Forge project/record readback,
onboarding and protected preview. The installed candidate did not run a new
real Codex turn; the real restart check above used the same source's debug
executable. Public download verification remains pending publication.

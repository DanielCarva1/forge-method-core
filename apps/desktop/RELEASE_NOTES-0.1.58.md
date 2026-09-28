# Forge Desktop 0.1.58 alpha.1

This update makes it easier to recover a conversation after the Codex
connection closes. The conversation shows **Reabrir conversa** instead of
leaving the person at a disabled composer. After a manual disconnect,
**Continuar conversa anterior** stays visible even when old messages remain
on screen. Reopening reads the saved conversation; it does not resend the
last message or repeat its actions. An empty, unsent conversation still does
not replace a real saved conversation.

## Verification and limits

The focused browser check covered both recovery actions and no-resend
behavior. A hidden native Windows build handled an injected disconnect
notification, then used real native disconnect/reconnect IPC without a Codex
Send. It also restarted and confirmed that an unsent empty thread was not
bookmarked. An actual unexpected Codex process loss was not exercised.

This remains an unsigned Windows x64 alpha updated manually by installing
the new NSIS package over the previous version. It bundles pinned Forge core
0.13.2, not source-tree core 0.13.3. Clean-machine setup, first-time ChatGPT
login completion, external browser launch, physical mobile use, and manual
screen reader acceptance remain NOT_RUN. Existing real-Codex chat/result,
change-request and restart evidence belongs to earlier alpha packages; it
was not repeated for this UI recovery update. Model-specific tokens and
BRL-equivalent cost remain UNKNOWN. No broad suite or manual GitHub CI ran.

## Candidate

One NSIS candidate was built: `Forge_0.1.58_x64-setup.exe`, 123,118,485
bytes, SHA-256
`402BCE522E39F7DF047997FFBC92EECE661FE71810ACF3E6E9FAF12D77C14DAB`.
Its 93-byte `.sha256` sidecar records the same hash. Silent installation over
0.1.57 exited 0 and did not change the candidate bytes. The installed
ProductVersion is 0.1.58; the installed executable SHA-256 is
`0C6F276F46CD8971FCE435B5E341B451950D5245DF3CCC1E67AF8FE1DC4A8135`.
The installed candidate passed the focused hidden-native recovery check with
real native reconnection and zero Codex Sends. This is same-machine upgrade
evidence, not clean-machine installation proof.

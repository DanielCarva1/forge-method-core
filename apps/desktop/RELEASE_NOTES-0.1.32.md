# Forge Desktop 0.1.32 alpha.1

This Windows x64 alpha brings the project's files closer to the conversation.
When a completed Codex reply cites one local file, **Conferir arquivo citado**
appears beside the reply. When it cites several, a compact list in that reply
lets the person choose. The app never guesses which of several files is the
result. Each choice still goes through the existing project-bound native
preview; choosing a file does not send a message. The action remains available
in original-text mode and after a conversation is reopened. A shorter notice
for an incomplete resumed reply leaves more room to read the conversation
while retaining the warning about possible changes and no automatic resend.

## Verification

- The browser UI suite covers one and several cited files, later replies while
  another preview is open, original-text mode, keyboard traversal, and the
  unchanged send/restart guards.
- The installed Windows candidate was exercised on a hidden desktop with a
  controlled Codex history. It opened real files from a disposable project,
  rejected an outside-project path, preserved the choice across process
  restart, and prepared a change without sending. The full installed native
  smoke also passed real folder selection and Forge project onboarding.
- Desktop `cargo check`, all 50 non-ignored Desktop tests, strict Desktop
  Clippy, the browser UI suite and the NSIS release build passed. No separate
  core-workspace run or manual GitHub CI run was made for this UI package.
- A pre-existing real Codex reply was inspected and found to cite three
  distinct project files. A follow-up attempt to exercise the new inline
  chooser against that real thread could not finish the Codex resume within
  its bounded timeout. This is **NOT_RUN**, not a product failure or a PASS.
- Same-machine installation over the public 0.1.31 alpha passed. This does
  not prove clean-machine setup or a real Codex Send in this new flow.

## Limits

- This is an alpha building block, not completion of all Forge UI stories or
  every backend capability. Mobile tabs, manual accessibility acceptance,
  fresh-account login completion, and clean-machine setup remain open.
- The preview is a protected, local inspection surface, not evidence of web
  publication. External-browser launch on the maintainer's active desktop
  remains unverified; the app does not silently open it.
- No auto-updater is included. Install the new NSIS package over the prior
  alpha. The installer is unsigned; setup may download WebView2 if it is not
  already present. Task-level model cost and Pro-quota savings remain unknown.

## Tested candidate

The single tested NSIS candidate is `Forge_0.1.32_x64-setup.exe`, 123,103,870
bytes, SHA-256
`8FB546E59839952143148D4A4C27C38C747EAB6DE522CDB91BBB36BB86D6522E`.
Silent installation over 0.1.31 returned exit 0. The installed executable
reports 0.1.32 and has SHA-256
`4E5F6C9A21F041978641786E9DFBFF62F2C5FE8B8EB0CBC1ED849256939F08D8`.
An unauthenticated download of the published installer and sidecar returned
the same size and SHA-256. Installing those public bytes silently returned
exit 0, retained the installed executable hash above, and passed the hidden
native result/restart smoke. This proves same-machine public-byte continuity,
not clean-machine setup.

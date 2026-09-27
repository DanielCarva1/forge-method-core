# Forge Desktop 0.1.21 alpha.1

This Windows x64 alpha improves recovery when a Codex reply is interrupted.
After reopening the conversation, the app clearly says the last reply was
incomplete, asks the person to inspect the conversation and project files
before requesting continuation, and does not resend anything automatically.
An earlier completed file result remains available even when the latest reply
is incomplete. Opening it still uses the native project-bound file check.

## Verification

- Browser UI tests cover an interrupted restored reply, a completed earlier
  result, a normal completed conversation, and zero automatic resends.
- A debug native Windows app reopened the real disposable Codex conversation,
  showed the interrupted-reply warning and earlier local HTML result, read the
  exact current Forge Work Focus, and sent no message. A controlled native
  full-process restart tested the result shortcut, preparation of a change in
  the same conversation, and rejection of an outside-project file.
- The disposable page created during that real conversation was independently
  tested in Chromium for add, list, persistence after reload, and removal.
  Forge's protected preview rendered the page but intentionally did not run its
  JavaScript.
- PASS: eight frontend unit tests, the browser UI suite, Desktop `cargo check`,
  all 49 Desktop crate tests, strict Clippy, and one NSIS release build.
- PASS: the exact candidate below installed silently over public 0.1.20. The
  installed 0.1.21 app passed hidden native smoke and real-conversation readback
  of the interruption warning, earlier result, and Forge Work Focus with zero
  sends and unchanged project files.

## Limits

This release does not make a long Codex turn faster or cheaper. The real agent
was interrupted before its final reply and before recording completion in
Forge; the Work Focus remains current. Two real turns consumed 2,221,650
observed tokens combined, mostly cached input, with no reliable per-task BRL
or Pro-quota attribution. This is not proof of a complete autonomous journey.
The installer is unsigned and has no automatic updater. Clean-machine setup,
fresh-account login, accessibility acceptance, and mobile are not yet verified.

## Candidate

The single locally tested candidate is `Forge_0.1.21_x64-setup.exe`,
123,045,449 bytes, SHA-256
`D98EEAB8215BBA5C64EBDFC8345FC41B67D8293FBD18C42BED2945AC44A41F66`.
The installed executable SHA-256 is
`7BE48E9959C3B7DED377B5564B5A3EEE575652E8B723917AFB7DD205C4D4B4CC`.
A commit is not publication; availability requires a public release and an
exact-byte download check.

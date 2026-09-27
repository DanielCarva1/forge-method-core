# Forge Desktop 0.1.19 alpha.1

This Windows x64 alpha makes a real project result easier to understand when
the Forge record has no steps yet. The conversation and local files remain
visible; the app no longer displays an unrecorded discovery stage or implies
that the work itself is missing. It does not create or infer Forge records from
chat text.

On a new Codex conversation, the app now names the exact Forge executable it
resolved for the project and tells the agent to use that copy rather than a
global, Cargo or WSL installation. This addresses a previously observed
first-use turn that chose a separately installed copy. It is instruction
handoff, not proof that every future agent turn will comply.

## Verified scope

- PASS: browser UI checks and hidden native read-only resume of an existing
  real Codex conversation with a local HTML result and an absent Forge record.
  The corrected view kept the chat/result and hid the false stage.
- PASS: eight frontend unit tests, Desktop Rust check, all
  49 Desktop crate tests and strict Clippy at version 0.1.19.
- PASS: a hidden native fake-login/protocol fixture captured `thread/start`
  and verified that the developer instructions name the same existing bundled
  Forge executable as the pinned Start Forge skill. This fixture did not run
  a provider model or complete a real login.
- PASS: the single NSIS candidate installed silently over public 0.1.18,
  exit 0. The installed app reports 0.1.19. Hidden native WebView smoke passed
  with real Forge project onboarding and local preview. The installed build
  resumed the existing real Codex conversation and generated HTML read-only,
  then prepared a change request without sending it. Bundled core and skill
  retained their pinned SHA-256 hashes.

## Limits

A fresh real Codex turn obeying the explicit bundled path, fresh-account login,
clean-machine install and manual accessibility acceptance have not been
verified. The installer remains unsigned and has no automatic updater. This
alpha is a building block, not a claim that the complete Forge experience is
finished. No provider-model turn was run for this package; task-specific BRL
and Pro allowance attribution remain UNKNOWN.

## Candidate

The single locally tested installer is `Forge_0.1.19_x64-setup.exe`,
123,086,775 bytes, SHA-256
`E96CC0BCE5664DD56140F48CE8FDB0831A65EDE9F8D17ACAD36FEAAD0FEC44A9`.
The installed executable SHA-256 is
`52C36C1CBDEC97CB03C297088D1CCAF4EAA83DE45C2464B4008BB291B8BBFEE1`.
The installer is publicly available only if an unauthenticated download
matches these exact bytes. A local installation alone is not publication.

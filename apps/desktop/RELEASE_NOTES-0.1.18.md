# Forge Desktop 0.1.18 alpha.1

This Windows x64 alpha makes the project record and local result easier to use
while talking with the Codex agent. If Forge reports a pending or suggested
question, it appears before optional technical history and has a shortcut from
the record summary. Asking about a question or option prepares an editable
Portuguese draft; it does not send a message, choose an option, or change the
Forge record. The question and options themselves still come from Forge.

The local HTML preview keeps a visible “protected preview” warning, while its
longer explanation opens on demand. The preview remains isolated: scripts,
external requests and interaction are blocked. A local preview still does not
mean the work has been published.

## Verified scope

- PASS: browser UI suite, eight frontend unit tests, Desktop Rust check, all
  49 Desktop unit tests and strict Clippy.
- PASS: hidden native WebView smoke using the current Forge project and a
  disposable local HTML file. It showed a real, nonempty Forge question,
  prepared a discussion draft without sending, and opened the protected-preview
  disclosure. The local HTML stayed isolated.
- PASS: one NSIS candidate installed silently over public 0.1.17, exit 0;
  the installed app reports 0.1.18. Hidden native WebView smoke passed again
  on the installed candidate. The installed Forge core and Start Forge skill
  match their pinned SHA-256 hashes.
- NOT_RUN: real conversation continuity specifically across this version
  upgrade. The native smoke used controlled conversation events and did not
  send or replay a real Codex turn.

## Limits

The native question check did not send a real Codex turn or establish that an
agent will resolve a decision. Real app-agent obedience to a typed Decision
Request, fresh-account browser login completion, a clean-machine install and
manual accessibility acceptance remain unverified. The installer is unsigned
and has no automatic updater. This alpha does not claim the whole product is
complete. No provider-model turn was run for this package; task-specific BRL
and subscription allowance costs are UNKNOWN.

## Candidate

The single locally tested installer is `Forge_0.1.18_x64-setup.exe`,
123,069,091 bytes, SHA-256
`5BF97B83BEF0D96D38F70D532D76E9AB8FFEA4F5ED7D69292AF0AFF2AFE5001C`.
The installed executable SHA-256 is
`305CC60D29E0B393D01CAB3BA5D4513CB81DA3784AE0635E24A5F704D879B88C`.
The installer is publicly available only if an unauthenticated download
matches these exact bytes. A local installation alone is not publication.

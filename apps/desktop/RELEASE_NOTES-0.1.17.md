# Forge Desktop 0.1.17 alpha.1

This Windows x64 alpha makes the project record easier to follow while chatting.
After an agent turn completes, is interrupted, or fails, the app automatically
reads the current project record from Forge. It does not infer progress from
the agent's answer. The record stays hidden while a turn is running, and a
failed read leaves manual retry available. No separate project store was added.

The app also tells its Codex agent to create a temporary JSON input, invoke
Forge, and clean up in separate tool calls. A previous combined shell command
was blocked by the host before Forge ran. This guidance does not bypass host
policy or guarantee that a real agent will follow it.

## Verified scope

- PASS: controlled browser UI suite, eight frontend unit tests, Desktop Rust
  check, all 49 Desktop unit tests, strict Clippy, and NSIS release build.
- PASS: the candidate installed silently over public 0.1.16 with exit code 0;
  the installed app reports 0.1.17. Its bundled Forge core and Start Forge
  guidance match the pinned SHA-256 hashes.
- PASS: hidden native WebView smoke on the installed candidate with a
  disposable Forge project. A
  controlled terminal agent event triggered a fresh, real `inspect_progress`
  call through Rust and displayed Forge's unchanged authoritative empty record.
  No real Codex turn was sent for this check.
- PASS: a native Codex-protocol fixture captured the first conversation's
  revised developer instruction and verified the installed guidance path and
  bytes. This does not prove that a real agent will obey the instruction.
- PASS: the installed Forge core 0.13.2 accepted a typed, read-only
  `decision_required` request on a disposable project when JSON creation and
  invocation were performed separately. The fixture's 15 files did not
  change; this was a standalone core test, not app-agent proof.

## Limits

Actual obedience to the revised instruction during a real Codex decision turn
is not yet verified. Fresh-account browser login completion, a genuinely clean
Windows machine, manual accessibility acceptance, and mobile remain unverified.
The installer is unsigned and has no automatic updater. This alpha does not
claim that the entire product is complete.

No new provider-model turn was run for this package. Per-model BRL spending and
subscription quota attributable to it remain UNKNOWN.

## Candidate

The single locally tested installer is `Forge_0.1.17_x64-setup.exe`,
123,090,502 bytes, SHA-256
`FD53B3133956F15FC4A34E0B4EA521FD67E3D3F902DE551C66BCA0C94AA7138A`.
The installed executable SHA-256 is
`CA2C1657B54A4EA8EB95B0C19E30363AD16BCAD442AC6AFD9A536D84599FEB50`.
The installer must not be called publicly available until the published
download is checked against this exact candidate and installed again.

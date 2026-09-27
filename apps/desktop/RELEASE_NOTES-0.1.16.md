# Forge Desktop 0.1.16 alpha.1

This Windows x64 alpha includes the Start Forge guidance alongside the bundled
Forge core 0.13.2. When a new project conversation begins, the app points Codex
to that exact packaged file instead of relying on a separately installed host
skill. The packaged guidance is byte-identical to the `v0.13.2` source and is
checked by SHA-256 during packaging. No global skill installation is changed.

The conversation screen also keeps its decorative foliage clear of the message
composer and footer. There is no new project store or change to Forge's
authoritative project state.

## Verified scope

- PASS: focused desktop Rust check, 49 unit tests, strict desktop Clippy, and
  the browser UI suite.
- PASS: the 0.1.16 NSIS candidate installed silently over public 0.1.15. The
  installed app reports 0.1.16; its bundled core and Start Forge guidance
  match the pinned hashes.
- PASS: hidden native WebView smoke on the installed candidate.
- PASS: a native Codex-protocol fixture captured the first conversation's
  developer instructions and verified the installed guidance path and bytes.
- PASS: one real authenticated Codex conversation on a disposable project read
  the packaged guidance, used Forge project context, answered in Portuguese,
  and resumed after a WebView reload without resending. The Codex trace shows
  it read the packaged file in bounded sections after its first read was
  truncated by tool output limits. No product file was changed or published.

## Limits

The real Codex check used an already authenticated account on this machine;
fresh-account browser login completion and a genuinely clean Windows machine
remain unverified. The typed read-only Decision Request flow, manual
accessibility acceptance, and mobile remain open. The installer is unsigned
and has no automatic updater. This is an alpha building block, not a claim
that the entire product is complete.

The bounded real-agent check consumed a cumulative 530,161 input tokens
(476,800 reported as cached input) and 3,959 output tokens on `gpt-6-sol`.
Cached input is a subset of input and reasoning output is a subset of output;
they are not added again. This is one check, not a controlled cost comparison.
Per-model BRL spending and subscription quota attributable to this package
remain UNKNOWN.

## Candidate

The single locally tested installer is `Forge_0.1.16_x64-setup.exe`,
123,136,684 bytes, SHA-256
`EFA67725B375DC33AAB811381DA692288D65320D7C2175691253C39006F8BA10`.
It must not be called publicly available until the published download is
checked against this exact file and installed again.

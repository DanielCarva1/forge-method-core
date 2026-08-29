# Codex CLI 0.150.0-alpha.8 on Codex Desktop 26.820.10647.0

This is the current native-Windows Codex dogfood result recorded for SD-06.
It covers one exact Codex CLI and Desktop combination. It does not claim support
for another Codex version, operating system, agent app, or Forge build.

## Plain result

Forge evaluated the real current-host journey through the existing built-in Rust
adapter. The run used a clean build of Forge `0.12.0-alpha.47` from source commit
`03a8742bc655285c1ee580e0cd4b0b61ad78e6ea`. The bundle records the exact Forge
executable hash, Codex CLI `0.150.0-alpha.8`, Codex Desktop `26.820.10647.0`,
and Windows `10.0.26200` on x86_64.

The result is intentionally mixed instead of pretending everything passed:

- 6 capabilities are `partially_supported`;
- 2 capabilities are `unsupported` in this exact journey;
- 14 assertions passed, 9 failed, and 1 Windows-to-WSL assertion was not
  applicable because Forge ran natively on Windows.

What worked in this run:

- one-chat activation and versioned JSON;
- resolution of the exact native-Windows project root;
- read-only guidance without changing Forge state;
- carrying the user's accepted story into Work Focus and reading it back;
- safe rejection of a missing isolation;
- a fresh replacement agent recovered the objective, issue, Work Focus, current
  activity, and next safe step without the previous transcript.

What this run did not exercise:

- rejection of an ambiguous project root;
- admission, rejection, and readback of test or review evidence;
- a linked active isolation and the wrong-owner rejection path;
- governed promotion preview, apply, readback, and exact retry.

Those missing actions remain visible as typed gaps. They are not counted as
passing just because Forge has code or separate tests for them.

## Proof limit

The Codex adapter is cooperative: Codex reports closed yes/no observations and
Forge validates the shape, derives the result, writes the bundle, and verifies
its files and hashes. Codex has no trusted Forge-native verifier in this build.
Therefore even a reported pass can be at most `partially_supported` and the
bundle does not independently prove that the host performed an action.

Run this to recheck the retained bundle:

```text
forge-core host-conformance verify --bundle-dir contracts/hosts/conformance-results/codex/0.150.0-alpha.8/bundle --json
```
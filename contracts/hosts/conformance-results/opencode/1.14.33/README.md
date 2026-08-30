# OpenCode 1.14.33 on native Windows

This is the current native-Windows OpenCode dogfood result recorded for SD-06.
It covers one exact OpenCode CLI, model, platform, and Forge build. It does not
claim support for another OpenCode version, model, operating system, or Forge
build.

## Plain result

OpenCode performed a real Forge-on-Forge journey for issue 67. The session used
OpenCode `1.14.33` with model `zai-coding-plan/glm-5.3`, Forge
`0.12.0-alpha.47`, and Windows `10.0.26200` on x86_64. A temporary closed
observation bridge translated the completed journey into the existing public
protocol; it did not act as the host or add a new product adapter.

The result is intentionally mixed:

- 6 capabilities are `partially_supported`;
- 2 capabilities are `unsupported` in this exact journey;
- 15 assertions passed, 8 failed, and 1 Windows-to-WSL assertion was not
  applicable because Forge ran natively on Windows.

What worked in this run:

- activation, versioned JSON, and the exact native-Windows project root;
- plain read-only guidance without changing Forge state;
- recovery of the accepted issue and Work Focus after a follow-up in the same
  OpenCode chat;
- admission and readback of valid cooperative evidence;
- durable rejection and readback of invalid evidence.

What did not work or was not exercised by OpenCode:

- the first prompt was refused until the user asked OpenCode to verify the
  repository and issue;
- OpenCode scanned the repository broadly before following Forge's resume
  packet and exceeded the requested bounded tool-call limit;
- ambiguous-root rejection was not exercised;
- isolated work and its rejection paths were not completed by the OpenCode
  session;
- governed promotion was not exercised;
- recovery was not ranked before new investigation on the first response.

The coordinator later created an official isolation to retain this result. That
action is deliberately excluded from the OpenCode capability result.

## Proof limit

Forge verified the bundle structure, derived result, files, and hashes. This
build has no trusted OpenCode-native proof verifier. Therefore adapter-reported
passes can be at most `partially_supported`; bundle integrity does not
independently prove that OpenCode performed each reported action.

Run this to recheck the retained bundle:

```text
forge-core host-conformance verify --bundle-dir contracts/hosts/conformance-results/opencode/1.14.33/bundle --json
```

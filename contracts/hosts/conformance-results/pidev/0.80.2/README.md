# Pi.dev 0.80.2 on native Windows

This is the current native-Windows Pi.dev dogfood result recorded for SD-06. It
covers one exact Pi package, model, tool route, platform, and Forge build. It
does not claim support for another Pi version, model, tool configuration,
operating system, or Forge build.

## Plain result

Pi.dev performed a real but incomplete Forge-on-Forge journey for issue 68. The
controlled session used Pi `0.80.2`, model `zai/glm-4.7`, a single real `bash`
tool route, Forge `0.12.0-alpha.47`, and Windows `10.0.26200` on x86_64. A
temporary closed observation bridge translated only the observed facts into the
existing public protocol; it did not act as the host or add a product adapter.

The result is intentionally mixed:

- 5 capabilities are `partially_supported`;
- 3 capabilities are `unsupported` in this exact journey;
- 13 assertions passed, 10 failed, and 1 Windows-to-WSL assertion was not
  applicable because Forge ran natively on Windows.

What worked in this run:

- Pi read the installed Start Forge skill and invoked the installed Forge CLI;
- activation, versioned JSON, and the exact native-Windows project root;
- plain read-only guidance without changing project or Forge workflow state;
- recovery of the accepted issue and Work Focus without prior transcript state;
- a follow-up `current-work detail` call succeeded when it used the normal
  Windows path instead of the extended `\\?\` path through Bash.

What did not work or was not exercised by Pi:

- preflight attempts showed that some model/tool combinations printed invented
  tool calls or exposed no tools; the retained run therefore used
  `zai/glm-4.7` with only `bash`;
- Pi invoked `start` twice even though the skill asks for one activation;
- its first `current-work detail` call lost one slash from the extended Windows
  path and failed;
- after completing the requested read-only steps in a follow-up, Pi ignored the
  requested stopping point, inspected its own CLI, and tried to launch another
  Pi process; the coordinator interrupted that recursive attempt;
- ambiguous-root rejection, cooperative evidence, isolated work, and governed
  promotion were not exercised by the Pi session.

The coordinator later created an official isolation to retain this result. That
action is deliberately excluded from the Pi capability result.

## Proof limit

Forge verified the bundle structure, derived result, files, and hashes. This
build has no trusted Pi-native proof verifier. Therefore adapter-reported
passes can be at most `partially_supported`; bundle integrity does not
independently prove that Pi performed each reported action.

Run this to recheck the retained bundle:

```text
forge-core host-conformance verify --bundle-dir contracts/hosts/conformance-results/pidev/0.80.2/bundle --json
```

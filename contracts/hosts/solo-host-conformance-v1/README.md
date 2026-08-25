# Forge solo host conformance kit v1

This folder is the complete public protocol kit. It is host-neutral: a new
agent app does not require a product-name branch in Forge.

## Try the protocol

Run the built-in reference adapter. Give Forge the real project folder that the
host is operating:

```text
forge-core host-conformance run \
  --builtin-adapter reference \
  --host-id example.host --host-version 1.0.0 \
  --platform-id example-platform --environment-id local-test \
  --canonical-root <existing-project-folder> \
  --output-dir <new-bundle-folder> --json
```

Forge starts its Rust reference adapter as a bounded process inside the resolved
canonical project folder. External adapters still use `--adapter <program>` and
the same stdin/stdout protocol. `protocol-contract.json` gives the closed field
and safety rules; `response.example.json` shows the response shape. An adapter
must copy the request bindings exactly. It reports assertions and closed fact
codes, never a final support label.

The reference adapter intentionally earns only `partially_supported`. Even an
all-true adapter response stays partial because this Forge build cannot verify
host-native proof. Future proof verifiers must be trusted by Forge and selected
by proof scheme, not by a host product name.

Raw chat, transcripts, logs, environment variables, arbitrary payloads, and
secrets are not accepted as evidence. Fact and gap codes are closed safe tokens.
Forge stores only basenames and hashes for adapter files and hashes literal
arguments instead of disclosing them.

The Windows-to-WSL assertion is `not_applicable` unless Forge itself observes a
Windows run targeting a WSL network root. Linux, macOS, and native Windows runs
do not claim that bridge succeeded.

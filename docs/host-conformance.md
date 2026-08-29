# Host conformance for solo developers and agents

Forge does not approve an agent app because it recognizes the app's name. Any
app can connect an adapter to the same public kit. Adding a new app does not
require a product-name rule in Forge.

Every result applies only to the exact host and adapter versions recorded in
its bundle. It is not a claim about older or newer versions.

The kit checks eight parts of the solo journey separately:

1. start Forge once for the chat;
2. find the one real project folder;
3. get guidance without changing files or Forge state;
4. carry the human's chat decision without agent self-approval;
5. submit and read back test or review evidence;
6. use a linked isolated work area;
7. apply an agreed change once and read it back;
8. let a fresh agent continue without needing the old chat transcript.

## Export the complete public kit

```text
forge-core host-conformance corpus --output-dir <new-folder> --json
```

The new folder contains the corpus, plain instructions, a machine-readable
protocol contract, and a response example. The runnable reference adapter is
built into `forge-core`, so the kit needs no Python runtime or copied script.
The exported files remain enough to build an external adapter without reading
Forge source code.

## Run a built-in adapter

```text
forge-core host-conformance run \
  --builtin-adapter reference \
  --host-id <id> --host-version <exact-version> \
  --platform-id <declared-label> --environment-id <declared-label> \
  --canonical-root <existing-project-folder> \
  --output-dir <new-bundle-folder> --json
```

Use `--builtin-adapter codex --observation-file <json>` for a completed Codex
journey. Forge supplies the built-in adapter ID and version so the caller cannot
declare a different implementation than the one that actually ran.

## Run an adapter

```text
forge-core host-conformance run \
  --adapter <program> --adapter-arg <arg> \
  --host-id <id> --host-version <exact-version> \
  --adapter-id <id> --adapter-version <exact-version> \
  --platform-id <declared-label> --environment-id <declared-label> \
  --canonical-root <existing-project-folder> \
  [--timeout-ms <1..300000>] \
  --output-dir <new-bundle-folder> --json
```

Forge starts the adapter inside the resolved canonical project folder and passes
each adapter argument separately. It never builds a shell command.
It stops an adapter that runs too long or writes too much. Adapter stderr is
drained but not shown, because it may contain a secret.

The adapter returns only yes/no observations, typed gap codes, and closed fact
codes. Raw chat, transcripts, logs, environment variables, arbitrary payloads,
and secret-like values are rejected. Forge creates the evidence files itself.
This reduces accidental disclosure, but no text filter can mathematically prove
that every unknown kind of secret is absent.

Forge calculates one result for each capability:

- `supported`: every required point passed and every pass has proof that Forge
  or a trusted native verifier actually checked;
- `partially_supported`: something works, but a failed point, missing API, or
  unverified adapter report remains;
- `unsupported`: none of the applicable required points passed.

This release has no trusted host-native proof verifier. Therefore an adapter
that simply says "everything passed" can never receive `supported`; Forge adds
`native_authenticity_unavailable` and caps it at `partially_supported`. The
protocol has a proof-scheme field for a future trusted verifier, but Forge does
not pretend that feature exists today.

The Windows-to-WSL check is based on facts the running Forge process can see. It
is `applicable` only for Forge running on Windows with a canonical root on a WSL
network share. It is `not_applicable` on Linux, macOS, and native Windows roots.
An unknown platform becomes `indeterminate`; it does not count as a pass.

## What is bound without exposing personal paths

The bundle keeps caller-supplied labels separate from measured facts. It binds:

- the host and adapter labels and versions supplied by the caller;
- Forge's measured OS and architecture;
- the Forge executable hash;
- the resolved canonical-root hash and root kind, but not its personal path;
- the resolved adapter executable basename, size, and streaming SHA-256;
- each file argument's basename, size, and streaming SHA-256;
- each literal argument's SHA-256 instead of its value;
- the exact separated argument order and its combined digest;
- the timeout, output limit, and public corpus hash.

## Recheck a saved bundle

```text
forge-core host-conformance verify --bundle-dir <bundle-folder> --json
```

Verification recalculates every file, every result, and the manifest digest. It
rejects missing, extra, changed, oversized, linked, unsafe, too-deep, or
inconsistent evidence. On systems where Forge can read link counts, hard-linked
files are rejected too. Files are opened once and the same open file is checked
and read, reducing path-swap races.

A clean bundle proves that these exact closed files are complete and unchanged.
It does not prove the adapter told the truth about the host. An incomplete,
tampered, or unsafe bundle is invalid; Forge never softens that into
`unsupported`.

## Codex cooperative adapter

The built-in Rust Codex adapter version `1.0.0` translates one closed,
same-owner observation from an exact, completed Codex journey into the public
protocol. It is a post-journey evaluation tool invoked explicitly with
`host-conformance run`; it is not run by every `forge start` or chat activation.
It rejects unknown fields, binds its own adapter ID and version, remains neutral
to the Codex host version, and never claims native authenticity.

This is useful for honest dogfood: Forge can bind exact versions, calculate all
eight results, retain an integrity-checked bundle, and show missing host
capabilities as typed gaps. It is not proof that Codex enforced the method.

Codex Desktop and the Codex CLI have different versions and must be recorded
separately. If Codex Desktop runs on Windows while Forge runs inside WSL, the
Linux runner can verify the canonical WSL project but cannot independently
observe the Windows side of that bridge. The result must keep that limitation
visible.

The current retained candidate result covers Codex CLI `0.150.0-alpha.8` on
Codex Desktop `26.820.10647.0` and native Windows. It is documented at
`contracts/hosts/conformance-results/codex/0.150.0-alpha.8/README.md`; its
`run-summary.json` and `bundle/` preserve six partially supported capabilities,
two unsupported capabilities, and every typed gap from that exact journey.
The earlier CLI `0.144.6` WSL result remains beside it as historical evidence.
Re-verifying either bundle establishes file integrity and result consistency
only, not the truth of the adapter-reported host actions.

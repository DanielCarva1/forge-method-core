# Real-host proof bundle

This page describes the **non-authoritative** P7F evidence format in
[`real-host-evidence-bundle-v0.yaml`](../contracts/spec/real-host-evidence-bundle-v0.yaml).
It is tooling for packaging evidence for review, not evidence that a journey has
run and not a P7F completion claim.

## What the checker can establish

[`check-real-host-evidence.py`](../scripts/check-real-host-evidence.py) performs a
closed, local check of:

- byte and parser bounds;
- alias-free YAML/JSON, duplicate keys, closed fields, and safe relative paths;
- exact file size and SHA-256 for every referenced artifact;
- the fixed scenario order: clean-host journey, concurrent conflict, then
  replacement-session resume;
- globally distinct session IDs, with at least two sessions for conflict and
  replacement/resume;
- release archive, release manifest, executable, version, platform, and source
  revision identity fields;
- exact argument-vector command logs and separately hashed stdout/stderr;
- a claim, gate result, verified principal, Admission, pre-effect WAL, effect,
  and receipt link for every claimed governed write;
- explicit ungoverned-write and residual-limitations disclosures; and
- an independent-review record field and hashed review artifact.

A successful result means only **structurally/content-integrity valid**. It does
not certify a production host, actor or reviewer independence, publication,
semantic truth, or P7F passage. A transcript is evidence, not authority;
referenced Forge receipts retain only the authority assigned by their own
contracts.

The evidence bundle's release subject must not collapse source checkpoint,
published prebuilt, project workflow release pin, or Domain Pack effective epoch.
Use the canonical [identity table](../README.md#four-identitiesdo-not-collapse-them).
A source-built `0.12.0` executable is not evidence of a published `0.12.0`.

## Capture procedure

1. Create a fresh evidence directory. Keep evidence files under its
   `artifacts/` directory and place the bundle YAML/JSON at the directory root.
2. Preserve exact release subjects separately: archive, embedded release
   manifest, and extracted executable. Record distinct artifact IDs plus release
   version, platform, source revision, and release ID. For a new-format archive,
   review that the embedded manifest's `release_tag` and `source_commit` bind the
   claimed tag/commit; structural checker success alone does not perform that
   semantic/supply-chain review.
3. Capture the three scenarios in contract order. Session IDs must not be
   reused. The conflict and replacement scenarios each require at least two
   sessions.
4. Record every process invocation as an argument array—not reconstructed shell
   text—and hash its captured stdout and stderr separately.
5. For each write described as governed, link seven distinct evidence
   artifacts: claim, gate result, verified principal, Admission, pre-effect WAL,
   effect, and receipt. Do not put direct editor or shell writes in this list.
   A claimed governed write is **Forge-mediated** only when all seven links
   semantically cover that target; file location or transcript presence is not
   mediation.
6. Disclose direct/editor/shell writes under `ungoverned_writes`. If none were
   observed, still include a non-empty statement, `observed: false`, and
   `entries: []`.
7. List at least one residual limitation. Add the reviewer-authored independence
   statement, limitations, disposition, and exact review-record artifact.
8. After files are immutable, calculate exact byte sizes and lowercase SHA-256
   digests. Every artifact row must be referenced; orphan rows fail closed.

The contract contains the complete bundle skeleton and exact field sets.

## Exact argv log

Each scenario's `command_log_ref` points to an alias-free YAML or JSON document
with this shape:

```json
{
  "schema_version": "forge_real_host_command_log_v0",
  "scenario_id": "clean_host_journey",
  "entries": [
    {
      "sequence": 1,
      "session_id": "session-clean-01",
      "argv": ["forge-core", "status", "--json"],
      "working_directory": "/absolute/captured/working-directory",
      "exit_code": 0,
      "stdout_ref": "clean-status-stdout",
      "stderr_ref": "clean-status-stderr"
    }
  ]
}
```

`argv` preserves argument boundaries. A value such as
`"forge-core status --json"` is shell text and rejects. Sequence values are
contiguous and one-based, every declared scenario session must appear, and
stdout/stderr references must resolve through the bundle artifact table. This
validates the log shape and bytes only; it cannot prove what process interpreted
the arguments or that an invocation used a public release interface.

## Run the checker

From the repository root:

```bash
python3 scripts/check-real-host-evidence.py path/to/evidence/bundle.json
```

JSON uses only the Python standard library and therefore works from the release
payload without installing dependencies. Alias-free YAML is also accepted when
PyYAML is installed.

Success prints both the narrow verdict and this mandatory warning:

> This result validates only structure and content integrity; it does not
> certify a production host, actor independence, publication, or P7F passage.

Any bound, parse, schema, order, session, path, reference, size, or digest error
fails closed and prints the same authority warning. The checker does not modify
the bundle, referenced evidence, product status, plan, release records, or
runtime state.

## Review questions beyond the checker

A human review still has to determine whether:

- the host was the claimed supported production host;
- the ordinary human goal and chat-only interaction were genuine;
- commands were public interfaces from the identified release;
- conflict happened before overlapping mediated writes;
- replacement chat reconstructed the required intent, epoch, gaps, evidence,
  and next action;
- linked authority artifacts semantically cover each claimed write;
- ungoverned writes and residual limitations are complete; and
- reviewer identity, independence, and conclusions are credible.

Those findings belong in the independent review. They must not be inferred from
checker success.

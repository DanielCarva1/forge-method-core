# Codex 0.144.6 dogfood result

This retained result covers one exact Codex CLI run. It does not claim support
for another Codex version, another operating system, or another agent app.

## Plain result

The cooperative adapter reported observations from an eight-part
solo-developer journey. Forge translated that same-owner report into a bundle
whose closed-file integrity can be checked again. All eight capabilities remain
`partially_supported`: the report contains 21 passed assertions and two failed
negative checks, while Forge classified the Windows-side bridge assertion as
not applicable to the Linux process that ran the evaluator.

In this same-owner report, the adapter reported that:

- Forge was invoked once for the chat and returned versioned JSON.
- An existing canonical WSL project was resolved without changing it.
- A human decision, cooperative evidence, and one deliberate rejection were
  read back from durable Forge state.
- An active isolated work area was linked correctly, and a missing isolation
  stopped safely.
- A documentation change was previewed, applied, read back, and an exact retry
  did not apply it twice.
- A replacement-agent exercise reconstructed durable state without the prior
  chat transcript and identified the next safe action.

Those statements are adapter-reported observations, not Forge-native proof that
Codex performed the actions. The promotion digests retained in
`run-summary.json` identify the preview and receipt reported by this same-owner
journey; they do not upgrade it to independent proof.

A separate cooperative bridge check opened the WSL Project Link from Windows
and hashed the same `.forge-method.yaml` file inside WSL. Both sides returned
identical bytes. This exercises the bridge on this machine, but the Forge
conformance runner itself ran on Linux, so it correctly records that transport
assertion as not applicable rather than pretending it independently observed
the Windows process.

What remains unproven in this exact run:

- Codex has no trusted Forge-native verifier, so same-owner reports cannot
  become `supported`.
- The Windows-to-WSL bridge passed the host-side same-file check, but remains a
  typed independence gap for the Linux Forge runner.
- Ambiguous-root and wrong-owner rejection were reported as not exercised.

The candidate support matrix deliberately contains no independent-evidence
claim and keeps `selected_host: null`. `run-summary.json` separates declared
identities, Forge-measured bundle bindings, and adapter-reported actions. The
`bundle/` folder is the retained machine record. Running
`forge-core host-conformance verify --bundle-dir <this-folder>/bundle --json`
rechecks the bundle structure, completeness, derived result, and digests; it
does not verify that the reported host actions occurred.

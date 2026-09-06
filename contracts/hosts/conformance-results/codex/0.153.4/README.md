# Codex CLI 0.153.4 on native Windows

Controlled same-owner delivery and fresh-agent recovery, recorded for #70 on
2026-09-05. This is candidate evidence, not independent host proof or release
qualification. Desktop version was not measured.

The installed Forge binary is alpha.49 from `eb2b68ca`. The delivery used the
Start Forge skill from `e1fa3792`; the fresh read-only recovery used its closeout
guidance update from `62d0f535`. Exact identities are in `run-summary.json`.

## Observed result

- 18 assertions passed, 5 were not exercised and remain failed assertions,
  and the Windows-to-WSL assertion was not applicable.
- All eight capabilities are `partially_supported`; none is `supported`.
- A separate CLI agent delivered a coordinator-supplied accepted README task
  through a linked worktree, cooperative evidence, promotion, canonical readback
  and exact retry. This was a simulated user task, not a new live user interview.
- Retry returned the same receipt without another canonical mutation.
- The delivery agent left Work Focus active. The coordinator then exercised the
  existing public closeout API successfully; no runtime fix was needed.
- A fresh CLI agent recovered the completed task without the earlier transcript.
  Project README/link and all sidecar file paths/hashes were unchanged afterward.

This run did not exercise ambiguous-root rejection, evidence rejection,
wrong-owner rejection, missing-isolation rejection, or interrupted recovery
being ranked before new work. Earlier runs are separate evidence, not silently
combined into this bundle. Autonomous closeout with the updated skill is also
not proven by a read-only recovery test. The controlled task is not a full
application build or whole-product completion.

Forge generated the bundle using its existing built-in cooperative adapter.
Recheck it with:

```text
forge-core host-conformance verify --bundle-dir contracts/hosts/conformance-results/codex/0.153.4/bundle --json
```

Verification proves bundle integrity and derived-result consistency, not that
adapter-reported actions have independent authenticity. The run summary links
the issue evidence and hashes retained local observations without storing chat
transcripts or personal fixture paths in the repository.

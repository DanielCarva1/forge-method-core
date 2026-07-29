# Codex cooperative conformance adapter

This version `1.0.0` adapter evaluates one completed Codex journey against
Forge's public eight-part solo host kit. It is an explicitly invoked,
post-journey assessment tool: `forge start` does not run it, and it is not part
of every chat activation. It is intentionally small and does not add
Codex-specific rules to Forge core.

The adapter accepts one closed observation file:

```text
python3 adapter.py --observation-file <observation.json>
```

Forge supplies the public request on standard input. The adapter requires the
declared adapter binding `forge.codex.cooperative` version `1.0.0`, remains
neutral to the declared host version, copies the request bindings exactly, and
writes one public response on standard output. Unknown fields, missing cases,
missing assertions, unsafe fact codes, links, oversized files, raw chat, and
free-form evidence are rejected.

The observation file has this shape:

```json
{
  "schema_version": "forge_codex_host_observation_v1",
  "evidence_mode": "cooperative_same_owner",
  "cases": [
    {
      "case_id": "activation",
      "assertions": {
        "forge_invoked_by_argv": true,
        "one_chat_activation_observed": true,
        "versioned_json_preserved": true
      },
      "gaps": [
        {
          "kind": "native_authenticity_unavailable",
          "code": "codex_host_native_proof_unavailable"
        }
      ],
      "fact_codes": [
        "codex_cooperative_observation",
        "one_chat_start_seen"
      ]
    }
  ]
}
```

The real file must contain exactly the cases and assertions requested by the
public corpus. Keep it outside the project snapshot, retain only closed fact
codes, and derive each `true` from a command result or durable Forge readback
from the same journey. Do not paste chat, logs, paths, environment variables,
or secrets into it.

## Honest limit

This is same-owner cooperative evidence. It cannot prove that Codex itself
performed an action, that a message physically came from a human, or that all
edits were mediated by Forge. The adapter never sends a native proof claim.
Forge therefore caps positive results at `partially_supported`.

Codex Desktop and the Codex CLI have separate versions. Record the exact CLI
version as the host version and disclose the exact Desktop version in the
environment evidence for that run.

When Codex runs on Windows but invokes Forge inside WSL, the current Linux
runner can prove the WSL project root but cannot independently see the Windows
side of the bridge. Keep that as a typed platform gap; do not convert Linux
`not_applicable` into a bridge pass.

## Retained exact result

The candidate result for the exact Codex CLI `0.144.6` journey is retained at
`contracts/hosts/conformance-results/codex/0.144.6/README.md`, with its
`run-summary.json` and integrity-checkable `bundle/`. The adapter-reported
successes remain `partially_supported`; the Windows side of the bridge remains
a typed gap. Bundle verification checks the retained files and derived result,
not whether Codex performed the reported actions.

# Security model

## Protected properties

Forge aims to protect authority provenance, deterministic policy selection,
project/sidecar binding, append-only history, replay/CAS ordering, secret
separation, and fail-closed handling of incomplete input.

## Trust domains

- **Untrusted/candidate:** project files, authored YAML, model output, audit
  projections, Domain Pack composition/resolution output.
- **Verified external/operator:** signed principal/registry material, explicit
  anchors, independent review and runtime/external evidence.
- **Trusted process boundary:** opaque verified capabilities, kernel gates,
  retained locks, prepared transactions and commit protocols.
- **Durable evidence:** hash-linked ledgers, receipts, immutable generations and
  content-addressed objects. Durable does not mean secret.

## Threats addressed

- malformed/open-schema input and namespace shadowing;
- caller-selected workflow, phase, release, readiness, or completion;
- edited audit output reused as authority;
- stale head/snapshot replay and generation rollback;
- partial multi-effect commits and crash ambiguity;
- traversal, symlink/junction/reparse and special-file escapes at checked
  filesystem boundaries;
- unauthorized overlapping write claims;
- supply-chain equivocation, revoked credentials and registry forks.

## Explicit limitations

Local filesystem confinement is cooperative between processes running as the
same OS principal. Forge does not isolate against a malicious same-principal
process that wins a race after validation, reads operator-accessible secrets,
or mutates a project after final snapshot. Use separate OS principals,
permissions, sandboxing, and remote immutable/CAS services for hostile tenants.

Forge reduces unknown unknowns but cannot eliminate them. Domain Packs and
representative evidence do not guarantee quality, compliance, safety, or
factual correctness.

## Secret handling

- Keep private keys and anchors outside project and sidecar roots.
- Never paste keys or opaque capabilities into chat, logs, YAML, or issues.
- Rotate/revoke through typed operator commands where available.
- Treat public audit projections as evidence only.
- The P7a.1 workflow credential command is a cooperative local signing proxy,
  not human-presence authentication. High-authority human/reviewer/runtime keys
  require a host/operator boundary outside the agent process. The one-call
  local `workflow action authorize` command accepts only packets marked
  `operator_credential_broker`; the other broker boundaries fail before local
  signing.
- The P7a.2 origin broker stores public keys only and signs outside Forge. Its
  envelope binds project, packet, minimal input, authenticated origin subject,
  separation domain, profile/kind, freshness, and nonce. Forge still relies on
  the configured host to authenticate that subject honestly.
- Broker verification alone never consumes replay state. The kernel commits
  the action and origin companion under the ledger lock before it appends the
  reserve/commit replay index; a durable companion can repair that index after
  response loss or crash. This is a fail-closed recoverable saga,
  not a claim that separate filesystem stores commit atomically. Rollback
  resistance still depends on protecting the state root and external trust
  anchors from joint rewrite.
- On Windows, credential files inherit the ACL of the derived operator
  directory; operators must protect that directory and the broker trust
  registry even when private broker keys live in a host keystore.

## Reporting a vulnerability

Do not open a public issue containing exploit details or secrets. Follow the
repository [security policy](../SECURITY.md), which prefers GitHub private
vulnerability reporting when that surface is available and documents the
non-sensitive fallback.

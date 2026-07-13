# Security policy

## Scope

Forge Method Core is an authority-bearing local governance runtime. Security
reports may concern signature/registry validation, authority escalation,
path/root confinement, replay or rollback, CAS/freshness, secret exposure,
claim/write admission, WAL/recovery ambiguity, Domain Pack namespace/trust, or
release provenance.

The detailed design boundary is in
[`docs/security-model.md`](docs/security-model.md).

## Supported versions

The project does not currently promise long-term security-support branches.
Source checkpoints and tagged prebuilt releases can differ: verify the exact
commit/tag and `forge-core --version` in every report. Fixes are evaluated for
the current source and supported release artifacts; do not assume an older
prebuilt contains a source-only fix.

## Report privately

Do not publish exploit details, private keys, credentials, operator anchors,
unredacted registries, or sensitive project paths in a public issue.

Preferred route: use **GitHub Security ? Report a vulnerability** for this
repository when private vulnerability reporting is available. If that surface
is unavailable, open a public issue containing no exploit or secret material
and ask the repository owner for a private reporting channel.

Include, when safe:

- affected commit/tag and `forge-core --version`;
- operating system/filesystem and host Adapter;
- affected command or contract family;
- minimal redacted reproduction;
- expected and observed authority/state transition;
- whether project, sidecar, or operator-owned material was exposed or mutated;
- crash/replay/concurrency conditions;
- impact and suggested embargo needs.

## Response expectations

No guaranteed response SLA is currently published. Maintainers should confirm a
private report, reproduce it against an exact revision, classify the affected
trust boundary, preserve evidence, and coordinate remediation/disclosure before
sharing exploit details publicly.

## Explicit non-claims

Forge's local filesystem protection is cooperative for processes under the same
OS principal. It does not promise isolation from a malicious same-principal
process that wins a race after validation, reads accessible secrets, or mutates
project state after a final snapshot. Use separate OS principals, permissions,
sandboxing, and remote immutable/CAS services for hostile tenants.

Forge also cannot guarantee discovery of every unknown unknown or the quality,
compliance, safety, or correctness of a product merely because governance gates
were followed.

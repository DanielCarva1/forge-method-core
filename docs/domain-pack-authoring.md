# Domain Pack authoring core

C7.1 is an in-memory, candidate-only authoring library. It helps an author make
and test a minimal generic Domain Pack without treating a template or a passing
report as installation, trust, review, publication, or activation.

## Public Rust APIs

`forge_core_decisions` exposes two pure functions:

```rust
pub fn generate_domain_pack_author_skeleton(
    request: &DomainPackAuthorSkeletonRequestDocument,
) -> DomainPackAuthorSkeletonDocument;

pub fn evaluate_domain_pack_author_test(
    request: &DomainPackAuthorTestRequestDocument,
) -> DomainPackAuthorTestReportDocument;
```

The input/output contracts are re-exported by `forge_core_contracts` from
`domain_pack_authoring`.

`generate_domain_pack_author_skeleton` returns documents and exact byte vectors
for a manifest, content sidecar, and license template. It does not choose a
directory or write a file. Its request must name the publisher, package,
namespace, version, sealed Core bundle/policy digests, independent project
requirements, provenance, authors, SPDX expression, and logical artifact paths.

The generated template is deliberately generic. Its dependency, conflict,
replacement, and contribution sections are explicit empty editable lists. Empty
lists do not satisfy a required domain or capability.

`evaluate_domain_pack_author_test` takes a typed direct candidate, the complete
composition request, and caller-supplied raw manifest/content/license sidecars.
It invokes the existing candidate validator and deterministic composer rather
than reimplementing either rule set. It can also consume optional exact-lock
comparison material, learning/promotion evidence, and reviewed-registry
snapshots. The returned report is canonical-JSON hashed diagnostic evidence.

## What the report means

The report contains separate sections for:

- structural request consistency;
- raw and canonical artifact binding;
- deterministic composition and explicit requirement gaps;
- optional exact-lock compatibility readiness;
- optional learning/promotion and reviewed-registry readiness;
- unsafe prose and external executable-capability diagnostics.

A `candidate_ready` result only means the supplied candidate material has no
reported C7.1 diagnostic. It is not a permission to run an adapter or evaluator,
trust an author, create a registry entry, sign or publish anything, write files,
apply lifecycle state, install a package, or activate a generation.

Missing requirements are preserved as `missing_domain` or `missing_capability`
diagnostics. Removing a candidate or its contribution cannot make those gaps
disappear.

## Author safety boundary

All prose in a candidate is untrusted data. The author workflow detects selected
prompt-injection and tool-execution language, but never sends prose to a model,
shell, tool runner, evaluator, or adapter. Non-built-in adapters and
Tool/Runtime/Credential/ExternalAuthority capability declarations are blocked as
external executable claims; C7.1 does not probe or execute them.

Exact-lock comparison reuses the existing compatibility evaluator. Its operation
value selects comparison semantics only. C7.1 returns a normalized digest and
diagnostics rather than a lifecycle request, receipt, active pointer, commit, or
activation handle.

Learning and reviewed-registry inputs are similarly evidence only. Promotion,
independent review, registry anchoring/evolution, signing, revocation, and
publication remain separate authority ceremonies.

## Non-authoritative CLI adapter

C7.1 exposes a narrow adapter over the pure APIs:

```text
forge-core domain-pack author skeleton \
  --request-file <request.yaml> \
  --output-root <absent-path> \
  [--json|--no-json]

forge-core domain-pack author test \
  --request-file <request.yaml> \
  [--json|--no-json]
```

The skeleton command reads one bounded typed request and materializes only the
exact bytes returned by `generate_domain_pack_author_skeleton`. Its output root
must not already exist; generated paths are normalized, protected Forge/Git
state components and prefix collisions are rejected, files are exclusively
created, and any failure removes the complete newly created root. The test
command reads one bounded typed request, calls `evaluate_domain_pack_author_test`,
and renders the returned report.

The adapter preserves raw bytes and separate raw/canonical bindings. It does not
infer trust or lifecycle authority from either API and cannot reach network,
subprocess, signing, publication, reviewed-registry mutation, lifecycle commit,
installation, activation, or Forge state mutation. Concurrent same-principal
filesystem namespace replacement is deferred to the C3.2 hardening campaign.

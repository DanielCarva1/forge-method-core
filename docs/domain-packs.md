# Domain Packs

Domain Packs expose domain-specific unknowns and methods without hard-coding
them into universal Rust core.

## Contributions

A schema-0.1 pack may declare namespaced policies, obligations, claims,
evaluators, advisory playbooks, hazards, lifecycle models, fixtures, domains,
capabilities, and adapters. These remain candidate data until every required
trust and lifecycle boundary is satisfied.

## Authority ladder

```text
manifest/content sidecars
  -> candidate validation
  -> deterministic composition/resolution (still untrusted)
  -> publisher + registry supply-chain verification
  -> independent semantic review + registry authorization
  -> exact capability/sandbox/project compatibility preflight
  -> immutable governed generation
  -> workflow effective epoch
```

Copying a fixture or changing `authority:` in YAML cannot skip this ladder.

## Package and state locations

Candidate manifest/content/package bytes are read only from the explicit
`--artifact-root`; that root does not become trusted storage. On admitted
lifecycle apply, exact raw bytes are copied under
`<state-root>/domain-packs/objects/<digest-token>`, complete immutable generation
records are published under
`<state-root>/domain-packs/generations/<generation>-<record-token>/`, and
`<state-root>/domain-packs/active.lock.yaml` selects the active generation.
Supply-chain/review documents and monotonic anchors live under an explicit
operator-owned root outside project/artifact/state controlled roots. See the
[storage table](operator-guide.md#state-and-ownership).

## Authoring rules

- Own one namespace; never shadow sealed core IDs.
- Bind raw bytes and canonical semantics separately.
- State compatibility and dependencies exactly.
- Declare capabilities; never claim runtime availability.
- Require representative evidence for consequential behavior.
- Include adverse/ablation fixtures proving load-bearing gates.
- Preserve explicit gaps when a pack is absent or removed.
- Keep local learning inert until promotion, independent review, and registry
  authorization complete.

These are the obligations of the Domain Pack extension path. Host integrations
and core authority forks have different obligations; see the three separate
[fork/customization paths](forking.md).

## Reference corpus

`docs/fixtures/domain-pack-reference-v0/` contains the governed
`forge.reference/game-development` proof. It exercises a novel-domain journey
through generic core code. It is an executable reference corpus, not a globally
trusted registry or automatically installed pack.

`docs/fixtures/domain-pack-v0/` contains a smaller neutral composition corpus.
Adversarial fixtures demonstrate namespace, authority, trust, and lifecycle
failures.

From a source checkout, the safe read-only reference checks are:

```bash
forge-core domain-pack validate \
  --manifest-file docs/fixtures/domain-pack-reference-v0/manifests/game-development.yaml \
  --content-file docs/fixtures/domain-pack-reference-v0/content/game-development.yaml \
  --artifact-root . --json

forge-core domain-pack compose \
  --request-file docs/fixtures/domain-pack-reference-v0/requests/agent-built-game.yaml \
  --artifact-root . --json
```

Success proves the exact candidate/composition boundary only. It does not
install the fixture or make its publisher/reviewer identities trustworthy.

## Demand discovery

P7c discovery starts from a host-proposed, typed demand carrying the exact
`DurableAssuranceEpochBinding` reconstructed from the accepted-intent ledger
record. The binding includes project, intent revision/digest, assurance epoch,
snapshot, sequence, state version, and ledger-head digests. The request also
carries exact reviewed registry entries and matching content documents; core
performs no model call,
keyword classification, network search, package selection, or lifecycle write.

```bash
forge-core domain-pack search \
  --request-file contracts/domain-pack-discovery/neutral-reviewed-match.yaml \
  --json
```

Discovery authority documents are byte-bounded and must be YAML-anchor/alias
free so deserialization cannot amplify a small input into unbounded material.
The projection is deterministic and `candidate_only`. Its demand digest binds
the normalized requirements, provenance, uncertainties, and accepted-intent
binding while remaining independent of candidate input order. Every match
retains the exact package, supply-chain record, reviewed-entry, and content
digests. A
requirement without a qualifying reviewed candidate remains a typed gap with a
next action. Persisted projections can be inspected without re-running search:

```bash
forge-core domain-pack explain \
  --projection-file <search-data.yaml> \
  --requirement-ref <requirement-id> \
  --json
```

`explain` validates projection invariants and its integrity digest first. The
binding must come from the host's current governed projection; copying or
fabricating binding fields cannot grant authority. `domain-pack acquire plan`
requires the original request as well as the projection, deterministically
replays discovery, and rejects any mismatch before selecting an exact
`candidate_id`. The resulting self-digested plan carries the normalized project
requirements needed by later resolver derivation. The pure acquisition
preparation layer can already join exact package material to that plan and
produce integrity-checked P6 resolution and composition inputs; those outputs
remain `candidate_only` and do not bypass the resolver. The plan always lists the outstanding operator, supply-chain,
reviewed-registry, capability, and lifecycle ceremonies.

`domain-pack acquire prepare` combines the original intent/request/projection
with one release/catalog material document and replays both planning and P6
input derivation; this avoids caller-authored resolver/composer documents.
`domain-pack acquire apply` is the separate mutating boundary. It requires that
prepared input, the exact current candidate id repeated as explicit operator
approval, fresh external supply-chain/reviewer roots, runtime and sandbox
policy, raw package artifacts, project snapshot CAS, and a principal.
Forge derives the resolution, composition, trust input, exact lock, preflight,
and lifecycle request internally; callers do not author those lifecycle
internals. The lifecycle TCB replays every decision before activating the first
immutable generation. It is clean-install-only: initialized projects continue
to use explicit upgrade, rollback, or remove operations.

Search, explain, planning, and pure acquisition preparation grant no trust,
download, installation, or activation authority. `contracts/domain-pack-discovery/`
includes neutral, uncovered-gap, and game-domain inputs; game behavior is
fixture data rather than a Rust branch. Only the explicit apply path can reach a
new effective epoch, and only after all five ceremonies succeed.

## Operator lifecycle

Use `domain-pack validate|compose|resolve` for read-only candidate work. Use
`status|recover|preflight|apply` only with explicit operator trust material.
Install, upgrade, rollback, and remove are intent-specific and CAS-bound.
Remove-to-empty may correctly produce a degraded generation with typed gaps.

An active generation blocks ordinary `workflow release-upgrade`. For an
admitted adjacent Core, `workflow release-status` returns an exact
`release-rebase-apply` argv bound to the workflow head, project snapshot,
lifecycle pointer/head, generation, exact lock, composition, and operator
registry heads. Rebase reuses persisted candidate-only inputs, revalidates the
external trust/review/capability roots, re-resolves and recomposes against the
target Core, and rejects any package or gap drift. The lifecycle pointer commits
first; one `core_domain_pack_rebased` workflow record then advances Core and
effective identities together. A persisted exact plan lets a replacement
process finish the joined event after a crash, while mixed pairs remain
non-admissible.

See the generated [command surface](generated/command-surface.md) for exact
flags and the [operator guide](operator-guide.md) for state/trust ownership.

The reference fixture deliberately contains no deployable private keys, signed
public registry, or globally trusted reviewer root. Production pack
authoring/signing/publishing is not yet a polished public SDK journey; do not
manufacture authority by copying test-generated material. The
[product compliance audit](product-compliance-audit.md) tracks that remaining
productization gap.

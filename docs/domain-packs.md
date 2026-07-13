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

## Operator lifecycle

Use `domain-pack validate|compose|resolve` for read-only candidate work. Use
`status|recover|preflight|apply` only with explicit operator trust material.
Install, upgrade, rollback, and remove are intent-specific and CAS-bound.
Remove-to-empty may correctly produce a degraded generation with typed gaps.

See the generated [command surface](generated/command-surface.md) for exact
flags and the [operator guide](operator-guide.md) for state/trust ownership.

The reference fixture deliberately contains no deployable private keys, signed
public registry, or globally trusted reviewer root. Production pack
authoring/signing/publishing is not yet a polished public SDK journey; do not
manufacture authority by copying test-generated material. The
[product compliance audit](product-compliance-audit.md) tracks that remaining
productization gap.

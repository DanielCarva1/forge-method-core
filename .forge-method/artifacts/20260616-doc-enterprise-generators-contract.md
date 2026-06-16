# Document And Enterprise Generators Contract

- created_at: 2026-06-16T00:19:10+00:00
- status: doc-enterprise-generators-added
- workflow: runtime-builder
- lifecycle: durable

## Problem

`doc-check` and `enterprise-check` had stable contracts, but agents still had to hand-write the artifacts that feed them. That preserved too much ambiguity in two places where Forge needs to be strict: document source-of-truth freshness and enterprise evidence gates.

## Runtime Contract

- `artifact doc-index` writes and registers a durable document index artifact, computes local source fingerprints and mtimes when omitted, defaults validation to `artifact doc-check --path <artifact>`, and rejects stale or incomplete handoffs.
- `artifact doc-shard` writes and registers a durable shard handoff with generated docs, shard index, original document decision, precedence rule, stale waiver, and `doc-check` validation.
- `artifact enterprise-track-map` writes and registers an enterprise track map with baseline required artifacts, conditional artifacts, evidence map, readiness gate, waiver policy, and `enterprise-check` validation.
- `artifact enterprise-readiness` writes and registers an enterprise readiness matrix with evidence status, NFR evidence, release impact, waivers, missing sources, and `enterprise-check` validation.
- `artifact enterprise-release-gate` writes and registers an enterprise release gate with gate decision, evidence status, release impact, waivers, and `enterprise-check` validation.

## Human Contract

Human-facing guidance now routes document and enterprise closure toward command-backed handoff instead of "write a compact artifact manually":

- document utility asks what the doc job is, which file owns truth, what is stale or duplicated, and how freshness is proved;
- lifecycle closure makes enterprise gates explicit: required artifacts, evidence consumers, waiver policy, and release impact;
- generated artifacts remain compact enough for future agents to load without replaying chat.

## Agent Contract

- Workflow refs name the generator first and the validator second.
- Facilitation packs keep richer questions and quality bars.
- Source and install smokes prove the commands work from both checkout and packaged skill.

## Validation

- `python -m unittest tests.test_runtime.RuntimeTests.test_artifact_document_generators_create_index_and_shard_with_freshness_proof -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_artifact_enterprise_generators_create_track_readiness_and_release_gates -v`
- `python -m unittest tests.test_runtime.RuntimeTests.test_packaged_modules_and_workflows_validate -v`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
- `python -m unittest discover -s tests`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Next Gap

Continue the post-parity audit by checking whether any remaining validators still require hand-written artifacts where a generator would improve human guidance or agent reliability.

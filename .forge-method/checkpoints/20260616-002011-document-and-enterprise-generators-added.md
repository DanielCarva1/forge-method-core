# Document and enterprise generators added

- created_at: 2026-06-16T00:20:11+00:00
- project: forge-method-core
- phase: 6-evolve
- status: doc-enterprise-generators-added
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact doc-index, artifact doc-shard, artifact enterprise-track-map, artifact enterprise-readiness, and artifact enterprise-release-gate so document freshness and enterprise gate closeouts are generated, registered, and validated before downstream handoff.

## Decisions

- Use first-class runtime generators for document freshness and enterprise evidence gate artifacts where validators already define stable contracts; keep rich human source-of-truth and gate questions in packs and compact state-machine handoff in workflow refs.

## Checks

- document generator test passed; enterprise generator test passed; packaged workflow validation test passed; workflow validate passed; workflow compactness passed; parity replay 90/90 passed; smoke-runtime.ps1 passed; smoke-install.ps1 passed; python -m unittest discover -s tests passed; verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/facilitation/document-utility.md
- skills/forge-method/facilitation/lifecycle-closure.md
- skills/forge-method/references/workflow-doc-index.md
- skills/forge-method/references/workflow-doc-shard.md
- skills/forge-method/references/workflow-track-decision.md
- skills/forge-method/references/workflow-readiness-check.md
- skills/forge-method/references/workflow-release-readiness.md
- tests/test_runtime.py
- scripts/smoke-runtime.ps1
- scripts/smoke-install.ps1
- CHANGELOG.md

## Artifacts

- .forge-method/artifacts/20260616-doc-enterprise-generators-contract.md

## Next Action

Continue post-parity Forge polish by auditing any remaining validator-only artifacts and optimizing slow Guidance Engine fixture replay.

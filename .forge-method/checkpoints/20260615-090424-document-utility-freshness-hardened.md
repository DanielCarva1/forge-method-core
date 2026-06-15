# Document Utility Freshness hardened

- created_at: 2026-06-15T09:04:24+00:00
- project: forge-method-core
- phase: 6-evolve
- status: document-utility-freshness-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Added artifact doc-check and tightened doc-index/doc-shard contracts so documentation utility work records source fingerprint, source mtime, stale-check proof, original-document handling, precedence rules, and stale waivers.

## Decisions

- Index/shard parity is now represented as a Forge-native freshness validation contract rather than only facilitation prose.

## Checks

- parity replay, workflow validation, compactness, config validation/index, unittest, smoke-runtime, smoke-install, and verify-fast passed.

## Failed Checks

- none

## Touched Files

- Guidance Engine document routing, artifact doc-check runtime command, doc-index/doc-shard workflows, document-utility pack/template, catalog modes, replay fixtures, benchmark/audit/plan/changelog, and runtime tests.

## Artifacts

- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md

## Next Action

Continue residual real-use transcript hardening; expand API/browser or eval-runner surfaces only if repeated projects justify them.

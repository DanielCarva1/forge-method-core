# Enterprise Artifact Map Depth hardened

- created_at: 2026-06-15T10:16:14+00:00
- project: forge-method-core
- phase: 6-evolve
- status: enterprise-artifact-map-depth-hardened
- workflow: runtime-builder
- active_story: <none>

## Summary

Closed the enterprise track/readiness gap by making enterprise projects carry explicit required and conditional artifact maps through track-decision, readiness-check, and release-readiness. Added artifact enterprise-check, enterprise templates, catalog metadata, workflow contracts, facilitation depth, replay fixtures, and benchmark/audit/changelog updates.

## Decisions

- Enterprise routing stays narrow: phrases like enterprise artifact map/readiness map route to lifecycle, but enterprise alone does not override quality routing.
- Enterprise baseline artifacts are risk-register, security-plan, privacy-data-plan, test-strategy, ci-quality-pipeline, nfr-evidence-audit, traceability-gate, and release-readiness; DevOps, compliance, and observability are conditional artifacts that must be named or explained.

## Checks

- Targeted unittest: 7 tests OK
- Parity replay: 82/82 passed
- Workflow validate, compactness, and config validate passed
- python -m unittest discover -s tests: 75 tests OK
- smoke-runtime.ps1, smoke-install.ps1, and verify-fast.ps1 passed

## Failed Checks

- none

## Touched Files

- skills/forge-method/scripts/forge_method_runtime.py
- skills/forge-method/catalog/workflows.json
- skills/forge-method/references
- skills/forge-method/facilitation
- skills/forge-method/templates
- skills/forge-method/fixtures/guidance-parity-replay.json
- tests/test_runtime.py

## Artifacts

- .forge-method/evidence/20260615-101549-validation-enterprise-artifact-map-depth-validation.md
- .forge-method/artifacts/20260612-bmad-forge-systematic-parity-audit.md
- .forge-method/artifacts/20260613-systematic-parity-plan.md
- .forge-method/artifacts/guidance-engine-benchmark.md

## Next Action

Continue residual parity hardening; prioritize bmad-spec depth and research/game brief strong-ish rows where transcript evidence still shows drift.

# Team Collaboration, Product Areas, And Repo Split Work Order

## Summary

Forge Method Core 1.34.0 adds guided support for multiple people using Forge on the same product. The release keeps `Forge Module` reserved for packaged runtime/workflow capabilities and introduces `Product Area` as the product-facing ownership and modularization boundary.

## Decisions

- Default to a monorepo-first Root Integrator Project.
- Route multi-human, GitHub org, trunk-based, CODEOWNERS, Product Area, handoff, monorepo, multi-repo, split repo, and standalone repo language through collaboration guidance before build work.
- Split a Product Area into a separate repo only after owner, contract, validation boundary, release boundary, and integration cost are explicit.
- When a split happens, seed the extracted repo as a Standalone Method Project with compact context and keep root integration evidence in the Root Integrator Project.
- Keep solo projects lightweight by making Product Area, owner, branch, PR, dependency, and handoff fields optional.

## Required Runtime Surfaces

- `team-operating-model`
- `product-area-map`
- `trunk-based-plan`
- `collaboration-handoff`
- `repo-split-plan`
- `facilitation/collaboration.md`
- Product Area and collaboration fields in story, sprint, status, and build work templates
- Guidance Engine routing coverage and parity replay fixtures

## Validation Plan

- `python scripts\test-runner.py --workers 4 --timeout 120 --report .forge-method\test-runs\manual.json`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`

## Release Target

Version `1.34.0`.

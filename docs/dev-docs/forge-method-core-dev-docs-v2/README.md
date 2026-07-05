# Forge Method Core v2 - audited development package

Date: 2026-06-28
Scientific window applied for papers: 2025-10-28 to 2026-06-28

This package transforms the audited research, the sources used, and the architectural recommendations into development documentation for `forge-method-core`.

## How to use

1. Read `00_master_development_doc.md` for the general direction.
2. Use `01_feature_specs.md` as the product and engineering backlog.
3. Use `02_implementation_plan.md` to sequence the implementation.
4. Use `03_architecture_and_contracts.md` for the new contracts.
5. Use `04_rust_refactor_guide.md` before asking agents to touch the Rust.
6. Use `05_eval_and_quality_plan.md` to turn research into gates, evals, and CI.
7. Use `06_protocol_security_plan.md` for MCP, A2A, identity, and governance.
8. Use `adrs/` to record initial technical decisions.
9. Use `data/` to import the backlog and evidence ledger into an issue tracker, spreadsheet, or dashboard.
10. Use `schemas/` as a draft of v0 YAML contracts.

## Central premise

Forge should not try to be the best agent. Forge should be the kernel of coordination, verification, and governance that allows using any agent with contract, evidence, gates, rollback, trace, and audit.

## Non-objectives

- Do not turn multi-agent into a product default.
- Do not move live semantics into hand-written Rust before stabilizing the contract.
- Do not accept MCP/A2A as safe by default.
- Do not treat memory as a vector DB without a policy.
- Do not sell speed without preview, verify, and undo.

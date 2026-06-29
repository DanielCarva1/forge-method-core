# Forge Core M0 ZIP reconciliation

Status: accepted planning checkpoint
Date: 2026-06-29
Branch: `codex/forge-m0-sidecar`
Scope: reconcile `forge-method-core-dev-docs-v2 (1).zip` against the current repository before starting M1 code.

## Verdict

Do not start M1 implementation from the ZIP verbatim.

The safe next state is:

1. Keep the extracted ZIP package as reference input under `docs/dev-docs/forge-method-core-dev-docs-v2/`.
2. Treat this reconciliation file as the authority bridge between the imported package and the current repo.
3. Close the sidecar M0 branch first.
4. Start M1 as a thin additive implementation of preview, ready, trace, and explain over the current runtime planner.
5. Defer broad Rust ergonomics refactors from the ZIP M0 unless they are split into a separate branch.

## Imported package inventory

Extracted from root ZIP into:

`D:\Forge-method-core\docs\dev-docs\forge-method-core-dev-docs-v2\`

The package contains 26 files:

- product/development docs:
  - `00_master_development_doc.md`
  - `01_feature_specs.md`
  - `02_implementation_plan.md`
  - `03_architecture_and_contracts.md`
  - `04_rust_refactor_guide.md`
  - `05_eval_and_quality_plan.md`
  - `06_protocol_security_plan.md`
  - `07_product_and_user_profiles.md`
  - `README.md`
- proposed ADRs under `adrs/`
- data tables under `data/`
- draft schemas under `schemas/`

## Current repo state checked during reconciliation

Current implemented surface:

| Area | Current status |
| --- | --- |
| Claim governance / `check-write` | Implemented. Owner write allowed; peer overlap and unclaimed writes rejected. |
| Project sidecar resolution | Implemented. `.forge-method.yaml` resolves sidecar state; core has explicit bootstrap-local exception. |
| Validation | Implemented. `forge-core validate` exists and validates current contract/reference surface. |
| Guide/status | Implemented. Workflow catalog/status/decision surface exists. |
| Trace | Partial. Telemetry contract exists, but no canonical `TraceEvent` runtime crate/CLI path yet. |
| Preview | Partial. Workflow concept exists; no top-level deterministic `preview` command yet. |
| Ready | Partial. Runtime readiness concepts exist; no top-level fail-closed `ready` command yet. |
| Explain | Not implemented as concrete command. |
| WorkflowGraph | Not implemented as first-class runtime/CLI graph. |
| Eval compare | Partial eval contracts/engine exist; no top-level `eval compare` runner. |

Operational state:

- Core resolves as bootstrap exception: `D:\Forge-method-core\.forge-method`.
- Darkest resolves through sidecar: `D:\darkest-roguelite\forge-darkest-roguelite\.forge-method`.
- Old nested `D:\Forge-method-core\darkest-roguelite\` is empty only.
- `D:\Forge-method-core\hostfully-related\` is also an empty local remnant.

## Forge promises extracted from the imported docs

The imported docs position Forge as a protocol/kernel that should provide:

1. Deterministic coordination/governance, not another agent model.
2. Safe mutation: preview before write, explicit authority, gates, evidence, rollback/undo where applicable.
3. Fail-closed readiness instead of optimistic green output.
4. Traceability: machine-readable run trace plus human-readable explain output.
5. Typed contracts as source of truth; agents/adapters/context may suggest, but do not become authority.
6. Graph-based orchestration only after a traceable single-operation kernel exists.
7. Single-agent baseline before recommending multi-agent orchestration.
8. Memory policy that prevents summaries from silently becoming authority.
9. Secure MCP/A2A adapters that cannot mutate state without validated authority.
10. Multi-principal governance for shared state across humans/agents/teams.

## Evidence and community/research signal in the package

The package includes `data/evidence_ledger.csv` and `data/research_to_product_matrix.csv` tying features to papers, official protocol references, and market/community signals.

Important signals to preserve:

- Multi-agent orchestration should not be default merely because it is fashionable; compare against strong single-agent baselines first.
- Developer demand clusters around runtime integration, dependency management, orchestration complexity, and evaluation reliability.
- Community/product demand is strongest for control, integration, observability, governance, and trust boundaries.
- MCP/A2A integration must be protocol-adapter work, not a backdoor into store mutation authority.
- Memory is a governance problem, not just retrieval.

This supports M1 -> M2 -> M3 sequencing: trace/preview/ready first, graph second, eval/budget third.

## Reconciliation against the sidecar decision

The imported docs predate or omit the new sidecar model. Adjust them as follows:

| Imported assumption | Reconciled rule |
| --- | --- |
| Tools/control plane read `.forge-method` directly. | Always resolve `state_root` via `forge-core project resolve` first. |
| Governance schema hardcodes `.forge-method/governance/arbitration.ndjson`. | Treat paths as relative to resolved `state_root`. |
| Guided start creates a Forge project. | Guided start must create `.forge-method.yaml` plus sibling sidecar for consumer repos. |
| New contracts do not mention project context. | Trace/graph/eval/memory/governance must carry or derive `project_id`, `project_root`, and resolved `state_root`. |
| Imported ADR numbering starts at ADR-0001. | Do not copy ADR numbers verbatim; repo already has `docs/adr/0001-forge-runtime-sidecar.md`. |

## Rejected or modified imported guidance

The ZIP recommends some broad M0 refactors. They are not adopted as immediate prerequisites:

| Imported item | Decision | Reason |
| --- | --- | --- |
| Add `thiserror` | Rejected for this repo | `AGENTS.md` forbids `anyhow`/`thiserror`; use manual enums. |
| Full `clap` migration | Defer | Useful later, but too broad before M1 and touches slow CLI surface. |
| Store module split | Defer | Useful ergonomics work, but high blast radius and not needed for M1 vertical. |
| Add tracing dependency everywhere | Defer/consider separately | M1 canonical trace should be explicit contract/NDJSON first; dependency creep needs review. |
| Codegen/snapshots everywhere | Defer except focused M1 snapshots | Good direction, but not a prerequisite for thin M1. |

## M0 closure checklist

M0 sidecar branch can be considered operationally closed after:

- [x] Claim hardening committed.
- [x] Sidecar/project-link resolver committed.
- [x] Global `$forge-method` startup updated for sidecar resolution.
- [x] Darkest state relocated to sibling sidecar and resolves correctly.
- [x] ZIP extracted to versionable docs reference.
- [x] This reconciliation checkpoint exists.
- [x] Original root ZIP removed after extracted docs were preserved under `docs/dev-docs/`.
- [ ] Remove empty local remnant directories if Windows releases locks.
- [x] Extracted docs + reconciliation planning docs committed in `9a7bcec`.
- [ ] Release active Forge claim.

## Decision for next feature branch

Next feature epic remains M1, but only after M0 closure commit:

- Branch: `codex/forge-m1-kernel`
- Scope: thin additive preview/ready/trace/explain vertical.
- Non-goals: graph executor, eval compare, memory policy, protocol adapters, full CLI refactor, store split.

See `docs/planning/forge-core-m1-subagent-implementation-plan.md`.



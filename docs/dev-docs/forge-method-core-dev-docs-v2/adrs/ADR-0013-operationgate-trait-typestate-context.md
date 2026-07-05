# ADR-0013 - `OperationGate` trait + typestate Context (gates in the kernel)

- **Status**: Accepted (V2.C seam implemented — trait `OperationGate`, typestate `Context<Unverified>`/`<Audited>`, feature `dangerous-bypass`; V3.A filled it in with `RiskAuditGate`/`CitationGate` in `builtin_gates.rs`)
- **Date**: 2026-07-02
- **Track**: V2.C / V3.A — kernel owns its pre-WAL gates
- **Supersedes**: none
- **Superseded by**: none

## Context

The pre-WAL gates (risk-audit and citation) lived in `execute_operation.rs` — inside the
**CLI** — and were gated only by argv flags (`--require-risk-audit`, `--require-citation`).
A kernel caller that was not the CLI — a test that invokes `execute_operation` directly,
or a future in-process MCP server — **silently bypassed them**. Nothing in the
`execute_operation` signature required the checks to have run beforehand.

This directly violated ADR-0024: the kernel is the **only PDP (Policy Decision Point) for
mutation**. If the mutation preconditions are enforcement points living in the CLI caller,
then a different caller has a different kernel — the very property that ADR-0024 pinned as
unrepresentable. Mutation could proceed without anyone having consulted the gate.

## Decision

### 1. `trait OperationGate` in the kernel — synchronous, Tower-inspired

```rust
pub trait OperationGate {
    fn evaluate(&self, plan: &RuntimePlan) -> Result<(), GateRejection>;
    fn name(&self) -> &'static str;
}
```

Modeled on Tower's `Service`/`Layer` pattern, but **synchronous (no async)** — honoring
the deterministic kernel constraint of ADR-0001. It is object-safe (`&self`, owned data):
the context stores the chain as `Vec<Box<dyn OperationGate>>`. A gate is a PEP (Policy
Enforcement Point) that the kernel consults; the PDP (the risk-audit ruleset, the citation
policy) is what backs the gate. The kernel does not know what a gate checks — it only knows
that it passes or rejects with a typed `GateRejection`.

`GateRejection` is an enum (`RiskAuditFailed { error_count, finding_paths }`,
`CitationCheckFailed { unresolved_source_ids }`, `Custom { code, message }`): it carries
enough structure for the envelope (V2.D / `TypedFailure`) and the MCP consumer to branch,
not just a string.

### 2. Typestate `Context<Unverified>` vs `<Audited>`

`RuntimeOperationExecutionContext` gains a state parameter `<S>`:

- **`Unverified`** — the context has not yet passed through the gate chain. **Cannot call
  `execute_operation`.** Built by `single_root`.
- **`Audited`** — the context has passed through the chain configuration. **Only this one
  can call `execute_operation`.**

The transition is `audited()` (after `.with_gate(Box::new(...))`). The signature of
`execute_operation` takes `&Context<Audited>` — the typestate makes "the gates were
configured" loud at the type level. The planner is invoked before the gate chain, so the
gate receives the `RuntimePlan` (what WILL happen), read-only.

### 3. `execute_operation` runs the gate chain internally, before the WAL

The `execute_operation` preamble consults each gate in attachment order, against the plan:
the first to reject wins (fail-closed). The rejection blocks the entire WAL append — the
mutation has no effect. This runs **before** the kernel's own `OperationContract`
authorization and does not replace it.

### 4. The two gates become `impl OperationGate` in `builtin_gates.rs`

`RiskAuditGate` and `CitationGate` are public structs in the kernel's `builtin_gates`
module. Each carries only config + the pre-resolved data (ruleset, evidence registry,
runtime source ids, trace identity) and calls the **unmodified** evaluator in
`forge-core-validate`
(`evaluate_risk_audit`, `validate_yaml_citation_references`). The risk-audit gate emits its
own `TraceEvent`s so that `forge explain` can narrate the audit.

### 5. `.dangerous_unchecked()` — escape hatch, never silent

The explicit bypass follows `rustls`'s `dangerous()` pattern: only available under the
`dangerous-bypass` feature flag, and it emits a `tracing::warn!`. A bypass is **visible in
the diff AND in the feature config** — never silent. For tests/legacy callers that
genuinely do not need gates. Real callers should prefer `audited()`.

### 6. CLI flags become config, not location

`--require-risk-audit` / `--require-citation` decide **which gates to attach**, not
**where** the check runs. The CLI still loads/parses the ruleset and resolves the runtime
half of the citation gate's Source Ledger (because the kernel cannot depend on
`forge-core-research` — research depends back via store/validate); those pre-resolved
inputs enter as **data** into the gate structs.

## Rationale (the real trade-off)

The alternative — leaving the gates in the CLI and adding a boolean `gates_run: bool` flag
to `execute_operation` — was rejected: a flag is runtime state that any caller can omit or
lie about. The typestate moves "the gates were configured" to the type level: the compiler
proves that `execute_operation` is only reachable from a `Context<Audited>`. The
`.dangerous_unchecked()` is the only path to `Audited` without gates, and it is
feature-gated — so a production binary (without the feature) physically cannot bypass.

## Consequences

**Positive:**

- The kernel **owns** its pre-WAL gates. A test or in-process MCP caller that invokes
  `execute_operation` directly must configure the gate chain to reach `Audited`;
  there is no silent path (default binary: no `dangerous-bypass`, no bypass).
- CLI flags become config (which gates to attach), not location. ADR-0024 is honored: the
  kernel is the only PDP for mutation, and now this is enforced at the type level, not in
  caller discipline.
- The `.dangerous_unchecked()` escape hatch makes any bypass visible in the diff (the call
  appears) AND in the feature config (`--features dangerous-bypass`). A reviewer who sees
  `dangerous_unchecked` knows exactly what is happening.
- `GateRejection` flows into `TypedFailure` (V2.D) in the envelope — the MCP consumer
  branches without parsing prose.

**Negative:**

- The context is no longer `Copy` (it owns `Vec<Box<dyn OperationGate>>`). Callers pass by
  reference to `execute_operation`. Accepted: historical call sites keep `single_root`
  (returns `Unverified`) and add `.audited()`.
- The citation gate needs the runtime half of the Source Ledger, which the kernel cannot
  resolve (cyclic dependency via research). Trade accepted: the CLI resolves it and passes
  it as data (`runtime_ids: HashSet<String>`) — the kernel stays decoupled from
  `forge-core-research`.

## Anti-goals

- **Does not** introduce async (Tower `Service` is async): the kernel stays deterministic
  (ADR-0001).
- **Does not** remove the `--require-risk-audit`/`--require-citation` CLI flags — they
  become config for which gates to attach.
- **Does not** replace the kernel's `OperationContract` authorization; the gates run before
  and are complementary.

## References

- Tokio `Service`/`Layer` (Tower) — synchronous pattern here: https://docs.rs/tower
- `rustls` `dangerous_configuration` (explicit escape hatch):
  https://docs.rs/rustls/latest/rustls/server/struct.ServerConfig.html
- Cliff Biffle — typestate pattern in Rust (Rust as a Language for Writing Real Things).
- In-repo: ADR-0001 (deterministic kernel, no async), ADR-0024 (kernel is the only PDP for
  mutation),
  `crates/forge-core-kernel/src/{gate.rs, builtin_gates.rs, wal_orchestration.rs (typestate)}`,
  ADR-0014 (rename `runtime`→`kernel`).

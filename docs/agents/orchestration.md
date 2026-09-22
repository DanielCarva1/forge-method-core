# Selective orchestration and continuity agreement

Accepted by the maintainer in the current conversation on 2026-09-19.
This is a host-agent working agreement, not a product plan, runtime receipt,
new governance engine, or guarantee of savings. Product sequencing remains in
`docs/development-plan.md`; desktop session continuity remains in
`apps/desktop/README.md`. Later explicit human instructions take precedence.

## Responsibility and autonomy

- The human intends to keep one long conversation, including context compactions.
  Do not require new chats, manual model switching, or worker setup per task.
- The parent owns decomposition, model selection, dispatch, rerouting, integration,
  verification, cost accounting, and the final user-visible result.
- Delegate selectively to Luna, Terra, or Sol when justified; Astra may do the
  work directly. Explicitly select the worker model and reasoning effort rather
  than relying on parent inheritance. Use compact task-local context when model
  overrides are incompatible with a full-history fork.
- A successful spawn confirms dispatch, not final model usage or task success.
  Record requested versus observed model identity when telemetry permits.
- Do not promise quality equivalence, a savings percentage, or perfect recall.
  Report unavailable evidence rather than substituting a confident estimate.
- Preserve existing approval boundaries for publication, irreversible effects,
  and material product/risk decisions. No automatic commit, push, or publication
  is authorized by this agreement.

## Initial routing policy (hypotheses, not benchmark conclusions)

| Work characteristics | Initial choice |
| --- | --- |
| Local, repeatable, established pattern, objective acceptance | Luna |
| Bounded investigation or moderate integration | Terra |
| Coupled modules, difficult diagnosis, substantial implementation | Sol |
| Architectural ambiguity, high risk, unresolved hard problem | Astra |

Size is not risk: small persistence, concurrency, or security changes may need
a stronger model. Prefer one coherent executor. Use parallel workers only for
genuinely independent responsibilities. No recursive fan-out by default.
The cheapest token price is not automatically the cheapest accepted delivery.

Before dispatch, provide outcome, acceptance criteria, owned paths/responsibility,
invariants, relevant evidence, verification commands, and stop/escalation rules.
Workers must not revert unrelated work and must accommodate concurrent changes.
After two attempts without material progress, stop and reassess routing or scope;
escalate sooner for security, data-loss, or contract uncertainty. Do not conceal
parent rewrites or repeated failed attempts from cost/rework accounting.

## Quality boundary

Apply the same acceptance criteria regardless of model. Follow `/eng` before
non-trivial mutations. Inspect the integrated diff and use executable checks or
runtime readback appropriate to the risk, not only the worker's success summary.
Model review is cooperative evidence, not independent proof. Avoid duplicate
investigations and repeated reviews that do not address a concrete risk.
Report PASS, FAIL, NOT_RUN, or PARTIAL with actual evidence and limitations.

## Focused Rust verification and alpha delivery

Maintainer clarification, 2026-09-19: ship coherent alpha building blocks, not
every story separately and not only after the whole product is complete.
Publication means the included capabilities are available and verified within
their declared scope; it does not imply full-product readiness. Keep unfinished
features and known limitations explicit. Installation/distribution/update work
is still required, not indefinitely deferred. Preserve publication approval.

Use this Rust feedback loop after the required engineering investigation:

1. Edit the affected source.
2. `cargo check -p <affected-crate>`.
3. `cargo test -p <affected-crate> <module-or-test-filter>`.
4. Fix and repeat the focused loop as needed.
5. Run `cargo test -p <affected-crate>` when the slice stabilizes.
6. Run related tests and only then relevant integration tests.

Do not run workspace-wide check/test/clippy or release builds for every small
edit. Reserve the full applicable workspace, Clippy, complete tests and release
build for final pre-merge/package validation, with any wider scope justified.
Do not omit an applicable required CI gate; run it at the appropriate boundary.
Documentation-only changes do not need Rust compilation.

Desktop has its own workspace: use
`--manifest-path apps/desktop/src-tauri/Cargo.toml -p forge-desktop` with the
commands above. A desktop-only change does not automatically require rebuilding
the separate core workspace. Reuse the documented build cache and locked/offline
dependencies when available. Verify filtered tests actually execute relevant
cases; a successful command selecting zero tests is not behavioral evidence.

## Honest economics

Optimize cost per accepted delivery under unchanged quality requirements.
Count parent planning, coordination, worker execution, review, retries,
integration, and repairs, not only the cheapest worker's successful attempt.

Keep three distinct quantities:

1. **Observed usage:** provider/host token counters and model attribution, with
   scope, timestamps, completeness, and counter semantics established first.
2. **API-equivalent estimate:** observed tokens weighted by dated model tariffs
   and a sourced, dated USD/BRL rate. Not an invoice or Pro quota conversion.
3. **Subscription economics:** observed aggregate allowance consumption and
   accepted outcomes per paid period. Concurrent work, rounded percentages,
   resets, and changing rate rules can prevent per-task attribution.

Never sum cumulative snapshots. Establish whether cached input is a subset of
input, and reasoning output a subset of output, before computing costs. Do not
double-count inherited cumulative counters, overlapping parent/child usage, or
duplicate events. Repeated input actually charged on separate requests still
counts (at its applicable cached/non-cached rate); do not deduplicate prompt text.
Separate pre-task history from the task's own baseline-to-end interval.
If model changes, segment usage by model; do not label a whole conversation with
its last selected model. Missing/unattributable data is UNKNOWN, not zero.

For each completed pilot slice, retain compact evidence in the existing desktop
checkpoint (or a referenced sanitized evidence artifact):

- scope, strategy, requested/observed models and roles;
- bounded baseline/end times and available usage evidence references;
- token categories, completeness and aggregation limitations;
- API-equivalent BRL estimate only when tariff/FX/usage inputs are established;
- account allowance snapshots, shared-work and rounding caveats;
- acceptance result, tests/runtime evidence, retries, parent rework and elapsed time;
- decision: keep routing, increase model capability, or execute directly.

Compare with both Astra-only and Sol-only where comparable evidence exists.
Do not claim causal savings from unrelated tasks or invent a counterfactual.
Do not duplicate full implementations merely to benchmark them without a bounded
experiment being agreed. Do not build a dashboard/framework before confirming
usable measurement. If supervision erases savings, abandon that delegation.

## Compaction and recovery protocol

Before a planned handoff or at each completed atomic slice, update the existing
session checkpoint with: accepted objective, current phase, changed files,
commands/evidence, active worker IDs and ownership, measurement boundaries,
unknowns/blockers, and exactly the next smallest step.

After compaction/reset, read in this order:

1. Root `AGENTS.md` and this agreement.
2. Latest applicable session checkpoint (desktop: `apps/desktop/README.md`).
3. `git status` and current branch/commit; preserve pending changes.
4. Active-worker status if the checkpoint records unfinished workers.
5. Relevant evidence and canonical product/runtime authority before implementation.

Do not reconstruct settled decisions from memory alone or repeat finished work.
Complete a small verifiable unit before expanding scope. If repeated compactions
produce no completed unit, stop broad exploration, checkpoint, and narrow scope.
Do not claim a compaction/reset occurred unless actually observed. Automatic
compaction timing and perfect retention are not controlled by the parent;
the durable files and entry-point link are the recovery mechanism.

## First bounded slice

Record this agreement and inspect available usage telemetry only. One Luna
read-only investigator, at most six tool calls, no nested agents or installations.
Parent owns documentation and verification; no app implementation in this slice.
No BRL spending cap is claimed enforceable before metering is understood. End
this slice with a measurement verdict and checkpoint rather than spending an
unbounded amount trying to recover missing counters.

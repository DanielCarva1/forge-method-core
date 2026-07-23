# C5.1-C5.3 post-BuildVerify durable episodes and replacement continuity

C5.1-C5.3 provide a typed, host-independent continuity path for the period after a
BuildVerify release candidate. The model prevents a later owner or replacement process
from silently combining a release, rollback baseline, observation, request, claim, or
completion from different durable episodes.

The serializable values remain candidate, audit, and recovery data. They do not admit a
release, deploy, roll back, advance phase, mutate integration state, acquire a claim,
sign, establish trust, install, activate, choose a host, or carry a retained Store lock.

## C5.1 candidate episode contract

`forge_core_contracts` exports `PostBuildVerifyEpisodeDocument` from
`post_build_verify_episode`. Its required state binds:

- `WorkflowGovernanceReleaseIdentity` as the exact release subject;
- a distinct exact previous release or exact BuildVerify snapshot as the rollback
  baseline;
- bounded deployment observations and operational evidence;
- bounded feedback and incident/bug intake, all repeating the exact release digest;
- a generation-chained evolution identity and content-addressed continuity subject;
- an exact next-action identity; and
- exactly one reference for each readiness, ready-release, reality-evidence,
  context-recovery, and evolve-project policy role.

`episode_digest` is a canonical JSON SHA-256 self-digest. Later snapshots of the same
`episode_id` increment `generation` and bind `previous_episode_digest`. Contract
validation checks shape and bindings only; it does not validate signatures, host time,
deployment truth, policy admission, or persistence authority.

## Pure decision API

`forge_core_decisions` exposes:

```rust
pub fn evaluate_post_build_verify_episode(
    document: &PostBuildVerifyEpisodeDocument,
) -> PostBuildVerifyEpisodeDecision;

pub fn verify_post_build_verify_episode_decision(
    document: &PostBuildVerifyEpisodeDocument,
    decision: &PostBuildVerifyEpisodeDecision,
) -> bool;
```

The evaluator chooses referenced policy roles only:

1. malformed or self-digest-invalid input is `blocked`;
2. no observed deployment is `release_readiness_required`;
3. a failed deployment or evidence that disproves readiness is
   `rollback_assessment_required`;
4. untriaged feedback or unresolved incident/bug intake is
   `evolution_triage_required`; and
5. otherwise the snapshot is `operational_monitoring`.

These outputs are not lifecycle transitions. In particular,
`rollback_assessment_required` is not permission to execute a rollback.

## C5.2 guarded runtime admission

`WorkflowGovernanceProjectAdapter::apply_post_build_verify_episode` is the explicit
mutation boundary. Its request supplies the candidate plus compare-and-swap bindings for
the current project snapshot, workflow-ledger head, and state version. The kernel also
verifies the active admitted release, durable phase, candidate generation, exact
predecessor digest, admitted policy/assurance state, and existing hard transition gate.

The only forward routes are:

- `BuildVerify -> ReadyOperate`, after the readiness boundary passes; and
- `ReadyOperate -> Evolve`, after the release boundary passes.

Ordinary automatic phase planning still has no successor after BuildVerify. Generic
ledger append rejects the typed episode event. Rollback assessment and evolution triage
carry neither a target phase nor an admitted gate, so they preserve the durable phase.

Historical C5.2 records use workflow-ledger epoch `0.7`. They retain the bounded event
summary and remain readable for audit compatibility, but a summary without the complete
episode snapshot cannot reconstruct replacement state.

## C5.3 complete epoch `0.8` persistence

New dedicated episode writes require the complete validated
`PostBuildVerifyEpisodeDocument` inside `PostBuildVerifyEpisodeAppliedEvent`. The TCB
compares the nested document with the event summary, release identity, generation,
predecessor, episode digest, and BuildVerify snapshot before append. The first complete
episode or coordination record advances the retained ledger to epoch `0.8`; every
successor remains at `0.8`.

The generic append path rejects complete episode and coordination events. Dedicated TCB
routes hold the retained workflow-ledger lock, enforce exact predecessor/CAS bindings,
and append one hash-chained record. Deserializing the record cannot recreate the kernel
admission that originally produced it.

## Durable coordination lifecycle

C5.3 persists closed coordination projections alongside the complete episode:

- `CoordinationRequestState`;
- `CoordinationCompletionState`; and
- `CoordinationHealthRecoveryState`.

Request lifecycle rules are:

```text
Pending -> Accepted | Rejected | Superseded | Expired
Accepted -> Applied | Rejected | Superseded | Expired
```

The sender owns the initial `Pending` append. The target driver owns every transition.
Terminal states cannot transition, immutable request fields cannot change, required
response evidence must be present, and strict RFC3339 UTC deadlines fail closed.
`Accepted` is an intermediate state; terminal statuses remain constrained by
`response.allowed_statuses`.

A request is protocol data, not mutation authority. Driver-applied work records an
evidence-only mutation handoff that binds the exact driver, requested operation, live
claim ID, authority references, and exact `ToolEffectContract` references.

Completion application requires the current durable state version, a non-invalidated
completion, required proof, the exact active claim ID, exact claimant, and exact lease
expiry. Stale, missing, expired, mismatched, or conflicting completion is rejected. An
exact retry returns the existing durable record instead of appending duplicate work.

Stalled or crashed runtime recovery cannot silently reassign automatically. Reviewed
handoff or reclaim requires both the exact durable request ID and exact live claim ID. If
a request is present, its target driver must record the recovery plan.

## Exact runtime IDs versus repository fixture paths

Runtime coordination joins use stable durable instance IDs:

- Request dependencies and recovery references use `RequestContract.id`;
- Claim dependencies, completion bindings, mutation handoffs, and recovery references
  use `ClaimContract.id`.

`contract_ref` identifies a shared schema definition and is not unique to an instance.
Repository fixture paths can remain useful static cross-references, but they do not name
a live durable Request or Claim during kernel apply. Treating either a schema path or an
instance-file path as a runtime ID fails closed.

Gate, effect, runtime-handoff, and decision dependencies resolve through the exact typed
repository reference index. Mutation-handoff effects must resolve specifically as
`ToolEffectContract` records.

## Fresh-process replacement projection

`WorkflowGovernanceProjectAdapter::recover_replacement_continuity` performs fresh durable
reads of the workflow ledger and claim WAL. It returns:

- the exact ledger head, current state version, and reconstructed durable phase;
- the active admitted release;
- the latest complete generation for every persisted episode and the active-release
  episode ID;
- latest request state by request ID;
- latest completion by task ID;
- latest health recovery by runtime ID; and
- claim snapshots classified as `live`, `expired`, or `non_active`.

A ledger containing only historical `0.7` summaries fails this reconstruction because it
cannot recover the rollback baseline, observations, intake, evolution identity, or next
action. This does not make the historical ledger unreadable; it makes the stronger
replacement claim unavailable.

Returned values are cloned audit/recovery data. They contain no retained lock, claim
capability, mutation authority, phase authority, release authority, signing key, private
key, trust authority, lifecycle authority, protected-anchor authority, install or
activation authority, or host-selection authority. `selected_host` remains none.

Private external broker keys must remain in their owner-specific backup procedure and
must never be copied into Forge state or Forge backups.

## Verification boundary

Focused source tests cover complete repeated snapshots, historical `0.7` compatibility,
dedicated append ownership, request transitions and evidence, typed references, exact
stable IDs, completion proof and conflicts, recovery joins, deterministic retries, claim
liveness, replacement reconstruction, and authority-free serialization.

Targeted compile-only `--all-targets` checks are green for the workflow-governance TCB
and kernel after the C5.3 source changes. Rust tests, runtime fresh-process integration,
failure injection, stress, fuzzing, MSRV/platform matrices, hosted CI, publication,
release, real-host, and field evidence remain deferred until every source story compiles.

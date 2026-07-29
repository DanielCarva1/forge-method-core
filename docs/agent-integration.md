# Agent integration contract

Forge is not a conversational model. It is a local typed governance boundary
driven by a host agent. A correct integration keeps the human in chat while
preserving the distinction between advice, evidence, and admitted authority.

## Required loop

1. Resolve the user-selected project root.
2. Run `forge-core start --root <root> --json` once per chat.
3. Execute only structured argv returned by Forge.
4. Initialize or resume workflow governance.
5. Check the durable release and perform only an exact returned upgrade.
6. Call `workflow next` without caller-selected phase, policy, bundle, or
   readiness target.
7. If Forge returns a missing-objective packet in `solo_cooperative`, materialize
   the unambiguous chat outcome through `workflow intent accept-cooperative`.
   If one irreducible choice remains, send the closed `decision_required` input
   and ask the returned question; that branch performs no Forge state write. In
   `strict_external`, continue to admit the externally signed human input
   through `workflow intent record`. Never ask the human to design the method.
8. Perform the highest-ranked feasible action.
9. Collect evidence from the tool/runtime/human named by the evaluator; never
   self-upgrade artifact presence into representative proof.
10. Record observations through an authorized surface and call `workflow next`.
11. Stop and explain typed gaps when authority or capability is unavailable.

The canonical bootstrap procedure is
[`skill/start-forge/SKILL.md`](../skill/start-forge/SKILL.md). The generated
[command surface](generated/command-surface.md) is the flag-level reference.

Before claiming readiness, report the four identities separately: installed
binary/source checkpoint, selected prebuilt asset, durable workflow release pin,
and Domain Pack effective epoch. Use the canonical
[identity table](../README.md#four-identitiesdo-not-collapse-them); never infer
one from another.

## JSON handling

- Treat `CliEnvelope.ok` as the result, not process exit alone.
- Preserve argv boundaries. Never shell-evaluate display command strings.
- Bind mutating follow-up to returned snapshot/head/CAS digests.
- Do not cache guidance across mutation; ask Forge again.
- Redact private keys and secret material; audit projections are not authority.

The normal `workflow next` response embeds `authorization.action_packets`,
registry setup state, and typed setup gaps. The standalone
`workflow action-packets` command exposes the same packets and registry status
for read-only diagnostics. Authority-bearing observations use those
Forge-derived packets and the external origin-broker bridge described in the
[operator guide](operator-guide.md). The host signs a minimal closed answer
bound to the returned packet; `workflow action apply` derives, verifies, and
records the exact request without exposing an intermediate attestation. Never
manufacture request, registry, ledger, or receipt documents in the host.

A `cooperative_same_owner` objective packet is a separate authority boundary,
not a weaker spelling of human approval. The host supplies only bounded outcome,
constraints, unacceptable outcomes, open uncertainties, carrying principal,
and generic host/session coordinates. Forge derives objective identity,
revision, assurance epoch, project snapshot, ledger head, packet binding, and
commit time. The resulting provenance supports same-owner continuity but never
claims a signature, verified human origin, independent identity, or satisfaction
of the `strict_external` boundary.

The packet's `input_contract.kind` is `cooperative_objective`, not the strict
broker `intent_revision` shape. It publishes both closed JSON templates,
`utf8_json_file` encoding, every relevant byte/item bound, denial of unknown
fields, and a structured argv template. A generic host must be able to execute
the lane from that packet without reading Forge source:

1. Materialize one temporary file from `unambiguous` when chat already contains
   a clear objective, or from `decision_required` when exactly one irreducible
   choice remains.
2. Preserve path boundaries, including paths with spaces, and execute the
   packet-derived `workflow intent accept-cooperative` argv.
3. Keep the human chat-only. For `decision_required`, ask the returned question;
   Forge has validated the current packet/state read-only and written nothing.
4. Remove the temporary host file, then obtain a fresh `workflow next`.

Exact accepted-input retries return the same durable receipt without appending.
Changed payloads, strict-profile use, consumed or stale packets, and changed
project snapshots fail closed.

After an objective exists, `workflow next` keeps its governed ranked action
unchanged and exposes a separate `authorization.objective_management_packet`.
Use it only when later chat actually corrects or clarifies the objective:

- `material_supersession` supplies a complete corrected proposal and a bounded
  reason. Forge creates the adjacent revision and Assurance epoch, binds its
  `previous_objective_digest` to the formerly active immutable record, and
  rejects a materially identical proposal.
- `non_material_clarification` supplies additions only. Forge preserves the
  outcome and every prior constraint, unacceptable outcome, and uncertainty,
  then appends only new bounded details. It cannot delete or replace direction.
- `decision_required` remains read-only when the correction itself contains one
  irreducible value choice.

Each accepted revision advances the ledger head, objective digest, and
Assurance epoch. Consequently objective-management packets and prepared
objective authority bound to the prior head/objective/epoch fail stale;
historical observations remain auditable. An exact retry is idempotent only
while that accepted revision is still the ledger head. Replacement-agent
readback uses the latest `active_cooperative_objective`, whose `revision_kind`,
`revision_reason`, and `previous_objective_digest` explain why it superseded or
clarified the prior objective.

## Agent autonomy boundary

Once `workflow next` exposes an active `agent_autonomy.binding`, ordinary work
inside the accepted objective proceeds autonomously. The binding contains the
objective id/revision/digest and assurance epoch plus current project snapshot,
ledger head, and state version. It becomes stale after objective supersession or
any bound state change. Strict-external readiness reports
`unsupported_profile`; it never misreports a missing cooperative objective.

The delegated classes are research/analysis, planning/strategy, reversible
local editing, testing/verification, documentation, changes of tactic/file/work
order/retry, external read-only research, reversible local commits, and local
evidence generation. Generating evidence locally is autonomous; admitting,
promoting, or publishing it remains governed by the existing evidence and
effect boundaries. The only human decision classes are product-objective
change, material trade-off, material risk acceptance, and
irreversible/external effect.

Every assessment input also carries a required closed effect descriptor. The
host derives it from the selected tool and concrete operation boundary,
independently of the model-declared work class, never from free-form task text
alone. Local read-only, local reversible, and compatible external read-only
operations may proceed. External mutation (for example Slack/Jira/email sends,
HTTP writes, or mutations to a staging system), unknown/ambiguous operations,
and contradictory class/effect pairs fail closed into a Decision Request.
Publication, remote push/merge, deployment, production mutation, secret use,
and destructive external effects are protected. Local git staging/commit is
local reversible work; a remote push is protected.

`workflow next` publishes structured assessment argv and a machine-readable
input contract: schema version, byte bounds, the complete enum lists,
unknown-fields policy, and the requirement to place the temporary input outside
the project snapshot. `workflow autonomy assess` is an optional read-only
validator at an actual semantic/effect boundary, not an authorization step and
never a per-edit/per-task command. It returns exactly one tagged branch:
`proceed_autonomously` or `decision_required`, whose concrete request preserves
the proposed-work summary, two alternatives beyond the recommendation, and
non-empty consequences. Assessment never writes the workflow ledger, replay
WAL, or Forge state; it supplements rather than replaces the governed ranked
next action.

`workflow action authorize` is a cooperative local one-call lane only for a
packet marked `operator_credential_broker`. Forge rejects that lane before
signing for human, independent-reviewer, and trusted-runtime broker packets.

The local signing bridge proves key possession only inside Forge's cooperative
same-OS-principal model. It does not prove human presence or reviewer
independence. An agent must never self-provision or use a `human`, `reviewer`, or
`runtime` local profile as evidence of a distinct actor. The external broker
vouches for the signed origin subject and separation domain; Forge does not
infer physical presence from those labels.

## Mediated versus direct writes

Call a write **Forge-mediated** only when the claimed target is covered by the
current claim/gate result, verified principal, Admission, pre-effect WAL, effect,
and durable receipt. Editor, shell, host-plugin, installer, or other filesystem
writes are direct/ungoverned unless that admitted transaction covers them.
Log/disclose direct writes rather than laundering them through a transcript.

## Human attention

Ask the human only for an irreducible decision returned by the admitted
workflow, consent to an operator-owned trust ceremony, or information the agent
cannot obtain. Translate the decision into natural chat. Never ask the human to
select a workflow or edit internal YAML.

The agent owns method discovery. It drafts representative journeys, falsifiers,
environment expectations, scenarios, and failure modes, then obtains the
required independent review. The human supplies desired outcomes, constraints,
preferences, unacceptable outcomes, uncertainties, and irreducible value
choices; ignorance of a development method must not become human homework.

## Evidence discipline

- A file is not automatically working behavior.
- A self-report is not an independent review.
- A mocked execution is not a representative session.
- A second agent is independent only when principal and evidence are distinct.
- External/runtime capability must be verified by corresponding authority.
- A representative-slice manifest is a proposal until an independent Reviewer
  origin admits its exact bytes through the evaluator-observation lane.
- Runtime evidence must come from a separately configured origin domain and
  match the latest definition, exact runtime subject, current intent/snapshot/
  effective epoch, and every declared scenario.
- Partial execution is only `supported`; current failure is `disproven`; a
  later accepted definition supersedes prior definitions.
- Use the existing evidence/action-packet lane. Do not invent a second slice
  store, mutation event, or caller-authored epistemic state.

- P7F bundle-checker success means structural/content-integrity validity only;
  it cannot prove a production host, chat-only behavior, semantics, actor or
  reviewer independence, publication, or P7F passage.
If authority is not provisioned, the correct result is a blocked gap. Never
hand-author a registry, signature, receipt, or ledger record to advance.

## Replacement agents

A replacement begins from `start` and `workflow resume`. It must not require
prior chat context. If durable state cannot reconstruct release, effective
Domain Pack generation, accepted intent/assurance epoch, all eight lens states,
governed evidence bindings, blockers, and next action, fail closed.

## Compatibility surfaces

`guide describe`, `guide status`, and `guide decide` are diagnostics. They do
not select authoritative P5/P6 workflow. New integrations use
`workflow init|resume|next`.

## Integration acceptance checklist

- Fresh and existing projects bootstrap idempotently.
- Paths with spaces remain one argv element.
- A stale snapshot/head is rejected and retried from new guidance.
- A consumed host event is idempotent and cannot authorize another packet.
- Broker absence/revocation blocks without falling back to a local human label.
- Missing evidence cannot complete a policy.
- Human questions appear only after prerequisite claims are verified.
- Replacement process returns the same durable epoch and next action.
- Every write claimed as governed has the complete mediated evidence chain;
  direct host/editor/shell writes are explicitly classified ungoverned.
- No private key, opaque capability, or operator anchor is exposed in chat.

## Cooperative evidence admission

For a `solo_cooperative` objective, an agent may submit `forge-core workflow evidence admit-cooperative` with the closed offer published by `workflow next`. The offer is bound to the active objective revision/digest, assurance epoch, accepted record, effective bundle, snapshot, ledger head, and state version. The kernel records admitted offers and bounded normalized rejections. The versioned solo descriptor is default-denied and is derived only from the currently selected policy's current unsatisfied claim and its bound evaluator; it never scans for a convenient claim in another policy. Frozen release artifacts and strict/profileless output are not mutated.

The initial route accepts only the kernel-executed current `project_snapshot` scenario and authoritative readback. Callers do not supply an outcome or observation time. Runtime, external-system, human-decision, repository-state substitutes, and unknown/inconclusive assertions fail closed. An admitted record can be `supporting` only for its explicit `cooperative_claim_ref`; it is never projected as a source-policy receipt and therefore leaves the selected source claim unsatisfied, including when that source uses RepresentativeRuntime. It does **not** prove independent semantic review, trusted-runtime separation, tamper resistance, human presence, or enterprise compliance. Rejected and stale records remain visible in `workflow next` but never support either claim.

`/start-forge` activation is once per chat, not once per task. After the objective is accepted, evidence operation is mechanical: inspect each fresh `data.cooperative_evidence_action_packet`; write `offer_template` to a temporary file outside the snapshot; replace only `required_replacements`; execute `argv` as tokens after replacing only `input_file_token`; delete the temporary file; and refresh `workflow next`. Never shell-parse the vector. If the packet is absent, report `data.cooperative_evidence_action_gap`. A `rejected` result is audited non-support, not success.

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

## Windows-to-WSL root association

A host-side Windows workspace and a WSL path are not assumed to be the same
project. When a native Windows binary is unavailable, a drive-path bridge first
uses the local, unversioned `forge_wsl_bridge_map_v1` association described in
the canonical start skill. The default location is
`%LOCALAPPDATA%\Forge Method\wsl-bridges.json`, and
`FORGE_WSL_BRIDGE_MAP` may select another local file. Each entry binds one exact
normalized `host_root` to one listed WSL `distribution` and one absolute existing
`linux_root`. Duplicate, prefix-only, malformed, or unknown-distribution entries
are rejected.

Without that association, `wslpath` is discovery only. A translated drive path
may be used only when exactly one distribution resolves it to an existing
project with a regular `.forge-method.yaml` Project Link. A fresh or unlinked
translated destination is not initialized: the agent stops before `start` or
`workflow init` and asks the operator to establish the local association. Direct
`\\wsl.localhost\<distribution>\...` roots already identify the Linux tree
and do not require a drive mapping. The chosen distribution, Linux root, binary,
and proof source are retained once per chat. No host brand, distribution, user,
drive, or repository is built into this contract.

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

`workflow resume` keeps the ordinary `workflow next` fields and adds the
versioned `replacement_continuity` block. That block is rebuilt from durable
project state only: objective history, decision events actually present in the
ledger, evidence history, claims, isolation ownership and real Git worktree
validation, promotion state, typed gaps, and a ranked next action. A question
calculated by the current policy remains under
`simulation.candidate_decision_requests`; it is not described as a decision
recovered from an earlier chat. Recovery commands are argv arrays, not shell
text; keep paths with spaces as one element. Recoverable promotions rank before
new work. Completed promotions never offer apply or recovery again. Missing,
mismatched, expired, released, or tampered state blocks automatic continuation
and is never silently repaired by resume.

The format does not depend on a particular agent host. Compatibility with
Codex, ZCode, Claude, Cursor, OpenCode, pi.dev, or another host remains a
candidate until that host passes its own end-to-end integration test. A host may
add its own review policy; Forge does not impose a universal two-reviewer rule.

## Legacy profile adoption

`forge-core workflow profile status --root <path> --json` is the read-only,
host-neutral check for historical ledgers that predate readiness profiles. A
legacy project remains `strict_external` until this command reports
`solo_adoption=eligible` and publishes `data.adopt_solo_argv`. Eligibility is
intentionally narrow: the genesis `project_imported` must omit
`readiness_profile`, and the history before adoption may contain only that
import plus kernel-admitted `release_upgraded` records. Workflow decisions,
evidence, claims, coordination, intent, broker origin, and every other
authority-bearing event make the first adoption route ineligible.

The host executes the returned argv only after the ordinary chat has clearly
chosen Solo Cooperative. The command rechecks both the exact ledger head and
the exact project snapshot while retaining the workflow lock. It appends one
`legacy_solo_profile_adopted` record with `cooperative_same_owner` provenance;
it does not rewrite the historical genesis or claim a verified human presence,
signature, reviewer separation, or broker event. An exact retry returns
`already_adopted` without another record even if project files changed after the
original transition; the retry receipt still identifies the original snapshot.
A later ledger record makes the old adoption argv a conflict rather than a
second write. A project whose genesis already selected `solo_cooperative`
reports `already_solo` and needs no migration. Explicit `strict_external`, stale,
tampered, reverse, conflicting, or unsupported history fails closed without a
write. `start` and `workflow init` never perform this transition silently.

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

The initial route accepts only the kernel-executed current `project_snapshot` scenario and authoritative readback. Callers do not supply an outcome or observation time. Runtime, external-system, human-decision, repository-state substitutes, and unknown/inconclusive assertions fail closed. An admitted record can be `supporting` only for its explicit `cooperative_claim_ref`; its audit publishes `valid_through_unix` while it is current. It is never projected as a source-policy receipt and therefore leaves the selected source claim unsatisfied, including when that source uses RepresentativeRuntime. It does **not** prove independent semantic review, trusted-runtime separation, tamper resistance, human presence, or enterprise compliance. Rejected and stale records remain visible in `workflow next` but never support either claim.

`/start-forge` activation is once per chat, not once per task. After the objective is accepted, evidence operation is mechanical: inspect `data.cooperative_evidence` first on every fresh `workflow resume` or `workflow next` response. A current `supporting` record satisfies the cooperative obligation through its published validity boundary, so `cooperative_evidence_action_packet` and `cooperative_evidence_action_gap` are both absent and the agent must not admit it again. Without current support, inspect the packet; write `offer_template` to a temporary file outside the snapshot; replace only `required_replacements`; execute `argv` as tokens after replacing only `input_file_token`; delete the temporary file; and refresh `workflow next`. Never shell-parse the vector. If neither current support nor a packet exists, report `data.cooperative_evidence_action_gap`. A `rejected` result is audited non-support, not success. Expiry or a changed snapshot, objective, or selected policy/claim/evaluator route makes the old record stale. A fresh response publishes a newly bound packet only when that cooperative route remains available.

## Governed promotion preview, exact-CAS apply, and recovery

When isolated work is ready for inspection, the host may run
`forge-core workflow promotion preview --root <canonical-project> --isolation-id <id> --json`.
The source root is derived only from the uniquely selected Active isolation
contract; callers cannot substitute an ambient path. The source must be an
exact linked Git worktree registered by the canonical repository, on the
contract branch, with its common repository and both HEAD identities bound.
Ordinary directories, different repositories/branches, traversal, aliases, and
hard links fail closed. The command retains and revalidates both source and
canonical destination trees and returns domain-separated filesystem/diff/write
set digests, objective/ledger/evidence/claim bindings, path-to-linked-claim
attribution, ambient conflicts, destructive deletes, excluded-root metadata,
created-file metadata and directory/type/mode effects, source-side build/dependency roots that are outside the promotable snapshot, a final observation time, validity boundary, and
unresolved gaps. A source-side `target` or `node_modules` root is an explicit blocking unsupported effect rather than a silently omitted change; destination-only cache roots are preserved outside promotion scope.

The preview acquires only pre-existing lifecycle, workflow, and claim locks; a
missing lock or state namespace is rejected without materializing it. Its stable
preview digest excludes only the volatile observation instant so apply can
re-admit the same still-valid bound candidate. The preview is strictly read-only and caller-carried: `authority` is
`read_only_candidate_no_apply_authority`, both mutation flags are false, and no
authority is granted by the digest. Same-owner cooperative evidence remains
explicitly unable to verify the selected source claim.

When the preview reports `apply_eligibility=eligible_local_reversible`, apply
that exact observation with
`forge-core workflow promotion apply --root <canonical-project> --isolation-id <id> --expected-preview-digest <sha256:...> --json`.
Forge re-derives the live isolation, linked claim principal, objective, ledger,
evidence, claims, source, destination, effect, and payload under retained locks.
This solo-cooperative lane supports only metadata-stable writes to existing
regular files and does not require the strict-external broker. `unknown` and
`supported` source assurance claims remain explicit `carried_assurance_gaps`;
`disproven` and `contradictory` claims remain blocking.

Success includes a durable self-digested receipt and fresh canonical readback.
An exact retry returns `already_committed` after verifying the receipt, exact
consumed replay authority, and readback, without another write.
`recovery_required` is emitted as
`typed_failure.type=recovery_required` when durable intent or commit state may
exist without a complete receipt. An apply failure includes
`typed_failure.data.can_recover=true` plus the exact
`typed_failure.data.recovery_argv` array. Execute that array as separate
components; a project path containing spaces remains one component. Preserve
the canonical tree plus Forge sidecar and run:

`forge-core workflow promotion recover --root <canonical-project> --isolation-id <id> --expected-preview-digest <sha256:...> --json`.

Recovery uses the already approved preview, durable intent, split-root effect
WAL, replay WAL, retained source, and actual canonical bytes. Each target must
be exactly its recorded old or new content; any third content, corrupt record,
contradictory receipt, changed binding, or terminal rollback stops before a new
write. A pre-write interruption resumes the same attempt without asking for a
new preview. A partial two-or-more-file write completes only the unambiguous
remainder. A commit is never applied again: recovery completes any missing
single-use replay acknowledgement, performs canonical readback, and
creates/verifies the receipt. Repeating recover converges to
`already_committed`. The generic effect-WAL recovery remains forbidden for this
split-root transaction. If recover itself fails, its typed failure has
`can_recover=false` and no recovery argv: stop rather than recursively invoking
recover. A legacy v1 intent created before effect Begin has no stored historical
preview observation. Recovery verifies its opaque self-digested intent, derives
a fresh candidate with the same semantic preview digest, requires the
destination to remain exactly unchanged, and records that fresh execution
without claiming the historical observation was reconstructed.

## Proving a host journey

A host name is never enough to claim support. Use the open eight-part kit in
[Host conformance](host-conformance.md). Forge runs any adapter through separated
argv, calculates each result itself, and keeps missing host APIs as visible gaps.
An adapter-only claim is never enough for `supported`; without trusted native
proof, the honest ceiling is `partially_supported`.

The Windows-to-WSL check applies only when the chosen Windows host is targeting
a canonical project root inside WSL. Bundle integrity does not replace native
host authenticity.

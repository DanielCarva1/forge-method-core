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
6. Call default `workflow resume` without caller-selected phase, policy, bundle,
   or readiness target.
7. Inspect repository evidence and orient the human in the language already used
   in the conversation. Pair useful technical facts with their practical meaning;
   do not alternate languages or dump raw Forge status as the explanation.
8. If Forge returns a missing-objective packet, materialize the unambiguous chat
   outcome through `workflow intent accept-cooperative`. If one irreducible choice
   remains, send the closed `decision_required` input and ask the returned
   question; that branch performs no Forge state write. Never ask the human to
   design the method.
9. Perform the highest-ranked feasible action in the same turn instead of ending
   after orientation.
10. Collect honest evidence from the named tool/runtime or bounded repository
    inspection without upgrading same-owner evidence into an independent claim.
11. Record observations through the typed cooperative surface and call default
    `workflow resume` again, even if the receipt embeds a next projection.
12. Stop and explain genuine capability gaps. Enterprise broker/signature setup
    is not a `solo_cooperative` capability gap.

The canonical bootstrap procedure is
[`skill/start-forge/SKILL.md`](../skill/start-forge/SKILL.md). The generated
[command surface](generated/command-surface.md) is the flag-level reference.

Before claiming readiness, report the four identities separately: source
checkpoint, executable selected by `PATH`, durable workflow release pin, and
Domain Pack effective epoch. Use the canonical
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
- Never expose secret material; audit projections are not authority.

The normal `workflow resume` response embeds typed action packets and setup
gaps. In the current `solo_cooperative` profile, the host uses the dedicated
cooperative objective/evidence paths and must not provision or invoke an
external-origin broker. Missing enterprise registries are non-blocking profile
metadata, not onboarding work. Never manufacture request, registry, ledger, or
receipt documents in the host.

A `cooperative_same_owner` objective packet is a separate authority boundary,
not a weaker spelling of human approval. The host supplies only bounded outcome,
constraints, unacceptable outcomes, open uncertainties, carrying principal,
and generic host/session coordinates. Forge derives objective identity,
revision, assurance epoch, project snapshot, ledger head, packet binding, and
commit time. The resulting provenance supports same-owner continuity while
making no claim of a signature, verified human origin, independent identity,
or enterprise compliance.

The packet's `input_contract.kind` is `cooperative_objective`. It publishes both
closed JSON templates, `utf8_json_file` encoding, every relevant byte/item
fields, and a structured argv template. A generic host must be able to execute
the lane from that packet without reading Forge source:

1. Materialize one temporary file from `unambiguous` when chat already contains
   a clear objective, or from `decision_required` when exactly one irreducible
   choice remains.
2. Preserve path boundaries, including paths with spaces, and execute the
   packet-derived `workflow intent accept-cooperative` argv.
3. Keep the human chat-only. For `decision_required`, ask the returned question;
   Forge has validated the current packet/state read-only and written nothing.
4. Remove the temporary host file, then obtain a fresh default `workflow resume`.

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

Once `workflow_resume_summary_v9` exposes an active
`agent_autonomy.binding`, ordinary work inside the accepted objective proceeds
autonomously. The binding contains the objective id/revision/digest and
assurance epoch plus current project snapshot, ledger head, and state version.
It becomes stale after objective supersession or any bound state change.
Profiles that require separated external authority are outside the current
integration target and must not silently alter the Solo Cooperative loop.

The delegated classes are research/analysis, planning/strategy, reversible
local editing, testing/verification, documentation, changes of tactic/file/work
order/retry, external read-only research, reversible local commits, and local
evidence generation. Generating evidence locally is autonomous; admitting,
promoting, or publishing it remains governed by the existing evidence and
effect boundaries. The only human decision classes are product-objective
change, material trade-off, material risk acceptance, and
irreversible/external effect.

`data.current_work` is the bounded product-work checkpoint. Read it before
reconstructing work from chat or scanning project documentation. `absent` means
no Work Focus has been accepted; `stale` means its objective revision or Phase
no longer matches. The field is advisory and cannot authorize mutation,
completion, merge, publication, or release. When a present focus needs more
context, execute its exact `detail_argv`; ordinary continuation should not load
the larger detail projection. The argv carries the ledger head observed by
`resume`, so detail reads only that bounded durable state instead of capturing
the project again. If the head changed, run `resume` again rather than removing
or rewriting the expected digest.

The host does not wait for the human to request research. When a doubt may
change the accepted outcome, expose an unacceptable outcome, alter material
risk, or make the next action unsafe, it first checks authoritative local
project evidence and then performs proportionate read-only external research
when needed. It compares competing hypotheses, contrary evidence, source
freshness, applicability, and limitations; explains the result and product
impact in the human's language; and continues with the next safe action. A
trivial reversible uncertainty does not justify broad research, and only a
remaining human-decision class returns to the human.

The research command surface is optional support for durable, decision-relevant
work; it is not a research trigger or a required wrapper around every search.
Use `forge-core research source add` when a consulted source should survive the
current turn, support a material decision, be checked or reused by another
agent, or retain provenance for governed evidence. `forge-core research source
list` recovers those sources, `forge-core research cite` resolves one source id,
`forge-core research check` checks source references in project artifacts, and
`forge-core research graph` shows which claims cite which sources. Registration
proves provenance and resolvability, not truth,
authority, or sufficiency. The closeout or durable evidence using the source
retains the question, relevant finding, contrary evidence, limitations, and
decision impact. Do not register every search result or trivial fact.

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

The v2 resume projection publishes structured assessment argv and a
machine-readable input contract: schema version, byte bounds, the complete enum
lists, unknown-fields policy, and the requirement to place the temporary input
outside the project snapshot. `workflow autonomy assess` is an optional
read-only validator at an actual semantic/effect boundary, not an authorization
step and never a per-edit/per-task command. It returns exactly one tagged branch:
`proceed_autonomously` or `decision_required`, whose concrete request preserves
the proposed-work summary, two alternatives beyond the recommendation, and
non-empty consequences. Assessment never writes the workflow ledger, replay
WAL, or Forge state; it supplements rather than replaces the governed ranked
next action.

External-origin authorization, hardware-backed presence, independent reviewer
custody, and compliance signing are deferred enterprise integration concerns.
A Solo Cooperative host must not provision, rotate, request, or invoke them.
If the runtime reports one as blocking under `readiness_profile=solo_cooperative`,
the host reports product/profile drift instead of asking the developer to
perform a trust ceremony.

## Mediated versus direct writes

Call a write **Forge-mediated** only when the claimed target is covered by the
current claim/gate result, verified principal, Admission, pre-effect WAL, effect,
and durable receipt. Editor, shell, host-plugin, installer, or other filesystem
writes are direct/ungoverned unless that admitted transaction covers them.
Log/disclose direct writes rather than laundering them through a transcript.

## Human attention

Ask the human only for an irreducible product decision, material risk
acceptance, an irreversible/external effect, or information the agent cannot
obtain. Translate the decision into natural chat. Never ask the human to select
a workflow, edit internal YAML, or operate enterprise trust infrastructure.

The agent owns method discovery. It drafts representative journeys, falsifiers,
environment expectations, scenarios, and failure modes, then runs the strongest
feasible checks for the current solo claim. Stronger independent review is
required only when a later claim explicitly promises independence. Ignorance of
a development method must not become human homework.

## Evidence discipline

- A file is not automatically working behavior.
- A self-report is same-owner evidence, never independent review.
- A mocked execution is not a representative session.
- Cooperative evidence must remain bound to its exact claim, subject, basis,
  producer, snapshot, and freshness window.
- Partial execution is only `supported`; current failure is `disproven`; a
  later accepted definition supersedes prior definitions.
- Stronger claims such as independent review, trusted-runtime execution, human
  presence, publication, or enterprise compliance require their own later
  evidence and must not block unrelated solo development.
- P7F bundle-checker success means structural/content-integrity validity only;
  it cannot prove a production host, semantics, actor independence, publication,
  or P7F passage.
- Never hand-author a receipt or ledger record to advance.

## Replacement agents

`start` is only the router into that activation: when it publishes a workflow
argv, the host must execute and consume that handoff before orienting the user.
A host must not present `start.data.state` or its compatibility reason as the
recovered workflow state.
A replacement begins from `start` and `workflow resume`. It must not require
prior chat context. If durable state cannot reconstruct release, effective
Domain Pack generation, accepted intent/assurance epoch, all eight lens states,
governed evidence bindings, blockers, and next action, fail closed.

The current executable returns the concise, versioned
`workflow_resume_summary_v9` activation view by default. V9 is the complete
current-state contract for the agent: it carries the current objective; the
full autonomy projection; every current evaluation, blocker, warning, active
isolation, recoverable promotion, cooperative evidence item, authorization
packet, and exact gap. Counts identify only older audit history omitted from
the activation view. `current_work` adds only the bounded accepted work focus;
it does not copy transcript, issue body, evidence body, or catalog content.

In V9, `current_work` is the accepted product-work continuity. For an active or
blocked focus, its `next_step` is the product step to continue. For a completed
or abandoned focus, that field is only a handoff toward a new Work Focus. Its
blocker count and references are focus-bound; top-level blockers and evaluation
blockers remain workflow-wide diagnostics. A zero focus blocker count therefore
does not claim that the whole workflow has no gaps.

An optional `current_work.focus.quick_cycle` keeps ordinary resume small: it
contains only derived state, closed-stage count, and expansion count. Full
accepted Quick Cycle closeouts and expansion history stay behind the existing
`detail_argv`. Detail can publish one exact `predecessor_detail_argv` for the
direct prior Work Focus; it does not offer a history listing or a chain walk.

When Work Focus has a collaboration plan, ordinary resume still returns only
bounded lane counts and the next ready lane. Follow `detail_argv` only when the
exact plan is needed. Its `focus.collaboration` joins each accepted lane to the
existing claim, isolation, and promotion state; it does not create a second
collaboration registry. This detail path reads those bounded state stores
directly and does not capture the project tree or scan project documentation.

`actions.recommended` remains the preferred exact governed Forge operation, but
the host executes it only when it is compatible with the accepted product step.
`current_evaluation.candidate_next_actions` is raw simulation or fallback
diagnostic data, not a competing recommendation. The host combines these scopes
into one primary next step instead of presenting unrelated choices. A published
Solo Cooperative evidence packet still outranks abstract capability acquisition
and human escalation inside its governance lane. References keep the activation
response small instead of copying packet or gap bodies.

`workflow_resume_summary_v1` through `workflow_resume_summary_v6` are legacy
views. V6 adds `selected_policy_evidence`, containing only current assessments
for the selected policy. Repository and applicability assessments retain their
summary, content-addressed basis, and limitations. Deterministic execution
assessments retain the result and reasons but omit stdout and stderr so normal
resume stays compact.
A host may continue from fields they actually publish but must not infer that
fields omitted by a legacy view represent absent obligations. Update the runtime when a
missing field is required for the next safe action; do not automatically run a
second resume mode.

`workflow report --root <same-root> --json` is the separate historical view. It
keeps the ordinary current guidance fields and adds the versioned
`replacement_continuity` block rebuilt from durable project state: objective
history, decision events actually present in the ledger, evidence history,
claims, isolation ownership and real Git worktree validation, promotion state,
typed gaps, and ranked actions. A question calculated by the current policy
remains under `simulation.candidate_decision_requests`; it is not described as
a decision recovered from an earlier chat. Run the report only for an explicit
history/audit request or a continuity diagnosis that requires omitted records.

During one chat, read-only Forge commands and ordinary repository work do not
require another resume. Refresh through `workflow resume --root <same-root>
--json` once after a successful Forge command changes durable workflow state,
or when context was lost and must be recovered. `workflow next` remains the
complete current guidance projection; it is not the chat-continuity refresh
surface.

A conforming host never issues two consecutive resume calls. It reuses the
complete current response until a successful operation capable of changing the
workflow evaluation has run. Repository inspection, validation, tests, status,
report, and help are read-only for this rule and do not trigger a refresh.

The format does not depend on a particular agent host. Compatibility with
Codex, ZCode, Claude, Cursor, OpenCode, pi.dev, or another host remains a
candidate until that host passes its own end-to-end integration test. A host may
add its own review policy; Forge does not impose a universal two-reviewer rule.

## Legacy profile adoption

`forge-core workflow profile status --root <path> --json` is the read-only,
host-neutral check for ledgers created before readiness profiles. When it
publishes an exact `data.adopt_solo_argv` and this chat has selected solo
dogfooding, the host executes that argv without reconstructing it.

Adoption appends one `legacy_solo_profile_adopted` record with
`cooperative_same_owner` provenance. It does not rewrite history or claim
verified human presence, reviewer separation, or enterprise authority. Exact
retries are idempotent; stale, tampered, conflicting, or unsupported history
fails closed. A project already created or adopted as `solo_cooperative` needs
no migration. `start` and `workflow init` never perform adoption silently.

## Compatibility surfaces

`guide describe`, `guide status`, and `guide decide` are diagnostics. They do
not select authoritative P5/P6 workflow. New integrations use
`workflow init|resume|next`.

## Integration acceptance checklist

- Fresh and existing projects bootstrap idempotently.
- Paths with spaces remain one argv element.
- A stale snapshot/head is rejected and retried from new guidance.
- A consumed cooperative packet is idempotent and cannot authorize another input.
- Missing enterprise broker/signature setup does not block Solo Cooperative.
- Missing required evidence cannot complete a policy.
- Human questions appear only for the closed irreducible decision classes.
- Replacement process returns the same durable epoch and next action.
- Every write claimed as governed has the complete mediated evidence chain;
  direct host/editor/shell writes are explicitly classified ungoverned.
- No secret or opaque capability is exposed in chat.

## Accepted objective as the solo discovery source

In `solo_cooperative`, the active objective accepted through the typed objective command is the authoritative project direction for `policy.workflow.discover-intent`. Forge derives a typed `cooperative_same_owner` grounding from the exact durable material-objective record and reports it as `cooperative-objective:<accepted-record-digest>`. The grounding is separate from evaluator evidence: it does not claim that an `authorized_human` observed anything, and it does not inherit an evaluator's provider, maximum age, observation count, or principal-diversity rules. The word `unambiguous` is the same owner's assertion that the objective is clear enough to guide work; Forge does not pretend that a heuristic or independent reviewer semantically proved it.

The grounding is current only while that material anchor is inside the active receipt window and has not been revoked. An exact idempotent objective retry, a policy-equivalent release transition, and a `non_material_clarification` preserve the material anchor. A `material_supersession` creates a new anchor and reopens discovery until the replacement is completed. `invalidate_all` starts a new receipt window, so an old anchor cannot be revived by a clarification; a new material acceptance is required. Revoking the anchor also removes the grounding.

Completing discovery records the exact grounding anchor in `grounding_anchor_digests` and binds the completion subject to that anchor, not to the repository snapshot. Later governed project edits or updates inside an already-existing top-level `.local` directory therefore do not reopen discovery. A missing or invalid grounding does not suppress the normal evidence packet or capability gap. This route means only that the same owner and carrying agent established the direction Forge should follow; it is not proof of human identity, a second actor, independent review, or enterprise separation. Enterprise-origin intent is a later profile concern.

## Cooperative evidence admission

For a `solo_cooperative` objective, agents use the one existing public command,
`forge-core workflow evidence admit-cooperative`, with the closed packet from
`workflow_resume_summary_v9.actions.cooperative_evidence_packet`. The offer is
bound to the active objective, accepted record, effective bundle, project
snapshot, ledger head, state version, carrying principal, selected policy,
claim, evaluator, subject, and scenario. Only `solo_cooperative` publishes this
same-owner lane.

The packet's typed `route.target` separates two meanings without adding another
command, event, ledger, or admission subsystem:

- `policy_applicability` asks whether the selected `when_applicable` policy
  applies to the current project. The same-owner agent supplies `applicable`,
  `not_applicable`, or `inconclusive`, a bounded summary, project-relative basis
  paths, and bounded limitations. `applicable` keeps the policy selected;
  `not_applicable` skips it while its basis remains current; `inconclusive`
  leaves progression at `applicability_required`. This result is routing only:
  it cannot satisfy a policy claim or capability and cannot stand for a human
  judgment or independent review.
- `source_claim` assesses the exact selected repository-inspection claim.
  Current v2 packets accept `pass`, `fail`, or `inconclusive`; only a current
  pass becomes supporting source evidence. Historical v1 records remain the
  kernel-derived cooperative snapshot route. A missing target is interpreted
  only as this legacy `source_claim` behavior.

Both current agent-assessed routes reuse the same producer, evaluator, subject,
idempotency, bounded input, content-addressed basis, rejection audit, ledger,
and TCB checks. Forge rejects absolute, traversing, missing, duplicate,
symlink-escaping, oversized, or unreadable basis paths. Reusing an offer id with
identical canonical bytes is idempotent; reusing it for different bytes is a
closed conflict. Rejected offers never supersede admitted ones.

## Stable product entry into Evolve

After the admitted `ReadyOperate` release boundary is complete, the agent can submit an
honest post-BuildVerify episode with `forge-core workflow episode apply`. The strict,
bounded input carries the episode document plus the exact snapshot, ledger head, and
state version returned by current Forge readback. Forge rechecks the active release,
phase policies, assurance, release gate, generation, predecessor, and all CAS bindings
before one atomic append. A stale retry is a conflict and cannot duplicate the episode.

This command enters `Evolve`; it does not perform feature work there. Once the human and
agent agree on a material change, the existing cooperative material-supersession command
records the new objective and immediately reopens Discovery with the prior objective in
history.

Applicability is deliberately basis-scoped. Changes outside its admitted basis
do not invalidate the routing result; changed or missing basis bytes do. Only
the newest admitted applicability result for the exact objective, policy,
producer, and bundle is current, so restoring old bytes cannot revive a
superseded result. Source-claim evidence retains its stricter evidence
freshness and completion-binding rules. Neither route proves independent
semantic review, trusted-runtime separation, human presence, representative
runtime behavior, tamper resistance, official host support, or enterprise
compliance.

Operation is mechanical. On each fresh default `workflow resume`, inspect the
packet target, copy `offer_template` to a temporary file outside the project,
replace every `required_replacements` token, set the selected assessment's
`basis_paths` and bounded `limitations`, and leave all binding/route/schema
fields unchanged. Execute the published `argv` as tokens: its `--root` is the
exact project root, and the host replaces only `input_file_token` with the
external temporary-file path. Delete that file and refresh default resume after
every admission or rejection. Do not shell-parse, rebuild, cache, or repair an
old vector. When no packet exists, report
`data.actions.cooperative_evidence_gap` exactly.

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
authority is granted by the digest. Same-owner cooperative evidence may verify
only an exact selected repository-inspection claim; it cannot stand in for
human, independent-reviewer, or representative-runtime evidence.

When the preview reports `apply_eligibility=eligible_local_reversible`, apply
that exact observation with
`forge-core workflow promotion apply --root <canonical-project> --isolation-id <id> --expected-preview-digest <sha256:...> --json`.
Forge re-derives the live isolation, linked claim principal, objective, ledger,
evidence, claims, source, destination, effect, and payload under retained locks.
This Solo Cooperative lane supports only metadata-stable writes to existing
regular files and uses same-owner profile authority. `unknown` and `supported`
source assurance claims remain explicit `carried_assurance_gaps`; `disproven`
and `contradictory` claims remain blocking.

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

---
name: start-forge
description: Start or resume Forge Method for the current project. Use when the user invokes /start-forge, asks to run Forge start, onboard a repo into Forge Method, initialize a Consumer Project Repo with a Forge Project Link, diagnose the next Forge bootstrap step, or resume work on a Forge project in a new chat/session.
---

# Start Forge

This is the single entry point for Forge Method in a project. Run it once per
chat/session — it bootstraps a fresh repo, fails closed on linked state loss,
or routes a healthy project into agent-native workflow governance.

## Core rules

- The current product target is `solo_cooperative` dogfooding by one developer
  using agents. External-origin brokers, FIDO-backed presence, independent actor
  custody, and compliance signing are later enterprise concerns. Do not provision,
  request, or present them as blockers in the Solo Cooperative journey.
- Use the `forge-core` binary. Never create `.forge-method/` manually inside a
  Consumer Project Repo. Consumer repos get only a `.forge-method.yaml` pointer;
  runtime state belongs in the sibling Forge Runtime Sidecar created by
  `forge-core start`.
- The P5 workflow ledger owns governed phase and progression. `state.yaml` and
  `start.data.next_step` remain bootstrap/compatibility projections; never use
  them to select workflow, phase, policy bundle, readiness target, completion,
  or evidence authority.
- Execute structured command arrays as argv. `data.next_step.command` is a
  human-readable display string, not a shell-safe command source. Never split,
  concatenate, or shell-evaluate it.
- A Project Link proves prior initialization. If `start` reports
  `data.state_loss`, do not recreate, normalize, or initialize the sidecar, and
  never run `project init` as repair. Preserve all roots. Only the typed
  `choices.inspect` action is currently available and read-only; restore and
  reinitialize-as-new are deferred and intentionally expose no apply argv.

### Guided activation contract

<!-- guided-activation-contract:start -->
Starting the runtime is not the user-facing outcome. Before asking the human
for project facts or a decision, inspect the repository using narrow read-only
queries and, when available, reconcile that evidence with Forge continuity.
Classify the entry without asking the human to choose a mode:

- `greenfield`: no meaningful implementation, product documentation, or project
  history exists yet;
- `brownfield_unmanaged`: meaningful project evidence exists but no healthy
  Forge continuity is available;
- `brownfield_managed`: meaningful project evidence and healthy Forge
  continuity both exist.

For either brownfield mode, activation is not complete until the agent can
explain, in plain language:

1. **What this project is**
2. **Where it is now**
3. **What happened recently**
4. **What is already planned**
5. **What is missing or uncertain**
6. **The next best step**
7. **Why this step is recommended**

Use authoritative evidence in this order: current repository and runtime truth,
durable Forge state, project documentation, Git history, then chat context. Mark
inferences and absence of evidence explicitly. Do not ask the human to
reconstruct information the agent can discover. If evidence is insufficient,
continue inspecting the repository; if the missing fact is genuinely a product
choice, explain the context, options, consequences, and recommendation before
asking one concise question.

For greenfield, say plainly that no established project was found, summarize
any seed material that does exist, and then ask what outcome the human wants to
create. A Forge setup or bridge failure does not waive orientation: provide the
best read-only project explanation available, label Forge continuity as
unavailable, and separate the setup repair from the recommended project step.
<!-- guided-activation-contract:end -->

## Workflow

1. **Resolve the project root.** Default to the current working directory. If
   the user names a path, use it as `--root`.

2. **Locate `forge-core`** from PATH or Cargo bin.

   ```bash
   forge-core --version 2>/dev/null \
     || forge-core.exe --version 2>/dev/null \
     || ~/.cargo/bin/forge-core --version 2>/dev/null \
     || echo "NOT_FOUND"
   ```

   On Windows only, when no native binary is found, the agent may prove and
   retain one Windows-to-WSL bridge for the rest of this chat. A Windows drive
   path and a Linux path may name different copies of a repo, so path conversion
   alone is never permission to initialize anything.

   1. Find `wsl.exe` through the host's normal executable lookup and enumerate
      distributions with `wsl.exe --list --quiet`. Do not assume a distribution
      name or Linux user.
   2. For a `\\wsl.localhost\<distribution>\...` root, use that exact
      distribution and convert only the remainder to `/...`; the UNC path
      already names the Linux tree directly.
   3. For a Windows drive root, first look for the local, unversioned association
      file. `FORGE_WSL_BRIDGE_MAP` may name it explicitly; otherwise use
      `%LOCALAPPDATA%\Forge Method\wsl-bridges.json`. It has this closed shape:

      ```json
      {
        "schema_version": "forge_wsl_bridge_map_v1",
        "mappings": [
          {
            "host_root": "X:\\path\\to\\workspace",
            "distribution": "chosen-distribution",
            "linux_root": "/absolute/linux/project/root"
          }
        ]
      }
      ```

      The file is local operator configuration and must stay outside every
      project and outside version control. Require a regular file, the exact
      schema, and exactly one mapping whose normalized `host_root` equals the
      selected Windows project root. Reject duplicate, partial-prefix, unknown-
      distribution, relative-root, or malformed matches. Never create or edit
      the association silently.
   4. When an exact association exists, verify that its distribution is listed
      and that its `linux_root` is the existing directory selected for this
      project. Use that distribution and root; do not also translate the drive
      path with `wslpath`.
   5. Without an association, `wslpath` may be used only as a read-only probe in
      each listed distribution. Accept exactly one result only when the
      translated directory already contains a regular `.forge-method.yaml`
      Project Link. That existing link is enough to let the later `start`
      command validate the full identity. If the translated destination has no
      existing Forge identity, or zero/multiple distributions match, stop before
      `start` or `workflow init` and explain that a local association is needed.
      This fail-closed rule deliberately prevents Forge from initializing a
      mounted copy while the user's real project lives elsewhere.
   6. Warm only the selected distribution with
      `wsl.exe -d <distribution> -- true`.
   7. Discover the binary inside that distribution with a constant lookup
      command (`command -v forge-core`, then `$HOME/.cargo/bin/forge-core`).
      Do not hard-code a home directory and do not install or build as fallback.
   8. Execute Forge as separate argv components:
      `wsl.exe -d <distribution> -- <linux-forge-core> <forge-args...>`,
      using the proven Linux root for every Forge `--root` in this chat.

   Retain the exact distribution, Linux root, binary path, and proof source
   (`direct_unc`, `explicit_mapping`, or `existing_project_link`) after this
   single discovery. `/start-forge` and bridge discovery both remain once per
   chat, not once per task. For each later structured Forge argv, replace only
   its first `forge-core` executable component, preserve every later component,
   verify its root is the proven Linux root, and execute those arguments after
   the retained `wsl.exe -d <distribution> -- <linux-forge-core>` prefix rather
   than invoking a missing Windows binary. Never rebuild argv from display text
   or shell-quote a combined command. If executable discovery, root association,
   or exact project identity cannot be proven, report that the bridge is
   unavailable and stop; do not silently switch roots. This fallback is agent-
   operated and host-neutral. Its success does **not** claim official conformance
   for Codex, ZCode, Cursor, Claude, OpenCode, pi.dev, or any other host.

   Record the selected binary's exact `forge-core --version` output. When the
   selected project is the Forge Core source repository itself, detect that
   identity only from its root `Cargo.toml`: it must declare the canonical
   `forge-method-core` repository URL and include `crates/forge-core-cli` as a
   workspace member. Read `[workspace.package].version` directly; do not run
   Cargo just to discover it. If the installed runtime and source versions
   differ, say plainly: "Forge instalado: X; código atual: Y." This is a
   development-version warning, not proof that Forge is inactive. Continue
   read-only activation for an already-linked healthy project, but never claim
   that uninstalled source behavior was dogfooded. A matching version string is
   also not proof that the installed binary came from this checkout or contains
   the same build. For the Forge Core source repository, run the read-only
   `git status --porcelain` check; when it is non-empty, say plainly that the
   source checkout has uncommitted changes. Do not invent commit, build, or
   binary provenance from a version match or a dirty/clean status. Do not build
   or install silently during `/start-forge`; update the runtime only after the
   relevant source block has been tested and accepted.

### Host support is evidence, not a name

Do not treat recognition of Codex, ZCode, Cursor, Claude, OpenCode, pi.dev, or
any other host as proof of support. When the user asks whether a host/adapter is
supported, or when developing one, use `forge-core host-conformance corpus`,
`run`, and `verify` as documented in `docs/host-conformance.md`. The adapter
returns closed observations and typed gaps; Forge calculates the three outcomes.
Do not let an adapter declare its own final support label. Adapter assertions
alone can reach only `partially_supported`; `supported` requires Forge-verifiable
or trusted native proof. Start Forge once per chat, not once per task.

Do not run the full kit on every `/start-forge`. Normal chat activation remains
once per chat. The Windows-to-WSL capability applies only when this Windows host
is targeting a canonical project root inside WSL. A bundle with matching hashes
proves integrity, not native host authenticity.
3. **Run `forge-core start`.** This is the zero-config bootstrap entry point.
   On a fresh repo with no Project Link and an unoccupied, symlink-free target it
   creates the Project Link + sidecar. If linked authority is missing, incomplete,
   inaccessible, or substituted—or unlinked target state already exists—it exits
   nonzero and performs no authority normalization. On a healthy repo it reports
   the current bootstrap state.

   ```bash
   forge-core start --root "<project-root>" --json
   ```

   Read `data.state`, `data.state_loss`, `data.actions_performed`,
   `data.project`, and `data.next_step` from the response. For state loss, verify
   `data.state_loss.schema_version`, treat `diagnosis_digest` as correlation only,
   and act only on a choice whose typed availability is `available_read_only`.
   Never convert a deferred restore or reinitialize-as-new choice into invented
   flags or commands. Use `data.next_step.command` only when explaining the action
   to a human; agents execute the structured argv.

4. **Enter agent-native workflow governance** when the Project Link and sidecar
   are healthy.

   First verify that the installed binary exposes `forge-core workflow --help`.
   For a current binary, every healthy `start` response supplies an exact argv
   shaped as `forge-core workflow init --root <project-root>`. Confirm its root
   matches `data.project.project_root` and execute that returned argv directly.
   `workflow init` is idempotent: a fresh ledger returns initialized and an
   existing ledger returns already initialized without discarding continuity.

   If command discovery exposes `workflow release-status`, run
   **`forge-core workflow release-status --root "<project-root>" --json`** next.
   The ledger-derived release is authoritative; the newest installed binary is
   not. When `data.upgrade_argv` is present, verify it is an argv array shaped
   as `forge-core workflow release-upgrade --root <same-project-root> ...`, has
   no registry/manifest/batch/bundle/release path flags, and execute that exact
   array directly. Do not rebuild it from display text or ask the human to
   select a release. Repeat `release-status` and require the returned active
   release to match the upgrade target. If no successor is available, continue
   without mutation. An older P5c binary that genuinely lacks this subcommand
   may continue on its existing implicit release; ordinary status/upgrade
   errors still fail closed rather than triggering fallback.

   After initialization and the release-status/upgrade check, run
   **`forge-core workflow profile status --root "<project-root>" --json`**
   exactly once. This command is read-only. `start` and `workflow init` never
   silently change an old profile-less project.

   - If `data.solo_adoption=eligible` and this chat has already clearly chosen
     Solo Cooperative (for example, the user asked to dogfood as a solo
     developer with agents), verify `data.adopt_solo_argv` is an argv array
     shaped as `forge-core workflow profile adopt-solo --root
     <same-project-root> --expected-head-digest <sha256> --expected-snapshot-digest
     <sha256> --json`, then execute that exact array as tokens.
   - If the chat has not chosen, ask one short plain-language question: "Este
     projeto antigo deve passar para o modo solo com agentes?" Do not execute
     adoption until the answer is clearly yes.
   - If status says `already_adopted` or `already_solo`, continue silently
     without another write. `already_solo` means the project started in Solo
     Cooperative and needs no migration. If it is `ineligible`, explain the
     returned reason plainly and do not invent a migration. A project explicitly
     configured for a non-solo profile is outside the current dogfood scope.

   After a successful adoption, run profile status once more and require
   `data.current_profile=solo_cooperative` with no adoption argv. The durable
   transition records same-owner provenance only; it never claims verified
   human presence, actor independence, or enterprise compliance. Never rebuild
   the adoption command from display text, change either CAS digest, or retry a
   stale/tampered error without refreshing status.

   Then run **`forge-core workflow resume --root "<project-root>" --json`**.
   `/start-forge` is run once per chat, not once per task. Treat this call as a
   capability probe even when the installed and source version strings match.
   The current capability is present only when
   `data.schema_version=workflow_resume_summary_v2` and `data.detail_argv` is an
   argv array shaped exactly as `forge-core workflow resume --root
   <same-project-root> --full --json`. Validate the root and every token. With a
   direct runtime, execute it verbatim; with the retained WSL bridge, replace
   only argv element zero with the proven packaged/runtime binary and preserve
   all later elements.

   V2 exposes all information needed for the current agent step: the effective
   Domain Pack identity, objective, the complete autonomy projection (including
   binding and input contract), the complete current evaluation, every current
   boundary recheck, human decisions, blockers and warnings, ranked actions,
   active isolations, recoverable promotions, current evidence, complete
   authorization packets, and the complete cooperative packet or exact gap. Its
   omitted-history counts refer only to older audit records, not hidden current
   obligations. Inspect those current fields before doing new work.

   `workflow_resume_summary_v1` is a legacy compatibility response. Validate its
   exact same-root `detail_argv`, use only fields it actually publishes, and
   never treat a v2-only field's absence as proof that no obligation exists.
   Execute its `detail_argv` only when information needed to decide or act is
   unavailable. Never reconstruct missing data, infer capability from matching
   version text, or claim that uninstalled source behavior ran.

   After **every** Forge operation in this same chat, even when its receipt
   embeds a `next` projection, refresh continuity with:

   ```bash
   forge-core workflow resume --root "<project-root>" --json
   ```

   Prefer v2. Use `data.detail_argv` only for an explicit audit or when the
   concise/legacy response lacks information needed for the next safe action or
   reports a blocker that it cannot explain. Execute a published recovery argv
   as tokens before lower-ranked work; `completed` promotions must not be
   applied or recovered again. In the rest of this skill, "refresh workflow"
   means this same-root default `workflow resume` call.

   In v2, `data.human_decisions.recovered_pending` contains only decision events
   actually recorded in the ledger. A question under
   `data.current_evaluation.candidate_decision_requests` was calculated now; do
   not tell the user it came from the previous chat. V1 publishes the same
   distinction as `human_decisions.recovered_pending` versus
   `human_decisions.calculated_now`; the full audit uses
   `replacement_continuity.durable_pending_decisions` versus
   `simulation.candidate_decision_requests`.

   Any unexpected healthy-state argv, integrity, binding, snapshot, ledger, or
   environment error fails closed. Report it; do not erase state, reinitialize
   over an error, reconstruct argv from the display command, or fall back to
   caller-selected routing.

   In v2, read obligations, evidence/capability gaps, Decision Requests, issues,
   and ranked next actions from `data.current_evaluation`, then read continuity
   recovery from `data.actions` and authority from `data.authorization`. Its
   action packets are read-only, current-state authority offers; they are not
   permission to act. V2 publishes the complete packet set. Legacy v1 publishes
   only packet references and an exact `action_packets_argv`; execute that argv
   when the selected governed action needs a packet, rather than treating a
   reference as the packet. `forge-core workflow action-packets --root
   "<project-root>" --json` remains an optional standalone diagnostic for v2
   and full responses. Select only the packet that matches the governed next
   action, satisfy its evidence work, and provide only the packet's closed
   semantic input. Never supply policy, phase, evaluator, target, registry,
   request, attestation, or digest fields yourself. The host agent performs the
   action and refreshes the workflow again. The human stays in chat and never
   operates Forge commands or edits Forge artifacts.

   When work has been completed in a Forge isolation, preview it by the
   isolation id rather than by supplying an ambient worktree path:

   ```bash
   forge-core workflow promotion preview --root "<project-root>" \
     --isolation-id "<active-isolation-id>" --json
   ```

   Treat the returned document as a read-only candidate only. Inspect its exact
   Git worktree/common-repository/branch/HEAD binding,
   source/destination snapshots, objective and evidence bindings, final
   observation validity, diff, write set, linked-claim path attribution,
   ambient conflicts, destructive deletes, excluded roots (including blocking source-side build/dependency caches), unsupported
   created-file metadata and directory/type/mode effects, carried assurance
   gaps, apply eligibility, and unresolved blocking gaps. A preview digest is
   CAS identity, not authority. If it is `eligible_local_reversible`, run:

   ```bash
   forge-core workflow promotion apply --root "<project-root>" \
     --isolation-id "<active-isolation-id>" \
     --expected-preview-digest "<preview_digest>" --json
   ```

   Never copy or merge the worktree manually. Accept success only with the
   durable receipt and canonical readback. An exact retry may return
   `already_committed` without another write. If Forge reports
   `typed_failure.type=recovery_required`, preserve the canonical tree and
   Forge sidecar. Execute `typed_failure.data.recovery_argv` directly only when
   `typed_failure.data.can_recover=true`; its components are already safe for
   paths containing spaces. Its canonical shape is:

   ```bash
   forge-core workflow promotion recover --root "<project-root>" \
     --isolation-id "<active-isolation-id>" \
     --expected-preview-digest "<preview_digest>" --json
   ```

   Never rebuild that argv from the diagnostic. If the recover command fails,
   it returns `can_recover=false` and no recovery argv: stop instead of
   looping. Do not ask the human to generate a replacement preview. Recovery compares
   the durable record with the real old/new bytes, completes only an
   unambiguous attempt, never reapplies a committed effect, and converges to
   one verified receipt. Corrupt, third-content, mismatched, or rolled-back
   state fails closed; report the plain-language diagnostic without editing
   either root. The generic effect-WAL recovery path remains forbidden for this
   split-root WAL. A legacy v1 pre-Begin intent did not store its historical
   observation timestamp. Recovery verifies that opaque self-digested intent,
   requires a fresh preview with the same semantic digest and an exactly
   unchanged destination, and records the fresh execution honestly; it never
   claims to have recreated the missing historical observation.

   A packet whose approval boundary is exactly `cooperative_same_owner` is the
   Solo Cooperative missing-objective lane. It is not human-origin authority.
   Read the packet's dedicated `cooperative_objective` input contract: it
   carries the two JSON variants, UTF-8 file encoding, byte/item limits, and a
   structured argv template. Do not reinterpret the packet as the legacy
   `intent_revision` JSON shape.

   - If the ordinary chat objective is unambiguous, materialize the
     `unambiguous` template into a temporary JSON file. Fill only outcome,
     constraints, unacceptable outcomes, open uncertainties, same-owner
     carrying principal, and generic host/session/interaction coordinates.
     The human does not edit the file or run a command.
   - If one irreducible value choice remains, materialize the
     `decision_required` template instead. Forge validates the current Solo
     profile, packet, snapshot, ledger head, boundary, and missing-objective
     state read-only, then returns the same typed Decision Request with zero
     workflow writes. Ask that concise question in chat; do not guess.
   - Execute the packet's argv array exactly, substituting its current packet
     digest, project root, and temporary-file path while preserving argument
     boundaries. Its canonical shape is:

   ```bash
   forge-core workflow intent accept-cooperative --root "<project-root>" \
     --packet-digest "<packet-sha256>" \
     --input-file "<temporary-cooperative-input.json>" --json
   ```

   Delete the temporary host file after Forge has read it. An exact retry is
   idempotent; a changed payload or stale packet must fail closed. On either
   `accepted` or a later chat answer, refresh the workflow; never cache
   the pre-acceptance packet. Never describe this lane as verified human
   presence, external origin, reviewer independence, or enterprise compliance.

   Once an objective is active, do not treat routine task changes as objective
   changes. A later material correction from chat uses the separate
   `authorization.objective_management_packet` and its `material_supersession`
   template, including the bounded reason and complete corrected proposal. A
   detail-only clarification uses `non_material_clarification`; provide only
   additions, because Forge preserves the outcome and all prior lists. Neither
   packet replaces the governed ranked next action. After acceptance refresh
   the workflow: prior head/digest-bound authority is stale, while immutable
   historical records remain auditable. On replacement, report the active
   objective's revision, `revision_kind`, `revision_reason`, and predecessor
   digest before continuing.

   With an active objective, read `data.agent_autonomy` from the v2 workflow
   resume response (or the legacy full response when v2 is unavailable).
   Its binding is read-only and exact to the current objective revision/digest,
   assurance epoch, project snapshot, ledger head, and state version. The agent
   now owns research, analysis, planning, strategy, reversible local edits,
   tests, verification, documentation, implementation tactics, file choice,
   work order, retries, read-only external research, reversible local commits,
   and local evidence generation. Generating evidence is autonomous; its
   admission, promotion, publication, and any authority claim remain governed.
   Do not ask the human to approve those implementation choices and do not
   create an authorization packet per edit, test, or task.

   Ask the human only for one of the four closed classes:
   `product_objective_change`, `material_tradeoff`,
   `material_risk_acceptance`, or `irreversible_or_external_effect`. Publication,
   remote push/merge, deployment, production mutation, secret use, destructive
   external effects, and non-destructive external mutations such as Slack/Jira/
   email sends, HTTP writes, or staging-system changes require an explicit human
   decision. A product-objective change must then use the objective supersession
   flow above; never disguise it as a tactic change.

   `data.agent_autonomy.assessment_argv` is the execution form and
   `input_contract` is the machine-readable contract. The host must derive the
   required effect descriptor from the selected tool and concrete operation,
   independently of the model's work-class claim; never infer it from free-form
   task text alone. Unknown/ambiguous effects and contradictory class/effect
   pairs fail closed. Keep the temporary input outside the project snapshot.
   Run the optional assessment only at a real semantic/effect boundary, never
   per edit/task. Refresh its binding after objective, assurance-epoch, snapshot,
   ledger-head, or state-version change. Local staging/commit stays autonomous;
   remote push is protected. Assessment performs zero Forge state writes.

   Enterprise-origin authorization, hardware-backed presence, independent
   reviewer/runtime custody, and compliance signing are outside the current Solo
   Cooperative skill path. Do not run credential, broker, trust, rotation,
   revocation, external-envelope, or signing commands during ordinary solo
   activation or work. Missing enterprise registries are non-blocking metadata
   under `readiness_profile=solo_cooperative`. If Forge reports one as a blocking
   solo setup gap, report product/profile drift and stop rather than asking the
   developer to provision enterprise infrastructure.

5. **Fallback for a binary without the `workflow` command.** Use this only when
   command discovery proves the selected executable lacks workflow governance;
   do not treat an ordinary workflow error as a version fallback.

   - Report that executable P5 workflow governance is unavailable and recommend
     upgrading Forge Core.
   - `guide describe`, `guide status`, `guide decide`, and a compatible
     `start.data.next_step.argv` may be used only for read-only legacy
     orientation or diagnostics.
   - Label their output `legacy_compatibility_only`. It cannot authorize a P5
     workflow, phase transition, completion, readiness claim, or mutation.
   - Do not invent an authoritative workflow choice from legacy output. Stop
     before authority-bearing work and tell the user what capability is absent.

6. **Keep output useful to the project owner.** Lead with the guided activation
   explanation above, not Forge internals. Include a short **Forge status** after
   the project orientation: active, unavailable, or blocked, plus the material
   consequence. Show executable argv, release identifiers, bootstrap states,
   Project Link paths, and sidecar paths only when the user asks for diagnostics
   or when one of those details is necessary to repair a failure. Do not expose
   private attestation material, present a legacy recommendation as authority,
   or ask the human to select a workflow or release.

## Safety checks

- Do not run broad cleanup or delete any existing Forge state unless the user
  explicitly asks for cleanup.
- Never execute `data.next_step.command` through a shell. Prefer the response's
  argv array and preserve every argument boundary, especially roots containing
  whitespace or shell metacharacters.
- Do not pass `--allow-bootstrap-core` for ordinary consumer projects; it is
  reserved for the Forge core repo itself.
- Do not pass workflow, phase, readiness target, registry, manifest, batch,
  bundle, or release-path selectors to agent-native workflow commands. A
  release id is permitted only inside the exact CAS-bound `upgrade_argv`
  returned by `release-status`.
- Do not provision or invoke deferred enterprise trust/signing infrastructure
  from the Solo Cooperative skill path.
- Do not initialize inside system folders, package caches, or temporary folders
  unless the user explicitly selected that root.

## Installing this skill

This file is the canonical source. Save it wherever your host agent reads skills.
Common conventions include `~/.pi/agent/skills/` (Pi),
`~/.agents/skills/` (Codex-compatible hosts), an MCP tool, or a project-local
`.skills/`. Pick the location your agent runtime expects.

### Cooperative evidence

Activate this behavior once when `/start-forge` begins the chat; do not ask the
user to restart Forge for each task.

After an objective is accepted, always inspect the fresh v2
`data.actions.cooperative_evidence_packet`. The packet's `route.target` decides
what the agent must assess; a missing `target` is legacy `source_claim`:

1. For `policy_applicability`, inspect the repository evidence named in the
   assessment basis and choose exactly one honest result:
   - `applicable`: the policy remains selected and Forge may next publish a
     `source_claim` packet;
   - `not_applicable`: Forge skips that policy while the content-addressed basis
     remains current;
   - `inconclusive`: progression stays at `applicability_required` and a fresh
     packet is required after more evidence exists.
   This is a technical same-owner assessment by the agent. Do not ask the human
   to read a diff or make this technical judgment unless the result exposes a
   genuinely material product, trade-off, risk, external, or irreversible
   decision. It never satisfies a policy claim or capability and never proves a
   human or independent review.
2. For `source_claim`, use the packet's declared assessment contract. A current
   supporting source-claim record can suppress another source packet; an
   applicability record cannot. `pass`, `fail`, and `inconclusive` must describe
   what the retained repository basis actually supports.
3. If no packet is present, inspect
   `data.actions.cooperative_evidence_gap` exactly. Do not invent a route or fall
   back to a different policy, claim, or evaluator. V1 does not publish the
   complete packet or gap: execute its validated `detail_argv` and inspect the
   equivalent full fields.
4. Copy `offer_template` to a temporary file **outside** the project snapshot.
   Replace every token named by `required_replacements` with an honest bounded
   value. For the selected assessment object, replace `basis_paths` with one or
   more project-relative regular-file paths and set `limitations` to any known
   bounded limitations. Do not edit its binding, route identity, producer,
   subject, schema, or scenario fields.
5. Execute the published `argv` as tokens. Its `--root` is already the exact
   project root; replace only `input_file_token` with the temporary file path.
   Never join, shell-parse, or reconstruct the vector.
6. Delete the temporary input and immediately run default `workflow resume`.
   Treat `rejected` as audited non-support, not success. Use only the new packet
   after any rejection, inconclusive result, expiry, objective/policy change, or
   basis drift; never edit and retry a stale binding.

Applicability validity is scoped to its content-addressed basis: unrelated
project changes do not invalidate it, a changed basis does, and a superseded old
answer never revives merely because earlier bytes return. Same-owner admission
must never be relabeled as independent review, trusted-runtime separation,
human presence, tamper-resistant proof, official host support, or compliance.

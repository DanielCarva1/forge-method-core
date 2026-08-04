# Getting started

## What the human does

The human talks to a host agent. They do not need to select workflows, edit
Forge YAML, operate the sidecar, or run governance commands. A typical request
is:

> Start Forge for this project, tell me what you need from me, and keep using
> Forge as you work.

The host agent must have the `forge-core` executable and canonical
`start-forge` skill available. The skill lives at
`skill/start-forge/SKILL.md` in this repository.

## Install the executable

### Prebuilt release

Download one archive for the host platform from the selected GitHub Release
and place both `forge-core` and its `forge` wrapper on `PATH`. Verify its
checksum, Sigstore bundle, embedded version, and `RELEASE-MANIFEST.json` as
described in the [root README](../README.md#install-and-start). Never use a
historical version label in a live guide as evidence of current availability.

New-format archives bind package version, exact release tag, exact source
commit, canonical skill, selected guides, and every payload file. Inspect the
selected archive itself; never infer its payload from a different source
checkout or from tag text alone.

### Source checkout

```bash
git clone https://github.com/DanielCarva1/forge-method-core.git
cd forge-method-core
cargo install --path crates/forge-core-cli --force
forge-core --version
```

The workspace package version and latest published binary tag can differ. The
workflow release pin and Domain Pack effective epoch differ from both. Use the
canonical [four-identity table](../README.md#four-identitiesdo-not-collapse-them),
select an exact source commit/tag, and verify `forge-core --version`.

## Install the host skill

Copy `skill/start-forge/SKILL.md` to a location recognized by the host agent.
Common locations include:

```text
Pi:              ~/.pi/agent/skills/start-forge/SKILL.md
Codex-compatible: ~/.agents/skills/start-forge/SKILL.md
```

Other hosts use different skill/plugin locations. Forge does not silently
write there because that would cross a host-owned boundary. The host agent
confirms both the skill path and binary version before claiming readiness.

## Start or resume a project

The agent runs one idempotent bootstrap command per chat:

```bash
forge-core start --root <project> --json
```

It executes returned structured argv—not the display string—then follows:

```text
start
  -> workflow init
  -> workflow release-status and exact returned upgrade_argv, if any
  -> workflow profile status
  -> workflow resume
  -> explain the project and current next-best action in plain language
  -> record an agreed missing objective through the cooperative packet, if needed
  -> perform and verify the highest-ranked feasible action
  -> workflow resume
```

The human-facing response is not a dump of those internal steps. The agent keeps
its explanation in the language already used by the human, without alternating
languages in ordinary prose. Exact commands and identifiers may keep their
source spelling, but useful technical detail is paired with its practical meaning
in plain language.

A healthy orientation normally answers four things compactly:

1. what the project is and whether it is healthy now;
2. what changed recently and what is already planned;
3. what remains, what the next step is, and why;
4. whether the agent needs one decision from the human.

When no genuine human decision is needed, the agent continues with the safest
highest-ranked feasible action in the same turn and reports the verified result.
It does not stop after announcing a future action. When a decision is genuinely
required, it explains the context, options, consequences, and recommendation,
then asks exactly one concise question.

During ordinary development, the human does not need to tell the agent to
research. If a doubt could change the desired outcome, expose an unacceptable
outcome, alter material risk, or make the next action unsafe, the agent performs
proportionate read-only research on its own. It checks project evidence first,
uses credible external sources when needed, compares competing explanations and
contrary evidence, explains the result and its practical impact, and keeps
working. It returns to the human only if a real product, trade-off, risk, or
external-effect decision remains.

The current onboarding target is `solo_cooperative`. External-origin brokers,
FIDO-backed presence, independent identities, and compliance signing are
enterprise-profile work and are not part of this loop.

The consumer repository receives only `.forge-method.yaml`. Default runtime
state is `<project-parent>/forge-<project-id>/.forge-method/`, inside the sibling
sidecar `<project-parent>/forge-<project-id>/`. Never create a consumer-local
`.forge-method/` manually. `preflight init` follows the Project Link and stores
its profile as `<state-root>/preflight.yaml`. Advanced state and recovery
locations are documented in the [Operator guide](operator-guide.md#state-and-ownership).

## What healthy output means

Healthy bootstrap proves that Project Link and sidecar resolve. It does not
prove every product capability, domain method, or piece of evidence exists.
`workflow resume` may correctly return obligations, evidence/capability gaps, a
gated product decision, a Domain Pack requirement, or a rebase/upgrade
requirement. Missing enterprise broker/signature setup is not a Solo
Cooperative gap.

After the objective is accepted, `workflow resume` exposes all eight universal
quality lenses and their current `unknown`, `supported`, `verified`, `disproven`,
or `waived` state. In `solo_cooperative`, unresolved lenses stay visible
throughout development but block only the final boundary that actually requires
them, rather than every ordinary development step.

The agent is responsible for proposing the method and representative checks,
including scenarios, ways the idea could be wrong, environment, and failure
modes. The human is not expected to author those technical details. When the
selected claim explicitly calls for repository inspection, the solo agent may
record its assessment plus project-relative basis files; Forge confines, reads,
and hashes those exact files before admitting the result. This is useful
same-owner technical evidence, not independent review, human presence,
representative runtime execution, or enterprise compliance. Those stronger
claims still require their own matching evidence. Basis is read only from exact regular-file handles in the retained project snapshot; links, outside paths, and excluded roots (`.git`, `.forge-method`, top-level `.local`, `target`, and `node_modules`) are rejected. For one objective/snapshot/route, the newest admitted assessment supersedes the previous one, but a rejected offer does not. Only a current pass supplies source satisfaction and is bound by exact record digest into completion; fail and inconclusive remain honest non-support, and expiry, supersession, or snapshot/basis drift removes completion validity. This exact retained read/hash route can establish applicability and an executable `LocalCommand` alternative, but never independent review or runtime separation.

A top-level `.local` directory is local-only only when it already exists before
workflow evidence capture. Changes to files inside that existing directory do not
change the workflow snapshot, while nested paths such as `src/.local` remain
governed. Creating or removing top-level `.local` after capture changes the root
namespace, stales the evidence, and requires a fresh admission; create the
Evidence admitted under an older snapshot projection may become stale after the
projection is corrected and must then be re-admitted; its historical ledger
record remains structurally valid.

Forge can describe only a write that passed its claim/gate, verified-principal,
Admission, WAL/recovery, and receipt path as **Forge-mediated**. A host agent's
editor or shell write is direct/ungoverned unless that transaction covers it.
A successful bootstrap or transcript does not change this boundary.

## Updating

Source installs:

```bash
git pull --ff-only
cargo install --path crates/forge-core-cli --force
forge-core --version
```

Prebuilt installs must replace binary and wrapper from the same release. A
newer binary never silently migrates project authority; the agent follows
`workflow release-status` and only executes its exact CAS-bound `upgrade_argv`.

## Recovery

- Re-run `start` in a new chat. If it reports `data.state_loss`, preserve the
  Project Link, sidecar namespace, and operator roots; do not recreate them.
- Read the independently versioned `data.state_loss.schema_version`,
  `diagnosis_digest`, and typed `choices`. Only `inspect` is currently
  `available_read_only`; execute its argv directly when inspection is needed.
- C2.2 now exposes explicit complete-state backup and restore commands:
  - backup creation: `forge-core backup create`;
  - backup verification: `forge-core backup verify`;
  - restore preflight: `forge-core restore preflight`;
  - restore application: `forge-core restore apply`.
  Always verify the exact archive and authority identity before applying a
  restore; source compilation is complete, while runtime, interruption,
  mixed-version, platform, and hosted evidence remains pending.
- The `restore_verified_backup` choice in the state-loss diagnosis remains a
  diagnosis choice rather than implicit authority or automatic execution; use
  the explicit restore preflight/apply surface. `reinitialize_as_new` is
  separately deferred, explicitly abandons prior authority, requires operator
  confirmation, and requires a different project identity and authority
  location. It does not publish executable argv.
- `start` retries and `project init` are not recovery paths and cannot normalize
  linked missing or partial state. Automatic bootstrap requires both no Project
  Link and an unoccupied, symlink-free target state path; preexisting sidecar
  state is preserved for explicit inspection.
- Use `workflow resume` once at the start of a new chat or after agent/process
  replacement. Its default summary returns the current objective, decisions,
  blockers and warnings, active work, recovery work, evidence, and ranked next
  action without repeating older history. Use `workflow report` only when the
  human explicitly requests a complete history/audit or a continuity diagnosis
  requires omitted records. Questions calculated now remain separate from
  recovered decisions. `resume` does not use the old chat, create missing locks,
  finish a release rebase, reconcile a generation, or repair inconsistent state.
- Use `domain-pack status` and `domain-pack recover` for lifecycle recovery.
- Do not delete the sidecar to fix an integrity error; preserve and inspect it.


For installation, state ownership, backup, and recovery details, see the
[Operator guide](operator-guide.md).

## Is my agent host supported?

Do not trust a product-name list. Run the public journey kit described in
[Host conformance](host-conformance.md). It reports the eight capabilities
separately, so one missing API does not hide what already works.

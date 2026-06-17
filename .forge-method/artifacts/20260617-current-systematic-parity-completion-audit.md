# Current systematic parity completion audit

- kind: internal-parity-audit
- created_at: 2026-06-17
- scope: external guided-method benchmark to Forge Method Core 1.30.0
- public_boundary: internal benchmark artifact only; do not copy benchmark language into public Forge docs, patch notes, README, or release notes
- current_verdict: high parity for guided human flows and stronger Forge-native agent runtime substrate; P2 utility surfaces are now translated as opt-in Forge-native contracts, with active execution still intentionally disabled by default

## Source snapshot

Current external docs were fetched into `%TEMP%/forge-systematic-parity-current` and summarized without loading the raw files into the agent context.

| Source | URL | Bytes | sha256_16 | command-like tokens |
|---|---|---:|---|---:|
| method | https://docs.bmad-method.org/llms-full.txt | 206541 | 816bac1d3f4dc888 | 66 |
| builder | https://bmad-builder-docs.bmad-method.org/llms-full.txt | 204963 | 94f67c7238ac4ae0 | 33 |
| cis | https://cis-docs.bmad-method.org/llms-full.txt | 79034 | b6690c87a065d5ff | 17 |
| game | https://game-dev-studio-docs.bmad-method.org/llms-full.txt | 82619 | a80861995cdb7fee | 26 |
| test architecture | https://bmad-code-org.github.io/bmad-method-test-architecture-enterprise/llms-full.txt | 514245 | d6bea3fdb473df50 | 50 |

## Forge inventory inspected

| Surface | Count | Notes |
|---|---:|---|
| compact workflow refs | 99 | state-machine references under `skills/forge-method/references` |
| facilitation packs | 29 | rich human guidance under `skills/forge-method/facilitation` |
| templates | 74 | durable artifact and handoff shapes |
| modules | 8 | software, creative, game, runtime-builder, test, enterprise, launch, etc. |
| agent/profile surface | 8 | compact agent profiles and persona overlays |
| repo scripts | 18 | install, smoke, verify, clone-install, release helpers |
| runtime scripts | 4 | runtime launcher/helper scripts |
| guidance replay fixture | 97 cases | benchmark-shaped transcript/routing replay after this audit patch |
| docs | 23 | quickstart, distribution, architecture, operation docs |

## Requirement audit

| Requirement from objective | Current evidence | Status | Remaining risk |
|---|---|---|---|
| Rich guided human experience | Guidance Engine emits intent, route reason, human prompt, alternatives, persona lens, facilitation pack; 29 packs cover discovery, brainstorm, evidence, product, creative, game, story, enterprise, lifecycle, runtime-builder. Replay covers 23 families and 97 transcript cases. | translated/proved by replay | Real tester feedback can still expose tone/taste gaps that fixtures do not predict. |
| Compact agent documentation | 99 workflow refs keep `trigger`, `inputs`, `steps`, `outputs`, `done_when`, `blocked_when`, `handoff`; rich language lives in packs and guide output. `workflow validate` passed. | translated/proved structurally | Need periodic compactness audit as packs/workflows grow. |
| Guided brainstorm/research/correct-course | Dedicated routes and packs for `brainstorming`, `market-scan`, `domain-scan`, `technical-feasibility-scan`, `problem-solving`, `investigation`, `correct-course`, context recovery. | translated/proved by replay | Keep adding failing real transcripts, not generic prose. |
| Product/spec/UX/architecture/story cycle | `discover-intent`, `product-requirements`, `write-spec`, `ux-plan`, `architecture`, `readiness-check`, `plan-sprint`, `story-creation`, `build-story`, `code-review`, review findings and evidence. | translated/proved by replay and workflow validation | Broad end-to-end story-cycle smoke remains heavier than replay. |
| Creative and game guided flows | Creative direction, design-thinking, innovation, storytelling/presentation narrative, game brief, GDD, engine setup, game sprint, playtest, game QA, game E2E, game test automation. | translated/proved by replay | Engine-specific examples should remain project-driven unless repeated gaps appear. |
| Test architecture / enterprise quality | Test strategy, risk, fixtures, test design, ATDD, automation, NFR, traceability, CI quality, enterprise artifact maps, readiness, release gate, API/browser utility contract. | translated/proved by replay and artifact commands | Utility execution remains project/provider-specific and opt-in. |
| Builder/factory/customization | Runtime-builder, agent/workflow/module builder, module validate/distribution, config customization, capability index, persona overlays, elicitation techniques. | translated/proved by replay/config validation | Isolated Docker eval runner remains deferred. |
| Scripts/install/package validation | `verify-fast`, `verify-all`, `smoke-runtime`, `smoke-install`, local install, clone-install smoke; published 1.30.0 was validated from `main` and `v1.30.0`. | translated/proved for current plugin | Full release check currently fails only because the worktree is dirty during this audit. |
| Hooks/local runtime helpers | Launcher/updater/install/smoke helpers exist; `hook-event-plan` plus opt-in dispatch scripts translate hook/event behavior without hidden startup work. | translated as opt-in surface | Always-on hooks remain intentionally out of the Codex plugin. |
| Persistent personal memory | Forge provides project-local state, artifacts, checkpoints, context packs, ledger. | non-goal for current runtime | A personal workspace memory product would need a privacy/product decision first. |
| Visual deck/presentation production | Forge routes narrative/pitch/deck structure through storytelling and presentation lens. | folded/deferred | Actual deck production is outside current Codex-native runtime scope. |

## Current parity matrix

| Capability family | Forge-native translation | Human experience contract | Agent/runtime contract | Status |
|---|---|---|---|---|
| Help/oracle/routing | `preflight`, `start`, `resume`, `guide`, Help Oracle, Guidance Engine | ask the next useful question, recover stale context, do not make humans know phases | JSON route, commands, state updates, stale-state guard | translated |
| Brainstorm/elicitation | `brainstorming`, elicitation techniques, persona lenses | stay divergent until options, criteria, rejects, and anti-goals exist | compact artifact/template and replay cases | translated |
| Research/evidence | market/domain/technical scans, Reality/Evidence Gate, research closeout | test adoption, feasibility, legal/ethical risk, and alternatives before build | research-scan artifact, source/evidence fields, checks | translated |
| Product/spec/UX/architecture | PRD/spec kernel/UX/architecture/readiness | shape WHAT and taste before HOW; challenge weak assumptions | templates, source trace, story impact, gates | translated |
| Story cycle | sprint planning, story creation, build-story, review, retrospective | move autonomously once decisions are accepted | state, sprint, story files, review findings, evidence | translated |
| Creative suite | creative session, design thinking, innovation strategy, storytelling | provocative creative facilitation without turning into generic planning | packs/templates/persona lens | translated |
| Game studio | game brief, GDD, engine architecture/setup, game sprint, playtest, QA, E2E | preserve fun, player experience, prototype proof, engine reality | game artifacts, checks, replay families | translated |
| Test/quality/enterprise | test architecture, ATDD, NFR, traceability, CI, enterprise release gates | convert quality anxiety into risk-ranked proof | artifact commands, gate inputs, validation evidence | translated |
| Builder/factory | runtime-builder, module/agent/workflow builder, module distribution | analyze behavior before scaffolding; split human and agent contract | catalog/workflow/template/config validation | translated |
| Config/capability/persona | config customization, capability index, persona overlays | let teams adjust behavior without making the method mushy | validated override model and generated index | translated |
| Docs/index/shard | doc-index, doc-shard, project-context, session-prep | make docs usable without context dumping | source fingerprint, mtime, precedence, context pack | translated |
| Install/distribution | local install, marketplace/package docs, clone install smoke | make tester install obvious and verifiable | smoke scripts and published version checks | translated |
| Isolated eval runner | `isolated-eval-runner`, `runtime-utility` pack, template, opt-in runner scripts | choose local/container/remote/waiver mode explicitly | no always-on runner; Docker execution requires opt-in flag | translated as opt-in |
| Hook/event wrappers | `hook-event-plan`, `runtime-utility` pack, template, opt-in dispatch scripts | make event, payload, side effects, dry-run, timeout, rollback visible | no hidden startup hook; dispatch requires explicit command | translated as opt-in |
| Generic API/browser utilities | `api-browser-utility`, `runtime-utility` pack, template, replay proof | shape reusable provider helpers with auth, fixtures, assertions, cleanup | implementation remains project/provider-specific | translated as opt-in |

## Findings

1. Release/version skepticism was under-routed before this audit. A user complaint like "the repo still shows 1.28; were you not going to validate?" previously stayed too close to the active workflow. Patch: version/GitHub/tag validation wording now routes to `release-readiness`; replay case `release_version_validation_complaint` proves it.
2. The current plugin has strong parity for guided flow families. The prior P2 gaps are translated as opt-in contracts and scripts, not always-on services, preserving the Forge/Codex overhead boundary.
3. Existing parity artifacts are useful but were not completion proof. This audit upgrades the current evidence snapshot and separates translated/proved, deferred, and non-goal surfaces.
4. The next real proof is not more generic docs. It is failing transcripts from users and focused fixtures that catch guidance energy, pacing, and route regressions.

## Validation from this audit

- `python skills/forge-method/scripts/forge_method_runtime.py workflow validate --root .` -> passed
- `python skills/forge-method/scripts/forge_method_runtime.py config validate --root .` -> passed
- `python skills/forge-method/scripts/forge_method_runtime.py parity replay --json` -> 97/97 passed after adding `release_version_validation_complaint`
- `python skills/forge-method/scripts/forge_method_runtime.py release check --root . --touches docs --json` -> failed only on `git_clean` because this audit intentionally has uncommitted state/artifact/runtime changes
- Clean clone/install smoke for `main` and `v1.30.0` was recorded in `.forge-method/evidence/20260617-175907-validation-validate-published-version-1-30-0.md`

## Required next work

1. Validate the opt-in P2 scripts on Windows and POSIX where available.
2. Decide later whether any opt-in contract deserves a first-class runtime artifact command; do not add one without repeated real project need.
3. Keep collecting real human transcripts and add fixtures when Forge feels cold, premature, or too mechanical.
4. Do not add always-on hooks, background runners, or generic provider helpers to normal Forge startup without a concrete product decision.

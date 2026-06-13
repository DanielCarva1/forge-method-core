# BMAD to Forge systematic parity audit

- kind: internal-parity-audit
- created_at: 2026-06-12
- status: first systematic pass, not completion proof
- scope: BMAD Method core, BMAD Builder, Creative Intelligence Suite, Game Dev Studio, Test Architecture Enterprise
- summary: Systematic first-pass parity audit comparing BMAD Method, Builder, CIS, Game Dev Studio, and TEA against Forge principles, workflows, facilitation packs, runtime contracts, scripts, state, and validation.
- Forge principle: translate behavior, not brand. BMAD is an internal benchmark; Forge remains a Codex-native file-backed runtime.

## Sources inspected

Primary BMAD docs:

- `https://docs.bmad-method.org/llms-full.txt`
- `https://bmad-builder-docs.bmad-method.org/llms-full.txt`
- `https://cis-docs.bmad-method.org/llms-full.txt`
- `https://game-dev-studio-docs.bmad-method.org/llms-full.txt`
- `https://bmad-code-org.github.io/bmad-method-test-architecture-enterprise/llms-full.txt`

Local Forge sources:

- `skills/forge-method/SKILL.md`
- `skills/forge-method/catalog/workflows.json`
- `skills/forge-method/references/workflow-*.md`
- `skills/forge-method/facilitation/*.md`
- `skills/forge-method/modules/*.yaml`
- `skills/forge-method/agents/profiles/*.yaml`
- `skills/forge-method/scripts/forge_method_runtime.py`
- `tests/fixtures/guidance_transcripts.json`
- `tests/test_runtime.py`

Extraction notes:

- BMAD Method core docs expose 65 `bmad-*` tokens, including about 48 direct workflow/agent commands.
- BMAD Builder docs expose module setup, agent builder, workflow builder, module builder, validation, customization, eval runner, session prep, progressive disclosure, subagent, memory, and packaging patterns.
- CIS docs expose 5 core creative workflows plus 6 named creative agents.
- Game Dev Studio docs expose 11 slash workflow commands plus engine setup, project context, game-specific lifecycle, playtest/performance/testing patterns.
- TEA docs are much larger than the others and focus on engagement models, fixture architecture, quality workflows, traceability/gates, NFR, CI, automation, API/browser utilities, and enterprise quality governance.
- Forge currently exposes 77 catalog workflows, 8 modules, 7 generic agent profiles, 11 facilitation packs, runtime state, ledger, checkpoints, evidence, evals, smoke scripts, install smoke, and Guidance Engine routing.

## Audit method

Parity is not name matching. A BMAD capability counts as translated to Forge only when Forge has:

1. a human-facing facilitation contract that makes the experience guided and useful;
2. a compact agent contract: workflow state machine, JSON/runtime output, state update, handoff, or script command;
3. durable state or artifact behavior when the capability changes progress;
4. validation evidence: fixture, unit test, smoke, workflow validate, or artifact gate;
5. a route from `$forge-method`, `guide --question`, `resume`, or another supported runtime command.

Parity labels:

- strong: Forge has route, workflow/state, artifact behavior, and tests or smoke coverage.
- partial: Forge has a concept/workflow but missing human richness, routing, state coupling, or validation depth.
- missing: no credible Forge equivalent yet.
- advantage: Forge is materially stronger for the Forge principle.

## Executive verdict

Forge is not at complete BMAD parity yet.

Forge has a stronger native runtime substrate than BMAD-style loose skill chaining: file-backed state, ledger, evidence, checkpoints, context recovery, runtime JSON, packaged workflow catalog, smoke scripts, install validation, and Guidance Engine. That is the correct Forge advantage.

The weak side is still BMAD's strength: the human guidance layer is not systematically rich across every workflow, and BMAD's help/status ritual is more consistently present at the end of workflows. Forge has many equivalent workflow names, but several are compact agent state machines without an equally strong facilitation pack, transcript fixture, or automatic "what next" experience.

The product direction is therefore:

- keep Forge's compact agent runtime;
- deepen the human packs and route behavior;
- make BMAD-style "next required workflow" a runtime invariant, not agent taste;
- translate BMAD modules into Forge workflows, packs, scripts, tests, and artifacts instead of copying public language.

## System parity matrix

| BMAD capability family | Forge equivalent now | Parity | Gap | Forge translation required |
| --- | --- | --- | --- | --- |
| `bmad-help` project-aware guidance | `preflight`, `start`, `resume`, `guide`, Guidance Engine | partial | Guide exists, but "runs after every workflow and always recommends the next required workflow" is not enforced as a universal runtime invariant. | Add end-of-workflow guidance contract: every state-changing command returns/records next required workflow, alternatives, and stale-state guard. |
| Workflow status / workflow map | `.forge-method/state.yaml`, `sprint.yaml`, `status`, `snapshot`, `context plan` | strong | Human explanation can still be dry compared to BMAD Help. | Keep compact JSON, improve human summaries and status affordances. |
| Planning tracks: Quick Flow / Method / Enterprise | Forge modules: software-builder, enterprise, runtime-builder, launch-ops, game-studio, test-architect | partial | Tracks exist but route depth is not as explicit as BMAD's track decision tree and required docs per track. | Add route decision artifact and track-specific required/optional workflow map. |
| Analysis: brainstorming | `brainstorming` workflow + facilitation pack | strong-ish | Need transcript tests for richer guided divergence, not just route. | Add fixtures for option generation, constraint capture, rejected directions. |
| Analysis: market/domain/technical research | `market-scan`, `domain-scan`, `technical-feasibility-scan`, Reality/Evidence Gate | strong | Need stronger "research closeout recommends next workflow" invariant. | Add research completion handoff template. |
| Product brief / PRFAQ | `discover-intent`, `product-requirements`, `reality-evidence-gate` | partial | No full PRFAQ/working-backwards stress-test workflow. Product brief is less coached. | Add `prfaq` or `working-backwards-challenge` workflow and rich facilitation pack. |
| PRD create/update/validate | `product-requirements`, `write-spec`, `spec-distillation`, `grill-gate` | partial | Missing BMAD-like create/update/validate modes, addendum, decision log, HTML/findings-style validation. | Expand product requirements pack and workflow metadata with modes and validation artifact. |
| Advanced elicitation | facilitation packs, `council-decision`, `grill-gate` | partial | Techniques are not consistently exposed as selectable moves in every discovery/planning flow. | Add facilitation technique index and routeable elicitation options in packs. |
| UX design | `ux-plan`, some game UX | partial | No rich UX designer workflow equivalent with EXPERIENCE/DESIGN style outputs and human taste calibration across product work. | Add UX facilitation pack, UX artifact template, transcript fixtures. |
| Architecture | `architecture`, `engine-architecture`, `security-plan`, enterprise plans | partial | Core architecture exists but less agent/persona guided and less tied to PRD validation. | Add architecture pack and readiness checks linking PRD/UX/security/story plan. |
| Project context generation | `context plan`, `context recover`, `current-pack`, `AGENTS.md` | advantage/partial | Forge has better runtime context packs, but lacks a user-facing `generate-project-context` workflow for existing/new code. | Add project-context artifact workflow that captures tech conventions for future agents. |
| Epics and stories after architecture | `create-epics`, `plan-sprint`, `game-story-creation` | partial | Previous bug created stories too early; fixed for new projects. Need stronger invariant that story generation requires approved decision artifacts. | Add tests/gate preventing stories before required track artifacts. |
| Implementation readiness | `readiness-check`, `gate`, `ready`, `release-readiness` | partial | Checks exist but not as cohesive multi-doc "can implement" review. | Add readiness matrix across spec, architecture, UX, risk, stories, validation. |
| Sprint planning/status | `plan-sprint`, `sprint.yaml`, `game-sprint-status`, `resume` | partial | Sprint lifecycle is compact but not as human-guided as BMAD's sprint planning/status rituals. | Add sprint planning facilitation pack and status transcript tests. |
| Create story | `story add`, `create-epics`, `game-story-creation` | partial | Generic story creation is more CLI-like than coached story authoring. | Add story-creation workflow with readiness checklist and acceptance/evidence guidance. |
| Dev story / implementation | `build-story`, mechanical work order, Codex Goal handoff | advantage/partial | Forge has better mechanical handoff; needs fewer procedural pauses in live Codex and better "continue until done" invariant. | Add transcript tests for no "ok/continue" on ready story loops. |
| Code review | `review` commands, `test-review`, `game-qa-review`, quality-reviewer | partial | No direct `code-review` workflow equivalent with findings triage + repair loop. | Add code-review workflow or map build-story review stage more explicitly. |
| Retrospective | `game-retrospective`, checkpoints, release evidence | partial | Game retrospective exists; general epic/project retro is missing. | Add `retrospective` workflow for software/runtime/product increments. |
| Correct course | `correct-course`, Guidance Engine override | strong after latest patch | Needs more transcript coverage for scope/taste/implementation contradictions. | Add fixtures across ready/evolve/build/discovery. |
| Quick Dev / Quick Flow | mechanical autonomy + `build-story`; no full quick-dev workflow | partial/missing | Forge lacks a single guided "clarify -> spec-lite -> implement -> review" flow for small changes. | Add `quick-dev` workflow with human path and headless mechanical path. |
| Document project | `doc-index`, `spec-distillation`, `context plan` | partial | No explicit brownfield document-project flow matching BMAD analyst discovery. | Add brownfield project-documentation artifact and tests. |
| Index/shard docs | `doc-index`, `doc-shard` | strong | Need more validation for source-of-truth and stale docs. | Add doc utility fixtures and artifact checks. |
| Editorial review prose/structure | `editorial-review` | strong-ish | Good catalog mapping; needs richer human review examples. | Add pack examples and tests. |
| Adversarial / edge case review | `edge-case-review`, `grill-gate` | partial | General adversarial review is narrower than BMAD utility set. | Add adversarial review mode to edge-case/grill. |
| Party mode / multi-agent discussion | `council-decision` | partial | Forge council is compact but lacks BMAD's richer named multi-agent experience. | Add council participant routing and richer human transcript mode while keeping compact decision artifact. |
| Named agents/personas | 7 generic profiles | partial | BMAD has named PM/Architect/Analyst/Dev/UX/Tech Writer plus module agents; Forge has roles but little human personality or specialization. | Add optional persona layer for human experience; keep agent profiles compact. |
| Fresh chats per workflow | Context recovery, checkpoints, compact packs | partial | Forge does not enforce/teach fresh chat boundaries the same way; Codex-native context can still drift. | Add hot-start/context-boundary guidance per workflow and recovery triggers. |
| Customization: per-agent/workflow overrides | `config-customization` only | partial/missing | No BMAD-like TOML override model for workflows, agents, templates, menus, or persistent facts. | Design Forge config override surface with validation and install persistence. |
| Central config / module help registry | `modules/*.yaml`, `catalog/workflows.json` | partial | Forge has package catalog but no user-editable help registry or generated capability index. | Add generated compact capability index and config validation for custom modules. |
| BMAD Builder: ideate module | `runtime-builder`, `brainstorming` | partial | No explicit module ideation flow with audience, architecture, agents/workflows, config, dependencies. | Add `module-ideation` workflow and pack. |
| BMAD Builder: build agent | `builder-scaffold`, `agent-analyze` | partial | No memory/autonomous/stateless agent builder path. | Add `agent-builder` workflow with agent type decision and templates. |
| BMAD Builder: build workflow | `builder-scaffold`, `workflow-analyze`, `skill-convert` | partial | Scaffold exists but not a coached workflow builder with quality dimensions. | Add `workflow-builder` workflow or enrich `builder-scaffold`. |
| BMAD Builder: create module/package | install scripts, local plugin packaging | partial | No module builder that creates setup skill, module metadata, marketplace manifest, cleanup scripts. | Add `module-builder` workflow with packaging artifacts. |
| BMAD Builder: validate module | `workflow-validate`, `agent validate`, `config validate`, smoke scripts | partial | Good low-level checks, missing structural/quality report for a whole extension/module. | Add `module-validate` combining workflows, agents, templates, packaging, docs. |
| Eval runner / Docker runner | Forge evals and smoke scripts | partial | Forge has local evals/smokes but not a BMAD-like eval runner or isolated runner story. | Add eval runner design or keep deferred if Codex-native local is enough. |
| Session prep | Context packs/checkpoints | partial | Need a first-class session prep workflow that loads only what a future agent needs. | Add `session-prep` alias/workflow using context plan/current pack. |
| Progressive disclosure pattern | Skill instructions and context plan | partial | Principle exists, not audited across every pack/workflow. | Add compactness verifier for workflows/facilitation refs. |
| Subagent orchestration patterns | optional agent profiles, council | partial/missing | No systematic parallel/hierarchical subagent patterns for Forge tasks. | Add orchestration guidance and tests only where Codex supports it. |
| Persistent memory agents | file-backed project state | different/partial | Forge stores project state, not personal companion memory. | Decide if this is in scope; if yes, translate as project memory only, not broad personal memory. |
| CIS brainstorming | `brainstorming` pack | strong-ish | Need richer named coach/tone optionality. | Add examples and facilitation techniques. |
| CIS design thinking | `design-thinking` pack | partial | Needs stages and outputs closer to CIS, without copying voice. | Expand pack and workflow tests. |
| CIS innovation strategy | `innovation-strategy` | partial | No facilitation pack. | Add pack and transcript fixtures. |
| CIS problem solving | `problem-solving` pack | strong-ish | Needs route coverage for stuck/frustrated human beyond current fixture. | Add fixtures. |
| CIS storytelling | `storytelling` | partial | No rich pack. | Add storytelling pack and output template. |
| CIS presentation master | no equivalent | missing | Forge has no presentation/communication craft workflow. | Add only if Forge scope includes pitch/deck narrative; otherwise document non-goal. |
| Game project context | context pack + game workflows | partial | Missing explicit game project-context generation. | Add game project context artifact workflow. |
| Game engine setup (Godot/Unity/Phaser) | `engine-architecture`, maybe tech feasibility | missing/partial | No engine-specific setup guides or templates. | Add engine setup workflows/templates only if game module remains product goal. |
| Game brainstorm/brief/GDD | `game-brief`, `gdd`, `brainstorming` | partial/strong | Brief fixed recently; GDD lacks rich pack. | Add GDD facilitation pack and transcript tests. |
| Narrative/mechanics design | `narrative-design`, `mechanics-design` | partial | Workflows exist but no rich packs/templates beyond templates. | Add packs and validation examples. |
| Quick prototype | `quick-prototype` | partial | Needs route and proof scripts for actual playable prototype expectations. | Add prototype acceptance/evidence template. |
| Game sprint/story/dev/review | `game-story-creation`, `game-sprint-status`, `build-story`, `game-qa-review` | partial | Good catalog, but lifecycle not proven end-to-end. | Add game transcript replay from brief to first playable slice. |
| Game playtest/performance/testing | `playtest-plan`, `performance-plan`, `game-test-*`, `game-e2e-scaffold` | partial | Workflows exist but need deeper engine/player evidence. | Add playtest/perf templates and smoke examples. |
| TEA engagement models | `test-engagement-model` | partial | Exists, but likely much thinner than TEA's five-model decision guide. | Expand model decision artifact and fixtures. |
| Teach testing | `teach-testing` | partial/strong | Route exists; need richer applied teaching examples. | Add fixture and examples. |
| Test design / ATDD | `atdd-plan`, `test-strategy` | partial | Need stronger risk/design/test layer matrix. | Expand templates. |
| Fixture architecture | `test-framework` | partial | Missing TEA's three-step fixture architecture guidance. | Add fixture architecture pattern to test framework pack/template. |
| CI / automation | `ci-quality-pipeline`, `test-automation` | partial | Needs concrete script-generation contracts and platform defaults. | Add command map and CI templates. |
| NFR assessment/evidence | `nfr-evidence-audit`, enterprise plans | partial | Good workflow id, depth unproven. | Add NFR matrix and gate fixtures. |
| Traceability and gate | `traceability-gate`, `gate`, `ready` | strong-ish | Needs two-phase traceability + decision semantics and waiver policy. | Expand traceability template and tests. |
| API/browser utilities | no direct Forge utility layer | missing/partial | Forge relies on Codex/browser/plugins, not TEA utility generation. | Decide scope: route to test-framework with provider-specific utilities where available. |
| Enterprise security/privacy/devops/compliance | enterprise module workflows | partial/strong | Good coverage, but not fully tied into track decision and readiness. | Add enterprise track required artifact map. |
| Package/distribution docs | install scripts, smoke-install, plugin-local scripts | strong-ish | Good Codex packaging; no BMAD-style custom module marketplace generator. | Add module builder if Forge supports external modules. |
| Hooks and local runtime helpers | launcher, updater, install/smoke scripts | partial | No systematic hook wrapper after experiment archival. | Revisit hooks only as future app/runtime need; avoid Codex overhead. |

## Severity summary

Original P0 gaps that blocked the stated Forge promise:

1. Universal Help/Next-Step Oracle: BMAD's strongest behavior is that help/status tells the user and agent what to do next after each workflow. Forge must make this a runtime invariant.
2. Human facilitation coverage: every high-level workflow that a human experiences must have a rich pack, not just a compact state machine.
3. PRD/UX/Quick Dev depth: Forge has skeletons but not BMAD-level coached create/update/validate flows.
4. Story lifecycle proof: Forge must prove it will not create stories before decision artifacts, and will not ask procedural "ok/continue" during mechanical loops.
5. Systematic parity fixture harness: current transcript fixtures are useful but not broad enough to prove parity.

P0 closure status as of the parity replay harness batch:

1. Help/Next-Step Oracle: implemented and validated.
2. Human facilitation coverage gate: implemented and validated.
3. PRD/UX/Quick Dev depth: translated into Forge workflows, packs, templates, routes, and fixtures.
4. Story lifecycle proof: implemented through `story-creation`, decision-source guard, and mechanical no-procedural-confirmation tests.
5. Systematic parity fixture harness: implemented as packaged `parity replay` and installed-smoke validation.

P1 gaps are important for product maturity:

1. Builder module ideation, agent builder, workflow builder, module builder, module validator.
2. Customization surface for agent/workflow/template overrides.
3. Named/persona human experience layer over compact agent profiles.
4. Game engine setup/project context and TEA fixture/traceability depth.
5. General retrospective, project documentation, and session prep workflows.

P2 gaps are optional or scope-dependent:

1. Persistent personal memory agents.
2. Presentation master / deck craft.
3. Isolated Docker eval runner.
4. Hook wrapper automation beyond the current Codex plugin needs.

## Command/token parity appendix

This appendix maps named BMAD/CIS/BMGD/TEA commands or workflow tokens to Forge-native targets. `partial` means the concept exists but needs richer facilitation, routing, state coupling, validation, or generated artifacts.

| Source token | Forge-native target | Status |
| --- | --- | --- |
| `bmad-help` | `preflight`, `start`, `resume`, `guide`, Guidance Engine | partial |
| `bmad-brainstorming` | `brainstorming` | strong-ish |
| `bmad-market-research` | `market-scan` | strong-ish |
| `bmad-domain-research` | `domain-scan` | strong-ish |
| `bmad-technical-research` | `technical-feasibility-scan` | strong-ish |
| `bmad-product-brief` | `discover-intent`, `product-requirements` | partial |
| `bmad-prfaq` | `working-backwards-challenge` candidate | missing |
| `bmad-prd` | `product-requirements` with create/update/validate modes | partial |
| `bmad-spec` | `write-spec`, future `quick-dev` | partial |
| `bmad-ux` | `ux-plan` plus future UX pack/template | partial |
| `bmad-create-architecture` | `architecture` | partial |
| `bmad-generate-project-context` | future `project-context` workflow, `context plan` | partial |
| `bmad-create-epics-and-stories` | `create-epics` | partial |
| `bmad-check-implementation-readiness` | `readiness-check` | partial |
| `bmad-sprint-planning` | `plan-sprint`, `sprint.yaml` | partial |
| `bmad-sprint-status` | `status`, `snapshot`, `resume`, `game-sprint-status` | partial |
| `bmad-create-story` | future story-creation pack, `story add`, `game-story-creation` | partial |
| `bmad-dev-story` | `build-story`, mechanical work order, Codex Goal handoff | partial/advantage |
| `bmad-code-review` | future `code-review`, current `review`, `test-review`, `game-qa-review` | partial |
| `bmad-retrospective` | future general `retrospective`, current `game-retrospective` | partial |
| `bmad-correct-course` | `correct-course`, Guidance Engine override | strong |
| `bmad-quick-dev` | future `quick-dev` | missing |
| `bmad-document-project` | future `document-project`, current `doc-index`, `context plan` | partial |
| `bmad-index-docs` | `doc-index` | strong-ish |
| `bmad-shard-doc` | `doc-shard` | strong-ish |
| `bmad-editorial-review-prose` | `editorial-review` mode | partial |
| `bmad-editorial-review-structure` | `editorial-review` mode | partial |
| `bmad-review-adversarial-general` | future adversarial review mode, current `grill-gate`/`edge-case-review` | partial |
| `bmad-review-edge-case-hunter` | `edge-case-review` | partial |
| `bmad-investigate` | `problem-solving`, research scans, future investigation workflow | partial |
| `bmad-qa-generate-e2e-tests` | `test-automation`, `game-e2e-scaffold` | partial |
| `bmad-checkpoint-preview` | `checkpoint`, `context recover`, future preview command | partial |
| `bmad-party-mode` | `council-decision` | partial |
| `bmad-agent-analyst` | `researcher` + `facilitator` profiles, optional persona layer | partial |
| `bmad-agent-pm` | `planner` + product requirements workflows, optional persona layer | partial |
| `bmad-agent-architect` | `spec-architect`, architecture workflows | partial |
| `bmad-agent-dev` | `implementer`, build-story | partial |
| `bmad-agent-ux-designer` | future UX persona/workflow pack | partial/missing |
| `bmad-agent-tech-writer` | document utility workflows, future tech writer persona | partial |
| `bmad-customize` | `config-customization`, future override model | partial |
| `bmad-bmb-setup` | install/setup scripts, future module registration workflow | partial |
| `bmad-agent-builder` | future `agent-builder` | missing |
| `bmad-workflow-builder` | future `workflow-builder`, current `builder-scaffold` | partial |
| `bmad-module-builder` | future `module-builder` | missing |
| `bmad-eval-runner` | `eval run`, smoke scripts, future eval runner | partial |
| `bmad-session-prep` | `context plan`, `context recover`, future `session-prep` | partial |
| `/cis-brainstorm` | `brainstorming` | strong-ish |
| `/cis-design-thinking` | `design-thinking` | partial |
| `/cis-innovation-strategy` | `innovation-strategy` plus future pack | partial |
| `/cis-problem-solving` | `problem-solving` | strong-ish |
| `/cis-storytelling` | `storytelling` plus future pack | partial |
| `/cis-agent-brainstorming-coach` | optional persona over `brainstorming` | partial/missing |
| `/cis-agent-design-thinking-coach` | optional persona over `design-thinking` | partial/missing |
| `/cis-agent-innovation-strategist` | optional persona over `innovation-strategy` | partial/missing |
| `/cis-agent-creative-problem-solver` | optional persona over `problem-solving` | partial/missing |
| `/cis-agent-storyteller` | optional persona over `storytelling` | partial/missing |
| `/cis-agent-presentation-master` | future presentation/story craft workflow or non-goal | missing |
| `/bmgd-generate-project-context` | future game project-context workflow | partial/missing |
| `/bmgd-brainstorm-game` | `brainstorming`, `game-brief` | partial |
| `/bmgd-game-brief` | `game-brief` | strong-ish |
| `/bmgd-create-gdd` | `gdd` | partial |
| `/bmgd-create-architecture` | `engine-architecture` | partial |
| `/bmgd-sprint-planning` | `plan-sprint`, `game-story-creation` | partial |
| `/bmgd-sprint-status` | `game-sprint-status`, `resume` | partial |
| `/bmgd-create-story` | `game-story-creation` | partial |
| `/bmgd-dev-story` | `build-story` with game artifacts | partial |
| `/bmgd-code-review` | `game-qa-review`, future code-review | partial |
| `/bmgd-quick-prototype` | `quick-prototype` | partial |
| `playtest-plan` | `playtest-plan` | partial |
| `performance-test` | `performance-plan` | partial |
| `test-framework` | `game-test-framework`, `test-framework` | partial |
| `e2e-scaffold` | `game-e2e-scaffold` | partial |
| `bmad-tea` | `test-architect` module | partial |
| `teach-me-testing` | `teach-testing` | partial/strong |
| `test-design` | `atdd-plan`, `test-strategy`, future test-design | partial |
| `bmad-tea-testarch-framework` | `test-framework` | partial |
| `bmad-tea-testarch-ci` | `ci-quality-pipeline` | partial |
| `bmad-tea-testarch-automate` | `test-automation` | partial |
| `nfr-assess` | `nfr-evidence-audit` | partial |
| `trace` | `traceability-gate` | strong-ish |
| `can-i-deploy` | `release-readiness`, `ready`, `gate` | partial |
| `fixtures-composition` | `test-framework` future fixture architecture section | partial |
| `api-request` | future test utility pattern under `test-framework` | missing/partial |
| `network-recorder` | future browser/API utility pattern under `test-framework` | missing/partial |
| `network-error-monitor` | future browser/API utility pattern under `test-framework` | missing/partial |
| `burn-in` | future reliability/performance test mode | missing/partial |

## Forge translation backlog

Use these as implementation increments. Each item must ship with workflow metadata, facilitation pack when human-facing, tests/fixtures, and evidence.

### P0.1 Help Oracle invariant

- Add a runtime function that computes next required workflow from state, catalog, open inputs, stories, evidence, and recent artifacts.
- Make state-changing commands call or expose it.
- Add tests proving stale `next_action` cannot dominate after workflow completion.
- Human output: one recommended next step, why, and at most three alternatives.
- Agent output: compact JSON with `required_next_workflow`, `reason`, `commands`, `state_update_required`.

### P0.2 Facilitation coverage gate

- Add workflow validation that flags human-facing workflows without facilitation packs.
- Define which workflows are agent-only and which require rich human facilitation.
- Add missing packs for `product-requirements`, `ux-plan`, `architecture`, `gdd`, `innovation-strategy`, `storytelling`, `plan-sprint`, `create-epics`, `readiness-check`.

### P0.3 PRD/UX/Quick Dev parity

- Expand product requirements into create/update/validate modes with decision log and addendum.
- Add UX design workflow with taste calibration, journeys, interaction model, accessibility, and rejection log.
- Add `quick-dev` for small scoped changes: clarify intent, write compact spec, implement, review, evidence, next step.

### P0.4 Story lifecycle guard

- Add tests that fail if new projects generate ready stories before required facilitation/decision artifacts.
- Add tests for mechanical loops that should continue without procedural confirmation.
- Add story creation workflow that requires accepted spec/architecture/UX/validation map for relevant tracks.

### P0.5 Parity replay harness

- Create a fixture set covering BMAD-like questions: help, confusion, brainstorm, research, PRD, UX, architecture, quick dev, story cycle, correct course, builder, CIS, game, TEA.
- Expected output is Forge-native workflow/phase/action, not BMAD wording.
- Run under unit tests and install smoke.

### P1.1 Builder parity

- Add `module-ideation`, `agent-builder`, `workflow-builder`, `module-builder`, `module-validate`.
- Generate compact Forge module metadata, skill files, workflow refs, packs, templates, tests, and install validation.

### P1.2 Customization surface

- Design a Forge override model for agent profiles, workflow metadata, facilitation packs, templates, and project conventions.
- Add validation and conflict reporting.

### P1.3 Persona layer

- Keep agent profiles compact for runtime.
- Add optional human-facing persona descriptors for PM/Architect/Researcher/UX/QA/Game/Builder roles.
- Do not let persona text bloat state or workflow docs.

### P1.4 Game and TEA depth

- Add game project context and engine setup workflows.
- Expand GDD, mechanics, narrative, playtest, performance, and game QA packs.
- Expand TEA engagement model, fixture architecture, CI, automation, NFR, traceability, and waiver/gate templates.

## Completion criteria for the full parity goal

The full objective is complete only when:

1. every BMAD capability family above is either translated, intentionally waived as a Forge non-goal, or deferred with explicit product rationale;
2. every translated human-facing flow has a facilitation pack and transcript fixture;
3. every translated agent-facing flow has compact workflow docs and state/runtime contract;
4. the parity replay harness passes;
5. install/runtime smoke proves the installed `$forge-method` uses the new behavior;
6. public Forge docs describe Forge by its own model, not as a clone or fork.

## Current status

This audit remains the gap map for the full objective. P0.1 through P0.5 are now implemented and have evidence/checkpoints in the Forge state, including packaged `parity replay` validation. This is not full parity completion: P1 capability depth and explicit deferral/waiver decisions still remain.

Immediate next step: implement P1.1 Builder parity unless a new audit shows a higher-severity regression. The next batch should translate module ideation, agent builder, workflow builder, module builder, and module validation into Forge-native workflows, packs, templates, scripts/tests, and install validation.

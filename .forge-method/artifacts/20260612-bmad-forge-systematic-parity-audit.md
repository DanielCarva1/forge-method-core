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
- Forge currently exposes 97 catalog workflows, 8 modules, 7 compact agent profiles, 25 facilitation packs, runtime state, ledger, checkpoints, evidence, evals, smoke scripts, install smoke, persona lenses, elicitation techniques, and Guidance Engine routing.

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
| Planning tracks: Quick Flow / Method / Enterprise | Forge modules plus `track-decision` | translated/partial | P1.4 adds a route decision artifact and required workflow map, but deeper track-specific doc trees can still improve. | Expand track-specific required/optional docs as future module depth is translated. |
| Analysis: brainstorming | `brainstorming` workflow + facilitation pack | strong-ish | Need transcript tests for richer guided divergence, not just route. | Add fixtures for option generation, constraint capture, rejected directions. |
| Analysis: market/domain/technical research | `market-scan`, `domain-scan`, `technical-feasibility-scan`, Reality/Evidence Gate, `research-closeout` | translated/strong | P1.4 adds research closeout handoff with sources, confidence, decision impact, uncertainty, and next workflow. | Add richer source-quality examples as research depth grows. |
| Product brief / PRFAQ | `discover-intent`, `product-requirements`, `working-backwards-challenge`, `reality-evidence-gate` | translated | P1.7 adds a working-backwards challenge workflow for customer-promise, FAQ objection, evidence gap, and decision-impact stress testing before PRD/UX/architecture harden the promise. | Add richer transcript examples only if real product sessions show weak coaching. |
| PRD create/update/validate | `product-requirements`, `write-spec`, `spec-distillation`, `grill-gate` | partial | Missing BMAD-like create/update/validate modes, addendum, decision log, HTML/findings-style validation. | Expand product requirements pack and workflow metadata with modes and validation artifact. |
| Advanced elicitation | facilitation packs, `council-decision`, `grill-gate`, elicitation technique index | translated | P1.3 adds a validated compact technique index and exposes technique ids through Persona Lens guidance without bloating state. | Expand technique use inside future deep packs as they are translated. |
| UX design | `ux-plan`, some game UX | partial | No rich UX designer workflow equivalent with EXPERIENCE/DESIGN style outputs and human taste calibration across product work. | Add UX facilitation pack, UX artifact template, transcript fixtures. |
| Architecture | `architecture`, `engine-architecture`, `security-plan`, enterprise plans | partial | Core architecture exists but less agent/persona guided and less tied to PRD validation. | Add architecture pack and readiness checks linking PRD/UX/security/story plan. |
| Project context generation | `project-context`, `context plan`, `context recover`, `current-pack`, `AGENTS.md` | translated/advantage | P1.4 adds a user-facing project-context workflow while preserving Forge's compact runtime context packs. | Add domain-specific context variants only when a module needs them. |
| Epics and stories after architecture | `create-epics`, `plan-sprint`, `game-story-creation` | partial | Previous bug created stories too early; fixed for new projects. Need stronger invariant that story generation requires approved decision artifacts. | Add tests/gate preventing stories before required track artifacts. |
| Implementation readiness | `readiness-check`, readiness matrix, `gate`, `ready`, `release-readiness` | translated | P1.4 adds readiness matrix output across spec, architecture, UX, risk, stories, validation, inputs, and findings. | Add stricter source coverage gates if future story creation finds gaps. |
| Sprint planning/status | `plan-sprint`, `sprint-status`, `sprint.yaml`, `game-sprint-status`, `resume` | translated/partial | P1.7 adds a routeable sprint-status ritual with artifact template and replay proof; deeper sprint planning facilitation can still improve. | Expand sprint planning examples if story-cycle sessions show weak guidance. |
| Create story | `story-creation`, `story add`, `create-epics`, `game-story-creation` | translated | P0 adds story-creation with readiness checklist, acceptance/evidence guidance, decision-source guard, and replay coverage. | Keep examples current as more project tracks use it. |
| Dev story / implementation | `build-story`, mechanical work order, Codex Goal handoff | advantage/partial | Forge has better mechanical handoff; needs fewer procedural pauses in live Codex and better "continue until done" invariant. | Add transcript tests for no "ok/continue" on ready story loops. |
| Code review | `code-review`, `review` commands, `test-review`, `game-qa-review`, quality-reviewer | translated | P1.4 adds a direct code-review workflow with findings triage and repair/readiness routing. | Add richer diff examples if future review quality needs proof. |
| Retrospective | `retrospective`, `game-retrospective`, checkpoints, release evidence | translated | P1.4 adds a general retrospective workflow for software/runtime/product increments. | Keep game-specific retro depth in the game module. |
| Correct course | `correct-course`, Guidance Engine override | strong after latest patch | Needs more transcript coverage for scope/taste/implementation contradictions. | Add fixtures across ready/evolve/build/discovery. |
| Quick Dev / Quick Flow | `quick-dev`, mechanical autonomy, `build-story` | translated | P0 adds a guided clarify -> spec-lite -> implement/handoff -> review -> evidence fast path for small scoped work. | Keep mechanical loops covered by transcript fixtures. |
| Document project | `project-context`, `doc-index`, `spec-distillation`, `context plan` | translated | P1.4 maps document-project intent to a compact project context artifact plus source map and session handoff. | Add brownfield-specific examples when project-context depth is expanded. |
| Index/shard docs | `doc-index`, `doc-shard` | strong | Need more validation for source-of-truth and stale docs. | Add doc utility fixtures and artifact checks. |
| Editorial review prose/structure | `editorial-review` | strong-ish | Good catalog mapping; needs richer human review examples. | Add pack examples and tests. |
| Adversarial / edge case review | `adversarial-review`, `edge-case-review`, `grill-gate` | translated | P1.7 adds a routeable adversarial-review workflow, compact artifact, and replay proof so assumption attack does not collapse into generic quality review. | Add richer examples if future reviews need multi-agent critique. |
| Party mode / multi-agent discussion | `council-decision`, Persona Lens participant routing | partial/stronger | P1.3 adds lens-based council participants and compact artifacts; richer live transcript style is still future human-experience depth. | Add transcript examples where council discussion quality matters, while keeping the durable artifact compact. |
| Named agents/personas | 7 compact profiles plus 13 Persona Lenses | translated | P1.3 adds PM, Architect, Analyst/Researcher, UX, QA, Game, Builder, Tech Writer, and CIS coach lenses while keeping default agent profiles compact. | Add more domain examples only when future packs need them. |
| Fresh chats per workflow | Context recovery, checkpoints, compact packs | partial | Forge does not enforce/teach fresh chat boundaries the same way; Codex-native context can still drift. | Add hot-start/context-boundary guidance per workflow and recovery triggers. |
| Customization: per-agent/workflow overrides | `config-customization`, Project Configuration, Override Model | translated | P1.2 adds validated workflow metadata, facilitation/template, agent metadata, convention, and custom capability overrides with package/team/local precedence. | Keep override surface narrow; route new runtime behavior through Builder Factory instead of freeform config. |
| Central config / module help registry | `config index`, `modules/*.yaml`, `catalog/workflows.json` | translated | P1.2 adds generated Capability Index from effective workflows, modules, agents, conventions, and custom capabilities. | Keep generated index compact and install-safe; add richer module registration later if needed. |
| BMAD Builder: ideate module | `module-ideation`, `builder-factory` pack, builder templates | translated | P1.1 adds explicit module ideation route, compact workflow, pack, template, and replay coverage. | Keep depth examples current as future modules are built. |
| BMAD Builder: build agent | `agent-builder`, `agent-analyze`, `builder-factory` pack | translated | P1.1 adds guided agent build route with agent type, capability, memory/autonomy, script, and quality handoff contract. | Add concrete generated-agent examples in a later builder depth batch if needed. |
| BMAD Builder: build workflow | `workflow-builder`, `workflow-analyze`, `skill-convert`, `workflow-validate` | translated | P1.1 adds coached workflow build route with compact state-machine, pack/template, catalog, and proof plan. | Add generated-workflow examples as future modules use the factory. |
| BMAD Builder: create module/package | `module-builder`, module manifest template, install/smoke validation path | translated | P1.1 adds module assembly workflow, manifest template, followed_by metadata, and validation handoff. | Future customization batch handles richer override/index behavior. |
| BMAD Builder: validate module | `module-validate`, `builder validate`, `workflow-validate`, parity replay | translated | P1.1 adds whole-extension validation workflow, validation report template, and structural plus quality checklist. | Keep install smoke as the packaging proof for changed packaged behavior. |
| Eval runner / Docker runner | Forge evals, parity replay, smoke scripts, CI | deferred | P2 decision: local evals/smokes cover the current plugin; Docker isolation waits until untrusted execution or reproducible cross-machine evals require it. | Revisit for standalone app/runtime or untrusted eval execution. |
| Session prep | `session-prep`, context packs/checkpoints | translated | P1.4 adds first-class session prep with read order, blockers, first command, next workflow, and load hints. | Add transcript examples for interrupted/changing sessions if needed. |
| Progressive disclosure pattern | Skill instructions and context plan | partial | Principle exists, not audited across every pack/workflow. | Add compactness verifier for workflows/facilitation refs. |
| Subagent orchestration patterns | optional agent profiles, council | partial/missing | No systematic parallel/hierarchical subagent patterns for Forge tasks. | Add orchestration guidance and tests only where Codex supports it. |
| Persistent memory agents | file-backed project state | non-goal | P2 decision: Forge owns project-local durable state, artifacts, checkpoints, context packs, and ledgers, not broad personal companion memory. | Revisit only for a separate personal workspace product. |
| CIS brainstorming | `brainstorming` pack plus Brainstorming Coach lens | strong-ish | Coach routing and techniques exist; richer option-generation transcripts are still future depth. | Add examples for option generation, constraints, and convergence. |
| CIS design thinking | `design-thinking` pack plus Design Thinking Coach lens | strong-ish | Route and lens proof exist; deeper outputs can still improve. | Expand examples and artifact expectations when design-thinking becomes a larger workflow. |
| CIS innovation strategy | `innovation-strategy` plus Innovation Strategist lens | partial | Lens route exists, but no facilitation pack. | Add pack and transcript fixtures. |
| CIS problem solving | `problem-solving` pack plus Creative Problem Solver lens | strong-ish | Coach route exists; more stuck/frustrated transcripts would improve proof. | Add fixtures for messy constraints and recovery. |
| CIS storytelling | `storytelling` plus Storyteller lens | partial | Lens route exists, but no rich pack. | Add storytelling pack and output template. |
| CIS presentation master | future creative/pitch workflow | deferred | P2 decision: deck craft is not required for the current Codex-native method runtime. | Revisit if launch/pitch/deck workflows become part of Forge project scope. |
| Game project context | `game-context` plus game lifecycle pack | translated | P1.5 adds explicit game context generation with player fantasy, loop, engine profile, source material, and playable-slice handoff. | Add domain examples only when real game projects expose gaps. |
| Game engine setup (Godot/Unity/Phaser) | `engine-setup`, `engine-architecture` | translated | P1.5 uses a compact engine profile instead of separate engine-specific public entrypoints, preserving engine assumptions, folder/runtime decisions, first-run check, and validation evidence. | Add engine-specific examples when a project needs them; avoid heavy engine smoke by default. |
| Game brainstorm/brief/GDD | `game-brief`, `gdd`, `brainstorming` | translated | P1.5 expands GDD template and game lifecycle prompts so broad ideas stay in guided game discovery before architecture/build. | Add richer transcript examples for messy ideation later. |
| Narrative/mechanics design | `narrative-design`, `mechanics-design` | translated | P1.5 expands templates/contracts for motivation, content units, player verbs, balance assumptions, and production risks. | Add balancing examples when a project reaches tuning depth. |
| Quick prototype | `quick-prototype` | translated | P1.5 adds playable-slice proof contract, acceptance evidence, commands/manual checks, and route replay. | Add runnable smoke only when a lightweight fixture exists. |
| Game sprint/story/dev/review | `game-story-creation`, `game-sprint-status`, `build-story`, `game-qa-review` | strong-ish | P1.5 adds first playable slice decision sources, story order, QA review artifact, and route proof; full game dev-story execution remains project-dependent. | Add end-to-end implementation replay when a real game fixture exists. |
| Game playtest/performance/testing | `playtest-plan`, `performance-plan`, `game-test-*`, `game-e2e-scaffold` | translated | P1.5 adds playtest/performance evidence templates and routes for player signals, frame/runtime budgets, and validation proof. | Add engine-specific measurement examples as projects demand them. |
| TEA engagement models | `test-engagement-model` | translated | P1.6 adds Quality Engagement Model semantics, narrow template, pack guidance, and replay coverage for advice/design/implementation/review/audit/gate selection. | Add examples only when real project use exposes missing modes. |
| Teach testing | `teach-testing` | translated | P1.6 adds applied teaching artifact and route proof that education leads to a concrete next quality workflow. | Add richer lessons later if Forge becomes a teaching product. |
| Test design / ATDD | `atdd-plan`, `test-strategy` | translated | P1.6 adds risk proof map, ATDD examples, edge cases, risk coverage, proof paths, and replay coverage. | Consider separate `test-design` id only if `test-strategy`/`atdd-plan` cannot express future needs. |
| Fixture architecture | `test-framework` | translated | P1.6 records framework-neutral Fixture Architecture: pure helper, framework wrapper, composition surface, lifecycle cleanup, command evidence. | Add framework-specific examples inside project artifacts only. |
| CI / automation | `ci-quality-pipeline`, `test-automation` | translated | P1.6 adds local/fast/full/release command contracts, artifacts, failure policy, automation fixtures, evidence links, and manual remainders. | Add provider-specific CI snippets only when a project needs them. |
| NFR assessment/evidence | `nfr-evidence-audit`, enterprise plans | translated | P1.6 adds NFR evidence matrix, claim status, gaps, waivers, release impact, gate updates, and replay coverage. | Add NFR domain examples over time. |
| Traceability and gate | `traceability-gate`, `gate`, `ready` | translated | P1.6 adds two-phase traceability and gate outcomes: pass, concerns, fail, missing evidence, waived; waiver requires owner/rationale/revisit/release impact. | Keep release-readiness consumption aligned with this contract. |
| API/browser utilities | project-specific test utilities under `test-framework` | deferred | Forge relies on Codex/browser/plugins and records provider-specific utility patterns inside project artifacts instead of making generic public utility workflows. | Revisit if repeated projects need reusable browser/API utility workflows. |
| Enterprise security/privacy/devops/compliance | enterprise module workflows | partial/strong | Good coverage, but not fully tied into track decision and readiness. | Add enterprise track required artifact map. |
| Package/distribution docs | install scripts, smoke-install, plugin-local scripts | strong-ish | Good Codex packaging; no BMAD-style custom module marketplace generator. | Add module builder if Forge supports external modules. |
| Hooks and local runtime helpers | launcher, updater, install/smoke scripts | deferred | P2 decision: hook experiments are archived reference; plugin keeps current launcher/runtime surface to avoid Codex overhead. | Revisit for a standalone Forge app or concrete lifecycle hook need. |

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

P1 maturity batches are now translated:

1. Builder module ideation, agent builder, workflow builder, module builder, module validator.
2. Customization surface for agent/workflow/template overrides.
3. Named/persona human experience layer over compact agent profiles.
4. Game engine setup/project context and TEA fixture/traceability depth.
5. General retrospective, project documentation, and session prep workflows.
6. Parity closure utilities: investigation, working-backwards challenge, sprint status, checkpoint preview, and adversarial review.

P2 decisions are now explicit:

1. Persistent personal memory agents: non-goal for the current runtime; Forge keeps project-local durable memory.
2. Presentation master / deck craft: deferred until pitch/deck workflows are an explicit Forge scope.
3. Isolated Docker eval runner: deferred until untrusted/reproducible eval execution is required.
4. Hook wrapper automation beyond the current Codex plugin needs: deferred for standalone app/runtime.
5. Generic API/browser utility layer: deferred as public surface; provider-specific patterns live in test artifacts.

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
| `bmad-prfaq` | `working-backwards-challenge` | translated |
| `bmad-prd` | `product-requirements` with create/update/validate modes | partial |
| `bmad-spec` | `write-spec`, `quick-dev` | partial |
| `bmad-ux` | `ux-plan` plus future UX pack/template | partial |
| `bmad-create-architecture` | `architecture` | partial |
| `bmad-generate-project-context` | `project-context`, `context plan` | translated |
| `bmad-create-epics-and-stories` | `create-epics` | partial |
| `bmad-check-implementation-readiness` | `readiness-check` with readiness matrix | translated |
| `bmad-sprint-planning` | `plan-sprint`, `sprint.yaml` | partial |
| `bmad-sprint-status` | `sprint-status`, `status`, `snapshot`, `resume`, `game-sprint-status` | translated |
| `bmad-create-story` | `story-creation`, `story add`, `game-story-creation` | translated |
| `bmad-dev-story` | `build-story`, mechanical work order, Codex Goal handoff | partial/advantage |
| `bmad-code-review` | `code-review`, `review`, `test-review`, `game-qa-review` | translated |
| `bmad-retrospective` | `retrospective`, `game-retrospective` | translated |
| `bmad-correct-course` | `correct-course`, Guidance Engine override | strong |
| `bmad-quick-dev` | `quick-dev` | translated |
| `bmad-document-project` | `project-context`, `doc-index`, `context plan` | translated |
| `bmad-index-docs` | `doc-index` | strong-ish |
| `bmad-shard-doc` | `doc-shard` | strong-ish |
| `bmad-editorial-review-prose` | `editorial-review` mode | partial |
| `bmad-editorial-review-structure` | `editorial-review` mode | partial |
| `bmad-review-adversarial-general` | `adversarial-review`, `grill-gate`, `edge-case-review` | translated |
| `bmad-review-edge-case-hunter` | `edge-case-review` | partial |
| `bmad-investigate` | `investigation`, `problem-solving`, research scans | translated |
| `bmad-qa-generate-e2e-tests` | `test-automation`, `game-e2e-scaffold` | partial |
| `bmad-checkpoint-preview` | `checkpoint-preview`, `checkpoint`, `context recover` | translated |
| `bmad-party-mode` | `council-decision` | partial |
| `bmad-agent-analyst` | `analyst-researcher` Persona Lens over `researcher` + `facilitator` profiles | translated |
| `bmad-agent-pm` | `product-manager` Persona Lens over product requirements workflows | translated |
| `bmad-agent-architect` | `architect` Persona Lens over `spec-architect` and architecture workflows | translated |
| `bmad-agent-dev` | `implementer`, build-story | partial |
| `bmad-agent-ux-designer` | `ux-designer` Persona Lens over UX workflows | translated |
| `bmad-agent-tech-writer` | `tech-writer` Persona Lens over document utility workflows | translated |
| `bmad-customize` | `config-customization`, Project Configuration, Capability Index | translated |
| `bmad-bmb-setup` | install/setup scripts, future module registration workflow | partial |
| `bmad-agent-builder` | `agent-builder` plus `agent-analyze` | translated |
| `bmad-workflow-builder` | `workflow-builder`, `workflow-analyze`, `skill-convert`, `workflow-validate` | translated |
| `bmad-module-builder` | `module-ideation`, `module-builder`, `module-validate` | translated |
| `bmad-eval-runner` | `eval run`, smoke scripts, future eval runner | partial |
| `bmad-session-prep` | `session-prep`, `context plan`, `context recover` | translated |
| `/cis-brainstorm` | `brainstorming` | strong-ish |
| `/cis-design-thinking` | `design-thinking` | partial |
| `/cis-innovation-strategy` | `innovation-strategy` plus future pack | partial |
| `/cis-problem-solving` | `problem-solving` | strong-ish |
| `/cis-storytelling` | `storytelling` plus future pack | partial |
| `/cis-agent-brainstorming-coach` | `brainstorming-coach` Persona Lens over `brainstorming` | translated |
| `/cis-agent-design-thinking-coach` | `design-thinking-coach` Persona Lens over `design-thinking` | translated |
| `/cis-agent-innovation-strategist` | `innovation-strategist` Persona Lens over `innovation-strategy` | translated |
| `/cis-agent-creative-problem-solver` | `creative-problem-solver` Persona Lens over `problem-solving` | translated |
| `/cis-agent-storyteller` | `storyteller` Persona Lens over `storytelling` | translated |
| `/cis-agent-presentation-master` | future presentation/story craft workflow or non-goal | missing |
| `/bmgd-generate-project-context` | `game-context` | translated |
| `/bmgd-brainstorm-game` | `brainstorming`, `game-brief`, `game-context` | strong-ish |
| `/bmgd-game-brief` | `game-brief` | strong-ish |
| `/bmgd-create-gdd` | `gdd` | translated |
| `/bmgd-create-architecture` | `engine-architecture`, `engine-setup` | translated |
| `/bmgd-sprint-planning` | `plan-sprint`, `game-story-creation` | partial |
| `/bmgd-sprint-status` | `game-sprint-status`, `resume` | partial |
| `/bmgd-create-story` | `game-story-creation` | partial |
| `/bmgd-dev-story` | `build-story` with game artifacts | partial |
| `/bmgd-code-review` | `game-qa-review`, `code-review` | strong-ish |
| `/bmgd-quick-prototype` | `quick-prototype` | translated |
| `playtest-plan` | `playtest-plan` | translated |
| `performance-test` | `performance-plan` | translated |
| `test-framework` | `game-test-framework`, `test-framework` | partial |
| `e2e-scaffold` | `game-e2e-scaffold` | partial |
| `bmad-tea` | `test-architect` module | translated |
| `teach-me-testing` | `teach-testing` | translated |
| `test-design` | `test-strategy`, `atdd-plan` | translated |
| `bmad-tea-testarch-framework` | `test-framework` | translated |
| `bmad-tea-testarch-ci` | `ci-quality-pipeline` | translated |
| `bmad-tea-testarch-automate` | `test-automation` | translated |
| `nfr-assess` | `nfr-evidence-audit` | translated |
| `trace` | `traceability-gate` | translated |
| `can-i-deploy` | `traceability-gate`, `release-readiness`, `ready`, `gate` | translated |
| `fixtures-composition` | `test-framework` Fixture Architecture | translated |
| `api-request` | provider-specific utility under `test-framework` artifacts | deferred |
| `network-recorder` | provider-specific utility under `test-framework` artifacts | deferred |
| `network-error-monitor` | provider-specific utility under `test-framework` artifacts | deferred |
| `burn-in` | `ci-quality-pipeline` release/investigation command contract | translated |

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

- Status: translated in the Builder Factory batch.
- Added `module-ideation`, `agent-builder`, `workflow-builder`, `module-builder`, and `module-validate`.
- Added `builder-factory` facilitation, builder templates, catalog/module metadata, Guidance Engine routes, parity replay coverage, and validation evidence.

### P1.2 Customization surface

- Status: translated in the Customization and Capability Index batch.
- Added Project Configuration, Override Model, Capability Index, `config index`, `config-customization` facilitation, workflow metadata overrides, agent metadata overrides, convention entries, custom capability entries, and stale-reference validation.

### P1.3 Persona layer

- Status: translated in the Persona Lens and Elicitation Layer batch.
- Added validated persona overlays for PM, Architect, Analyst/Researcher, UX, QA, Game, Builder, Tech Writer, and CIS coach roles.
- Added a compact elicitation technique index, persona facilitation pack, Guidance Engine `persona_lens` output, replay cases, council participant routing, Capability Index exposure, and compactness guards.
- Agent profiles, state, workflow refs, and default recommendations remain compact.

### P1.4 Lifecycle closure

- Status: translated in the Lifecycle Closure batch.
- Added `track-decision`, `project-context`, `session-prep`, `code-review`, `retrospective`, `research-closeout`, and readiness matrix output.
- Added lifecycle facilitation, compact workflow refs, templates, catalog/module metadata, Guidance Engine routes, replay coverage, and Capability Index exposure.
- Added precedence guard so runtime-builder/systematic parity batch names outrank loose domain words while handoff/context/review/retro requests still route to Lifecycle Closure.

### P1.5 Game Studio depth

- Status: translated in the Game Studio Depth batch.
- Added `game-context` and `engine-setup` workflows, compact refs, templates, catalog/module metadata, and Guidance Engine routes.
- Expanded GDD, narrative, mechanics, quick prototype, playtest, performance, game lifecycle, and game QA templates/contracts.
- Added replay coverage for game-context, engine-setup, GDD, quick prototype, playtest, performance, and game QA routes.
- Kept engine-specific setup as profile-driven guidance instead of separate public commands; heavy engine smoke remains deferred until a real lightweight game fixture exists.

### P1.6 TEA depth

- Status: translated in the Test Architecture Enterprise Depth batch.
- Expanded TEA engagement model, fixture architecture, CI, automation, NFR, traceability, and waiver/gate templates.
- Added two-phase traceability gate decision semantics and ADR.
- Added Guidance Engine route depth and replay coverage for teach, engagement, strategy, framework, CI, ATDD, automation, review, NFR, traceability, and P1.6 internal runtime-builder precedence.

### P1.7 Parity closure utilities

- Status: translated in the Parity Closure Utilities batch.
- Added `investigation`, `working-backwards-challenge`, `sprint-status`, `checkpoint-preview`, and `adversarial-review`.
- Added compact workflow refs, artifact templates, catalog/module membership, Guidance Engine routing, and parity replay cases.
- Corrected adversarial/red-team routing precedence so explicit assumption attack does not collapse into generic quality review.

## Completion criteria for the full parity goal

The full objective is complete only when:

1. every BMAD capability family above is either translated, intentionally waived as a Forge non-goal, or deferred with explicit product rationale;
2. every translated human-facing flow has a facilitation pack and transcript fixture;
3. every translated agent-facing flow has compact workflow docs and state/runtime contract;
4. the parity replay harness passes;
5. install/runtime smoke proves the installed `$forge-method` uses the new behavior;
6. public Forge docs describe Forge by its own model, not as a clone or fork.

## Current status

This audit remains the gap map for the full objective. P0.1 through P0.5 and P1.1 through P1.7 are now implemented and have evidence/checkpoints in the Forge state, including packaged `parity replay` validation. P2 scope decisions are recorded in `.forge-method/artifacts/20260615-p2-scope-decisions-and-polish-plan.md`. This is not full goal completion: release/version planning and real-use transcript hardening still remain.

Immediate next step: validate the closure utilities in source and installed skill contexts, then run release planning for a coherent versioned batch.

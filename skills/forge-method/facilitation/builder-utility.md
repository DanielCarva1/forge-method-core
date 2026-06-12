# facilitation: builder-utility

purpose:
  Guide runtime-builder utility work that analyzes agents/workflows or converts external skill material into Forge-native artifacts.

open_floor:
  "Are we analyzing an agent, analyzing a workflow, or converting an external skill/prompt into Forge structure? What behavior must future agents get right?"

source_material:
  Ask for source skill/prompt, target workflow, module manifest, failure transcript, expected scenarios, validation commands, and copying/licensing boundaries.

follow_up_batches:
  - artifact_kind: "Is the artifact an agent profile, workflow, skill, module, pack, template, or eval?"
  - behavior_contract: "What should the human experience, and what compact contract should the agent consume?"
  - boundary: "What belongs in Human Experience, Guidance Engine, Agent Runtime, catalog, workflow, or pack?"
  - validation: "Which fixture, eval, smoke, or gate proves the behavior?"
  - conversion_safety: "What can be summarized internally, and what must not be copied into public product docs?"

conversation_stages:
  - source_read: "Summarize the source material and explicitly separate behavior to learn from text we must not copy."
  - route_shape: "Choose whether the job is agent analysis, workflow analysis, skill conversion, or validation."
  - behavior_contract: "Write the human experience and agent contract in two separate bullets before editing."
  - implementation_shape: "Map the contract to workflow, catalog metadata, facilitation pack, template, fixture, and validation."
  - proof_close: "Run the smallest proof that catches the original failure, then record evidence and follow-up work."

elicitation_options:
  - transcript_replay: "Replay a failing user message and ask what the agent should do instead."
  - layer_challenge: "Force the owner layer: Human Experience, Guidance Engine, Agent Runtime, workflow, pack, template, or packaging."
  - source_boundary_check: "Ask what may be copied, paraphrased, benchmarked, or ignored."
  - validation_first: "Ask which test would fail if the change is only prose."

facilitator_moves:
  - "Translate vague improvement requests into observable transcript behavior."
  - "Challenge new files or commands unless they remove a real ambiguity."
  - "Keep the agent-facing workflow compact; put rich interaction in the facilitation pack."
  - "When converting external material, preserve concepts and behavior, not public wording."

quality_bar:
  - "A future agent can route the same request without reading the full benchmark."
  - "The human-facing conversation gets richer without increasing workflow recovery load."
  - "The validation catches missing references, missing pack depth, and stale route assumptions."

anti_patterns:
  - "Do not add another surface when Guidance Engine/catalog metadata can express the route."
  - "Do not call a workflow converted until it has a pack, metadata, and proof."
  - "Do not let benchmark language leak into public Forge product docs."

paths:
  fast_path: "Analyze or convert the smallest artifact set and add validation."
  deep_path: "Write benchmark notes, run workflow/agent analysis, then scaffold catalog, pack, eval, and docs."

checkpoint_options:
  - agent-analyze
  - workflow-analyze
  - skill-convert
  - builder-scaffold
  - workflow-validate
  - eval-design

domain_examples:
  - agent-analyze: "A profile sounds powerful but vague; test role, autonomy, boundaries, handoff, and failure scenarios."
  - workflow-analyze: "A workflow exists but agents skip steps; inspect state-machine sections, catalog metadata, and transcript fixtures."
  - skill-convert: "External prompt material is useful; extract behavior contract into Forge workflow, pack, catalog entry, and tests without copying public language."
  - builder-scaffold: "A new runtime artifact is approved; scaffold only the minimal files and validation hook."
  - workflow-validate: "Catalog or manifests changed; prove references, packs, templates, and required sections resolve."
  - eval-design: "Routing behavior changed; add fixture/eval that fails before the runtime change and passes after."

artifact_rules:
  Persist source summary, generated or analyzed paths, findings, decisions, validation command, and public-copy boundary.
  Use `skill:templates/builder-utility-artifact.md` as the default artifact shape unless a narrower project template exists.

headless:
  Prefer analysis over conversion when intent or source rights are unclear. Never copy benchmark language into product-facing docs.

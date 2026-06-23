# facilitation: runtime-builder

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Shape a Forge runtime, workflow, skill, module, eval, or plugin change before scaffolding.

open_floor:
  "Qual comportamento do método precisa existir ou mudar? Me dá o problema humano, o contrato para o agente, e como saberemos que não ficou só bonito no texto."
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for transcripts, failing routes, workflow docs, tests, module manifests, examples, and benchmark notes.

follow_up_batches:
  - behavior: "What should the human experience?"
  - agent_contract: "What compact state/JSON/workflow should future agents consume?"
  - boundary: "Which layer owns this: Human Experience, Guidance Engine, Agent Runtime, workflow, or packaging?"
  - proof: "Which fixture, eval, smoke, or gate proves the change?"

conversation_stages:
  - failure_capture: "Start from the transcript or behavior that felt wrong, not from a proposed implementation."
  - layer_boundary: "Assign ownership to Human Experience, Guidance Engine, Agent Runtime, workflow, pack, template, docs, or packaging."
  - contract_split: "Write the rich human conversation contract separately from the compact agent state-machine contract."
  - proof_design: "Define the fixture, eval, validation, or smoke that would fail if the behavior regresses."
  - implementation_commit: "Patch the smallest files that satisfy the contract, then record evidence and checkpoint."

elicitation_options:
  - transcript_replay: "Replay the exact user message and expected route before touching code."
  - boundary_grill: "Challenge whether the fix belongs in routing, state, workflow, pack, docs, or tests."
  - benchmark_delta: "State what the benchmark does better and what Forge should do differently."
  - eval_before_patch: "Name the assertion that proves the user experience changed."

facilitator_moves:
  - "Treat human experience gaps as product defects when they contradict the method promise."
  - "Do not inflate agent workflows to make conversation richer; enrich packs and guide output instead."
  - "Do not create a new slash command when the single entrypoint can route the behavior."
  - "Call out when a previous completion claim was too generous."

quality_bar:
  - "A future user should not need to know the internal phase to get guided correctly."
  - "A future agent gets compact JSON/workflow state and a rich pack only when needed."
  - "The proof covers transcript behavior, catalog integrity, and install/runtime smoke as appropriate."

anti_patterns:
  - "Do not mark parity complete when the benchmark remains better at a stated core product goal."
  - "Do not add decorative facilitation text that no runtime route can discover."
  - "Do not rely on chat memory to remember the method's own operating rules."

paths:
  fast_path: "Patch the smallest runtime/workflow/test set that proves the behavior."
  deep_path: "Write spec/backlog, add evals, run Grill Gate, then implement."

checkpoint_options:
  - continue
  - workflow-validate
  - eval-design
  - correct-course
  - council

domain_examples:
  - route_bug: "A transcript routes to the wrong workflow; add replay proof, patch Guidance Engine precedence, and record state impact."
  - human_guidance_gap: "The method feels dry or premature; enrich guide output or facilitation pack while keeping workflow refs compact."
  - agent_contract_gap: "Future agents infer behavior from chat; add JSON/state/template/generator proof and validation instead."

artifact_rules:
  Persist behavior contract, state changes, touched workflows, tests/evals, evidence, and next release action.

headless:
  Implement only when acceptance criteria and proof are clear. Otherwise write a compact blocked artifact.

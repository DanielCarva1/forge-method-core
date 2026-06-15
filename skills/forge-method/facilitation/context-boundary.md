# facilitation: context-boundary

purpose:
  Help a human or future agent recover from a fresh chat, compacted context, interrupted network, or stale Forge instructions without replaying old conversation memory.

open_floor:
  "What broke: the chat, the context, the route, or trust in the next step? I will re-anchor on files, name the first command, and keep the read set tight."

source_material:
  Ask for the current folder, latest human message, whether the chat/tool crashed, and any visible Forge output. Then prefer state, sprint, latest checkpoint, evidence, context health, and Help Oracle over prior chat memory.

follow_up_batches:
  - boundary: "Are we continuing the same workflow, recovering stale context, or classifying fresh human intent?"
  - read_set: "Which files must be read first, and which broad docs should stay unloaded?"
  - stale_state: "What old next_action or chat instruction might be unsafe now?"
  - proof: "Which command proves the recovered route is current?"
  - handoff: "What should the next chat or agent do first?"

conversation_stages:
  - reanchor: "Run preflight/start/resume or reload before trusting memory."
  - classify_interruption: "Name whether this is fresh chat, compacted context, stale state, or route contradiction."
  - load_minimum: "Read state, sprint, latest checkpoint, and active workflow before broad context."
  - verify_route: "Use Help Oracle and, when there is fresh intent, Guidance Engine."
  - recover_or_continue: "Run context recovery only when state/context is stale, overloaded, or contradictory."
  - handoff: "Persist a compact recovery artifact, first command, and next workflow."

elicitation_options:
  - stale_memory_cut: "Ask what prior chat claim should be ignored until files prove it."
  - first_command: "Name exactly one command the next agent should run first."
  - read_budget: "Split must-read, useful-later, and do-not-load-yet files."
  - contradiction_probe: "Compare state next_action, active workflow, latest checkpoint, and human intent."
  - recovery_threshold: "Decide whether normal resume is enough or context recover is required."

facilitator_moves:
  - "Do not ask the human to remember the phase, workflow, or project state."
  - "Do not continue from old chat claims when launcher output exists."
  - "Do not run broad repository scans before route and read-first files are known."
  - "If the human gives a substantive new message, route it through Guidance Engine after resume."
  - "Make fresh-chat recovery feel calm and explicit: current state, next workflow, first command, and why."

quality_bar:
  - "The human knows whether to continue, reload, or recover context."
  - "The agent has a compact read order and first command."
  - "Stale chat instructions are named as unsafe."
  - "The next workflow comes from durable state or Guidance Engine, not memory."

anti_patterns:
  - "Do not say 'we were doing X' unless files prove it."
  - "Do not dump all docs into recovery."
  - "Do not invent a new workflow because the chat restarted."
  - "Do not hide route uncertainty behind confident prose."

paths:
  fast_path: "Run preflight/start/resume, inspect Help Oracle context boundary, and continue from the returned workflow."
  deep_path: "Run reload, context health, context recover, audit, context plan, then route fresh human intent through Guidance Engine."

checkpoint_options:
  - context-recovery
  - session-prep
  - checkpoint-preview
  - guidance-engine

artifact_rules:
  Persist interruption type, authoritative state, read-first files, stale-memory warnings, context health, recovery commands, and next workflow.

domain_examples:
  - fresh_chat: "The human opens a new chat and asks to continue; run launcher startup, trust state, and route any new intent through guide."
  - network_drop: "The previous chat lost connection; do not reconstruct from memory, use latest checkpoint and Help Oracle."
  - stale_instruction: "The agent appears to follow old instructions; run reload and discard prior chat claims."
  - overloaded_context: "Context is too large; write compact recovery and load only state, sprint, checkpoint, active workflow, and evidence."

headless:
  Use durable files and launcher output only. If route is ambiguous, return the smallest project-choice question and stop before broad reads.

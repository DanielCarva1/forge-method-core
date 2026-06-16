# facilitation: config-customization

purpose:
  Help a human or team customize Forge behavior through validated Project Configuration without creating stale docs or hidden chat-only conventions.

open_floor:
  "What should Forge do differently in this project, and should that change apply to the team or only to this local workspace?"

source_material:
  Ask for the current route, target workflow or agent, desired convention, existing config, expected human experience, and proof command.

follow_up_batches:
  - scope: "Is this team policy, local preference, or a one-off artifact note?"
  - surface: "Is the change workflow metadata, agent profile metadata, project convention, or a custom capability index entry?"
  - effect: "What should inspect, guide, or future agent recovery show differently?"
  - proof: "Which command proves the override works and fails when stale?"

conversation_stages:
  - intent_capture: "Restate the desired behavior in one sentence before choosing keys."
  - scope_choice: "Choose team config for shared behavior or local config for personal/project-machine behavior."
  - key_mapping: "Map the request to supported flat keys and name rejected keys explicitly."
  - conflict_review: "Explain any local value that overrides a team value."
  - validation: "Run config validate before trusting the change."
  - index_generation: "Generate the capability index when future agents need compact effective context."
  - handoff: "Record changed keys, validation result, and affected workflow or agent."

elicitation_options:
  - guided_key_selection: "Offer the smallest set of config keys that satisfies the behavior."
  - stale_reference_check: "Ask what should happen if a workflow, pack, template, or agent is renamed."
  - capability_card: "Turn a custom capability into title, summary, kind, workflow, and command fields."
  - local_vs_team: "Compare who will be surprised if this applies globally versus locally."

facilitator_moves:
  - "Prefer a validated override over a prose instruction that future agents may miss."
  - "Keep Project Configuration narrow; route new behavior to runtime-builder or builder-factory."
  - "Reject unsupported keys before writing files."
  - "Use local config for personal preferences and team config for shared rules."
  - "Generate the Capability Index after meaningful changes."
  - "Treat missing workflow, pack, template, or agent references as stale config."
  - "Explain the runtime-visible effect in inspect, guide metadata, or index output."
  - "Do not edit packaged runtime files when a project-level override is enough."

quality_bar:
  - "A future agent can see the effective behavior without reading chat history."
  - "A future human can understand whether the change is team or local."
  - "Invalid or stale config fails loudly."
  - "Valid config changes one observable runtime surface."

anti_patterns:
  - "Do not create a manual registry when config index can generate it."
  - "Do not use config to rename workflow references or move source files."
  - "Do not hide conventions in README prose when they affect runtime behavior."
  - "Do not silently accept local values that override team policy."

paths:
  fast_path: "Write one supported key, validate config, and confirm inspect output."
  deep_path: "Map multiple overrides, generate the Capability Index, add a test fixture, and write a durable artifact."

checkpoint_options:
  - continue
  - config-validate
  - config-index
  - workflow-validate
  - correct-course

domain_examples:
  - team_defaults: "A team wants default model, commit policy, or verification tier changed; write the override, inspect effective config, and validate merged behavior."
  - local_experiment: "One developer needs a temporary convention or capability entry; keep it local, name the boundary, and avoid shipping it as team policy."
  - capability_index: "Agents keep missing available workflows; regenerate the capability index so future sessions see the effective route surface."

artifact_rules:
  Persist changed config paths, keys, scope, validation output, index path when generated, and any rejected unsupported behavior.

headless:
  If the request names exact keys and scope, write and validate them. Otherwise produce a proposed key map and one blocking question.

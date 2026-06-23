# facilitation: persona-lenses

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Apply named role and coach lenses to live Forge guidance without bloating agent profiles, state, or workflow references.

open_floor:
  "Which lens would make this easier to think through: product, architecture, research, UX, QA, game, builder, tech writing, or a creative coach?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for the decision, current workflow, desired role lens, constraints, audience, evidence, and what would make the guidance feel useful.

follow_up_batches:
  - role: "Which specialist perspective should lead, and which perspectives should challenge it?"
  - technique: "Should we diverge, stress-test, walk through the experience, map evidence, or trace implementation?"
  - compactness: "What should future agents remember, and what should remain live conversation only?"
  - proof: "Which route, council output, or artifact proves the lens changed the guidance?"

conversation_stages:
  - lens_selection: "Select the smallest useful lens from the user's language."
  - workflow_alignment: "Keep the recommended workflow grounded in current state and intent."
  - technique_pick: "Choose one or two elicitation techniques, not a long menu."
  - council_routing: "Use the lens to pick council participants only when multiple perspectives help."
  - compact_handoff: "Persist decisions and risks, not personality narration."

elicitation_options:
  - persona_lens: "Use the named lens to frame the next question."
  - technique_index: "Pick a compact technique id and state its purpose."
  - dissenting_lens: "Invite one challenging lens when the leading lens may miss a risk."
  - council: "Route a live council when the choice is taste-heavy, strategic, or expensive."

facilitator_moves:
  - "Prefer role clarity over theatrical personality."
  - "Do not add persona text to default agent recommendations."
  - "Use the current workflow; the lens changes how we ask, not the durable state machine."
  - "Keep technique names compact so future agents can resume without a transcript."
  - "Use council only when it adds a real second perspective."
  - "When the human names a role, reflect that role and move to a concrete next workflow."
  - "If the requested role is not supported, route to the closest lens and say what is missing."
  - "Do not invent long expert identities that the runtime cannot validate."

quality_bar:
  - "Humans can request a PM, Architect, UX, QA, Game, Builder, Tech Writer, or coach lens naturally."
  - "Guidance Engine returns a compact persona_lens object."
  - "Capability Index lists persona lenses and techniques compactly."
  - "Council participant routing changes based on the selected lens."

anti_patterns:
  - "Do not turn Persona Lens into a new public slash command."
  - "Do not duplicate Agent Profile fields with long persona prose."
  - "Do not persist live council transcript as future context."
  - "Do not let a lens override a stronger correct-course or mechanical-build route."

paths:
  fast_path: "Detect the lens, keep the workflow, and ask one lens-specific next question."
  deep_path: "Select lens, choose technique, run council if useful, write compact decision artifact."

checkpoint_options:
  - continue
  - council
  - guide-route
  - correct-course
  - config-index

domain_examples:
  - architect_lens: "The human asks for architecture judgment; select Architect lens and keep tradeoffs compact in the target workflow."
  - ux_lens: "Taste, interaction, or accessibility is the real job; select UX lens and avoid turning it into generic product planning."
  - quality_lens: "Testing or release confidence is unclear; select QA lens and route to the narrowest quality workflow with evidence."

artifact_rules:
  Persist lens id, selected technique ids, workflow, participants when council runs, compact decision, and next action.

headless:
  Return the persona_lens object and continue the recommended workflow. Do not require a live persona discussion unless the workflow needs human judgment.

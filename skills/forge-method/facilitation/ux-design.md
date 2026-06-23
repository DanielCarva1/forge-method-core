# facilitation: ux-design

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Shape the human experience, interaction model, taste, accessibility, and UX evidence before implementation stories.

open_floor:
  "What should the user understand, feel, and successfully do in the first real session?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for PRD/spec, audience, current screens, references, anti-references, brand constraints, accessibility needs, devices, workflows, and support pain.

follow_up_batches:
  - user_journey: "What does the user try to do from arrival to success?"
  - emotion: "What should feel calm, powerful, playful, serious, fast, trusted, or avoided?"
  - interface: "Which surfaces, controls, states, and empty/error/loading paths matter?"
  - constraints: "What platform, device, accessibility, localization, or density constraints apply?"
  - proof: "What prototype, screenshot, usability check, visible artifact, or acceptance evidence proves the UX, and which 1-3 variants should the human react to?"
  - taste: "Which references, anti-references, and patterns define what good and bad look like?"
  - rejection: "Which common UI choices would make this feel wrong for the domain?"

conversation_stages:
  - load_intent: "Connect UX to user value and product requirements."
  - calibrate_taste: "Capture references, anti-references, tone, and rejected patterns."
  - map_journeys: "Describe primary workflows, states, and user decisions."
  - specify_interaction: "Define controls, information hierarchy, edge states, and accessibility."
  - rejection_log: "Preserve visual, copy, layout, density, and interaction decisions not to use."
  - proof_design: "Choose screenshot, visual-alignment-prototype, usability, accessibility, or workflow evidence."
  - visual_options: "Show rough alternatives when taste, layout, density, or workflow is still uncertain; one artifact is enough when the direction is narrow."
  - handoff: "Persist UX plan, assumptions, proof target, and next workflow."

elicitation_options:
  - first_session: "Walk through the first successful use in plain language."
  - frustration_scan: "Ask where the user might feel lost, delayed, embarrassed, or unsafe."
  - control_inventory: "List controls and states the user naturally expects."
  - taste_rejection: "Name visual or interaction patterns that would make the product feel wrong."
  - density_check: "Ask whether repeated users need speed and scan density or slower guided focus."
  - accessibility_pass: "Ask what keyboard, contrast, screen-reader, motion, and error recovery constraints matter."

facilitator_moves:
  - "Do not reduce UX to colors."
  - "Do not create stories for screens whose workflow is not understood."
  - "Treat accessibility, empty states, and error states as normal UX, not polish."
  - "Keep taste decisions separate from compact agent acceptance criteria."
  - "Use anti-references to prevent generic UI from sneaking back in."
  - "Translate taste into concrete layout, controls, states, density, and copy decisions."

quality_bar:
  - "The UX plan explains workflows, states, controls, density, and proof."
  - "The UX plan captures accepted and rejected visible directions, not only written UX claims."
  - "The human can judge whether the experience has taste from an inspectable artifact when the product is user-facing."
  - "A future implementer can build without inventing interaction intent."
  - "Rejected patterns are explicit enough to stop a future agent from producing generic UI."
  - "Accessibility and first-session proof are part of the plan."

anti_patterns:
  - "Do not create a landing page when the user needs a working tool."
  - "Do not use generic hero/gradient/card-heavy language as UX thinking."
  - "Do not let implementation convenience erase the user's repeated workflow."
  - "Do not bury the main product object behind decorative composition."
  - "Do not claim taste calibration when no reference, anti-reference, or rejection exists."

paths:
  fast_path: "Capture journey, surfaces, controls, states, proof target, and next stories."
  deep_path: "Run UX plan, prototype proof, accessibility review, and Grill Gate before stories."

checkpoint_options:
  - ux-plan
  - visual-alignment-prototype
  - product-requirements
  - architecture
  - create-epics
  - build-story

artifact_rules:
  Persist user journeys, surfaces, controls, states, accessibility, taste decisions, rejected patterns, visible proof, and next workflow.

domain_examples:
  - ux_create: "Create a UX plan from PRD, audience, journeys, controls, states, accessibility, and proof."
  - ux_update: "Update the UX plan when workflow, density, copy, platform, or constraints change."
  - ux_validate: "Return findings for missing states, generic taste, inaccessible flows, and unproven interactions."

headless:
  Use existing product/spec artifacts first. If taste references are missing, ask for them instead of inventing a visual direction.

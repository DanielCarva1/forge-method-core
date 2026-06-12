# facilitation: ux-design

purpose:
  Shape the human experience, interaction model, taste, accessibility, and UX evidence before implementation stories.

open_floor:
  "What should the user understand, feel, and successfully do in the first real session?"

source_material:
  Ask for PRD/spec, audience, current screens, references, anti-references, brand constraints, accessibility needs, devices, workflows, and support pain.

follow_up_batches:
  - user_journey: "What does the user try to do from arrival to success?"
  - emotion: "What should feel calm, powerful, playful, serious, fast, trusted, or avoided?"
  - interface: "Which surfaces, controls, states, and empty/error/loading paths matter?"
  - constraints: "What platform, device, accessibility, localization, or density constraints apply?"
  - proof: "What prototype, screenshot, usability check, or acceptance evidence proves the UX?"

conversation_stages:
  - load_intent: "Connect UX to user value and product requirements."
  - calibrate_taste: "Capture references, anti-references, tone, and rejected patterns."
  - map_journeys: "Describe primary workflows, states, and user decisions."
  - specify_interaction: "Define controls, information hierarchy, edge states, and accessibility."
  - handoff: "Persist UX plan, assumptions, proof target, and next workflow."

elicitation_options:
  - first_session: "Walk through the first successful use in plain language."
  - frustration_scan: "Ask where the user might feel lost, delayed, embarrassed, or unsafe."
  - control_inventory: "List controls and states the user naturally expects."
  - taste_rejection: "Name visual or interaction patterns that would make the product feel wrong."

facilitator_moves:
  - "Do not reduce UX to colors."
  - "Do not create stories for screens whose workflow is not understood."
  - "Treat accessibility, empty states, and error states as normal UX, not polish."
  - "Keep taste decisions separate from compact agent acceptance criteria."

quality_bar:
  - "The UX plan explains workflows, states, controls, density, and proof."
  - "The human can judge whether the experience has taste."
  - "A future implementer can build without inventing interaction intent."

anti_patterns:
  - "Do not create a landing page when the user needs a working tool."
  - "Do not use generic hero/gradient/card-heavy language as UX thinking."
  - "Do not let implementation convenience erase the user's repeated workflow."

paths:
  fast_path: "Capture journey, surfaces, controls, states, proof target, and next stories."
  deep_path: "Run UX plan, prototype proof, accessibility review, and Grill Gate before stories."

checkpoint_options:
  - ux-plan
  - product-requirements
  - architecture
  - create-epics
  - build-story

artifact_rules:
  Persist user journeys, surfaces, controls, states, accessibility, taste decisions, rejected patterns, proof, and next workflow.

headless:
  Use existing product/spec artifacts first. If taste references are missing, ask for them instead of inventing a visual direction.

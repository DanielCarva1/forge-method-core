# facilitation: visual-alignment

purpose:
  Give the human something visible to judge early and repeatedly during initial shaping, then turn corrections into durable product, UX, architecture, or story decisions.

open_floor:
  "Antes de construir de verdade: qual primeira tela, mockup, fluxo ou imagem faria voce dizer 'sim, e isso' ou 'nao, ta indo pro lugar errado'?"

source_material:
  Ask for discovery notes, PRD, UX plan, references, anti-references, sketches, screenshots, brand/taste constraints, target device, and first-session story.

follow_up_batches:
  - first_moment: "What should the human see first, and what should they understand without explanation?"
  - prototype_form: "Should this be a sketch, wireframe, static screen, screenshot, clickable mock, style tile, or runnable thin slice?"
  - option_count: "Should we show one narrow proof, two contrasting directions, or three exploration lanes?"
  - judgment: "What can the human judge from it: layout, density, tone, workflow, affordances, content, accessibility, or emotion?"
  - mismatch: "What would make this feel wrong enough to correct course?"
  - correction: "If it misses, do we revise UX, PRD, architecture, story order, or visual direction?"
  - proof: "What artifact path, screenshot, command, or manual check proves the prototype exists?"

conversation_stages:
  - choose_surface: "Pick the first inspectable surface instead of boiling the whole product."
  - make_visible: "Create or specify 1-3 visible artifacts/options appropriate to the uncertainty."
  - inspect_with_human: "Compare against intent, references, anti-references, and first-session goal."
  - preserve_decisions: "Record accepted visual decisions, rejected directions, and mismatches."
  - route_fix: "Send corrections to UX, PRD, architecture, quick prototype, story creation, or correct-course."

elicitation_options:
  - first_screen_walk: "Ask the human to narrate what they expect to see and do in the first successful minute."
  - reference_triplet: "Collect one close reference, one anti-reference, and one detail worth stealing."
  - judgment_lens: "Pick what the artifact must validate: layout, density, tone, workflow, affordance, content, accessibility, or emotion."
  - mismatch_trigger: "Name the visible mismatch that would force a correction before build."
  - fidelity_choice: "Choose sketch, wireframe, static screen, screenshot, clickable mock, style tile, or runnable thin slice."

facilitator_moves:
  - "Do not let a polished spec substitute for something the human can see."
  - "Do not claim alignment when the human only approved words."
  - "Prefer one inspectable surface over a giant fake prototype; use two or three only when exploring meaningful alternatives."
  - "Use ugly wireframes when speed matters; use richer visuals when taste is the risk."
  - "Treat visual mismatch as product truth, not cosmetic feedback."

quality_bar:
  - "The human can inspect and correct a visible artifact before build."
  - "The artifact answers a specific product question."
  - "Accepted and rejected directions are durable enough for a future agent."
  - "Next workflow changes when the prototype exposes a mismatch."

anti_patterns:
  - "Do not skip visual proof for user-facing products because text artifacts passed."
  - "Do not produce decorative mockups that cannot validate workflow or taste."
  - "Do not ask for final branding when a wireframe would reveal the real issue."

paths:
  fast_path: "One screen or wireframe, mismatch list, accepted/rejected decisions, next workflow."
  deep_path: "Two or three contrasting directions, reference/anti-reference calibration, interactive prototype when useful, accessibility pass, Grill Gate, then stories."

checkpoint_options:
  - ux-plan
  - product-requirements
  - architecture
  - quick-prototype
  - story-creation
  - correct-course

domain_examples:
  - saas_tool: "A dense dashboard should prove information hierarchy, repeated-use speed, empty states, and action affordances before stories."
  - consumer_app: "A first-session mockup should prove emotional tone, onboarding, trust cues, and the primary action."
  - game_or_vtt: "A table, HUD, map, or character sheet preview should prove player fantasy and usability before engine work expands."

artifact_rules:
  Persist prototype form, option count, artifact paths, first visible moment, inspection criteria, mismatches, accepted/rejected decisions, proof, and next workflow.

headless:
  If no visual asset can be generated, specify the exact artifact to create and block build until the human can inspect it.

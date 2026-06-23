# facilitation: evidence-research

> **Presence:** The agent is an excited expert friend who matches the human's energy. This is creative collaboration, not a form to fill.

purpose:
  Turn an idea, claim, market question, domain uncertainty, or feasibility risk into grounded evidence before planning.

open_floor:
  "What claim are we about to believe too quickly, and what evidence would make the next decision defensible?"
  "At any point you can say 'I don't know — research who does this, how, what succeeds, new trends, and tell me your recommendation.' If you didn't understand a question I asked, tell me and I'll research and explain better."

source_material:
  Ask for the idea, target users, constraints, known alternatives, legal/ethical risks, docs, links, data, prior artifacts, and the decision this research must unlock.

follow_up_batches:
  - claim: "What are we trying to prove, disprove, or make safer?"
  - scope: "Is this market, domain, technical, legal, safety, feasibility, or user-pain research?"
  - decision: "What decision becomes different after this research?"
  - standard: "What quality of evidence is enough for the next workflow?"
  - contradiction: "What would make this idea invalid, smaller, or not worth building?"
  - source_quality: "How recent, authoritative, direct, and biased are the sources?"
  - visual_examples: "Which reference examples, anti-examples, screenshots, patterns, or prototype cues should the human see after research?"
  - output: "Which compact scan or gate should the next agent consume?"

conversation_stages:
  - frame_claim: "Name the assumption and the decision that depends on it."
  - choose_lens: "Pick reality gate, market scan, domain scan, or technical feasibility scan; do not default every research request to domain."
  - gather_evidence: "Collect sources, examples, constraints, and alternatives without drifting into implementation."
  - challenge_claim: "Separate evidence, inference, speculation, and taste."
  - scan_shape: "Extract the artifact research-scan fields the next agent can validate."
  - example_handoff: "When research informs a user-facing direction, route useful examples into visual-alignment-prototype before requirements harden."
  - stance: "Say continue, pivot, prototype, more research, or block; explain why."
  - route_next: "Persist the stance, open questions, and next workflow."

elicitation_options:
  - falsifier: "Ask what result would make the team stop, pivot, or shrink scope."
  - competitor_walk: "Compare alternatives by job-to-be-done, switching cost, and unmet pain."
  - feasibility_slice: "Find the smallest technical proof that tests the risky part."
  - evidence_grade: "Grade sources by recency, authority, directness, and bias."

facilitator_moves:
  - "Do not let excitement replace evidence."
  - "Do not treat market scarcity as proof of opportunity."
  - "Name when a source supports only a weaker claim."
  - "When the human asks for research, first ask what decision the research unlocks."
  - "When evidence conflicts, keep the contradiction visible instead of smoothing it into a summary."
  - "Turn research into a decision, not a reading list."

quality_bar:
  - "The output distinguishes facts, inferences, guesses, and unresolved risk."
  - "The next workflow can act without rereading every source."
  - "The human understands what changed because of the evidence."
  - "For user-facing work, research produces examples or anti-examples the human can inspect, not just a written summary."
  - "Market scans name alternatives and adoption friction; domain scans name risks and review needs; technical scans name proof paths."
  - "artifact research-scan writes research_question, decision_to_unlock, claim, sources/source_gaps, evidence_grade, findings, contradictions_or_falsifiers, uncertainty, stance, workflow-specific fields, validation, and next_workflow."
  - "artifact research-check passes before downstream planning uses the scan as evidence."

anti_patterns:
  - "Do not browse forever without a decision target."
  - "Do not bury legal, safety, or feasibility blockers under optimism."
  - "Do not write implementation stories from unvalidated claims."
  - "Do not route competitor/adoption questions to domain-scan or feasibility questions to market-scan."
  - "Do not cite weak secondary summaries as if they prove primary behavior."

paths:
  fast_path: "Run the narrowest scan, run artifact research-scan, run artifact research-check, record stance and next workflow."
  deep_path: "Run reality gate, domain/market/technical scans, close with research-closeout, then Grill Gate before spec."

checkpoint_options:
  - reality-evidence-gate
  - market-scan
  - domain-scan
  - technical-feasibility-scan
  - product-requirements
  - visual-alignment-prototype
  - correct-course

artifact_rules:
  Use artifact research-scan for durable scan output; persist claim, source links, evidence grade, contradictions, stance, visual/reference examples when relevant, workflow-specific proof fields, open questions, validation, and next workflow.

domain_examples:
  market: "Alternatives, switching cost, adoption friction, demand signal, pricing pressure, trust barrier."
  domain: "Rules, norms, source material, safety/legal/ethical risks, accepted practices, expert review."
  technical: "Physical possibility, model/tool/API limits, required data, operational limits, cheapest proof path."

headless:
  Use primary sources when possible, record uncertainty, and stop research when the decision threshold is met or blocked.

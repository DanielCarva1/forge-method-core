# facilitation: problem-solving

purpose:
  Convert frustration, ambiguity, or stuckness into a precise problem frame and next reversible move.

open_floor:
  "Descreve o que está errado do jeito cru mesmo: o que você esperava, o que aconteceu, onde dói, e o que você já tentou."

source_material:
  Ask for logs, examples, screenshots, transcripts, artifacts, prior decisions, constraints, and symptoms.

follow_up_batches:
  - current_vs_desired: "What is the gap between current and desired state?"
  - symptoms: "What evidence shows the problem?"
  - causes: "Which causes are plausible, and what would disprove each?"
  - leverage: "What is the smallest reversible test or repair?"

conversation_stages:
  - discharge: "Let the human describe the failure plainly before compressing it into a tidy issue."
  - reproduce: "Collect evidence, examples, logs, transcripts, artifacts, or exact moments where expectation diverged."
  - frame: "State current behavior, desired behavior, impact, and what is not yet known."
  - hypotheses: "List likely causes and what evidence would confirm or disprove each."
  - probe: "Choose the smallest reversible action, correction, or research step and persist it."

elicitation_options:
  - five_whys: "Use only when the problem is causal, not when the user is asking for a taste decision."
  - counterexample: "Ask for one case where the behavior works or the complaint does not apply."
  - timeline: "Reconstruct when the route, artifact, or expectation went wrong."
  - rollback_probe: "Find the smallest state, doc, or code change that would restore trust."

facilitator_moves:
  - "Validate the signal without turning frustration into therapy or vague apology."
  - "Separate symptom, cause, and repair so the agent does not patch the wrong layer."
  - "When the user identifies a product requirement, treat it as evidence, not optional opinion."
  - "Prefer one reversible probe over broad refactoring."

quality_bar:
  - "The problem statement is testable and tied to evidence."
  - "The next action can be executed or explicitly asks for one blocking fact."
  - "The human sees that the system understood what hurt and what changes."

anti_patterns:
  - "Do not jump to implementation before reproducing the failure."
  - "Do not smooth over anger when it points at a real route or product defect."
  - "Do not ask the human to restate evidence already present in files or transcripts."

paths:
  fast_path: "Name the problem, pick one hypothesis, and run a reversible probe."
  deep_path: "Map hypotheses, evidence, risks, and correction plan before implementation."

checkpoint_options:
  - continue
  - correct-course
  - brainstorm
  - domain-scan
  - council

artifact_rules:
  Persist problem statement, hypotheses, evidence, chosen probe, blocked facts, and next action.

headless:
  Use available evidence to produce a problem frame and next probe. Mark unknowns; do not fabricate certainty.

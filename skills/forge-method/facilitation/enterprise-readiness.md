# facilitation: enterprise-readiness

purpose:
  Guide security, privacy, compliance, deployment, observability, risk, and release readiness without turning them into generic checklists.

open_floor:
  "Which production claim must be trustworthy before a user, customer, team, or regulator depends on this?"

source_material:
  Ask for requirements, architecture, data flows, threat model, privacy needs, compliance obligations, deployment target, CI, monitoring, incidents, evidence, and waivers.

follow_up_batches:
  - production_claim: "What are we claiming is safe, compliant, observable, deployable, or ready?"
  - risk: "What failure hurts users, data, money, operations, or trust?"
  - evidence: "Which check, doc, control, log, test, or waiver proves the claim?"
  - ownership: "Who or what maintains this after release?"
  - gate: "Is the result pass, conditional, fail, waived, or needs research?"

conversation_stages:
  - classify_concern: "Pick security, privacy, compliance, DevOps, observability, risk, release, or mixed."
  - map_surface: "Trace data, users, systems, environments, controls, and dependencies."
  - evidence_gap: "Compare claims to actual checks and artifacts."
  - decide_gate: "Record pass/condition/fail/waive with rationale."
  - route_followup: "Create risk item, story, waiver, release gate, or operate handoff."

elicitation_options:
  - data_flow: "Ask what data enters, where it goes, who sees it, and how long it lives."
  - threat_prompt: "Ask who could misuse, break, leak, or overload the system."
  - ops_walk: "Walk deploy, rollback, monitor, alert, incident, and support path."
  - waiver_test: "Ask what evidence justifies shipping despite a known gap."

facilitator_moves:
  - "Do not accept compliance theater."
  - "Do not let scary enterprise words bloat a small project."
  - "Tie every control to a risk or requirement."
  - "Make waivers explicit and uncomfortable enough to revisit."

quality_bar:
  - "Claims map to evidence, gaps, owners, and gate decisions."
  - "A future agent can find the command, artifact, or waiver that proves readiness."
  - "Production risk is neither ignored nor exaggerated."

anti_patterns:
  - "Do not paste generic checklists with no product context."
  - "Do not claim ready without release evidence."
  - "Do not hide unknowns behind conditional language."

paths:
  fast_path: "Map the claim to evidence/gaps and produce the next risk or release action."
  deep_path: "Run risk, security, privacy, DevOps, compliance, observability, traceability, and release readiness in sequence."

checkpoint_options:
  - risk-register
  - security-plan
  - privacy-data-plan
  - devops-deployment-plan
  - compliance-checklist
  - observability-plan
  - traceability-gate
  - release-readiness

artifact_rules:
  Persist claims, risks, controls, checks, evidence, waivers, owners, gate stance, and next workflow.

headless:
  Inspect existing artifacts and commands first. If evidence is missing, return a gap matrix and the smallest proof-producing action.
